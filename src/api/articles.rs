use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::http::{StatusCode, header};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::error::{ApiError, ApiResult};
use crate::dto::{ArticleDetail, Headline};
use crate::AppState;
use news_flash::models::{ArticleFilter, ArticleOrder, CategoryID, FeedID, Marked, OrderBy};

#[derive(Deserialize)]
pub struct ListParams {
    pub feed: Option<String>,
    pub category: Option<String>,
    pub saved: Option<String>,
    pub tag: Option<String>,
    pub search: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn list(State(state): State<AppState>, Query(params): Query<ListParams>) -> ApiResult<Json<Vec<Headline>>> {
    let limit = params.limit.unwrap_or(30).clamp(1, 200);
    if let Some(q) = params.search
        && !q.trim().is_empty()
    {
        return Ok(Json(state.engine.search(&q, limit).await?));
    }
    let mut filter = ArticleFilter {
        order: Some(ArticleOrder::NewestFirst),
        order_by: Some(OrderBy::Published),
        limit: Some(limit),
        offset: Some(params.offset.unwrap_or(0).max(0)),
        ..ArticleFilter::default()
    };
    if let Some(feed) = params.feed {
        filter.feeds = Some(vec![FeedID::new(&feed)]);
    }
    if let Some(category) = params.category {
        filter.categories = Some(vec![CategoryID::new(&category)]);
    }
    if let Some(saved) = params.saved {
        filter.marked = Some(match saved.as_str() {
            "true" | "1" => Marked::Marked,
            "false" | "0" => Marked::Unmarked,
            _ => return Err(ApiError::bad_request("saved must be true/false or 1/0")),
        });
    }
    if let Some(tag) = params.tag {
        let ids = state.app_db.lock().await.article_ids_for_tag(&tag)?;
        if ids.is_empty() {
            return Ok(Json(Vec::new()));
        }
        filter.ids = Some(ids.into_iter().map(|id| news_flash::models::ArticleID::new(&id)).collect());
    }
    Ok(Json(state.engine.get_headlines(filter).await?))
}

#[derive(Deserialize)]
pub struct PatchArticleRequest {
    pub read: Option<bool>,
    pub saved: Option<bool>,
}

#[derive(Deserialize)]
pub struct MarkReadRequest {
    pub read: Option<bool>,
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<ArticleDetail>> {
    let mut article = state.engine.get_article_detail(&id).await?;
    let (note, tags) = state.app_db.lock().await.note_and_tags(&id)?;
    article.note = note;
    article.tags = tags;
    state.engine.render_article_content(&mut article).await?;
    Ok(Json(article))
}

pub async fn patch_article(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchArticleRequest>,
) -> ApiResult<Json<Value>> {
    if req.read.is_none() && req.saved.is_none() {
        return Err(ApiError::bad_request("nothing to update"));
    }
    if let Some(read) = req.read {
        state.engine.set_article_read(&id, read).await?;
    }
    if let Some(saved) = req.saved {
        state.engine.mark_article_saved(&id, saved).await?;
        let mut app_db = state.app_db.lock().await;
        if saved {
            app_db.save_article(&id, None, &[])?;
        } else {
            app_db.unsave_article(&id)?;
        }
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn mark_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<MarkReadRequest>,
) -> ApiResult<Json<Value>> {
    state.engine.set_article_read(&id, req.read.unwrap_or(true)).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn unread(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    state.engine.set_article_read(&id, false).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn read_all(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    state.engine.mark_all_read().await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn favicon(State(state): State<AppState>, Path(feed_id): Path<String>) -> Response {
    match state.engine.get_favicon(&feed_id).await {
        Ok(Some((content_type, data))) => (
            [(header::CONTENT_TYPE, content_type)],
            data,
        ).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn thumbnail(State(state): State<AppState>, Path(article_id): Path<String>) -> Response {
    match state.engine.get_article_thumbnail(&article_id).await {
        Ok(Some((content_type, data))) => (
            [(header::CONTENT_TYPE, content_type)],
            data,
        ).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
