use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BRANDING_KEY: &str = "branding";
pub const EMAIL_BRANDING_KEY: &str = "email_branding";
pub const SUPPORT_LINKS_KEY: &str = "support_links";
pub const SUBMITTER_REMINDERS_KEY: &str = "submitter_reminders";
pub const BCC_EMAILS_KEY: &str = "bcc_emails";
pub const SIGNING_PREFERENCES_KEY: &str = "signing_preferences";
pub const STORAGE_SETTINGS_KEY: &str = "storage_settings";
pub const MCP_ENABLED_KEY: &str = "mcp_enabled";

pub const EMAIL_SMTP_KEY: &str = "action_mailer_smtp";
pub const EMAIL_GMAIL_KEY: &str = "action_mailer_gmail";
pub const ESIGN_CERTS_KEY: &str = "esign_certs";
pub const TIMESTAMP_SERVER_URL_KEY: &str = "timestamp_server_url";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Account {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub timezone: String,
    #[serde(default)]
    pub locale: String,
    #[serde(default)]
    pub logo_file: String,
    #[serde(default)]
    pub favicon_file: String,
    #[serde(default)]
    pub archived_at: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountConfigRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EncryptedConfigRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value_encrypted: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountAccessRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountLinkedAccountRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub linked_account_id: String,
    #[serde(default)]
    pub account_type: String,
}
