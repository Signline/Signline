use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Submission {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub created_by_user_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub submitters_order: String,
    #[serde(default)]
    pub template_fields: Value,
    #[serde(default)]
    pub template_schema: Value,
    #[serde(default)]
    pub template_submitters: Value,
    #[serde(default)]
    pub preferences: Value,
    #[serde(default)]
    pub variables: Value,
    #[serde(default)]
    pub variables_schema: Value,
    #[serde(default)]
    pub documents: Value,
    #[serde(default)]
    pub audit_trail_file: String,
    #[serde(default)]
    pub combined_document_file: String,
    #[serde(default)]
    pub expire_at: String,
    #[serde(default)]
    pub completed_at: String,
    #[serde(default)]
    pub archived_at: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

impl Submission {
    pub fn status(&self, submitters: &[Submitter]) -> &'static str {
        if !self.completed_at.is_empty() {
            return "completed";
        }
        if submitters.iter().any(|s| !s.declined_at.is_empty()) {
            return "declined";
        }
        if !self.expire_at.is_empty() {
            if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&self.expire_at) {
                if exp < chrono::Utc::now() {
                    return "expired";
                }
            }
        }
        if !submitters.is_empty() && submitters.iter().all(|s| !s.completed_at.is_empty()) {
            return "completed";
        }
        if submitters.iter().any(|s| !s.sent_at.is_empty() || !s.opened_at.is_empty()) {
            return "sent";
        }
        "pending"
    }

    pub fn to_api_json(
        &self,
        submitters: &[Submitter],
        events: &[SubmissionEvent],
        public_url: &str,
    ) -> Value {
        let status = self.status(submitters);
        let submitters_json: Vec<Value> = submitters
            .iter()
            .map(|s| s.to_api_json(public_url))
            .collect();
        let events_json: Vec<Value> = events.iter().map(|e| e.to_api_json()).collect();

        let audit_trail_url = if !self.audit_trail_file.is_empty() {
            json!(format!(
                "{}/api/submissions/{}/download?audit=true",
                public_url, self.id
            ))
        } else {
            Value::Null
        };

        let combined_document_url = if !self.combined_document_file.is_empty() {
            json!(format!(
                "{}/api/submissions/{}/download",
                public_url, self.id
            ))
        } else {
            Value::Null
        };

        json!({
            "id": self.id,
            "slug": self.slug,
            "name": self.name,
            "source": if self.source.is_empty() { "invite" } else { &self.source },
            "submitters_order": if self.submitters_order.is_empty() { "random" } else { &self.submitters_order },
            "status": status,
            "template_id": self.template_id,
            "account_id": self.account_id,
            "created_by_user_id": self.created_by_user_id,
            "template_fields": self.template_fields,
            "template_schema": self.template_schema,
            "template_submitters": self.template_submitters,
            "preferences": self.preferences,
            "variables": self.variables,
            "documents": self.documents,
            "audit_trail_url": audit_trail_url,
            "combined_document_url": combined_document_url,
            "submitters": submitters_json,
            "submission_events": events_json,
            "expire_at": if self.expire_at.is_empty() { Value::Null } else { json!(self.expire_at) },
            "completed_at": if self.completed_at.is_empty() { Value::Null } else { json!(self.completed_at) },
            "archived_at": if self.archived_at.is_empty() { Value::Null } else { json!(self.archived_at) },
            "created_at": self.created,
            "updated_at": self.updated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Submitter {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub submission_id: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub external_id: String,
    #[serde(default)]
    pub values: Value,
    #[serde(default)]
    pub preferences: Value,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub documents: Value,
    #[serde(default)]
    pub attachments: Value,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub ua: String,
    #[serde(default)]
    pub timezone: String,
    #[serde(default)]
    pub sent_at: String,
    #[serde(default)]
    pub opened_at: String,
    #[serde(default)]
    pub completed_at: String,
    #[serde(default)]
    pub declined_at: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

impl Submitter {
    pub fn role(&self) -> String {
        self.preferences
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("First Party")
            .to_string()
    }

    pub fn status(&self) -> &'static str {
        if !self.declined_at.is_empty() {
            "declined"
        } else if !self.completed_at.is_empty() {
            "completed"
        } else if !self.opened_at.is_empty() {
            "opened"
        } else if !self.sent_at.is_empty() {
            "sent"
        } else {
            "awaiting"
        }
    }

    pub fn to_api_json(&self, public_url: &str) -> Value {
        let values = if self.values.is_object() || self.values.is_array() {
            self.values.clone()
        } else {
            json!({})
        };
        let documents = if self.documents.is_array() {
            self.documents.clone()
        } else {
            json!([])
        };
        json!({
            "id": self.id,
            "submission_id": self.submission_id,
            "uuid": self.uuid,
            "slug": self.slug,
            "email": self.email,
            "name": self.name,
            "phone": self.phone,
            "role": self.role(),
            "status": self.status(),
            "external_id": if self.external_id.is_empty() { Value::Null } else { json!(self.external_id) },
            "values": values,
            "metadata": if self.metadata.is_object() { self.metadata.clone() } else { json!({}) },
            "preferences": if self.preferences.is_object() { self.preferences.clone() } else { json!({}) },
            "documents": documents,
            "embed_src": format!("{}/s/{}", public_url.trim_end_matches('/'), self.slug),
            "ip": if self.ip.is_empty() { Value::Null } else { json!(self.ip) },
            "ua": if self.ua.is_empty() { Value::Null } else { json!(self.ua) },
            "timezone": if self.timezone.is_empty() { Value::Null } else { json!(self.timezone) },
            "sent_at": if self.sent_at.is_empty() { Value::Null } else { json!(self.sent_at) },
            "opened_at": if self.opened_at.is_empty() { Value::Null } else { json!(self.opened_at) },
            "completed_at": if self.completed_at.is_empty() { Value::Null } else { json!(self.completed_at) },
            "declined_at": if self.declined_at.is_empty() { Value::Null } else { json!(self.declined_at) },
            "created_at": self.created,
            "updated_at": self.updated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubmitterVersion {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub submitter_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub phone: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubmissionEvent {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub submission_id: String,
    #[serde(default)]
    pub submitter_id: String,
    #[serde(default)]
    pub event_type: String,
    #[serde(default)]
    pub event_timestamp: String,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

impl SubmissionEvent {
    pub fn to_api_json(&self) -> Value {
        json!({
            "id": self.id,
            "submission_id": self.submission_id,
            "submitter_id": if self.submitter_id.is_empty() { Value::Null } else { json!(self.submitter_id) },
            "event_type": self.event_type,
            "event_timestamp": if self.event_timestamp.is_empty() { &self.created } else { &self.event_timestamp },
            "data": if self.data.is_object() { self.data.clone() } else { json!({}) },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompletedDocument {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub submitter_id: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}
