use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::error::ApiResult;
use crate::AppState;

#[derive(Deserialize)]
pub struct SuggestParams {
    pub q: Option<String>,
}

pub async fn suggestions(State(state): State<AppState>, Query(params): Query<SuggestParams>) -> ApiResult<Json<Value>> {
    let q = params.q.unwrap_or_default();
    if q.trim().is_empty() {
        return Ok(Json(json!({ "suggestions": [] })));
    }
    let suggestions = state.engine.search(&q, 8).await?;
    Ok(Json(json!({ "suggestions": suggestions })))
}
