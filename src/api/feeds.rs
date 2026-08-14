use axum::extract::State;
use axum::Json;

use crate::api::error::ApiResult;
use crate::dto::FeedSummary;
use crate::AppState;

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<FeedSummary>>> {
    Ok(Json(state.engine.get_feeds().await?))
}
