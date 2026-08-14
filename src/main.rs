use rssea::config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rssea=info,tower_http=info".into()),
        )
        .init();

    let config = Config::parse();
    rssea::run(config).await
}
