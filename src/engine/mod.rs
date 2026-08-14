use std::sync::Arc;

use news_flash::error::NewsFlashError;
use news_flash::models::PluginID;
use news_flash::NewsFlash;

use crate::config::Config;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rssea-engine-test-{}", std::process::id()));
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
}
