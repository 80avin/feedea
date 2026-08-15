use std::sync::Arc;
use std::time::Duration;

use crate::app_db::AppDb;
use crate::engine::Engine;
use tokio::sync::Mutex;

pub const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(30 * 60);

async fn read_interval(app_db: &Arc<Mutex<AppDb>>, default_interval: Duration) -> Duration {
    match app_db.lock().await.get_setting("sync_interval_minutes") {
        Ok(Some(raw)) => match raw.parse::<u64>() {
            Ok(minutes) if minutes > 0 => Duration::from_secs(minutes * 60),
            _ => default_interval,
        },
        _ => default_interval,
    }
}

pub async fn scheduler_loop(engine: Engine, app_db: Arc<Mutex<AppDb>>, default_interval: Duration) {
    loop {
        if let Err(error) = engine.sync_all().await {
            tracing::warn!(%error, "scheduled sync failed");
        }
        let interval = read_interval(&app_db, default_interval).await;
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_db;
    use crate::config::Config;
    use crate::engine::Engine;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[test]
    fn default_interval_is_thirty_minutes() {
        assert_eq!(DEFAULT_SYNC_INTERVAL, std::time::Duration::from_secs(30 * 60));
    }

    #[tokio::test]
    async fn scheduler_syncs_feed() {
        use std::sync::atomic::{AtomicU64, Ordering};

        let server = crate::engine::tests::FeedServer::start(
            crate::engine::tests::RSS.to_string(),
            6,
        );
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "feedea-sched-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config { data_dir: dir, host: "127.0.0.1".into(), port: 0, allow_private_proxy: false };
        let engine = Engine::new(&config).await.unwrap();
        engine.add_feed(&server.url, Some("Sched Feed".into()), None).await.unwrap();
        let before = engine.last_sync().await;
        let app_db = Arc::new(Mutex::new(app_db::open(&config.data_dir).unwrap()));

        let handle = tokio::spawn(scheduler_loop(engine.clone(), app_db, std::time::Duration::from_millis(50)));
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        handle.abort();

        let after = engine.last_sync().await;
        assert!(after > before);
        let unread = engine
            .with_nf(|nf| nf.unread_count_all())
            .await
            .unwrap();
        assert_eq!(unread, 2);
        server.stop();
    }
}
