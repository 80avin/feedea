use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rssea::api;
use rssea::config::Config;
use rssea::engine::Engine;
use rssea::AppState;
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

async fn spawn_app() -> (String, axum::Router) {
    let server = feed_server::FeedServer::start(RSS.to_string(), 10);
    let dir = std::env::temp_dir().join(format!("rssea-api-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config { data_dir: dir, host: "127.0.0.1".into(), port: 0 };
    let engine = Engine::new(&config).await.unwrap();
    let router = api::router(AppState { engine: engine.clone() });
    (server.url, router)
}

#[tokio::test]
async fn add_source_sync_and_read_articles() {
    let (feed_url, app) = spawn_app().await;

    let add_resp = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/sources")
            .header("content-type", "application/json")
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
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(refresh_resp.status(), StatusCode::OK);

    let single_resp = app.clone()
        .oneshot(Request::builder().method("POST")
            .uri(format!("/api/sources/{}/refresh", url_encode(&feed_id)))
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(single_resp.status(), StatusCode::OK);
    let body = single_resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["new_articles"], 0);

    let list_resp = app.clone()
        .oneshot(Request::builder().uri("/api/articles?offset=0&limit=10").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let body = list_resp.into_body().collect().await.unwrap().to_bytes();
    let articles: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let arr = articles.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["title"], "API Article");

    let id = arr[0]["id"].as_str().unwrap().to_string();
    let detail_resp = app.clone()
        .oneshot(Request::builder().uri(format!("/api/articles/{id}")).body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
}
