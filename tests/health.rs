use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rssea::app_db;
use rssea::config::Config;
use rssea::engine::Engine;
use rssea::AppState;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_ok_with_version() {
    let dir = PathBuf::from(format!("/tmp/rssea-health-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let cfg = Config {
        data_dir: dir,
        host: "127.0.0.1".into(),
        port: 3000,
    };
    let engine = Engine::new(&cfg).await.unwrap();
    let app_db = Arc::new(Mutex::new(app_db::open(&cfg.data_dir).unwrap()));
    let app = rssea::api::router(AppState { engine, app_db });
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}
