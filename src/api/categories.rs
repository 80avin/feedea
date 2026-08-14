use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::error::{ApiError, ApiResult};
use crate::dto::CategoryNode;
use crate::AppState;

#[derive(Deserialize)]
pub struct CreateCategory {
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateCategory {
    pub name: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteCategory {
    #[serde(default)]
    pub remove_children: bool,
}

pub async fn list(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let categories = state.engine.get_category_tree().await?;
    Ok(Json(json!({ "categories": categories })))
}

pub async fn create(State(state): State<AppState>, Json(req): Json<CreateCategory>) -> ApiResult<Json<CategoryNode>> {
    let id = state.engine.add_category(&req.name, req.parent_id.as_deref()).await?;
    let parent_id = req.parent_id.unwrap_or_else(|| news_flash::models::NEWSFLASH_TOPLEVEL.as_str().to_string());
    Ok(Json(CategoryNode {
        category_id: id,
        name: req.name,
        parent_id,
        unread_count: 0,
        children: Vec::new(),
    }))
}

pub async fn update(State(state): State<AppState>, Path(id): Path<String>, Json(req): Json<UpdateCategory>) -> ApiResult<Json<CategoryNode>> {
    ensure_exists(&state, &id).await?;
    if req.name.is_none() && req.parent_id.is_none() {
        return Err(ApiError::bad_request("nothing to update"));
    }
    if let Some(parent_id) = &req.parent_id {
        if parent_id == &id {
            return Err(ApiError::bad_request("cannot move a category into itself"));
        }
        let tree = state.engine.get_category_tree().await?;
        if let Some(node) = find_node(&tree, &id)
            && contains_node(node, parent_id)
        {
            return Err(ApiError::bad_request("cannot move a category into its own descendant"));
        }
    }
    if let Some(name) = &req.name {
        state.engine.rename_category(&id, name).await?;
    }
    if let Some(parent_id) = &req.parent_id {
        state.engine.move_category(&id, parent_id).await?;
    }
    let tree = state.engine.get_category_tree().await?;
    let node = find_node(&tree, &id).ok_or_else(|| ApiError::not_found("category not found"))?;
    Ok(Json(node.clone()))
}

pub async fn remove(State(state): State<AppState>, Path(id): Path<String>, Json(req): Json<DeleteCategory>) -> ApiResult<Json<Value>> {
    ensure_exists(&state, &id).await?;
    if req.remove_children {
        let article_ids = state.engine.category_article_ids(&id).await?;
        let mut app_db = state.app_db.lock().await;
        for article_id in article_ids {
            app_db.unsave_article(&article_id)?;
        }
    }
    state.engine.remove_category(&id, req.remove_children).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn mark_read(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    ensure_exists(&state, &id).await?;
    state.engine.mark_category_read(&id).await?;
    Ok(Json(json!({ "ok": true })))
}

async fn ensure_exists(state: &AppState, id: &str) -> Result<(), ApiError> {
    let (categories, _) = state.engine.get_categories().await?;
    if categories.iter().any(|c| c.category_id.as_str() == id) {
        Ok(())
    } else {
        Err(ApiError::not_found("category not found"))
    }
}

fn find_node<'a>(nodes: &'a [CategoryNode], id: &str) -> Option<&'a CategoryNode> {
    for node in nodes {
        if node.category_id == id {
            return Some(node);
        }
        if let Some(found) = find_node(&node.children, id) {
            return Some(found);
        }
    }
    None
}

fn contains_node(node: &CategoryNode, id: &str) -> bool {
    if node.category_id == id {
        return true;
    }
    node.children.iter().any(|child| contains_node(child, id))
}
