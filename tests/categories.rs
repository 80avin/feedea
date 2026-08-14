use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rssea::api;
use rssea::app_db;
use rssea::auth;
use rssea::config::Config;
use rssea::engine::Engine;
use rssea::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

mod feed_server;

async fn spawn_app() -> (String, axum::Router, feed_server::FeedServer) {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = feed_server::FeedServer::start(String::new(), 1);
    let dir = std::env::temp_dir().join(format!(
        "rssea-categories-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config { data_dir: dir, host: "127.0.0.1".into(), port: 0 };
    let engine = Engine::new(&config).await.unwrap();
    let mut db = app_db::open(&config.data_dir).unwrap();
    db.set_password_hash(&auth::hash_password("test-pass").unwrap()).unwrap();
    let app_db = Arc::new(Mutex::new(db));
    let router = api::router(AppState { engine: engine.clone(), app_db });
    (server.url.clone(), router, server)
}

async fn login_cookie(app: &axum::Router) -> String {
    let resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/login")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"password":"test-pass"}"#))
            .unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    resp.headers().get(axum::http::header::SET_COOKIE)
        .expect("login should set a session cookie")
        .to_str().unwrap().to_string()
}

fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_string()
}

async fn get_tree(app: &axum::Router, cookie: &str) -> serde_json::Value {
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/categories")
            .header(axum::http::header::COOKIE, cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn post_category(app: &axum::Router, cookie: &str, body: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/categories")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, cookie)
            .body(Body::from(body.to_string())).unwrap())
        .await.unwrap()
}

fn find_node<'a>(nodes: &'a [serde_json::Value], id: &str) -> Option<&'a serde_json::Value> {
    for node in nodes {
        if node["category_id"] == id {
            return Some(node);
        }
        if let Some(children) = node["children"].as_array()
            && let Some(found) = find_node(children, id)
        {
            return Some(found);
        }
    }
    None
}

fn collect_ids(nodes: &[serde_json::Value], out: &mut Vec<String>) {
    for node in nodes {
        out.push(node["category_id"].as_str().unwrap().to_string());
        if let Some(children) = node["children"].as_array() {
            collect_ids(children, out);
        }
    }
}

#[tokio::test]
async fn categories_requires_auth() {
    let (_feed_url, app, _server) = spawn_app().await;
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/categories").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn categories_crud_tree_reparent_and_remove_children() {
    let (_feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let create_resp = post_category(&app, &cookie, r#"{"name":"Tech"}"#).await;
    assert_eq!(create_resp.status(), StatusCode::OK);
    let body = create_resp.into_body().collect().await.unwrap().to_bytes();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let tech_id = node["category_id"].as_str().unwrap().to_string();
    assert_eq!(node["name"], "Tech");
    assert_eq!(node["parent_id"], "NewsFlash.Toplevel");
    assert_eq!(node["unread_count"], 0);

    let create_resp = post_category(&app, &cookie, &format!(r#"{{"name":"Rust","parent_id":"{tech_id}"}}"#)).await;
    assert_eq!(create_resp.status(), StatusCode::OK);
    let body = create_resp.into_body().collect().await.unwrap().to_bytes();
    let node: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let rust_id = node["category_id"].as_str().unwrap().to_string();
    assert_eq!(node["parent_id"], tech_id);

    let create_resp = post_category(&app, &cookie, &format!(r#"{{"name":"Nightly","parent_id":"{rust_id}"}}"#)).await;
    assert_eq!(create_resp.status(), StatusCode::OK);
    let body = create_resp.into_body().collect().await.unwrap().to_bytes();
    let nightly_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["category_id"].as_str().unwrap().to_string();

    let tree = get_tree(&app, &cookie).await;
    let categories = tree["categories"].as_array().unwrap();
    let tech = find_node(categories, &tech_id).expect("Tech should be in tree");
    assert_eq!(tech["name"], "Tech");
    let rust = find_node(categories, &rust_id).expect("Rust should be in tree");
    assert_eq!(rust["parent_id"], tech_id);
    assert_eq!(find_node(categories, &nightly_id).expect("Nightly should be in tree")["parent_id"], rust_id);

    let rename_resp = app.clone()
        .oneshot(Request::builder().method("PATCH")
            .uri(format!("/api/categories/{tech_id}"))
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"name":"Technology"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(rename_resp.status(), StatusCode::OK);
    let body = rename_resp.into_body().collect().await.unwrap().to_bytes();
    let renamed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(renamed["name"], "Technology");
    assert_eq!(renamed["category_id"], tech_id);

    let tree = get_tree(&app, &cookie).await;
    let tech = find_node(tree["categories"].as_array().unwrap(), &tech_id).unwrap();
    assert_eq!(tech["name"], "Technology");

    let read_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri(format!("/api/categories/{tech_id}/read"))
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(read_resp.status(), StatusCode::OK);

    let delete_resp = app.clone()
        .oneshot(Request::builder().method("DELETE")
            .uri(format!("/api/categories/{tech_id}"))
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{}"#)).unwrap())
        .await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);

    let tree = get_tree(&app, &cookie).await;
    let categories = tree["categories"].as_array().unwrap();
    assert!(find_node(categories, &tech_id).is_none(), "Tech should be gone");
    let rust = find_node(categories, &rust_id).expect("Rust should be reparented to top level");
    assert_eq!(rust["parent_id"], "NewsFlash.Toplevel");
    assert_eq!(find_node(categories, &nightly_id).expect("Nightly should still exist")["parent_id"], rust_id);

    let delete_resp = app.clone()
        .oneshot(Request::builder().method("DELETE")
            .uri(format!("/api/categories/{rust_id}"))
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"remove_children":true}"#)).unwrap())
        .await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);

    let tree = get_tree(&app, &cookie).await;
    let mut ids = Vec::new();
    collect_ids(tree["categories"].as_array().unwrap(), &mut ids);
    assert!(!ids.contains(&rust_id), "Rust should be gone: {ids:?}");
    assert!(!ids.contains(&nightly_id), "Nightly should be gone with remove_children: {ids:?}");
}
