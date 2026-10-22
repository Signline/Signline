pub mod repo;
pub mod schema;

use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Paginated list response returned by PocketBase `/api/collections/:collection/records`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbList<T> {
    pub page: usize,
    #[serde(rename = "perPage")]
    pub per_page: usize,
    #[serde(rename = "totalItems")]
    pub total_items: usize,
    #[serde(rename = "totalPages")]
    pub total_pages: usize,
    pub items: Vec<T>,
}

/// Manages the lifecycle of the underlying PocketBase daemon and provides an authenticated
/// HTTP client for interacting with all 34 Signline collections.
#[derive(Clone)]
pub struct PocketBaseClient {
    pub base_url: String,
    pub http: Client,
    pub admin_email: String,
    pub admin_password: String,
    token: Arc<RwLock<String>>,
    _child: Arc<RwLock<Option<Child>>>,
}

impl PocketBaseClient {
    /// Connects to an existing PocketBase instance or spawns and initializes a managed child daemon.
    pub async fn connect_or_spawn(config: &AppConfig) -> AppResult<Self> {
        std::fs::create_dir_all(&config.pb_data_dir)?;
        std::fs::create_dir_all(&config.storage_dir)?;

        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let mut child_proc: Option<Child> = None;

        // Check if PocketBase is already listening on pb_url
        let health_url = format!("{}/api/health", config.pb_url);
        let is_running = http.get(&health_url).send().await.map(|r| r.status().is_success()).unwrap_or(false);

        if !is_running {
            info!(
                "PocketBase not running at {}; bootstrapping superuser and starting daemon...",
                config.pb_url
            );

            // Ensure superuser credentials exist in pb_data
            let upsert_status = Command::new(&config.pb_bin_path)
                .arg("superuser")
                .arg("upsert")
                .arg(&config.pb_admin_email)
                .arg(&config.pb_admin_password)
                .arg(format!("--dir={}", config.pb_data_dir.display()))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            if let Err(e) = upsert_status {
                warn!("PocketBase superuser upsert warning: {}", e);
            }

            let bind_addr = config
                .pb_url
                .trim_start_matches("http://")
                .trim_start_matches("https://")
                .trim_end_matches('/');

            let child = Command::new(&config.pb_bin_path)
                .arg("serve")
                .arg(format!("--http={}", bind_addr))
                .arg(format!("--dir={}", config.pb_data_dir.display()))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| AppError::PocketBase(format!("Failed to spawn PocketBase binary: {}", e)))?;

            child_proc = Some(child);

            // Wait up to 10 seconds for PocketBase health endpoint to respond
            let mut ready = false;
            for _ in 0..50 {
                tokio::time::sleep(Duration::from_millis(200)).await;
                if let Ok(resp) = http.get(&health_url).send().await {
                    if resp.status().is_success() {
                        ready = true;
                        break;
                    }
                }
            }
            if !ready {
                return Err(AppError::PocketBase(
                    "Timed out waiting for PocketBase daemon to become healthy".to_string(),
                ));
            }
            info!("PocketBase daemon started and healthy at {}", config.pb_url);
        } else {
            info!("Connected to running PocketBase instance at {}", config.pb_url);
        }

        let client = Self {
            base_url: config.pb_url.trim_end_matches('/').to_string(),
            http,
            admin_email: config.pb_admin_email.clone(),
            admin_password: config.pb_admin_password.clone(),
            token: Arc::new(RwLock::new(String::new())),
            _child: Arc::new(RwLock::new(child_proc)),
        };

        client.authenticate().await?;
        schema::ensure_all_collections(&client).await?;

