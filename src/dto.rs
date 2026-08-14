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
