use crate::error::{AppError, AppResult};
use crate::pb::PocketBaseClient;
use serde_json::{json, Value};
use std::collections::HashSet;
use tracing::info;

fn text_field(name: &str) -> Value {
    json!({
        "name": name,
        "type": "text",
        "required": false,
        "max": 10000000
    })
}

fn number_field(name: &str) -> Value {
    json!({
        "name": name,
        "type": "number",
        "required": false
    })
}

fn bool_field(name: &str) -> Value {
    json!({
        "name": name,
        "type": "bool",
        "required": false
    })
}

fn date_field(name: &str) -> Value {
    json!({
        "name": name,
        "type": "date",
        "required": false
    })
}

fn json_field(name: &str) -> Value {
    json!({
        "name": name,
        "type": "json",
        "required": false,
        "maxSize": 20000000
    })
}

fn build_collection_def(name: &str, mut fields: Vec<Value>, indexes: Vec<&str>) -> Value {
    // Always append created and updated autodate fields for PocketBase 0.25+
    fields.push(json!({
        "name": "created",
        "type": "autodate",
        "onCreate": true,
        "onUpdate": false
    }));
    fields.push(json!({
        "name": "updated",
        "type": "autodate",
        "onCreate": true,
        "onUpdate": true
    }));

    json!({
        "name": name,
        "type": "base",
        "fields": fields,
        "indexes": indexes
    })
}

/// Idempotently creates all 34 Signline domain collections in PocketBase on startup.
pub async fn ensure_all_collections(pb: &PocketBaseClient) -> AppResult<()> {
    let token = pb.get_token().await?;
    let list_url = format!("{}/api/collections?perPage=200", pb.base_url);
    let resp = pb
        .http
        .get(&list_url)
        .header("Authorization", &token)
        .send()
        .await?;

    let existing_json: Value = resp.json().await?;
    let mut existing_names = HashSet::new();
    if let Some(items) = existing_json.get("items").and_then(|i| i.as_array()) {
        for item in items {
            if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                existing_names.insert(name.to_string());
            }
        }
    }

    let definitions = all_collection_definitions();
    let mut created_count = 0usize;

    for def in definitions {
        let name = def["name"].as_str().unwrap_or_default();
        if existing_names.contains(name) {
            continue;
        }

        let create_url = format!("{}/api/collections", pb.base_url);
        let create_resp = pb
            .http
            .post(&create_url)
            .header("Authorization", &token)
            .json(&def)
            .send()
            .await?;

        if !create_resp.status().is_success() {
            let status = create_resp.status();
            let body = create_resp.text().await.unwrap_or_default();
            return Err(AppError::PocketBase(format!(
                "Failed to create collection '{}' ({}): {}",
                name, status, body
            )));
        }
        created_count += 1;
    }

    if created_count > 0 {
        info!(
            "Migrated {} new PocketBase collections (34 total domain collections active)",
            created_count
        );
    } else {
        info!("All 34 PocketBase domain collections verified");
    }

    Ok(())
}

