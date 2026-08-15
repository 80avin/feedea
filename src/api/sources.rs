use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::dto::{FeedSummary, SourceGroup};

#[derive(Deserialize)]
pub struct AddSourceRequest {
    pub url: String,
    pub title: Option<String>,
    pub category_id: Option<String>,
}

#[derive(Deserialize)]
pub struct DiscoverRequest {
    pub url: String,
}

#[derive(Deserialize)]
pub struct UpdateSourceRequest {
    pub title: Option<String>,
    pub category_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ImportOpmlRequest {
    pub opml: String,
}

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let feeds = state.engine.get_feeds().await?;
    let (categories, _) = state.engine.get_categories().await?;
    let name_by_id: HashMap<String, String> = categories
        .into_iter()
        .map(|c| (c.category_id.as_str().to_string(), c.label))
        .collect();
    let mut groups: Vec<SourceGroup> = Vec::new();
    for feed in feeds {
        let category_id = feed.category_id.clone();
        let category_name = name_by_id
            .get(&category_id)
            .cloned()
            .unwrap_or_else(|| category_id.clone());
        if let Some(group) = groups.iter_mut().find(|g| g.category_id == category_id) {
            group.feeds.push(feed);
        } else {
            groups.push(SourceGroup {
                category_id,
                category_name,
                feeds: vec![feed],
            });
        }
    }
    Ok(Json(json!({ "groups": groups })))
}

pub async fn add(
    State(state): State<AppState>,
    Json(req): Json<AddSourceRequest>,
) -> ApiResult<Json<FeedSummary>> {
    url::Url::parse(&req.url).map_err(|_| ApiError::bad_request("invalid url"))?;
    let discovered = state.engine.discover(&req.url).await?;
    let title = discovered.title.or(req.title.clone());
    let feed = match discovered.feed_url {
        Some(feed_url) => {
            state
                .engine
                .add_feed(&feed_url, title, req.category_id)
                .await?
        }
        None => {
            state
                .engine
                .add_feed(&req.url, req.title, req.category_id)
                .await?
        }
    };
    Ok(Json(feed))
}

pub async fn discover(
    State(state): State<AppState>,
    Json(req): Json<DiscoverRequest>,
) -> ApiResult<Json<Value>> {
    url::Url::parse(&req.url).map_err(|_| ApiError::bad_request("invalid url"))?;
    let discovered = state.engine.discover(&req.url).await?;
    Ok(Json(json!({
        "title": discovered.title,
        "feed_url": discovered.feed_url,
        "alternatives": discovered.alternatives.into_iter().map(|(label, url)| json!({ "label": label, "url": url })).collect::<Vec<_>>(),
    })))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSourceRequest>,
) -> ApiResult<Json<FeedSummary>> {
    ensure_feed_exists(&state, &id).await?;
    if req.title.is_none() && req.category_id.is_none() {
        return Err(ApiError::bad_request("nothing to update"));
    }
    if let Some(title) = &req.title {
        state.engine.rename_feed(&id, title).await?;
    }
    if let Some(category_id) = &req.category_id {
        state.engine.move_feed(&id, category_id).await?;
    }
    let feeds = state.engine.get_feeds().await?;
    let feed = feeds
        .into_iter()
        .find(|f| f.id == id)
        .ok_or_else(|| ApiError::not_found("feed not found"))?;
    Ok(Json(feed))
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    ensure_feed_exists(&state, &id).await?;
    let article_ids = state.engine.feed_article_ids(&id).await?;
    let mut app_db = state.app_db.lock().await;
    for article_id in article_ids {
        app_db.unsave_article(&article_id)?;
    }
    drop(app_db);
    state.engine.remove_feed(&id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn mark_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    ensure_feed_exists(&state, &id).await?;
    state.engine.mark_feed_read(&id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn import_opml(
    State(state): State<AppState>,
    Json(req): Json<ImportOpmlRequest>,
) -> ApiResult<Json<Value>> {
    state.engine.import_opml(&req.opml).await?;
    Ok(Json(json!({ "imported": true })))
}

pub async fn export_opml(State(state): State<AppState>) -> ApiResult<impl IntoResponse> {
    let xml = state.engine.export_opml().await?;
    Ok(([(header::CONTENT_TYPE, "text/xml")], xml))
}

pub async fn refresh(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
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

async fn ensure_feed_exists(state: &AppState, id: &str) -> Result<(), ApiError> {
    let feeds = state.engine.get_feeds().await?;
    if feeds.iter().any(|feed| feed.id == id) {
        Ok(())
    } else {
        Err(ApiError::not_found("feed not found"))
    }
}
