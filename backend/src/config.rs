use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::env;
use std::path::PathBuf;

/// Runtime configuration for the Signline Rust backend.
/// Centralizes paths, network addresses, PocketBase credentials, and the derived
/// AES-256-GCM master key used to protect `encrypted_configs` and API tokens.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub public_url: String,
    pub secret_key_base: String,
    pub aes_key: [u8; 32],
    pub root_dir: PathBuf,
    pub pb_bin_path: PathBuf,
    pub pb_data_dir: PathBuf,
    pub pb_url: String,
    pub pb_admin_email: String,
    pub pb_admin_password: String,
    pub storage_dir: PathBuf,
    pub public_dir: PathBuf,
    pub frontend_dist_dir: PathBuf,
    pub multitenant: bool,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let default_root = env::current_dir()
            .ok()
            .and_then(|cwd| {
                if cwd.join("VERSION").exists() || cwd.join(".version").exists() {
                    Some(cwd)
                } else if cwd.ends_with("backend") {
                    cwd.parent().map(|p| p.to_path_buf())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let root_dir = env::var("SIGNLINE_ROOT")
            .map(PathBuf::from)
            .unwrap_or(default_root);

        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(3000);

        let public_url = env::var("PUBLIC_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| env::var("APP_URL").ok().filter(|v| !v.trim().is_empty()))
            .unwrap_or_else(|| format!("http://localhost:{}", port));

        let secret_key_base = env::var("SECRET_KEY_BASE").unwrap_or_else(|_| {
            "signline-v0.1.0b-default-secret-key-base-keep-secure-in-production-9f8e7d6c5b4a".to_string()
        });

        let mut hasher = Sha256::new();
        hasher.update(secret_key_base.as_bytes());
        let digest = hasher.finalize();
        let mut aes_key = [0u8; 32];
        aes_key.copy_from_slice(&digest[..32]);

        let pb_bin_path = env::var("POCKETBASE_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let deb_pb = PathBuf::from("/usr/lib/signline/bin/pocketbase");
                if deb_pb.exists() && !root_dir.join("bin/pocketbase").exists() {
                    deb_pb
                } else {
                    root_dir.join("bin/pocketbase")
                }
            });

        let pb_data_dir = env::var("POCKETBASE_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root_dir.join("pb_data"));

        let pb_url = env::var("POCKETBASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string());

        let pb_admin_email = env::var("POCKETBASE_ADMIN_EMAIL")
            .unwrap_or_else(|_| "admin@signline.local".to_string());

        let pb_admin_password = env::var("POCKETBASE_ADMIN_PASSWORD")
            .unwrap_or_else(|_| "signline_pb_admin_12345".to_string());

        let storage_dir = env::var("STORAGE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| pb_data_dir.join("blobs"));

        let public_dir = env::var("PUBLIC_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let candidate = root_dir.join("public");
                if candidate.exists() {
                    candidate
                } else {
                    let deb_public = PathBuf::from("/usr/share/signline/public");
                    if deb_public.exists() {
                        deb_public
                    } else {
                        candidate
                    }
                }
            });

        let frontend_dist_dir = env::var("FRONTEND_DIST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let candidate = root_dir.join("frontend/dist");
                if candidate.join("index.html").exists() {
                    candidate
                } else {
                    let deb_dist = PathBuf::from("/usr/share/signline/frontend/dist");
                    if deb_dist.join("index.html").exists() {
                        deb_dist
                    } else {
                        candidate
                    }
                }
            });

        let multitenant = env::var("MULTITENANT")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        Self {
            host,
            port,
            public_url,
            secret_key_base,
            aes_key,
            root_dir,
            pb_bin_path,
            pb_data_dir,
            pb_url,
            pb_admin_email,
            pb_admin_password,
            storage_dir,
            public_dir,
            frontend_dist_dir,
            multitenant,
        }
    }

    /// Encrypts plaintext string or JSON using AES-256-GCM with a random 96-bit nonce.
    /// Output format: `base64(nonce || ciphertext_with_tag)`.
    pub fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        let cipher = Aes256Gcm::new_from_slice(&self.aes_key)
            .map_err(|e| format!("cipher init error: {}", e))?;
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| format!("encryption error: {}", e))?;

        let mut combined = Vec::with_capacity(12 + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(BASE64.encode(combined))
    }

    /// Decrypts a payload previously produced by `encrypt`.
    pub fn decrypt(&self, encoded: &str) -> Result<String, String> {
        let combined = BASE64
            .decode(encoded.trim())
            .map_err(|e| format!("base64 decode error: {}", e))?;
        if combined.len() < 13 {
            return Err("encrypted payload too short".to_string());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let cipher = Aes256Gcm::new_from_slice(&self.aes_key)
            .map_err(|e| format!("cipher init error: {}", e))?;
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("decryption error: {}", e))?;
        String::from_utf8(plaintext).map_err(|e| format!("utf8 error: {}", e))
    }

    pub fn encrypt_json(&self, value: &serde_json::Value) -> Result<String, String> {
        let serialized = serde_json::to_string(value).map_err(|e| e.to_string())?;
        self.encrypt(&serialized)
    }

    pub fn decrypt_json(&self, encoded: &str) -> Result<serde_json::Value, String> {
        if encoded.trim().is_empty() {
            return Ok(serde_json::json!({}));
        }
        let plain = self.decrypt(encoded)?;
        serde_json::from_str(&plain).map_err(|e| e.to_string())
    }
}