        Ok(client)
    }

    /// Authenticates against PocketBase v0.23+ `_superusers` (with fallback to legacy `/api/admins/auth-with-password`).
    pub async fn authenticate(&self) -> AppResult<String> {
        let auth_url = format!("{}/api/collections/_superusers/auth-with-password", self.base_url);
        let resp = self
            .http
            .post(&auth_url)
            .json(&json!({
                "identity": self.admin_email,
                "password": self.admin_password,
            }))
            .send()
            .await?;

        let val: Value = if resp.status().is_success() {
            resp.json().await?
        } else {
            let legacy_url = format!("{}/api/admins/auth-with-password", self.base_url);
            let legacy_resp = self
                .http
                .post(&legacy_url)
                .json(&json!({
                    "identity": self.admin_email,
                    "password": self.admin_password,
                }))
                .send()
                .await?;
            if !legacy_resp.status().is_success() {
                let status = legacy_resp.status();
                let text = legacy_resp.text().await.unwrap_or_default();
                return Err(AppError::PocketBase(format!(
                    "PocketBase superuser auth failed ({}): {}",
                    status, text
                )));
            }
            legacy_resp.json().await?
        };

        let token = val
            .get("token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| AppError::PocketBase("Missing token in PocketBase auth response".to_string()))?
            .to_string();

        let mut guard = self.token.write().await;
        *guard = token.clone();
        Ok(token)
    }

    pub async fn get_token(&self) -> AppResult<String> {
        let current = self.token.read().await.clone();
        if current.is_empty() {
            self.authenticate().await
        } else {
            Ok(current)
        }
    }

    /// Creates a record in the given PocketBase collection.
    pub async fn create<T: DeserializeOwned>(&self, collection: &str, data: &Value) -> AppResult<T> {
        let token = self.get_token().await?;
        let url = format!("{}/api/collections/{}/records", self.base_url, collection);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", &token)
            .json(data)
            .send()
            .await?;

        if resp.status().as_u16() == 401 {
            let new_token = self.authenticate().await?;
            let retry = self
                .http
                .post(&url)
                .header("Authorization", &new_token)
                .json(data)
                .send()
                .await?;
            if !retry.status().is_success() {
                let err_txt = retry.text().await.unwrap_or_default();
                return Err(AppError::PocketBase(format!(
                    "PB create {} failed: {}",
                    collection, err_txt
                )));
            }
            return Ok(retry.json().await?);
        }

        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(AppError::PocketBase(format!(
                "PB create {} failed: {}",
                collection, err_txt
            )));
        }

        Ok(resp.json().await?)
    }

    /// Updates a record by its PocketBase `id`.
    pub async fn update<T: DeserializeOwned>(
        &self,
        collection: &str,
        id: &str,
        data: &Value,
    ) -> AppResult<T> {
        let token = self.get_token().await?;
        let url = format!("{}/api/collections/{}/records/{}", self.base_url, collection, id);
        let resp = self
            .http
            .patch(&url)
            .header("Authorization", &token)
            .json(data)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(AppError::PocketBase(format!(
                "PB update {}/{} failed: {}",
                collection, id, err_txt
            )));
        }

        Ok(resp.json().await?)
    }

    /// Fetches a single record by its PocketBase `id`.
    pub async fn get_by_id<T: DeserializeOwned>(&self, collection: &str, id: &str) -> AppResult<T> {
        let token = self.get_token().await?;
        let url = format!("{}/api/collections/{}/records/{}", self.base_url, collection, id);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", &token)
            .send()
            .await?;

        if resp.status().as_u16() == 404 {
            return Err(AppError::NotFound(format!("{} '{}' not found", collection, id)));
        }

        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(AppError::PocketBase(format!(
                "PB get {}/{} failed: {}",
                collection, id, err_txt
            )));
        }

        Ok(resp.json().await?)
    }

    /// Deletes a record by its PocketBase `id`.
    pub async fn delete(&self, collection: &str, id: &str) -> AppResult<()> {
        let token = self.get_token().await?;
        let url = format!("{}/api/collections/{}/records/{}", self.base_url, collection, id);
        let resp = self
            .http
            .delete(&url)
            .header("Authorization", &token)
            .send()
            .await?;

        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(AppError::PocketBase(format!(
                "PB delete {}/{} failed: {}",
                collection, id, err_txt
            )));
        }
        Ok(())
    }

    /// Lists records in a collection with optional PocketBase filter and sort expressions.
    pub async fn list<T: DeserializeOwned>(
        &self,
        collection: &str,
        page: usize,
        per_page: usize,
        filter: Option<&str>,
        sort: Option<&str>,
    ) -> AppResult<PbList<T>> {
        let token = self.get_token().await?;
        let url = format!("{}/api/collections/{}/records", self.base_url, collection);
        let mut query: Vec<(&str, String)> = vec![
            ("page", page.max(1).to_string()),
            ("perPage", per_page.clamp(1, 500).to_string()),
        ];
        if let Some(f) = filter {
            if !f.trim().is_empty() {
                query.push(("filter", f.to_string()));
            }
        }
        if let Some(s) = sort {
            if !s.trim().is_empty() {
                query.push(("sort", s.to_string()));
            }
        }

        let resp = self
            .http
            .get(&url)
            .header("Authorization", &token)
            .query(&query)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            return Err(AppError::PocketBase(format!(
                "PB list {} failed: {}",
                collection, err_txt
            )));
        }

        Ok(resp.json().await?)
    }

    /// Convenience helper returning all matching records (up to 500).
    pub async fn list_all<T: DeserializeOwned>(
        &self,
        collection: &str,
        filter: Option<&str>,
        sort: Option<&str>,
    ) -> AppResult<Vec<T>> {
        let res = self.list::<T>(collection, 1, 500, filter, sort).await?;
        Ok(res.items)
    }

    /// Finds the first matching record for a filter expression, if any.
    pub async fn find_first<T: DeserializeOwned>(
        &self,
        collection: &str,
        filter: &str,
        sort: Option<&str>,
    ) -> AppResult<Option<T>> {
        let res = self.list::<T>(collection, 1, 1, Some(filter), sort).await?;
        Ok(res.items.into_iter().next())
    }
}

/// Escapes a string literal for safe inclusion inside a PocketBase filter expression.
pub fn pb_escape(val: &str) -> String {
    val.replace('\\', "\\\\").replace('\'', "\\'").replace('"', "\\\"")
}
