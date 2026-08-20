use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::error::ApiResult;
use crate::dto::CategoryCard;
use news_flash::models::{ArticleFilter, ArticleOrder, CategoryID, OrderBy};

pub async fn overview(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let (categories, category_mappings) = state.engine.get_categories().await?;
    let path_by_id = crate::engine::opml_import::category_paths(&categories, &category_mappings);
    let db_path = state.engine.data_dir().join("engine/data/database.sqlite");
    let totals = crate::engine::queries::category_totals(&db_path)?;
    let category_unread = state.engine.category_unread_map().await?;

    let mut seen = std::collections::HashSet::new();
    let mut cards = Vec::new();
    for cat in categories {
        seen.insert(cat.category_id.as_str().to_string());
        let name = path_by_id
            .get(cat.category_id.as_str())
            .filter(|p| !p.is_empty())
            .map(|p| p.join(" / "))
            .unwrap_or_else(|| cat.label.clone());
        cards.push(build_card(&state, &cat.category_id, name, &totals, &category_unread).await?);
    }
    for t in &totals {
        if seen.insert(t.category_id.clone()) {
            let name = path_by_id
                .get(&t.category_id)
                .filter(|p| !p.is_empty())
                .map(|p| p.join(" / "))
                .unwrap_or_else(|| t.category_id.clone());
            cards.push(
                build_card(
                    &state,
                    &CategoryID::new(&t.category_id),
                    name,
                    &totals,
                    &category_unread,
                )
                .await?,
            );
        }
    }

    let all_total = crate::engine::queries::total_article_count(&db_path)?;
    let all_unread = state.engine.with_nf(|nf| nf.unread_count_all()).await?;

    Ok(Json(json!({
        "cards": cards,
        "all": { "total_count": all_total, "unread_count": all_unread },
    })))
}

async fn build_card(
    state: &AppState,
    category_id: &CategoryID,
    name: String,
    totals: &[crate::engine::queries::CategoryTotals],
    category_unread: &std::collections::HashMap<String, i64>,
) -> ApiResult<CategoryCard> {
    let filter = ArticleFilter {
        order: Some(ArticleOrder::NewestFirst),
        order_by: Some(OrderBy::Published),
        limit: Some(10),
        categories: Some(vec![category_id.clone()]),
        ..ArticleFilter::default()
    };
    let items = state.engine.get_headlines(filter).await?;
    Ok(CategoryCard {
        category_id: category_id.as_str().to_string(),
        name,
        total_count: totals
            .iter()
            .find(|t| t.category_id == category_id.as_str())
            .map(|t| t.total)
            .unwrap_or(0),
        unread_count: category_unread
            .get(category_id.as_str())
            .copied()
            .unwrap_or(0),
        items,
    })
}
