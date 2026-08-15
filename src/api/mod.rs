pub mod articles;
pub mod auth;
pub mod categories;
pub mod error;
pub mod feeds;
pub mod health;
pub mod overview;
pub mod saved;
pub mod search;
pub mod settings;
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
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": {"code": "unauthorized", "message": "not logged in"}})),
        )
            .into_response()
    }
}

pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/feeds", get(feeds::list))
        .route("/api/overview", get(overview::overview))
        .route("/api/articles", get(articles::list))
        .route("/api/search/suggestions", get(search::suggestions))
        .route(
            "/api/articles/{id}",
            get(articles::detail).patch(articles::patch_article),
        )
        .route("/api/articles/{id}/read", post(articles::mark_read))
        .route("/api/articles/{id}/unread", post(articles::unread))
        .route(
            "/api/articles/{id}/save",
            post(saved::save).put(saved::save).delete(saved::unsave),
        )
        .route("/api/read-all", post(articles::read_all))
        .route("/api/saved", get(saved::list))
        .route("/api/tags", get(saved::tags))
        .route(
            "/api/categories",
            get(categories::list).post(categories::create),
        )
        .route(
            "/api/categories/{id}",
            patch(categories::update).delete(categories::remove),
        )
        .route("/api/categories/{id}/read", post(categories::mark_read))
        .route("/api/favicon/{feed_id}", get(articles::favicon))
        .route("/api/thumbnail/{article_id}", get(articles::thumbnail))
        .route("/api/sources", get(sources::list).post(sources::add))
        .route("/api/sources/discover", post(sources::discover))
        .route("/api/sources/import-opml", post(sources::import_opml))
        .route("/api/sources/export-opml", get(sources::export_opml))
        .route(
            "/api/sources/{id}",
            patch(sources::update).delete(sources::remove),
        )
        .route("/api/sources/{id}/read", post(sources::mark_read))
        .route("/api/sources/{id}/refresh", post(sources::refresh))
        .route("/api/sources/refresh-all", post(sources::refresh_all))
        .route("/api/settings", get(settings::get).patch(settings::update))
        .route("/api/settings/password", post(settings::change_password))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ));

    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/login", post(auth::login))
        .route("/api/logout", post(auth::logout))
        .route("/api/session", get(auth::session))
        .route("/img", get(crate::proxy::proxy_image))
        .merge(protected)
        .fallback(crate::assets::fallback)
        .with_state(state)
}
