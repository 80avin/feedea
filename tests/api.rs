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
    <title>API Feed</title>
    <link>http://127.0.0.1/</link>
    <description>api</description>
    <item>
      <title>API Article</title>
      <link>http://example.com/one</link>
      <guid isPermaLink="false">api-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>API body.</description>
    </item>
    <item>
      <title>Article Two</title>
      <link>http://example.com/two</link>
      <guid isPermaLink="false">api-2</guid>
      <pubDate>Sun, 10 Aug 2026 10:00:00 GMT</pubDate>
      <description>Alpha body.</description>
    </item>
  </channel>
</rss>
"#;

mod feed_server;

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

async fn spawn_app() -> (String, axum::Router, feed_server::FeedServer, Arc<Mutex<app_db::AppDb>>, std::path::PathBuf) {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let server = feed_server::FeedServer::start(RSS.to_string(), 10);
    let dir = std::env::temp_dir().join(format!(
        "rssea-api-test-{}-{}",
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

#[tokio::test]
async fn add_source_sync_and_read_articles() {
    let (feed_url, app, _server, _db, _dir) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(format!(r#"{{"url":"{feed_url}","title":"API Feed"}}"#)))
            .unwrap())
        .await.unwrap();
    assert_eq!(add_resp.status(), StatusCode::OK);
    let body = add_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["title"], "API Feed");
    let feed_id = json["id"].as_str().unwrap().to_string();

    let refresh_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri("/api/sources/refresh-all")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    let single_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri(format!("/api/sources/{}/refresh", url_encode(&feed_id)))
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(single_resp.status(), StatusCode::OK);
    let body = single_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["new_articles"], 0);

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
    assert_eq!(arr[0]["title"], "API Article");

    let id = arr[0]["id"].as_str().unwrap().to_string();
    let detail_resp = app.clone()
        .oneshot(Request::builder().uri(format!("/api/articles/{id}"))
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn detail_unknown_article_returns_404() {
    let (_feed_url, app, _server, _db, _dir) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles/does-not-exist")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "not_found");
}

#[tokio::test]
async fn add_source_with_invalid_url_returns_400() {
    let (_feed_url, app, _server, _db, _dir) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);
    let resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"url":"not a url"}"#))
            .unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn refresh_unknown_feed_returns_404() {
    let (_feed_url, app, _server, _db, _dir) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);
    let resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri("/api/sources/does-not-exist/refresh")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

async fn add_and_sync(app: &axum::Router, cookie: &str, feed_url: &str) {
    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, cookie)
            .body(Body::from(format!(r#"{{"url":"{feed_url}","title":"API Feed"}}"#)))
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
async fn search_filters_and_suggestions() {
    let (feed_url, app, _server, _db, _dir) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);
    add_and_sync(&app, &cookie, &feed_url).await;

    let search_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?search=API")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(search_resp.status(), StatusCode::OK);
    let body = search_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["title"], "API Article");

    let alpha_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?search=Alpha")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(alpha_resp.status(), StatusCode::OK);
    let body = alpha_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["title"], "Article Two");

    let sug_resp = app.clone()
        .oneshot(Request::builder().uri("/api/search/suggestions?q=API")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(sug_resp.status(), StatusCode::OK);
    let body = sug_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let suggestions = json["suggestions"].as_array().unwrap();
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0]["title"], "API Article");

    let none_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?search=zzznope")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(none_resp.status(), StatusCode::OK);
    let body = none_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn saved_and_tag_filters() {
    let (feed_url, app, _server, db, dir) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);
    add_and_sync(&app, &cookie, &feed_url).await;

    let saved_true_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?saved=true")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(saved_true_resp.status(), StatusCode::OK);
    let body = saved_true_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.as_array().unwrap().is_empty());

    let unsaved_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?saved=false")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(unsaved_resp.status(), StatusCode::OK);
    let body = unsaved_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);

    let unknown_tag_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?tag=zzznope")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(unknown_tag_resp.status(), StatusCode::OK);
    let body = unknown_tag_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.as_array().unwrap().is_empty());

    let list_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?offset=0&limit=10")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let body = list_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let id = arr[0]["id"].as_str().unwrap().to_string();

    {
        let engine_db = rusqlite::Connection::open(dir.join("engine/data/database.sqlite")).unwrap();
        engine_db.execute("UPDATE articles SET marked = 0 WHERE article_id = ?1", rusqlite::params![id]).unwrap();
        let db = db.lock().await;
        let now = chrono::Utc::now().to_rfc3339();
        db.conn.execute(
            "INSERT OR REPLACE INTO saved (article_id, saved_at, updated_at) VALUES (?1, ?2, ?2)",
            rusqlite::params![id, now],
        ).unwrap();
        db.conn.execute(
            "INSERT OR REPLACE INTO saved_tags (article_id, tag) VALUES (?1, ?2)",
            rusqlite::params![id, "favorites"],
        ).unwrap();
    }

    let tag_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?tag=favorites")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(tag_resp.status(), StatusCode::OK);
    let body = tag_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], id);

    let saved_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?saved=true")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(saved_resp.status(), StatusCode::OK);
    let body = saved_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], id);
}
