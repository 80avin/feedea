use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde::de::Deserializer;
use serde_json::{Value, json};

use crate::AppState;
use crate::api::error::{ApiError, ApiResult};
use crate::auth;
use crate::dto::{Settings, SettingsStats};

fn deserialize_keep_articles_days<'de, D>(deserializer: D) -> Result<Option<Option<i64>>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(Option::<i64>::deserialize(deserializer)?))
}

#[derive(Deserialize)]
pub struct UpdateSettings {
    pub theme: Option<String>,
    pub accent: Option<String>,
    pub sync_interval_minutes: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_keep_articles_days")]
    pub keep_articles_days: Option<Option<i64>>,
}

#[derive(Deserialize)]
pub struct PasswordChange {
    pub current_password: String,
    pub new_password: String,
}

pub async fn get(State(state): State<AppState>) -> ApiResult<Json<Settings>> {
    let app_db = state.app_db.lock().await;
    let theme = app_db.theme()?;
    let accent = app_db.accent()?;
    let sync_interval_minutes = app_db.sync_interval_minutes()?;
    drop(app_db);
    let keep_articles_days = state.engine.keep_articles_days().await?;
    let stats = build_stats(&state).await?;
    Ok(Json(Settings {
        theme,
        accent,
        sync_interval_minutes,
        keep_articles_days,
        stats,
    }))
}

pub async fn update(
    State(state): State<AppState>,
    Json(req): Json<UpdateSettings>,
) -> ApiResult<Json<Value>> {
    if let Some(theme) = &req.theme {
        state.app_db.lock().await.set_theme(theme)?;
    }
    if let Some(accent) = &req.accent {
        state.app_db.lock().await.set_accent(accent)?;
    }
    if let Some(minutes) = req.sync_interval_minutes {
        if minutes <= 0 {
            return Err(ApiError::bad_request(
                "sync_interval_minutes must be positive",
            ));
        }
        state
            .app_db
            .lock()
            .await
            .set_sync_interval_minutes(minutes)?;
    }
    if let Some(days) = req.keep_articles_days {
        state.engine.set_keep_articles_days(days).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn change_password(
    State(state): State<AppState>,
    Json(req): Json<PasswordChange>,
) -> ApiResult<Json<Value>> {
    let hash = state
        .app_db
        .lock()
        .await
        .password_hash()?
        .ok_or_else(|| ApiError::forbidden("no password configured"))?;
    if !auth::verify_password(&req.current_password, &hash) {
        return Err(ApiError::unauthorized("wrong password"));
    }
    let new_hash = auth::hash_password(&req.new_password)?;
    state.app_db.lock().await.set_password_hash(&new_hash)?;
    Ok(Json(json!({ "ok": true })))
}

async fn build_stats(state: &AppState) -> ApiResult<SettingsStats> {
    let feeds = state.engine.get_feeds().await?.len();
    let db_path = state.engine.data_dir().join("engine/data/database.sqlite");
    let articles = crate::engine::queries::total_article_count(&db_path)?;
    let unread = state.engine.with_nf(|nf| nf.unread_count_all()).await?;
    let database_size_bytes = state.engine.database_size_bytes().await?;
    let last_sync = state.engine.last_sync().await.to_rfc3339();
    Ok(SettingsStats {
        feeds,
        articles,
        unread,
        database_size_bytes,
        last_sync,
    })
}
