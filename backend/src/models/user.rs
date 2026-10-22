use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct User {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub email: String,
    #[serde(default, skip_serializing)]
    pub password_hash: String,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
    #[serde(default)]
    pub role: String,
    #[serde(default, skip_serializing)]
    pub otp_secret: String,
    #[serde(default)]
    pub otp_required_for_login: bool,
    #[serde(default)]
    pub consumed_timestep: i64,
    #[serde(default)]
    pub failed_attempts: i64,
    #[serde(default)]
    pub locked_at: String,
    #[serde(default, skip_serializing)]
    pub reset_password_token: String,
    #[serde(default)]
    pub reset_password_sent_at: String,
    #[serde(default)]
    pub sign_in_count: i64,
    #[serde(default)]
    pub current_sign_in_at: String,
    #[serde(default)]
    pub last_sign_in_at: String,
    #[serde(default)]
    pub current_sign_in_ip: String,
    #[serde(default)]
    pub last_sign_in_ip: String,
    #[serde(default)]
    pub signature_file: String,
    #[serde(default)]
    pub initials_file: String,
    #[serde(default)]
    pub archived_at: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

impl User {
    pub fn full_name(&self) -> String {
        let joined = format!("{} {}", self.first_name.trim(), self.last_name.trim())
            .trim()
            .to_string();
        if joined.is_empty() {
            self.email.clone()
        } else {
            joined
        }
    }

    pub fn to_public_json(&self) -> Value {
        json!({
            "id": self.id,
            "uuid": self.uuid,
            "account_id": self.account_id,
            "email": self.email,
            "first_name": self.first_name,
            "last_name": self.last_name,
            "full_name": self.full_name(),
            "role": if self.role.is_empty() { "admin" } else { &self.role },
            "otp_required_for_login": self.otp_required_for_login,
            "sign_in_count": self.sign_in_count,
            "current_sign_in_at": self.current_sign_in_at,
            "last_sign_in_at": self.last_sign_in_at,
            "signature_file": self.signature_file,
            "initials_file": self.initials_file,
            "archived_at": if self.archived_at.is_empty() { Value::Null } else { json!(self.archived_at) },
            "created_at": self.created,
            "updated_at": self.updated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccessTokenRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub token_encrypted: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpTokenRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub token_prefix: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub archived_at: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfigRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}
