use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::auth;
use crate::AppState;

pub const SESSION_COOKIE: &str = "feedea_session";
const SESSION_TTL_SECS: i64 = 30 * 24 * 3600;

#[derive(Deserialize)]
pub struct LoginRequest { pub password: String }

pub async fn login(State(state): State<AppState>, Json(req): Json<LoginRequest>) -> Response {
    let hash = state.app_db.lock().await.password_hash().unwrap_or(None);
    let Some(hash) = hash else {
        return (StatusCode::FORBIDDEN, Json(json!({"error": {"code": "setup_required", "message": "no password configured"}}))).into_response();
    };
    if !auth::verify_password(&req.password, &hash) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": {"code": "unauthorized", "message": "wrong password"}}))).into_response();
    }
    let token = auth::generate_token();
    let token_hash = auth::sha256_hex(&token);
    let expires = (chrono::Utc::now() + chrono::Duration::seconds(SESSION_TTL_SECS)).to_rfc3339();
    let mut app_db = state.app_db.lock().await;
    let _ = app_db.create_session(&token_hash, &expires);
    drop(app_db);
    (
        StatusCode::OK,
        [(
            axum::http::header::SET_COOKIE,
            format!(
                "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={SESSION_TTL_SECS}"
            ),
        )],
        Json(json!({"ok": true})),
    ).into_response()
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = cookie_value(&headers, SESSION_COOKIE) {
        let token_hash = auth::sha256_hex(&token);
        let _ = state.app_db.lock().await.delete_session(&token_hash);
    }
    (
        StatusCode::OK,
        [(
            axum::http::header::SET_COOKIE,
            format!("{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0"),
        )],
        Json(json!({"ok": true})),
    ).into_response()
}

pub async fn session(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let authed = is_authenticated(&state, &headers).await;
    let setup_required = state.app_db.lock().await.password_hash().ok().flatten().is_none();
    (
        StatusCode::OK,
        Json(json!({
            "authenticated": authed,
            "version": crate::version(),
            "setup_required": setup_required,
        })),
    ).into_response()
}

pub async fn is_authenticated(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(token) = cookie_value(headers, SESSION_COOKIE) else { return false };
    let token_hash = auth::sha256_hex(&token);
    state.app_db.lock().await.session_exists(&token_hash).unwrap_or(false)
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let mut it = part.trim().splitn(2, '=');
        if it.next() == Some(name) {
            return it.next().map(|v| v.to_string());
        }
    }
    None
}
