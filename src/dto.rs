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
}
