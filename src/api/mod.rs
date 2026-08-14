pub mod articles;
pub mod error;
pub mod feeds;
pub mod health;
pub mod sources;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/feeds", get(feeds::list))
        .route("/api/articles", get(articles::list))
        .route("/api/articles/{id}", get(articles::detail))
        .route("/api/favicon/{feed_id}", get(articles::favicon))
        .route("/api/thumbnail/{article_id}", get(articles::thumbnail))
        .route("/api/sources", post(sources::add))
        .route("/api/sources/{id}/refresh", post(sources::refresh))
        .route("/api/sources/refresh-all", post(sources::refresh_all))
        .with_state(state)
}
