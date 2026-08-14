pub mod articles;
pub mod auth;
pub mod categories;
pub mod error;
pub mod feeds;
pub mod health;
pub mod overview;
pub mod saved;
pub mod search;
pub mod sources;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde_json::json;

use crate::AppState;

pub async fn require_auth(
    State(state): State<AppState>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    if crate::api::auth::is_authenticated(&state, req.headers()).await {
        next.run(req).await
    } else {
        (StatusCode::UNAUTHORIZED, Json(json!({"error": {"code": "unauthorized", "message": "not logged in"}}))).into_response()
    }
}

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/feeds", get(feeds::list))
        .route("/api/overview", get(overview::overview))
        .route("/api/articles", get(articles::list))
        .route("/api/search/suggestions", get(search::suggestions))
        .route("/api/articles/{id}", get(articles::detail))
        .route("/api/articles/{id}/save", post(saved::save).put(saved::save).delete(saved::unsave))
        .route("/api/saved", get(saved::list))
        .route("/api/tags", get(saved::tags))
        .route("/api/categories", get(categories::list).post(categories::create))
        .route("/api/categories/{id}", patch(categories::update).delete(categories::remove))
        .route("/api/categories/{id}/read", post(categories::mark_read))
        .route("/api/favicon/{feed_id}", get(articles::favicon))
        .route("/api/thumbnail/{article_id}", get(articles::thumbnail))
        .route("/api/sources", post(sources::add))
        .route("/api/sources/{id}/refresh", post(sources::refresh))
        .route("/api/sources/refresh-all", post(sources::refresh_all))
        .route_layer(axum::middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/login", post(auth::login))
        .route("/api/logout", post(auth::logout))
        .route("/api/session", get(auth::session))
        .merge(protected)
        .with_state(state)
}
