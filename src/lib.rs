pub mod api;
pub mod app_db;
pub mod auth;
pub mod config;
pub mod engine;

use config::Config;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    config.ensure_data_dir()?;
    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port)).await?;
    tracing::info!("rssea {} listening on {}", crate::version(), listener.local_addr()?);
    let app = api::router(config);
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert_eq!(crate::version(), "0.1.0");
    }
}
