use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::error::ApiResult;
use crate::dto::CategoryCard;
use news_flash::models::{ArticleFilter, ArticleOrder, FeedID, OrderBy};

const MAX_OVERVIEW_ITEMS: i64 = 6;

pub async fn overview(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let index = state.engine.category_index().await?;

    let db_path = state.engine.data_dir().join("engine/data/database.sqlite");
    let totals = {
        let db_path = db_path.clone();
        tokio::task::spawn_blocking(move || {
            crate::engine::queries::direct_category_totals_map(&db_path)
        })
        .await
        .map_err(anyhow::Error::from)??
    };

    let card_ids = index.card_ids();

    // Group directly-mapped feeds by their resolved card category so each card
    // can fetch ITS OWN newest articles. A single batched query with a global
    // window starves categories whose articles fall outside it; per-card
    // queries guarantee each card's previews.
    let mut feed_ids_by_card: HashMap<String, Vec<FeedID>> = HashMap::new();
    for (feed_id, category_id) in &index.cat_by_feed {
        feed_ids_by_card
            .entry(category_id.clone())
            .or_default()
            .push(FeedID::new(feed_id));
    }

    let mut tasks = tokio::task::JoinSet::new();
    for id in &card_ids {
        let Some(feed_ids) = feed_ids_by_card.get(id) else {
            continue;
        };
        let engine = state.engine.clone();
        let id = id.clone();
        let feed_ids = feed_ids.clone();
        tasks.spawn(async move {
            let filter = ArticleFilter {
                order: Some(ArticleOrder::NewestFirst),
                order_by: Some(OrderBy::Published),
                limit: Some(MAX_OVERVIEW_ITEMS),
                feeds: Some(feed_ids),
                ..ArticleFilter::default()
            };
            let result = engine.with_nf(move |nf| nf.get_fat_articles(filter)).await;
            (id, result)
        });
    }

    let mut items_by_card: HashMap<String, Vec<crate::dto::Headline>> = HashMap::new();
    while let Some(joined) = tasks.join_next().await {
        let (id, result) = joined.map_err(anyhow::Error::from)?;
        let articles = result?;
        let headlines =
            crate::engine::Engine::headlines_from_articles(articles, &index.title_by_feed);
        items_by_card.insert(id, headlines);
    }

    let mut cards = Vec::with_capacity(card_ids.len());
    for id in &card_ids {
        let (total, _) = totals.get(id).copied().unwrap_or((0, 0));
        cards.push(CategoryCard {
            category_id: id.clone(),
            name: index.display_name(id),
            total_count: total,
            unread_count: index.unread_direct.get(id).copied().unwrap_or(0),
            items: items_by_card.remove(id).unwrap_or_default(),
        });
    }

    let all_total = {
        tokio::task::spawn_blocking(move || crate::engine::queries::total_article_count(&db_path))
            .await
            .map_err(anyhow::Error::from)??
    };
    let all_unread = state.engine.with_nf(|nf| nf.unread_count_all()).await?;

    Ok(Json(json!({
        "cards": cards,
        "all": { "total_count": all_total, "unread_count": all_unread },
    })))
}
