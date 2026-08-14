use std::collections::HashMap;
use std::sync::Arc;

use news_flash::error::NewsFlashError;
use news_flash::feed_api::FeedHeaderMap;
use news_flash::models::{ArticleFilter, CategoryID, FeedID, FeedMapping, Marked, PluginID, Read, Url};
use news_flash::NewsFlash;

pub struct Discovered {
    pub title: Option<String>,
    pub feed_url: Option<String>,
    pub alternatives: Vec<(String, String)>,
}

use crate::config::Config;
use crate::dto::{ArticleDetail, FeedSummary, Headline};

pub mod content;
pub mod queries;
pub mod sync;

#[derive(Clone)]
pub struct Engine {
    nf: Arc<NewsFlash>,
    client: reqwest::Client,
    mutation_lock: Arc<tokio::sync::Mutex<()>>,
    data_dir: std::path::PathBuf,
}

impl Engine {
    pub async fn new(config: &Config) -> anyhow::Result<Engine> {
        config.ensure_data_dir()?;
        let engine_dir = config.data_dir.join("engine");
        let config_dir = engine_dir.join("config");
        let data_dir = engine_dir.join("data");
        let nf = tokio::task::spawn_blocking(move || {
            NewsFlash::builder()
                .plugin(PluginID::new("local_rss"))
                .config_dir(&config_dir)
                .data_dir(&data_dir)
                .create()
                .map_err(anyhow::Error::from)
        })
        .await??;
        Ok(Engine {
            nf: Arc::new(nf),
            client: reqwest::Client::new(),
            mutation_lock: Arc::new(tokio::sync::Mutex::new(())),
            data_dir: config.data_dir.clone(),
        })
    }

    pub fn data_dir(&self) -> &std::path::Path {
        &self.data_dir
    }

