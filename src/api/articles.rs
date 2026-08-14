use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum::http::{StatusCode, header};
use serde::Deserialize;

use crate::api::error::{ApiError, ApiResult};
use crate::dto::{ArticleDetail, Headline};
use crate::AppState;
use news_flash::models::{ArticleFilter, ArticleOrder, CategoryID, FeedID, OrderBy};

#[derive(Deserialize)]
pub struct ListParams {
    pub feed: Option<String>,
    pub category: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

pub async fn list(State(state): State<AppState>, Query(params): Query<ListParams>) -> ApiResult<Json<Vec<Headline>>> {
    let mut filter = ArticleFilter {
        order: Some(ArticleOrder::NewestFirst),
        order_by: Some(OrderBy::Published),
        limit: Some(params.limit.unwrap_or(30).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0).max(0)),
        ..ArticleFilter::default()
    };
    if let Some(feed) = params.feed {
        filter.feeds = Some(vec![FeedID::new(&feed)]);
    }
    if let Some(category) = params.category {
        filter.categories = Some(vec![CategoryID::new(&category)]);
    }
    Ok(Json(state.engine.get_headlines(filter).await?))
}

pub async fn detail(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<ArticleDetail>> {
    match state.engine.get_article_detail(&id).await {
        Ok(article) => Ok(Json(article)),
        Err(error) if crate::engine::is_not_found(&error) => Err(ApiError::not_found("article not found")),
        Err(error) => Err(ApiError::from(error)),
    }
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
