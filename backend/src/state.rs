use crate::config::AppConfig;
use crate::pb::PocketBaseClient;
use std::sync::Arc;

/// Shared application state injected into all Axum route handlers and background workers.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub pb: PocketBaseClient,
}

impl AppState {
    pub fn new(config: AppConfig, pb: PocketBaseClient) -> Self {
        Self {
            config: Arc::new(config),
            pb,
        }
    }
}
