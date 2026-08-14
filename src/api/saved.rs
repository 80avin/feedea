use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::api::error::ApiResult;
use crate::dto::Headline;
use crate::AppState;

#[derive(Deserialize)]
pub struct SaveRequest {
    pub note: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Deserialize)]
pub struct ListParams {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct MonthGroup {
    pub month: String,
    pub items: Vec<Headline>,
}

pub async fn save(State(state): State<AppState>, Path(id): Path<String>, Json(req): Json<SaveRequest>) -> ApiResult<Json<Value>> {
    state.engine.mark_article_saved(&id, true).await?;
    let mut app_db = state.app_db.lock().await;
    app_db.save_article(&id, req.note.as_deref(), &req.tags)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn unsave(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    state.engine.mark_article_saved(&id, false).await?;
    let mut app_db = state.app_db.lock().await;
    app_db.unsave_article(&id)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list(State(state): State<AppState>, Query(params): Query<ListParams>) -> ApiResult<Json<Value>> {
    let limit = params.limit.unwrap_or(50).clamp(1, 200);
    let offset = params.offset.unwrap_or(0).max(0);
    let (rows, _total) = state.app_db.lock().await.saved_articles(0, i64::MAX)?;
    let saved_at: HashMap<String, String> = rows.iter().cloned().collect();
    let ids: Vec<String> = rows.into_iter().map(|(id, _)| id).collect();
    let mut headlines = state.engine.get_headlines_by_ids(ids).await?;
    headlines.retain(|h| h.marked);
    let total = headlines.len() as i64;
    headlines.sort_by(|a, b| {
        let sa = saved_at.get(&a.id);
        let sb = saved_at.get(&b.id);
        sb.cmp(&sa)
    });
    let mut items: Vec<Headline> = headlines.into_iter().skip(offset as usize).take(limit as usize).collect();
    {
        let app_db = state.app_db.lock().await;
        for item in &mut items {
            let (note, tags) = app_db.note_and_tags(&item.id)?;
            item.note = note;
            item.tags = Some(tags);
        }
    }
    let mut months: Vec<MonthGroup> = Vec::new();
    for item in items {
        let month = saved_at[&item.id][..7].to_string();
        match months.last_mut() {
            Some(group) if group.month == month => group.items.push(item),
            _ => months.push(MonthGroup { month, items: vec![item] }),
        }
    }
    Ok(Json(json!({ "months": months, "total": total })))
}

pub async fn tags(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let tags = state.app_db.lock().await.all_tags()?;
    Ok(Json(json!({ "tags": tags })))
}
