use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
pub struct FeedSummary {
    pub id: String,
    pub title: String,
    pub website: Option<String>,
    pub feed_url: Option<String>,
    pub icon_url: Option<String>,
    pub category_id: String,
    pub unread_count: i64,
    pub error_count: i32,
    pub error_message: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct SourceGroup {
    pub category_id: String,
    pub category_name: String,
    pub feeds: Vec<FeedSummary>,
}

#[derive(Serialize, Debug, Clone)]
pub struct CategoryCard {
    pub category_id: String,
    pub name: String,
    pub total_count: i64,
    pub unread_count: i64,
    pub items: Vec<Headline>,
}

#[derive(Serialize, Debug, Clone)]
pub struct CategoryNode {
    pub category_id: String,
    pub name: String,
    pub parent_id: String,
    pub unread_count: i64,
    pub children: Vec<CategoryNode>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Headline {
    pub id: String,
    pub title: Option<String>,
    pub feed_id: String,
    pub feed_title: Option<String>,
    pub url: Option<String>,
    pub date: chrono::DateTime<chrono::Utc>,
    pub summary: Option<String>,
    pub thumbnail_url: Option<String>,
    pub unread: bool,
    pub marked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ArticleDetail {
    pub id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub feed_id: String,
    pub feed_title: Option<String>,
    pub url: Option<String>,
    pub date: chrono::DateTime<chrono::Utc>,
    pub html: Option<String>,
    pub summary: Option<String>,
    pub unread: bool,
    pub marked: bool,
    pub thumbnail_url: Option<String>,
    pub plain_text: Option<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct SettingsStats {
    pub feeds: usize,
    pub articles: i64,
    pub unread: i64,
    pub database_size_bytes: u64,
    pub last_sync: String,
}

#[derive(Serialize, Debug)]
pub struct Settings {
    pub theme: Option<String>,
    pub sync_interval_minutes: i64,
    pub keep_articles_days: Option<i64>,
    pub stats: SettingsStats,
}
