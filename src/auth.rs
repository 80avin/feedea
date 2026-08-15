use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use rand::RngExt;

use crate::AppState;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let mut salt_bytes = [0u8; 16];
    rand::rng().fill(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes).map_err(anyhow::Error::from)?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(anyhow::Error::from)?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else { return false };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn ensure_password_setup(state: &AppState) -> anyhow::Result<()> {
    let mut app_db = state.app_db.lock().await;
    if app_db.password_hash()?.is_none() {
        let password = generate_token();
        let hash = hash_password(&password)?;
        app_db.set_password_hash(&hash)?;
        eprintln!("========================================================");
        eprintln!("feedea initial password: {password}");
        eprintln!("log in at /api/login (use the web UI) and change it in Settings");
        eprintln!("========================================================");
    }
    Ok(())
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
    fn hash_and_verify_password_roundtrip() {
        let hash = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &hash));
        assert!(!verify_password("wrong password", &hash));
        assert!(!verify_password("", &hash));
    }

    #[test]
    fn verify_password_rejects_garbage_hash() {
        assert!(!verify_password("any", "not-an-argon2-hash"));
        assert!(!verify_password("any", ""));
    }

    #[test]
    fn hashes_are_salted() {
        let a = hash_password("same-pass").unwrap();
        let b = hash_password("same-pass").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn generate_token_is_64_hex_chars() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_eq!(t1.len(), 64);
        assert!(t1.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(t1, t2);
    }

    #[test]
    fn sha256_hex_known_vector() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(sha256_hex(""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[tokio::test]
    async fn ensure_password_setup_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("feedea-auth-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config { data_dir: dir.clone(), host: "127.0.0.1".into(), port: 0, allow_private_proxy: false };
        let engine = Engine::new(&config).await.unwrap();
        let app_db = Arc::new(Mutex::new(app_db::open(&dir).unwrap()));
        let state = AppState { engine, app_db: app_db.clone(), allow_private_proxy: false };
        ensure_password_setup(&state).await.unwrap();
        ensure_password_setup(&state).await.unwrap();
        assert!(app_db.lock().await.password_hash().unwrap().is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
