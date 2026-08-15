use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::api::error::ApiResult;
use crate::dto::FeedSummary;

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Vec<FeedSummary>>> {
    Ok(Json(state.engine.get_feeds().await?))
}
