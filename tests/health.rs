use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rssea::config::Config;
use std::path::PathBuf;
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_ok_with_version() {
    let cfg = Config {
        data_dir: PathBuf::from("/tmp/rssea-health-test"),
        host: "127.0.0.1".into(),
        port: 3000,
    };
    let app = rssea::api::router(cfg);
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
    assert_eq!(json["version"], "0.1.0");
}
