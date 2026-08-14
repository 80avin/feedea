pub mod health;

use crate::config::Config;
use axum::routing::get;

pub fn router(_config: Config) -> axum::Router {
    axum::Router::new().route("/api/health", get(health::health))
}
