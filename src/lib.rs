pub mod api;
pub mod app_db;
pub mod auth;
pub mod config;
pub mod dto;
pub mod engine;

use config::Config;

#[derive(Clone)]
pub struct AppState {
    pub engine: engine::Engine,
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    config.ensure_data_dir()?;
    let engine = engine::Engine::new(&config).await?;
    tokio::spawn(engine::sync::scheduler_loop(engine.clone(), engine::sync::DEFAULT_SYNC_INTERVAL));
    let state = AppState { engine };
    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!("rssea {} listening on {}", crate::version(), listener.local_addr()?);
    axum::serve(listener, api::router(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert_eq!(crate::version(), "0.1.0");
    }
}
