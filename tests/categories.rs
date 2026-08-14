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

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Prune Feed</title>
    <link>http://127.0.0.1/</link>
    <description>prune</description>
    <item>
      <title>Prune Article</title>
      <link>http://example.com/prune</link>
      <guid isPermaLink="false">prune-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>Prune body.</description>
    </item>
    <item>
      <title>Other Article</title>
      <link>http://example.com/other</link>
      <guid isPermaLink="false">prune-2</guid>
      <pubDate>Sun, 10 Aug 2026 10:00:00 GMT</pubDate>
      <description>Other body.</description>
    </item>
  </channel>
</rss>
"#;

mod feed_server;

async fn spawn_app() -> (String, axum::Router, feed_server::FeedServer, Arc<Mutex<app_db::AppDb>>, std::path::PathBuf) {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = feed_server::FeedServer::start(RSS.to_string(), 10);
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
    let router = api::router(AppState { engine: engine.clone(), app_db: app_db.clone() });
    (server.url.clone(), router, server, app_db, config.data_dir.clone())
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
    let (_feed_url, app, _server, _db, _dir) = spawn_app().await;
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/categories").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn categories_crud_tree_reparent_and_remove_children() {
    let (_feed_url, app, _server, _db, _dir) = spawn_app().await;
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

#[tokio::test]
async fn delete_category_with_remove_children_prunes_saved_articles() {
    let (feed_url, app, _server, app_db, dir) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let create_resp = post_category(&app, &cookie, r#"{"name":"Del"}"#).await;
    assert_eq!(create_resp.status(), StatusCode::OK);
    let body = create_resp.into_body().collect().await.unwrap().to_bytes();
    let del_id = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["category_id"].as_str().unwrap().to_string();

    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(format!(r#"{{"url":"{feed_url}","title":"Prune Feed","category_id":"{del_id}"}}"#)))
            .unwrap())
        .await.unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);

    let refresh_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri("/api/sources/refresh-all")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    let list_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?offset=0&limit=10")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let body = list_resp.into_body().collect().await.unwrap().to_bytes();
    let articles: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = articles.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let id = arr[0]["id"].as_str().unwrap().to_string();

    let save_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri(format!("/api/articles/{id}/save"))
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"note":"must prune"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(save_resp.status(), StatusCode::OK);

    let saved_resp = app.clone()
        .oneshot(Request::builder().uri("/api/saved")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(saved_resp.status(), StatusCode::OK);
    let body = saved_resp.into_body().collect().await.unwrap().to_bytes();
    let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(saved["total"], 1, "saved article should be listed before delete");

    let delete_resp = app.clone()
        .oneshot(Request::builder().method("DELETE")
            .uri(format!("/api/categories/{del_id}"))
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"remove_children":true}"#)).unwrap())
        .await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::OK);

    let saved_resp = app.clone()
        .oneshot(Request::builder().uri("/api/saved")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(saved_resp.status(), StatusCode::OK);
    let body = saved_resp.into_body().collect().await.unwrap().to_bytes();
    let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(saved["total"], 0, "saved article must be pruned when its category is deleted");
    assert!(saved["months"].as_array().unwrap().is_empty());

    let engine_db = rusqlite::Connection::open(dir.join("engine/data/database.sqlite")).unwrap();
    let engine_count: i64 = engine_db
        .query_row("SELECT COUNT(*) FROM articles WHERE article_id = ?1", rusqlite::params![id], |r| r.get(0))
        .unwrap();
    assert_eq!(engine_count, 1, "article should survive as an orphan in the engine db");
    let db = app_db.lock().await;
    let saved_count: i64 = db.conn
        .query_row("SELECT COUNT(*) FROM saved WHERE article_id = ?1", rusqlite::params![id], |r| r.get(0))
        .unwrap();
    assert_eq!(saved_count, 0, "app-db saved row must be gone after prune");
}
