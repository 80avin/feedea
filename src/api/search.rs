use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::error::ApiResult;

#[derive(Deserialize)]
pub struct SuggestParams {
    pub q: Option<String>,
}

pub async fn suggestions(
    State(state): State<AppState>,
    Query(params): Query<SuggestParams>,
) -> ApiResult<Json<Value>> {
    let q = params.q.unwrap_or_default();
    if q.trim().is_empty() {
        return Ok(Json(json!({ "suggestions": [] })));
    }
    let suggestions = state.engine.search(&q, 8).await?;
    Ok(Json(json!({ "suggestions": suggestions })))
}
