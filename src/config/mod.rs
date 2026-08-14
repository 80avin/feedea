use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "rssea", version, about = "Self-hosted feed aggregator")]
pub struct Cli {
    #[arg(long, env = "RSSEA_DATA_DIR")]
    pub data_dir: Option<PathBuf>,
    #[arg(long, env = "RSSEA_HOST", default_value = "0.0.0.0")]
    pub host: String,
    #[arg(long, env = "RSSEA_PORT", default_value_t = 3000)]
    pub port: u16,
    #[arg(long, env = "RSSEA_ALLOW_PRIVATE_PROXY", default_value_t = false)]
    pub allow_private_proxy: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub host: String,
    pub port: u16,
    pub allow_private_proxy: bool,
}

impl Config {
    pub fn parse() -> Self {
        let cli = Cli::parse();
        Config {
            data_dir: cli.data_dir.unwrap_or_else(default_data_dir),
            host: cli.host,
            port: cli.port,
            allow_private_proxy: cli.allow_private_proxy,
        }
    }

    pub fn data_file(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }

    pub fn ensure_data_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)
    }
}

pub fn default_data_dir() -> PathBuf {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() => PathBuf::from(home).join(".local/share/rssea"),
        _ => PathBuf::from("."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_data_dir_uses_home() {
        let home = std::env::var("HOME").expect("HOME set in test env");
        assert_eq!(
            default_data_dir(),
            PathBuf::from(format!("{home}/.local/share/rssea"))
        );
    }

    #[test]
    fn data_file_joins_under_data_dir() {
        let cfg = Config {
            data_dir: PathBuf::from("/tmp/rssea-test"),
            host: "127.0.0.1".into(),
            port: 3000,
            allow_private_proxy: false,
        };
        assert_eq!(cfg.data_file("rssea.sqlite"), PathBuf::from("/tmp/rssea-test/rssea.sqlite"));
    }

    #[test]
    fn cli_reads_allow_private_proxy_env() {
        unsafe { std::env::set_var("RSSEA_ALLOW_PRIVATE_PROXY", "true") };
        let cli = Cli::try_parse_from(["rssea"]).unwrap();
        unsafe { std::env::remove_var("RSSEA_ALLOW_PRIVATE_PROXY") };
        assert!(cli.allow_private_proxy);

        unsafe { std::env::remove_var("RSSEA_ALLOW_PRIVATE_PROXY") };
        let cli = Cli::try_parse_from(["rssea"]).unwrap();
        assert!(!cli.allow_private_proxy);
    }
}
