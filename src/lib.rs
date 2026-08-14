pub mod api;
pub mod app_db;
pub mod auth;
pub mod config;
pub mod engine;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert_eq!(crate::version(), "0.1.0");
    }
}
