use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rssea::api;
use rssea::app_db;
use rssea::config::Config;
use rssea::engine::Engine;
use rssea::AppState;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::Mutex;
use tower::ServiceExt;

async fn spawn_app() -> axum::Router {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "rssea-static-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config {
        data_dir: dir,
        host: "127.0.0.1".into(),
        port: 3000,
        allow_private_proxy: false,
    };
    let engine = Engine::new(&cfg).await.unwrap();
    let app_db = Arc::new(Mutex::new(app_db::open(&cfg.data_dir).unwrap()));
    api::router(AppState { engine, app_db, allow_private_proxy: false })
}

#[tokio::test]
async fn serves_index_html_at_root() {
    let app = spawn_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), "text/html");
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("id=\"root\""));
}

#[tokio::test]
async fn serves_hashed_asset_with_content_type() {
    let asset = rssea::assets::Assets::iter()
        .find(|p| p.starts_with("assets/"))
        .expect("dist should contain hashed assets");
    let expected = if asset.ends_with(".js") {
        "text/javascript"
    } else if asset.ends_with(".css") {
        "text/css"
    } else {
        "text/html"
    };
    let app = spawn_app().await;
    let resp = app
        .oneshot(Request::builder().uri(format!("/{asset}")).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), expected);
    assert!(!resp.into_body().collect().await.unwrap().to_bytes().is_empty());
}

#[tokio::test]
async fn spa_fallback_serves_index_html_for_client_route() {
    let app = spawn_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/feeds/some-article-id").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), "text/html");
}

#[tokio::test]
async fn missing_asset_returns_404_not_index_html() {
    let app = spawn_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/assets/does-not-exist.js").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn api_health_still_works() {
    let app = spawn_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), "application/json");
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn img_proxy_is_not_served_by_fallback() {
    let app = spawn_app().await;
    let resp = app
        .oneshot(Request::builder().uri("/img").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
