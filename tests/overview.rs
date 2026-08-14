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
    <title>Overview Feed</title>
    <link>http://127.0.0.1/</link>
    <description>overview</description>
    <item>
      <title>Overview Alpha</title>
      <link>http://example.com/alpha</link>
      <guid isPermaLink="false">overview-alpha-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>Alpha body.</description>
    </item>
    <item>
      <title>Overview Beta</title>
      <link>http://example.com/beta</link>
      <guid isPermaLink="false">overview-beta-1</guid>
      <pubDate>Tue, 12 Aug 2026 11:30:00 GMT</pubDate>
      <description>Beta body.</description>
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
        "rssea-overview-test-{}-{}",
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

#[tokio::test]
async fn overview_requires_auth() {
    let (_feed_url, app, _server) = spawn_app().await;
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/overview").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn overview_returns_per_category_cards_and_all_totals() {
    let (feed_url, app, _server) = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(format!(r#"{{"url":"{feed_url}","title":"Overview Feed"}}"#)))
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

    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/overview")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let cards = json["cards"].as_array().expect("cards should be an array");
    let toplevel = cards.iter().find(|card| card["name"] == "NewsFlash.Toplevel")
        .expect("cards should contain a NewsFlash.Toplevel card");
    assert!(toplevel["total_count"].as_i64().unwrap() >= 2);
    let items = toplevel["items"].as_array().expect("card items should be an array");
    assert!(!items.is_empty());
    assert_eq!(toplevel["unread_count"].as_i64().unwrap(), 2);

    assert!(json["all"]["total_count"].as_i64().unwrap() >= 2);
    assert!(json["all"]["unread_count"].as_i64().unwrap() >= 2);
}