    pub async fn with_nf<T, F>(&self, f: F) -> anyhow::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&NewsFlash) -> Result<T, NewsFlashError> + Send + 'static,
    {
        let nf = self.nf.clone();
        tokio::task::spawn_blocking(move || f(&nf))
            .await?
            .map_err(anyhow::Error::from)
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn mutation_guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.mutation_lock.lock().await
    }

    pub async fn last_sync(&self) -> chrono::DateTime<chrono::Utc> {
        self.nf.last_sync().await
    }

    pub async fn keep_articles_days(&self) -> anyhow::Result<Option<i64>> {
        let dur = self.nf.get_keep_articles_duration().await;
        Ok(dur.map(|d| d.num_days()))
    }

    pub async fn set_keep_articles_days(&self, days: Option<i64>) -> anyhow::Result<()> {
        let dur = days.map(chrono::Duration::days);
        self.nf.set_keep_articles_duration(dur).await?;
        Ok(())
    }

    pub async fn database_size_bytes(&self) -> anyhow::Result<u64> {
        let size = self.with_nf(|nf| nf.database_size()).await?;
        Ok(size.on_disk)
    }

    pub async fn add_feed(
        &self,
        url: &str,
        title: Option<String>,
        category_id: Option<String>,
    ) -> anyhow::Result<FeedSummary> {
        let url = Url::parse(url)?;
        let category_id = category_id.map(|c| CategoryID::new(&c));
        let _guard = self.mutation_guard().await;
        let (feed, feed_mapping, _, _) = self.nf.add_feed(&url, title, category_id, &self.client).await?;
        let feed_id = feed.feed_id.clone();
        if let Err(error) = self.nf.fetch_feed(&feed_id, &self.client, reqwest::header::HeaderMap::new()).await {
            tracing::warn!(%error, "initial sync of new feed failed");
        }
        Ok(FeedSummary {
            id: feed.feed_id.as_str().to_string(),
            title: feed.label,
            website: feed.website.map(|u| u.to_string()),
            feed_url: feed.feed_url.map(|u| u.to_string()),
            icon_url: feed.icon_url.map(|u| u.to_string()),
            category_id: feed_mapping.category_id.as_str().to_string(),
            unread_count: 0,
            error_count: feed.error_count,
            error_message: feed.error_message,
        })
    }

    pub async fn sync_all(&self) -> anyhow::Result<HashMap<String, i64>> {
        let _guard = self.mutation_guard().await;
        let header: FeedHeaderMap = HashMap::new();
        let counts = self.nf.sync(&self.client, header).await?;
        Ok(counts.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
    }

    pub async fn fetch_feed(&self, feed_id: &str) -> anyhow::Result<i64> {
        let _guard = self.mutation_guard().await;
        let feed_id = FeedID::new(feed_id);
        Ok(self.nf.fetch_feed(&feed_id, &self.client, reqwest::header::HeaderMap::new()).await?)
    }

    pub async fn discover(&self, url: &str) -> anyhow::Result<Discovered> {
        let url = news_flash::models::Url::parse(url)?;
        let id = news_flash::models::FeedID::new(url.as_str());
        let semaphore = self.nf.get_semaphore();
        let parsed = news_flash::feed_parser::download_and_parse_feed(&url, &id, None, semaphore, &self.client).await;
        match parsed {
            Ok(news_flash::feed_parser::ParsedUrl::SingleFeed(feed)) => {
                Ok(Discovered {
                    title: Some(feed.label),
                    feed_url: feed.feed_url.map(|u| u.to_string()),
                    alternatives: Vec::new(),
                })
            }
            Ok(news_flash::feed_parser::ParsedUrl::MultipleFeeds(feeds)) => {
                let first = feeds.first();
                Ok(Discovered {
                    title: first.map(|f| f.label.clone()),
                    feed_url: first.and_then(|f| f.feed_url.clone()).map(|u| u.to_string()),
                    alternatives: feeds.into_iter().map(|f| (f.label, f.feed_url.map(|u| u.to_string()).unwrap_or_default())).collect(),
                })
            }
            Err(_) => Ok(Discovered { title: None, feed_url: None, alternatives: Vec::new() }),
        }
    }

    pub async fn rename_feed(&self, id: &str, title: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.rename_feed(&FeedID::new(id), title, &self.client).await?;
        Ok(())
    }

    pub async fn move_feed(&self, id: &str, to_category: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        let (_, mappings) = self.with_nf(|nf| nf.get_feeds()).await?;
        let from = mappings
            .into_iter()
            .find(|m| m.feed_id.as_str() == id)
            .ok_or_else(|| anyhow::anyhow!("feed not found"))?;
        let to = FeedMapping {
            feed_id: from.feed_id.clone(),
            category_id: news_flash::models::CategoryID::new(to_category),
            sort_index: from.sort_index,
        };
        self.nf.move_feed(&from, &to, &self.client).await?;
        Ok(())
    }

    pub async fn remove_feed(&self, id: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.remove_feed(&FeedID::new(id), &self.client).await?;
        Ok(())
    }

    pub async fn mark_feed_read(&self, id: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.set_feed_read(&[FeedID::new(id)], &self.client).await?;
        Ok(())
    }

    pub async fn import_opml(&self, opml: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.import_opml(opml, true, &self.client).await?;
        Ok(())
    }

    pub async fn export_opml(&self) -> anyhow::Result<String> {
        self.nf.export_opml().await.map_err(anyhow::Error::from)
    }

    pub async fn feed_article_ids(&self, feed_id: &str) -> anyhow::Result<Vec<String>> {
        let id = FeedID::new(feed_id);
        let filter = ArticleFilter {
            feeds: Some(vec![id]),
            ..ArticleFilter::default()
        };
        let ids = self.with_nf(move |nf| nf.get_article_ids(filter)).await?;
        Ok(ids.into_iter().map(|a| a.as_str().to_string()).collect())
    }

    pub async fn get_feeds(&self) -> anyhow::Result<Vec<FeedSummary>> {
        let unread = self.with_nf(|nf| nf.unread_count_feed_map(false)).await?;
        let (feeds, mappings) = self.with_nf(|nf| nf.get_feeds()).await?;
        let category_by_feed: HashMap<String, String> = mappings
            .into_iter()
            .map(|m| (m.feed_id.as_str().to_string(), m.category_id.as_str().to_string()))
            .collect();
        let mut out = Vec::with_capacity(feeds.len());
        for feed in feeds {
            let id = feed.feed_id.as_str().to_string();
            out.push(FeedSummary {
                category_id: category_by_feed.get(&id).cloned().unwrap_or_else(|| "NewsFlash.Toplevel".to_string()),
                unread_count: unread.get(&feed.feed_id).copied().unwrap_or(0),
                id,
                title: feed.label,
                website: feed.website.map(|u| u.to_string()),
                feed_url: feed.feed_url.map(|u| u.to_string()),
                icon_url: feed.icon_url.map(|u| u.to_string()),
                error_count: feed.error_count,
                error_message: feed.error_message,
            });
        }
        Ok(out)
    }

    pub async fn get_categories(
        &self,
    ) -> anyhow::Result<(
        Vec<news_flash::models::Category>,
        Vec<news_flash::models::CategoryMapping>,
    )> {
        self.with_nf(|nf| nf.get_categories()).await
    }

    pub async fn category_unread_map(&self) -> anyhow::Result<std::collections::HashMap<String, i64>> {
        let unread = self.with_nf(|nf| nf.unread_count_feed_map(false)).await?;
        let (_, mappings) = self.with_nf(|nf| nf.get_feeds()).await?;
        let mut out: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for m in mappings {
            let cat = m.category_id.as_str().to_string();
            let count = unread.get(&m.feed_id).copied().unwrap_or(0);
            *out.entry(cat).or_insert(0) += count;
        }
        Ok(out)
    }

    pub async fn add_category(&self, name: &str, parent: Option<&str>) -> anyhow::Result<String> {
        let _guard = self.mutation_guard().await;
        let parent = parent.map(news_flash::models::CategoryID::new);
        let (category, _) = self.nf.add_category(name, parent.as_ref(), &self.client).await?;
        Ok(category.category_id.as_str().to_string())
    }

    pub async fn rename_category(&self, id: &str, name: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.rename_category(&news_flash::models::CategoryID::new(id), name, &self.client).await?;
        Ok(())
    }

    pub async fn remove_category(&self, id: &str, remove_children: bool) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.remove_category(&news_flash::models::CategoryID::new(id), remove_children, &self.client).await?;
        Ok(())
    }

    pub async fn move_category(&self, id: &str, parent_id: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        let mapping = news_flash::models::CategoryMapping {
            parent_id: news_flash::models::CategoryID::new(parent_id),
            category_id: news_flash::models::CategoryID::new(id),
            sort_index: Some(i32::MAX),
        };
        self.nf.move_category(&mapping, &self.client).await?;
        Ok(())
    }

    pub async fn mark_category_read(&self, id: &str) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.set_category_read(&[news_flash::models::CategoryID::new(id)], &self.client).await?;
        Ok(())
    }

    pub async fn category_article_ids(&self, category_id: &str) -> anyhow::Result<Vec<String>> {
        let id = news_flash::models::CategoryID::new(category_id);
        let filter = news_flash::models::ArticleFilter {
            categories: Some(vec![id]),
            ..news_flash::models::ArticleFilter::default()
        };
        let ids = self.with_nf(move |nf| nf.get_article_ids(filter)).await?;
        Ok(ids.into_iter().map(|a| a.as_str().to_string()).collect())
    }

    pub async fn get_category_tree(&self) -> anyhow::Result<Vec<crate::dto::CategoryNode>> {
        let (categories, mappings) = self.get_categories().await?;
        let unread = self.category_unread_map().await?;
        let mut children_by_parent: HashMap<String, Vec<news_flash::models::CategoryID>> = HashMap::new();
        for mapping in mappings {
            children_by_parent
                .entry(mapping.parent_id.as_str().to_string())
                .or_default()
                .push(mapping.category_id);
        }
        let name_by_id: HashMap<String, String> = categories
            .into_iter()
            .map(|c| (c.category_id.as_str().to_string(), c.label))
            .collect();

        fn build(
            parent_id: &str,
            children_by_parent: &HashMap<String, Vec<news_flash::models::CategoryID>>,
            name_by_id: &HashMap<String, String>,
            unread: &HashMap<String, i64>,
            depth: usize,
        ) -> Vec<crate::dto::CategoryNode> {
            if depth >= 100 {
                return Vec::new();
            }
            let mut nodes = Vec::new();
            if let Some(children) = children_by_parent.get(parent_id) {
                for child in children {
                    let id = child.as_str().to_string();
                    nodes.push(crate::dto::CategoryNode {
                        category_id: id.clone(),
                        name: name_by_id.get(&id).cloned().unwrap_or_default(),
                        parent_id: parent_id.to_string(),
                        unread_count: unread.get(&id).copied().unwrap_or(0),
                        children: build(&id, children_by_parent, name_by_id, unread, depth + 1),
                    });
                }
            }
            nodes
        }

        Ok(build(
            news_flash::models::NEWSFLASH_TOPLEVEL.as_str(),
            &children_by_parent,
            &name_by_id,
            &unread,
            0,
        ))
    }

    pub async fn get_headlines(&self, filter: ArticleFilter) -> anyhow::Result<Vec<Headline>> {
        let articles = self.with_nf(|nf| nf.get_fat_articles(filter)).await?;
        let feed_titles: HashMap<String, String> = self
            .get_feeds()
            .await?
            .into_iter()
            .map(|f| (f.id, f.title))
            .collect();
        let mut out = Vec::with_capacity(articles.len());
        for a in articles {
            let feed_id = a.feed_id.as_str().to_string();
            out.push(Headline {
                id: a.article_id.as_str().to_string(),
                title: a.title,
                feed_id: feed_id.clone(),
                feed_title: feed_titles.get(&feed_id).cloned(),
                url: a.url.map(|u| u.to_string()),
                date: a.date,
                summary: a.summary,
                thumbnail_url: a.thumbnail_url,
                unread: a.unread == Read::Unread,
                marked: a.marked == Marked::Marked,
                note: None,
            });
        }
        Ok(out)
    }

    pub async fn get_headlines_by_ids(&self, ids: Vec<String>) -> anyhow::Result<Vec<Headline>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids.into_iter().map(|id| news_flash::models::ArticleID::new(&id)).collect::<Vec<_>>();
        let filter = news_flash::models::ArticleFilter {
            order: Some(news_flash::models::ArticleOrder::NewestFirst),
            order_by: Some(news_flash::models::OrderBy::Published),
            ids: Some(ids),
            ..news_flash::models::ArticleFilter::default()
        };
        self.get_headlines(filter).await
    }

    pub async fn search(&self, q: &str, limit: i64) -> anyhow::Result<Vec<Headline>> {
        let db_path = self.data_dir().join("engine/data/database.sqlite");
        let hits = crate::engine::queries::search(&db_path, q, limit)?;
        let ids = hits.into_iter().map(|h| h.article_id).collect();
        self.get_headlines_by_ids(ids).await
    }

    pub async fn search_suggestions(&self, q: &str) -> anyhow::Result<Vec<Headline>> {
        self.search(q, 8).await
    }

    pub async fn mark_article_saved(&self, id: &str, saved: bool) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        let article_id = news_flash::models::ArticleID::new(id);
        let marked = if saved { news_flash::models::Marked::Marked } else { news_flash::models::Marked::Unmarked };
        self.nf.set_article_marked(&[article_id], marked, &self.client).await?;
        Ok(())
    }

    pub async fn set_article_read(&self, id: &str, read: bool) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        let article_id = news_flash::models::ArticleID::new(id);
        let read = if read { news_flash::models::Read::Read } else { news_flash::models::Read::Unread };
        self.nf.set_article_read(&[article_id], read, &self.client).await?;
        Ok(())
    }

    pub async fn mark_all_read(&self) -> anyhow::Result<()> {
        let _guard = self.mutation_guard().await;
        self.nf.set_all_read(&self.client).await?;
        Ok(())
    }

    pub async fn get_article_detail(&self, article_id: &str) -> anyhow::Result<ArticleDetail> {
        let id = news_flash::models::ArticleID::new(article_id);
        let a = self.with_nf(move |nf| nf.get_fat_article(&id)).await?;
        let feed_id = a.feed_id.as_str().to_string();
        let feed_title = self
            .get_feeds()
            .await?
            .into_iter()
            .find(|f| f.id == feed_id)
            .map(|f| f.title);
        Ok(ArticleDetail {
            id: a.article_id.as_str().to_string(),
            title: a.title,
            author: a.author,
            feed_id,
            feed_title,
            url: a.url.map(|u| u.to_string()),
            date: a.date,
            html: a.scraped_content.or(a.html),
            summary: a.summary,
            unread: a.unread == Read::Unread,
            marked: a.marked == Marked::Marked,
            thumbnail_url: a.thumbnail_url,
            plain_text: a.plain_text,
            note: None,
            tags: Vec::new(),
        })
    }
    pub async fn render_article_content(&self, article: &mut crate::dto::ArticleDetail) -> anyhow::Result<()> {
        if let (Some(html), Some(base)) = (&article.html, &article.url) {
            article.html = Some(crate::engine::content::rewrite_html(html, base));
        }
        Ok(())
    }

    pub async fn get_favicon(&self, feed_id: &str) -> anyhow::Result<Option<(String, Vec<u8>)>> {
        let feed_id = FeedID::new(feed_id);
        let icon = self.with_nf(move |nf| nf.load_icon_from_db(&feed_id)).await?;
        let data = icon.highres.or(icon.lowres);
        Ok(data.map(|bytes| (icon.format.unwrap_or_else(|| "image/x-icon".to_string()), bytes)))
    }

    pub async fn get_article_thumbnail(&self, article_id: &str) -> anyhow::Result<Option<(String, Vec<u8>)>> {
        let article_id = news_flash::models::ArticleID::new(article_id);
        let _guard = self.mutation_guard().await;
        match self.nf.get_article_thumbnail(&article_id, &self.client).await? {
            Some(thumbnail) => {
                let format = thumbnail.format.unwrap_or_else(|| "image/jpeg".to_string());
                Ok(thumbnail.data.map(|d| (format, d)))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::config::Config;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread::{self, JoinHandle};

    pub struct FeedServer {
        pub url: String,
        handle: JoinHandle<()>,
    }

    impl FeedServer {
        pub fn start(rss_body: String, connections: usize) -> FeedServer {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let addr = listener.local_addr().unwrap();
            let url = format!("http://{addr}/feed.xml");
            let handle = thread::spawn(move || {
                let mut served = 0;
                let mut last_request: Option<std::time::Instant> = None;
                while served < connections {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            served += 1;
                            last_request = Some(std::time::Instant::now());
                            let _ = stream.set_nonblocking(false);
                            let mut buf = [0u8; 4096];
                            let _ = stream.read(&mut buf);
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                rss_body.len(),
                                rss_body
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            if last_request.is_some_and(|t| t.elapsed() > std::time::Duration::from_millis(500)) {
                                break;
                            }
                            thread::sleep(std::time::Duration::from_millis(5));
                        }
                        Err(_) => break,
                    }
                }
            });
            FeedServer { url, handle }
        }

        pub fn stop(self) {
            self.handle.join().unwrap();
        }
    }

    pub const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Feed</title>
    <link>http://127.0.0.1/</link>
    <description>test</description>
    <item>
      <title>Article Alpha</title>
      <link>http://example.com/alpha</link>
      <guid isPermaLink="false">alpha-1</guid>
      <pubDate>Mon, 11 Aug 2026 10:00:00 GMT</pubDate>
      <description>Alpha body.</description>
    </item>
    <item>
      <title>Article Beta</title>
      <link>http://example.com/beta</link>
      <guid isPermaLink="false">beta-1</guid>
      <pubDate>Tue, 12 Aug 2026 11:30:00 GMT</pubDate>
      <description>Beta body.</description>
    </item>
  </channel>
</rss>
"#;

    fn tmp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "rssea-engine-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn engine_inits_headless_with_dirs() {
        let dir = tmp_dir();
        let config = Config {
            data_dir: dir.clone(),
            host: "127.0.0.1".into(),
            port: 0,
        };
        let engine = Engine::new(&config).await.unwrap();
        assert!(dir.join("engine/config").exists());
        assert!(dir.join("engine/data").exists());
        let empty = engine
            .with_nf(|nf| nf.is_database_empty())
            .await
            .unwrap();
        assert!(empty);
    }

    #[tokio::test]
    async fn add_feed_and_sync_pulls_articles() {
        let server = crate::engine::tests::FeedServer::start(RSS.to_string(), 6);
        let dir = tmp_dir();
        let config = Config {
            data_dir: dir,
            host: "127.0.0.1".into(),
            port: 0,
        };
        let engine = Engine::new(&config).await.unwrap();
        let feed = engine.add_feed(&server.url, Some("Test Feed".into()), None).await.unwrap();
        assert_eq!(feed.title, "Test Feed");
        let article_count = engine.with_nf(|nf| nf.get_article_ids(news_flash::models::ArticleFilter::default()).map(|v| v.len())).await.unwrap();
        assert_eq!(article_count, 2);
        let counts = engine.sync_all().await.unwrap();
        assert_eq!(counts.get(&server.url).copied(), Some(0));
        let counts2 = engine.sync_all().await.unwrap();
        assert_eq!(counts2.get(&server.url).copied(), Some(0));
        server.stop();
    }

    #[tokio::test]
    async fn reads_return_feeds_and_headlines() {
        let server = crate::engine::tests::FeedServer::start(RSS.to_string(), 6);
        let config = Config { data_dir: tmp_dir(), host: "127.0.0.1".into(), port: 0 };
        let engine = Engine::new(&config).await.unwrap();
        let _feed = engine.add_feed(&server.url, Some("Test Feed".into()), None).await.unwrap();
        engine.sync_all().await.unwrap();

        let feeds = engine.get_feeds().await.unwrap();
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].title, "Test Feed");

        let filter = news_flash::models::ArticleFilter {
            order: Some(news_flash::models::ArticleOrder::NewestFirst),
            order_by: Some(news_flash::models::OrderBy::Published),
            ..news_flash::models::ArticleFilter::default()
        };
        let headlines = engine.get_headlines(filter).await.unwrap();
        assert_eq!(headlines.len(), 2);
        assert_eq!(headlines[0].title.as_deref(), Some("Article Beta"));
        assert!(headlines[0].unread);
        assert!(!headlines[0].marked);

        let detail = engine.get_article_detail(&headlines[0].id).await.unwrap();
        assert_eq!(detail.title.as_deref(), Some("Article Beta"));
        server.stop();
    }
}
