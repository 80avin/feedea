use std::time::Duration;

use crate::engine::Engine;

pub const DEFAULT_SYNC_INTERVAL: Duration = Duration::from_secs(30 * 60);

pub async fn scheduler_loop(engine: Engine, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        if let Err(error) = engine.sync_all().await {
            tracing::warn!(%error, "scheduled sync failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::engine::Engine;

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
            "rssea-sched-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config { data_dir: dir, host: "127.0.0.1".into(), port: 0 };
        let engine = Engine::new(&config).await.unwrap();
        engine.add_feed(&server.url, Some("Sched Feed".into()), None).await.unwrap();

        let handle = tokio::spawn(scheduler_loop(engine.clone(), std::time::Duration::from_millis(50)));
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        handle.abort();

        let unread = engine
            .with_nf(|nf| nf.unread_count_all())
            .await
            .unwrap();
        assert_eq!(unread, 2);
        server.stop();
    }
}
