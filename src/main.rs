use feedea::config::Config;

fn main() -> anyhow::Result<()> {
    let config = Config::parse();
    // Bound glibc malloc arenas before the tokio runtime spawns worker
    // threads, so freed pages are reused across threads instead of being
    // stranded in per-thread arenas (which makes RSS grow and never return).
    feedea::memtrim::set_arena_max(config.memory_arena_max);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "feedea=info,tower_http=info".into()),
        )
        .init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(feedea::run(config))
}
