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

async fn spawn_app() -> axum::Router {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "rssea-settings-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let config = Config { data_dir: dir, host: "127.0.0.1".into(), port: 0 };
    let engine = Engine::new(&config).await.unwrap();
    let mut db = app_db::open(&config.data_dir).unwrap();
    db.set_password_hash(&auth::hash_password("test-pass").unwrap()).unwrap();
    let app_db = Arc::new(Mutex::new(db));
    api::router(AppState { engine, app_db, allow_private_proxy: false })
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

async fn get_settings(app: &axum::Router, cookie: &str) -> serde_json::Value {
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/settings")
            .header(axum::http::header::COOKIE, cookie)
            .body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn patch_settings(app: &axum::Router, cookie: &str, body: &str) -> axum::response::Response {
    app.clone()
        .oneshot(Request::builder().method("PATCH").uri("/api/settings")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, cookie)
            .body(Body::from(body.to_string())).unwrap())
        .await.unwrap()
}

#[tokio::test]
async fn settings_requires_auth() {
    let app = spawn_app().await;
    let resp = app.clone()
        .oneshot(Request::builder().uri("/api/settings").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn settings_defaults_and_patch_roundtrip() {
    let app = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let settings = get_settings(&app, &cookie).await;
    assert!(settings["theme"].is_null(), "theme defaults to null");
    assert_eq!(settings["sync_interval_minutes"].as_i64(), Some(30));
    assert!(settings["keep_articles_days"].is_null(), "keep_articles_days defaults to null");
    assert!(settings["stats"]["feeds"].is_number());
    assert!(settings["stats"]["articles"].is_number());
    assert!(settings["stats"]["unread"].is_number());
    assert!(settings["stats"]["database_size_bytes"].is_number());
    assert!(settings["stats"]["last_sync"].is_string());

    let resp = patch_settings(&app, &cookie, r#"{"theme":"dark"}"#).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(get_settings(&app, &cookie).await["theme"], "dark");

    let resp = patch_settings(&app, &cookie, r#"{"sync_interval_minutes":15}"#).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(get_settings(&app, &cookie).await["sync_interval_minutes"].as_i64(), Some(15));

    let resp = patch_settings(&app, &cookie, r#"{"keep_articles_days":30}"#).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(get_settings(&app, &cookie).await["keep_articles_days"].as_i64(), Some(30));

    let resp = patch_settings(&app, &cookie, r#"{"keep_articles_days":null}"#).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(get_settings(&app, &cookie).await["keep_articles_days"].is_null(), "null keep_articles_days keeps everything");
}

#[tokio::test]
async fn password_change_requires_current_password() {
    let app = spawn_app().await;
    let cookie = cookie_pair(&login_cookie(&app).await);

    let wrong = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/settings/password")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"current_password":"wrong","new_password":"new-pass"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let correct = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/settings/password")
            .header("content-type", "application/json")
            .header(axum::http::header::COOKIE, &cookie)
            .body(Body::from(r#"{"current_password":"test-pass","new_password":"new-pass"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(correct.status(), StatusCode::OK);

    let old_login = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/login")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"password":"test-pass"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);

    let new_login = app.clone()
        .oneshot(Request::builder().method("POST").uri("/api/login")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"password":"new-pass"}"#)).unwrap())
        .await.unwrap();
    assert_eq!(new_login.status(), StatusCode::OK);
}
