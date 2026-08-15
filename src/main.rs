use feedea::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "feedea=info,tower_http=info".into()),
        )
        .init();

    let config = Config::parse();
    feedea::run(config).await
}
