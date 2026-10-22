use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateFolder {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub author_id: String,
    #[serde(default)]
    pub parent_folder_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub archived_at: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Template {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub author_id: String,
    #[serde(default)]
    pub folder_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub external_id: String,
    #[serde(default)]
    pub shared_link: bool,
    #[serde(default)]
    pub schema: Value,
    #[serde(default)]
    pub fields: Value,
    #[serde(default)]
    pub submitters: Value,
    #[serde(default)]
    pub preferences: Value,
    #[serde(default)]
    pub variables_schema: Value,
    #[serde(default)]
    pub documents: Value,
    #[serde(default)]
    pub archived_at: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

impl Template {
    pub fn to_api_json(&self, folder_name: Option<&str>) -> Value {
        let schema = if self.schema.is_array() {
            self.schema.clone()
        } else {
            json!([])
        };
        let fields = if self.fields.is_array() {
            self.fields.clone()
        } else {
            json!([])
        };
        let submitters = if self.submitters.is_array() {
            self.submitters.clone()
        } else {
            json!([{"name": "First Party", "uuid": "submitter-1"}])
        };
        let documents = if self.documents.is_array() {
            self.documents.clone()
        } else {
            json!([])
        };
        let preferences = if self.preferences.is_object() {
            self.preferences.clone()
        } else {
            json!({})
        };

        json!({
            "id": self.id,
            "slug": self.slug,
            "name": self.name,
            "source": if self.source.is_empty() { "native" } else { &self.source },
            "external_id": if self.external_id.is_empty() { Value::Null } else { json!(self.external_id) },
            "folder_id": if self.folder_id.is_empty() { Value::Null } else { json!(self.folder_id) },
            "folder_name": folder_name.unwrap_or("Default"),
            "shared_link": self.shared_link,
            "author_id": self.author_id,
            "account_id": self.account_id,
            "schema": schema,
            "fields": fields,
            "submitters": submitters,
            "preferences": preferences,
            "variables_schema": self.variables_schema,
            "documents": documents,
            "archived_at": if self.archived_at.is_empty() { Value::Null } else { json!(self.archived_at) },
            "created_at": self.created,
            "updated_at": self.updated,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DynamicDocument {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub head: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateVersion {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub author_id: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub updated: String,
}
