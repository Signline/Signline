pub mod account;
pub mod submission;
pub mod template;
pub mod user;
pub mod webhook;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Represents a content-addressed or UUID-addressed binary blob stored in PocketBase/disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageBlob {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub byte_size: i64,
    #[serde(default)]
    pub checksum: String,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub service_name: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmailMessageRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub author_id: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmailEventRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub emailable_type: String,
    #[serde(default)]
    pub emailable_id: String,
    #[serde(default)]
    pub message_id: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub event_datetime: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchEntryRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub record_type: String,
    #[serde(default)]
    pub record_id: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub ngram: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}
