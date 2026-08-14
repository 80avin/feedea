use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::api::error::{ApiError, ApiResult};
use crate::dto::FeedSummary;
use crate::AppState;

#[derive(Deserialize)]
pub struct AddSourceRequest {
    pub url: String,
    pub title: Option<String>,
    pub category_id: Option<String>,
}

pub async fn add(State(state): State<AppState>, Json(req): Json<AddSourceRequest>) -> ApiResult<Json<FeedSummary>> {
    url::Url::parse(&req.url).map_err(|_| ApiError::bad_request("invalid url"))?;
    Ok(Json(state.engine.add_feed(&req.url, req.title, req.category_id).await?))
}

pub async fn refresh(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<serde_json::Value>> {
    let feeds = state.engine.get_feeds().await?;
    if !feeds.iter().any(|feed| feed.id == id) {
        return Err(ApiError::not_found("feed not found"));
    }
    let count = state.engine.fetch_feed(&id).await?;
    Ok(Json(serde_json::json!({ "new_articles": count })))
}

pub async fn refresh_all(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let counts = state.engine.sync_all().await?;
    Ok(Json(serde_json::json!({ "feeds": counts })))
}
