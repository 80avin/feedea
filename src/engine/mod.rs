use std::collections::HashMap;
use std::sync::Arc;

use news_flash::error::NewsFlashError;
use news_flash::feed_api::FeedHeaderMap;
use news_flash::models::{ArticleFilter, CategoryID, FeedID, Marked, PluginID, Read, Url};
use news_flash::NewsFlash;

use crate::config::Config;
use crate::dto::{ArticleDetail, FeedSummary, Headline};

pub mod sync;

#[derive(Clone)]
pub struct Engine {
    nf: Arc<NewsFlash>,
    client: reqwest::Client,
    mutation_lock: Arc<tokio::sync::Mutex<()>>,
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
        })
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
            });
        }
        Ok(out)
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
        })
    }
    pub async fn get_favicon(&self, feed_id: &str) -> anyhow::Result<Option<(String, Vec<u8>)>> {
        let feed_id = FeedID::new(feed_id);
        let icon = self.with_nf(move |nf| nf.load_icon_from_db(&feed_id)).await?;
        let data = icon.highres.or(icon.lowres);
        Ok(data.map(|bytes| (icon.format.unwrap_or_else(|| "image/x-icon".to_string()), bytes)))
    }

    pub async fn get_article_thumbnail(&self, article_id: &str) -> anyhow::Result<Option<(String, Vec<u8>)>> {
        let article_id = news_flash::models::ArticleID::new(article_id);
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

    const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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
        let counts = engine.sync_all().await.unwrap();
        assert_eq!(counts.get(&server.url).copied(), Some(2));
        let article_count = engine.with_nf(|nf| nf.get_article_ids(news_flash::models::ArticleFilter::default()).map(|v| v.len())).await.unwrap();
        assert_eq!(article_count, 2);
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
