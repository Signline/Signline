use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const ALL_WEBHOOK_EVENTS: &[&str] = &[
    "form.viewed",
    "form.started",
    "form.completed",
    "form.declined",
    "form.delegated",
    "submission.created",
    "submission.completed",
    "submission.expired",
    "submission.archived",
    "template.created",
    "template.updated",
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookUrl {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub events: Value,
    #[serde(default)]
    pub secret: Value,
    #[serde(default)]
    pub hmac_secret: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

impl WebhookUrl {
    pub fn events_list(&self) -> Vec<String> {
        if let Some(arr) = self.events.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            vec![]
        }
    }

    pub fn to_api_json(&self) -> Value {
        json!({
            "id": self.id,
            "account_id": self.account_id,
            "url": self.url,
            "events": self.events_list(),
            "secret": if self.secret.is_object() { self.secret.clone() } else { json!({}) },
            "hmac_secret": self.hmac_secret,
            "sha1": self.sha1,
            "created_at": self.created,
            "updated_at": self.updated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookEvent {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub webhook_url_id: String,
    #[serde(default)]
    pub record_id: String,
    #[serde(default)]
    pub record_type: String,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebhookAttempt {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub webhook_event_id: String,
    #[serde(default)]
    pub attempt: i64,
    #[serde(default)]
    pub response_status_code: i64,
    #[serde(default)]
    pub response_body: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}
