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
    <title>Saved Feed</title>
    <link>http://127.0.0.1/</link>
    <description>saved</description>
    <item>
      <title>Saved Article</title>
      <link>http://example.com/saved</link>
      <guid isPermaLink="false">saved-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>Saved body.</description>
    </item>
    <item>
      <title>Other Article</title>
      <link>http://example.com/other</link>
      <guid isPermaLink="false">saved-2</guid>
      <pubDate>Sun, 10 Aug 2026 10:00:00 GMT</pubDate>
      <description>Other body.</description>
    </item>
  </channel>
</rss>
"#;

mod feed_server;

async fn spawn_app() -> (String, axum::Router, feed_server::FeedServer) {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = feed_server::FeedServer::start(RSS.to_string(), 10);
    let dir = std::env::temp_dir().join(format!(
        "rssea-saved-test-{}-{}",
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

async fn add_and_sync(app: &axum::Router, cookie: &str, feed_url: &str) {
    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, cookie)
            .body(Body::from(format!(r#"{{"url":"{feed_url}","title":"Saved Feed"}}"#)))
            .unwrap())
        .await.unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);
    let refresh_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources/refresh-all")
            .header(axum::http::header::COOKIE, cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn save_unsave_with_note_and_tags() {
    let (feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);
    add_and_sync(&app, &cookie, &feed_url).await;

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
            .body(Body::from(r#"{"note":"must read","tags":["favorites","work"]}"#))
            .unwrap())
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
    assert_eq!(saved["total"], 1);
    let months = saved["months"].as_array().unwrap();
    assert_eq!(months.len(), 1);
    let month = chrono::Utc::now().format("%Y-%m").to_string();
    assert_eq!(months[0]["month"], month);
    let items = months[0]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], id);
    assert_eq!(items[0]["note"], "must read");

    let tags_resp = app.clone()
        .oneshot(Request::builder().uri("/api/tags")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(tags_resp.status(), StatusCode::OK);
    let body = tags_resp.into_body().collect().await.unwrap().to_bytes();
    let tags: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let tags_arr = tags["tags"].as_array().unwrap();
    assert!(tags_arr.iter().any(|t| t == "favorites"));
    assert!(tags_arr.iter().any(|t| t == "work"));

    let detail_resp = app.clone()
        .oneshot(Request::builder().uri(format!("/api/articles/{id}"))
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
    let body = detail_resp.into_body().collect().await.unwrap().to_bytes();
    let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(detail["note"], "must read");
    assert!(detail["tags"].as_array().unwrap().iter().any(|t| t == "favorites"));
    assert_eq!(detail["marked"], true);

    let marked_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?saved=true")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(marked_resp.status(), StatusCode::OK);
    let body = marked_resp.into_body().collect().await.unwrap().to_bytes();
    let marked: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let marked_arr = marked.as_array().unwrap();
    assert_eq!(marked_arr.len(), 1);
    assert_eq!(marked_arr[0]["id"], id);

    let del_resp = app.clone()
        .oneshot(Request::builder().method("DELETE")
            .uri(format!("/api/articles/{id}/save"))
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(del_resp.status(), StatusCode::OK);

    let detail_resp = app.clone()
        .oneshot(Request::builder().uri(format!("/api/articles/{id}"))
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
    let body = detail_resp.into_body().collect().await.unwrap().to_bytes();
    let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(detail["marked"], false);
    assert!(detail["note"].is_null());
    assert!(detail["tags"].as_array().unwrap().is_empty());

    let saved_resp = app.clone()
        .oneshot(Request::builder().uri("/api/saved")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(saved_resp.status(), StatusCode::OK);
    let body = saved_resp.into_body().collect().await.unwrap().to_bytes();
    let saved: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(saved["total"], 0);
    assert!(saved["months"].as_array().unwrap().is_empty());
}