fn all_collection_definitions() -> Vec<Value> {
    vec![
        // 1. accounts
        build_collection_def(
            "accounts",
            vec![
                text_field("uuid"),
                text_field("name"),
                text_field("timezone"),
                text_field("locale"),
                text_field("logo_file"),
                text_field("favicon_file"),
                date_field("archived_at"),
            ],
            vec!["CREATE UNIQUE INDEX idx_accounts_uuid ON accounts (uuid)"],
        ),
        // 2. app_users
        build_collection_def(
            "app_users",
            vec![
                text_field("uuid"),
                text_field("account_id"),
                text_field("email"),
                text_field("password_hash"),
                text_field("first_name"),
                text_field("last_name"),
                text_field("role"),
                text_field("otp_secret"),
                bool_field("otp_required_for_login"),
                number_field("consumed_timestep"),
                number_field("failed_attempts"),
                date_field("locked_at"),
                text_field("reset_password_token"),
                date_field("reset_password_sent_at"),
                number_field("sign_in_count"),
                date_field("current_sign_in_at"),
                date_field("last_sign_in_at"),
                text_field("current_sign_in_ip"),
                text_field("last_sign_in_ip"),
                text_field("signature_file"),
                text_field("initials_file"),
                date_field("archived_at"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_app_users_email ON app_users (email)",
                "CREATE UNIQUE INDEX idx_app_users_uuid ON app_users (uuid)",
                "CREATE INDEX idx_app_users_account_id ON app_users (account_id)",
            ],
        ),
        // 3. access_tokens
        build_collection_def(
            "access_tokens",
            vec![
                text_field("user_id"),
                text_field("token_encrypted"),
                text_field("sha256"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_access_tokens_sha256 ON access_tokens (sha256)",
                "CREATE INDEX idx_access_tokens_user_id ON access_tokens (user_id)",
            ],
        ),
        // 4. mcp_tokens
        build_collection_def(
            "mcp_tokens",
            vec![
                text_field("user_id"),
                text_field("name"),
                text_field("token_prefix"),
                text_field("sha256"),
                date_field("archived_at"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_mcp_tokens_sha256 ON mcp_tokens (sha256)",
                "CREATE INDEX idx_mcp_tokens_user_id ON mcp_tokens (user_id)",
            ],
        ),
        // 5. account_accesses
        build_collection_def(
            "account_accesses",
            vec![text_field("account_id"), text_field("user_id")],
            vec!["CREATE UNIQUE INDEX idx_account_accesses_pair ON account_accesses (account_id, user_id)"],
        ),
        // 6. account_linked_accounts
        build_collection_def(
            "account_linked_accounts",
            vec![
                text_field("account_id"),
                text_field("linked_account_id"),
                text_field("account_type"),
            ],
            vec!["CREATE UNIQUE INDEX idx_account_linked_pair ON account_linked_accounts (account_id, linked_account_id)"],
        ),
        // 7. account_configs
        build_collection_def(
            "account_configs",
            vec![
                text_field("account_id"),
                text_field("key"),
                json_field("value"),
            ],
            vec!["CREATE UNIQUE INDEX idx_account_configs_key ON account_configs (account_id, key)"],
        ),
        // 8. encrypted_configs
        build_collection_def(
            "encrypted_configs",
            vec![
                text_field("account_id"),
                text_field("key"),
                text_field("value_encrypted"),
            ],
            vec!["CREATE UNIQUE INDEX idx_encrypted_configs_key ON encrypted_configs (account_id, key)"],
        ),
        // 9. user_configs
        build_collection_def(
            "user_configs",
            vec![
                text_field("user_id"),
                text_field("key"),
                json_field("value"),
            ],
            vec!["CREATE UNIQUE INDEX idx_user_configs_key ON user_configs (user_id, key)"],
        ),
        // 10. encrypted_user_configs
        build_collection_def(
            "encrypted_user_configs",
            vec![
                text_field("user_id"),
                text_field("key"),
                text_field("value_encrypted"),
            ],
            vec!["CREATE UNIQUE INDEX idx_encrypted_user_configs_key ON encrypted_user_configs (user_id, key)"],
        ),
        // 11. template_folders
        build_collection_def(
            "template_folders",
            vec![
                text_field("account_id"),
                text_field("author_id"),
                text_field("parent_folder_id"),
                text_field("name"),
                date_field("archived_at"),
            ],
            vec![
                "CREATE INDEX idx_template_folders_account_id ON template_folders (account_id)",
                "CREATE INDEX idx_template_folders_parent ON template_folders (parent_folder_id)",
            ],
        ),
        // 12. templates
        build_collection_def(
            "templates",
            vec![
                text_field("account_id"),
                text_field("author_id"),
                text_field("folder_id"),
                text_field("slug"),
                text_field("name"),
                text_field("source"),
                text_field("external_id"),
                bool_field("shared_link"),
                json_field("schema"),
                json_field("fields"),
                json_field("submitters"),
                json_field("preferences"),
                json_field("variables_schema"),
                json_field("documents"),
                date_field("archived_at"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_templates_slug ON templates (slug)",
                "CREATE INDEX idx_templates_account_id ON templates (account_id)",
                "CREATE INDEX idx_templates_folder_id ON templates (folder_id)",
            ],
        ),
        // 13. template_accesses
        build_collection_def(
            "template_accesses",
            vec![text_field("template_id"), text_field("user_id")],
            vec!["CREATE UNIQUE INDEX idx_template_accesses_pair ON template_accesses (template_id, user_id)"],
        ),
        // 14. template_sharings
        build_collection_def(
            "template_sharings",
            vec![
                text_field("template_id"),
                text_field("account_id"),
                text_field("ability"),
            ],
            vec!["CREATE UNIQUE INDEX idx_template_sharings_pair ON template_sharings (account_id, template_id)"],
        ),
        // 15. template_versions
        build_collection_def(
            "template_versions",
            vec![
                text_field("template_id"),
                text_field("account_id"),
                text_field("author_id"),
                text_field("sha1"),
                json_field("data"),
            ],
            vec!["CREATE UNIQUE INDEX idx_template_versions_sha1 ON template_versions (template_id, sha1)"],
        ),
        // 16. dynamic_documents
        build_collection_def(
            "dynamic_documents",
            vec![
                text_field("template_id"),
                text_field("uuid"),
                text_field("sha1"),
                text_field("head"),
                text_field("body"),
            ],
            vec![
                "CREATE INDEX idx_dynamic_documents_template ON dynamic_documents (template_id)",
                "CREATE UNIQUE INDEX idx_dynamic_documents_uuid ON dynamic_documents (uuid)",
            ],
        ),
        // 17. dynamic_document_versions
        build_collection_def(
            "dynamic_document_versions",
            vec![
                text_field("dynamic_document_id"),
                text_field("sha1"),
                json_field("areas"),
                text_field("document_file"),
            ],
            vec!["CREATE UNIQUE INDEX idx_dyn_doc_versions_sha1 ON dynamic_document_versions (dynamic_document_id, sha1)"],
        ),
        // 18. document_metadata
        build_collection_def(
            "document_metadata",
            vec![
                text_field("account_id"),
                text_field("blob_checksum"),
                json_field("text_runs"),
            ],
            vec!["CREATE UNIQUE INDEX idx_document_metadata_checksum ON document_metadata (account_id, blob_checksum)"],
        ),
        // 19. submissions
        build_collection_def(
            "submissions",
            vec![
                text_field("account_id"),
                text_field("template_id"),
                text_field("created_by_user_id"),
                text_field("slug"),
                text_field("name"),
                text_field("source"),
                text_field("submitters_order"),
                json_field("template_fields"),
                json_field("template_schema"),
                json_field("template_submitters"),
                json_field("preferences"),
                json_field("variables"),
                json_field("variables_schema"),
                json_field("documents"),
                text_field("audit_trail_file"),
                text_field("combined_document_file"),
                date_field("expire_at"),
                date_field("completed_at"),
                date_field("archived_at"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_submissions_slug ON submissions (slug)",
                "CREATE INDEX idx_submissions_account_id ON submissions (account_id)",
                "CREATE INDEX idx_submissions_template_id ON submissions (template_id)",
            ],
        ),
        // 20. submitters
        build_collection_def(
            "submitters",
            vec![
                text_field("account_id"),
                text_field("submission_id"),
                text_field("uuid"),
                text_field("slug"),
                text_field("name"),
                text_field("email"),
                text_field("phone"),
                text_field("external_id"),
                json_field("values"),
                json_field("preferences"),
                json_field("metadata"),
                json_field("documents"),
                json_field("attachments"),
                text_field("ip"),
                text_field("ua"),
                text_field("timezone"),
                date_field("sent_at"),
                date_field("opened_at"),
                date_field("completed_at"),
                date_field("declined_at"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_submitters_slug ON submitters (slug)",
                "CREATE INDEX idx_submitters_submission_id ON submitters (submission_id)",
                "CREATE INDEX idx_submitters_account_id ON submitters (account_id)",
                "CREATE INDEX idx_submitters_email ON submitters (email)",
            ],
        ),
        // 21. submitter_versions
        build_collection_def(
            "submitter_versions",
            vec![
                text_field("submitter_id"),
                text_field("slug"),
                text_field("email"),
                text_field("name"),
                text_field("phone"),
            ],
            vec![
                "CREATE INDEX idx_submitter_versions_sub_id ON submitter_versions (submitter_id)",
                "CREATE INDEX idx_submitter_versions_slug ON submitter_versions (slug)",
            ],
        ),
        // 22. submission_events
        build_collection_def(
            "submission_events",
            vec![
                text_field("account_id"),
                text_field("submission_id"),
                text_field("submitter_id"),
                text_field("event_type"),
                date_field("event_timestamp"),
                json_field("data"),
            ],
            vec![
                "CREATE INDEX idx_submission_events_sub_id ON submission_events (submission_id)",
                "CREATE INDEX idx_submission_events_submitter_id ON submission_events (submitter_id)",
            ],
        ),
        // 23. completed_submitters
        build_collection_def(
            "completed_submitters",
            vec![
                text_field("account_id"),
                text_field("submission_id"),
                text_field("submitter_id"),
                text_field("template_id"),
                text_field("source"),
                number_field("sms_count"),
                date_field("completed_at"),
                bool_field("is_first"),
                text_field("verification_method"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_completed_submitters_sub_id ON completed_submitters (submitter_id)",
                "CREATE INDEX idx_completed_submitters_acc_id ON completed_submitters (account_id)",
            ],
        ),
        // 24. completed_documents
        build_collection_def(
            "completed_documents",
            vec![text_field("submitter_id"), text_field("sha256")],
            vec![
                "CREATE INDEX idx_completed_documents_sha256 ON completed_documents (sha256)",
                "CREATE INDEX idx_completed_documents_sub_id ON completed_documents (submitter_id)",
            ],
        ),
        // 25. document_generation_events
        build_collection_def(
            "document_generation_events",
            vec![text_field("submitter_id"), text_field("event_name")],
            vec!["CREATE INDEX idx_doc_gen_events ON document_generation_events (submitter_id, event_name)"],
        ),
        // 26. lock_events
        build_collection_def(
            "lock_events",
            vec![text_field("key"), text_field("event_name")],
            vec!["CREATE INDEX idx_lock_events ON lock_events (key, event_name)"],
        ),
        // 27. email_messages
        build_collection_def(
            "email_messages",
            vec![
                text_field("account_id"),
                text_field("author_id"),
                text_field("uuid"),
                text_field("subject"),
                text_field("body"),
                text_field("sha1"),
            ],
            vec![
                "CREATE INDEX idx_email_messages_account_id ON email_messages (account_id)",
                "CREATE INDEX idx_email_messages_uuid ON email_messages (uuid)",
            ],
        ),
        // 28. email_message_assets
        build_collection_def(
            "email_message_assets",
            vec![
                text_field("account_id"),
                text_field("sha1"),
                text_field("data"),
            ],
            vec!["CREATE UNIQUE INDEX idx_email_message_assets_sha1 ON email_message_assets (account_id, sha1)"],
        ),
        // 29. email_events
        build_collection_def(
            "email_events",
            vec![
                text_field("account_id"),
                text_field("emailable_type"),
                text_field("emailable_id"),
                text_field("message_id"),
                text_field("tag"),
                text_field("event_type"),
                text_field("email"),
                json_field("data"),
                date_field("event_datetime"),
            ],
            vec![
                "CREATE INDEX idx_email_events_account_id ON email_events (account_id)",
                "CREATE INDEX idx_email_events_email ON email_events (email)",
            ],
        ),
        // 30. search_entries
        build_collection_def(
            "search_entries",
            vec![
                text_field("account_id"),
                text_field("record_type"),
                text_field("record_id"),
                text_field("content"),
                text_field("ngram"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_search_entries_rec ON search_entries (record_type, record_id)",
                "CREATE INDEX idx_search_entries_acc ON search_entries (account_id)",
            ],
        ),
        // 31. webhook_urls
        build_collection_def(
            "webhook_urls",
            vec![
                text_field("account_id"),
                text_field("url"),
                json_field("events"),
                json_field("secret"),
                text_field("hmac_secret"),
                text_field("sha1"),
            ],
            vec![
                "CREATE INDEX idx_webhook_urls_account_id ON webhook_urls (account_id)",
                "CREATE INDEX idx_webhook_urls_sha1 ON webhook_urls (sha1)",
            ],
        ),
        // 32. webhook_events
        build_collection_def(
            "webhook_events",
            vec![
                text_field("uuid"),
                text_field("account_id"),
                text_field("webhook_url_id"),
                text_field("record_id"),
                text_field("record_type"),
                text_field("event_type"),
                text_field("status"),
            ],
            vec![
                "CREATE INDEX idx_webhook_events_url_id ON webhook_events (webhook_url_id)",
                "CREATE INDEX idx_webhook_events_acc_id ON webhook_events (account_id)",
            ],
        ),
        // 33. webhook_attempts
        build_collection_def(
            "webhook_attempts",
            vec![
                text_field("webhook_event_id"),
                number_field("attempt"),
                number_field("response_status_code"),
                text_field("response_body"),
            ],
            vec!["CREATE INDEX idx_webhook_attempts_event_id ON webhook_attempts (webhook_event_id)"],
        ),
        // 34. storage_blobs
        build_collection_def(
            "storage_blobs",
            vec![
                text_field("uuid"),
                text_field("key"),
                text_field("filename"),
                text_field("content_type"),
                number_field("byte_size"),
                text_field("checksum"),
                json_field("metadata"),
                text_field("service_name"),
            ],
            vec![
                "CREATE UNIQUE INDEX idx_storage_blobs_uuid ON storage_blobs (uuid)",
                "CREATE UNIQUE INDEX idx_storage_blobs_key ON storage_blobs (key)",
                "CREATE INDEX idx_storage_blobs_checksum ON storage_blobs (checksum)",
            ],
        ),
    ]
}
