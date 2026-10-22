use crate::auth::{
    password::{hash_password, verify_password},
    rbac::require_admin,
    resolve_testing_account, sha256_hex, AuthContext,
};
use crate::crypto::cert::{
    ensure_account_cert, generate_self_signed_bundle, parse_pkcs12_bundle,
};
use crate::error::{AppError, AppResult};
use crate::models::{
    account::{
        Account, BCC_EMAILS_KEY, BRANDING_KEY, EMAIL_BRANDING_KEY, EMAIL_GMAIL_KEY,
        EMAIL_SMTP_KEY, ESIGN_CERTS_KEY, MCP_ENABLED_KEY, SIGNING_PREFERENCES_KEY,
        STORAGE_SETTINGS_KEY, SUBMITTER_REMINDERS_KEY, SUPPORT_LINKS_KEY,
        TIMESTAMP_SERVER_URL_KEY,
    },
    user::{AccessTokenRecord, McpTokenRecord, User},
    webhook::{WebhookAttempt, WebhookEvent, WebhookUrl, ALL_WEBHOOK_EVENTS},
    EmailEventRecord, EmailMessageRecord,
};
use crate::pb::pb_escape;
use crate::services::{
    branding::{is_valid_hex_color, resolve_branding},
    email::{
        begin_gmail_oauth, connect_gmail_oauth, is_gmail_connected, parse_oauth_state,
        send_email,
    },
    reminders::{process_pending_reminders, reminder_presets_json},
    storage::{decode_data_uri_or_base64, store_blob},
    webhooks::dispatch_webhook,
};
use crate::state::AppState;
use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha1::{Digest as _, Sha1};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        // 1. Profile
        .route("/api/settings/profile", get(get_profile).put(update_profile))
        // 2. Account
        .route(
            "/api/settings/account",
            get(get_account_settings).put(update_account_settings),
        )
        // 3. Personalization / White-label
        .route(
            "/api/settings/personalization",
            get(get_personalization).put(update_personalization),
        )
        .route(
            "/api/settings/personalization/logo",
            post(upload_logo).delete(delete_logo),
        )
        .route(
            "/api/settings/personalization/favicon",
            post(upload_favicon).delete(delete_favicon),
        )
        // 4. Email (SMTP, Test, Templates, Events)
        .route(
            "/api/settings/email",
            get(get_email_settings).put(update_smtp_settings),
        )
        .route(
            "/api/settings/email/smtp",
            get(get_email_settings)
                .put(update_smtp_settings)
                .post(update_smtp_settings)
                .delete(delete_smtp_settings),
        )
        .route("/api/settings/email/test", post(send_test_email))
        .route(
            "/api/settings/email/templates",
            get(list_email_templates).post(upsert_email_template),
        )
        .route("/api/settings/email/events", get(list_email_events))
        // 5. Gmail OAuth 2.0 Integration
        .route(
            "/api/settings/email/gmail",
            get(get_gmail_settings)
                .post(save_gmail_credentials)
                .put(save_gmail_credentials)
                .delete(disconnect_gmail),
        )
        .route("/api/settings/email/gmail/connect", get(gmail_connect_url))
        .route(
            "/api/settings/email/gmail/callback",
            get(gmail_oauth_callback).post(gmail_oauth_callback_post),
        )
        // 6. Notifications & Automated Reminders
        .route(
            "/api/settings/notifications",
            get(get_notifications_settings)
                .put(update_notifications_settings)
                .post(update_notifications_settings),
        )
        .route(
            "/api/settings/notifications/run_reminders",
            post(trigger_reminders_now),
        )
        // 7. E-Signature Certificates & TSA
        .route(
            "/api/settings/esign",
            get(get_esign_settings)
                .post(update_esign_settings)
                .put(update_esign_settings)
                .delete(regenerate_self_signed_esign),
        )
        // 8. Users / Team Management
        .route("/api/settings/users", get(list_users).post(create_user))
        .route(
            "/api/settings/users/:id",
            put(update_user).delete(archive_user),
        )
        .route("/api/settings/users/:id/restore", post(restore_user))
        // 9. Webhooks
        .route(
            "/api/settings/webhooks",
            get(list_webhooks).post(create_webhook),
        )
        .route(
            "/api/settings/webhooks/:id",
            put(update_webhook).delete(delete_webhook),
        )
        .route("/api/settings/webhooks/:id/test", post(test_webhook))
        .route(
            "/api/settings/webhooks/events/:id/resend",
            post(resend_webhook_event),
        )
        // 10. Storage
        .route(
            "/api/settings/storage",
            get(get_storage_settings).put(update_storage_settings),
        )
        // 11. REST API & Sandbox
        .route("/api/settings/api", get(get_api_settings).post(rotate_api_token))
        // 12. MCP Server
        .route(
            "/api/settings/mcp",
            get(get_mcp_settings)
                .post(create_mcp_token)
                .put(toggle_mcp_server),
        )
        .route("/api/settings/mcp/tokens/:id", delete(revoke_mcp_token))
        .route("/api/settings/mcp/:id", delete(revoke_mcp_token))
}

// ---------------------------------------------------------------------------
// 1. Profile Settings
// ---------------------------------------------------------------------------
async fn get_profile(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let prefs = state
        .pb
        .get_user_config(&auth.user.id, "notifications")
        .await?
        .unwrap_or_else(|| json!({ "completed_notification": true }));

    Ok(Json(json!({
        "user": auth.user.to_public_json(),
        "preferences": prefs,
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileReq {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub current_password: Option<String>,
    pub new_password: Option<String>,
    pub signature: Option<String>,
    pub initials: Option<String>,
    pub preferences: Option<Value>,
}

async fn update_profile(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<UpdateProfileReq>,
) -> AppResult<Json<Value>> {
    let mut patch = json!({});
    if let Some(obj) = patch.as_object_mut() {
        if let Some(fn_val) = req.first_name {
            obj.insert("first_name".to_string(), json!(fn_val.trim()));
        }
        if let Some(ln_val) = req.last_name {
            obj.insert("last_name".to_string(), json!(ln_val.trim()));
        }
        if let Some(em_val) = req.email {
            let em = em_val.trim().to_lowercase();
            if !em.is_empty() && em.contains('@') {
                obj.insert("email".to_string(), json!(em));
            }
        }
        if let Some(new_pw) = req.new_password {
            if !new_pw.is_empty() {
                if let Some(curr) = req.current_password {
                    let fresh: User = state.pb.get_by_id("app_users", &auth.user.id).await?;
                    if !verify_password(&curr, &fresh.password_hash) {
                        return Err(AppError::bad_request("Current password is incorrect"));
                    }
                }
                let hashed = hash_password(&new_pw)?;
                obj.insert("password_hash".to_string(), json!(hashed));
            }
        }
        if let Some(sig) = req.signature {
            obj.insert("signature_file".to_string(), json!(sig));
        }
        if let Some(ini) = req.initials {
            obj.insert("initials_file".to_string(), json!(ini));
        }
    }

    let updated: User = state.pb.update("app_users", &auth.user.id, &patch).await?;
    if let Some(prefs) = req.preferences {
        let _ = state
            .pb
            .set_user_config(&auth.user.id, "notifications", prefs)
            .await;
    }

    let prefs = state
        .pb
        .get_user_config(&auth.user.id, "notifications")
        .await?
        .unwrap_or_else(|| json!({ "completed_notification": true }));

    Ok(Json(json!({
        "user": updated.to_public_json(),
        "preferences": prefs,
    })))
}

// ---------------------------------------------------------------------------
// 2. Account Settings
// ---------------------------------------------------------------------------
async fn get_account_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let signing_prefs = state
        .pb
        .get_account_config(Some(&auth.account.id), SIGNING_PREFERENCES_KEY)
        .await?
        .unwrap_or_else(|| {
            json!({
                "with_signature_id": true,
                "flatten_result_pdf": true,
                "allow_to_decline": true,
                "allow_typed_signature": true,
                "enforce_signing_order": true,
                "require_signing_reason": false,
            })
        });

    Ok(Json(json!({
        "account": auth.account,
        "signing_preferences": signing_prefs,
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateAccountReq {
    pub name: Option<String>,
    pub timezone: Option<String>,
    pub locale: Option<String>,
    pub signing_preferences: Option<Value>,
}

async fn update_account_settings(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<UpdateAccountReq>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let mut patch = json!({});
    if let Some(obj) = patch.as_object_mut() {
        if let Some(name) = req.name {
            if !name.trim().is_empty() {
                obj.insert("name".to_string(), json!(name.trim()));
            }
        }
        if let Some(tz) = req.timezone {
            obj.insert("timezone".to_string(), json!(tz.trim()));
        }
        if let Some(loc) = req.locale {
            obj.insert("locale".to_string(), json!(loc.trim()));
        }
    }

    let updated: Account = state
        .pb
        .update("accounts", &auth.account.id, &patch)
        .await?;

    if let Some(prefs) = req.signing_preferences {
        state
            .pb
            .set_account_config(&auth.account.id, SIGNING_PREFERENCES_KEY, prefs)
            .await?;
    }

    let signing_prefs = state
        .pb
        .get_account_config(Some(&auth.account.id), SIGNING_PREFERENCES_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    Ok(Json(json!({
        "account": updated,
        "signing_preferences": signing_prefs,
    })))
}

// ---------------------------------------------------------------------------
// 3. Personalization / White-label Settings
// ---------------------------------------------------------------------------
async fn get_personalization(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let branding = resolve_branding(&state, Some(&auth.account)).await?;
    Ok(Json(branding))
}

#[derive(Debug, Deserialize)]
pub struct PersonalizationReq {
    pub product_name: Option<String>,
    pub primary_color: Option<String>,
    pub accent_color: Option<String>,
    pub email_branding: Option<Value>,
    pub email_sender_name: Option<String>,
    pub email_footer: Option<String>,
    pub support_links: Option<Value>,
    pub support_email: Option<String>,
    pub support_url: Option<String>,
    pub terms_url: Option<String>,
    pub privacy_url: Option<String>,
    pub logo_base64: Option<String>,
    pub favicon_base64: Option<String>,
}

async fn update_personalization(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<PersonalizationReq>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;

    let mut current_branding = state
        .pb
        .get_account_config(Some(&auth.account.id), BRANDING_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    if let Some(obj) = current_branding.as_object_mut() {
        if let Some(pname) = req.product_name {
            obj.insert("product_name".to_string(), json!(pname.trim()));
        }
        if let Some(pcol) = req.primary_color {
            let trimmed = pcol.trim();
            if trimmed.is_empty() {
                obj.remove("primary_color");
            } else if is_valid_hex_color(trimmed) {
                obj.insert("primary_color".to_string(), json!(trimmed));
            } else {
                return Err(AppError::bad_request(
                    "Invalid primary_color format; expected #RGB or #RRGGBB",
                ));
            }
        }
        if let Some(acol) = req.accent_color {
            let trimmed = acol.trim();
            if trimmed.is_empty() {
                obj.remove("accent_color");
            } else if is_valid_hex_color(trimmed) {
                obj.insert("accent_color".to_string(), json!(trimmed));
            } else {
                return Err(AppError::bad_request(
                    "Invalid accent_color format; expected #RGB or #RRGGBB",
                ));
            }
        }
    }

    state
        .pb
        .set_account_config(&auth.account.id, BRANDING_KEY, current_branding)
        .await?;

    if let Some(eb) = req.email_branding {
        state
            .pb
            .set_account_config(&auth.account.id, EMAIL_BRANDING_KEY, eb)
            .await?;
    } else if req.email_sender_name.is_some() || req.email_footer.is_some() {
        let mut current_eb = state
            .pb
            .get_account_config(Some(&auth.account.id), EMAIL_BRANDING_KEY)
            .await?
            .unwrap_or_else(|| json!({}));
        if let Some(obj) = current_eb.as_object_mut() {
            if let Some(sn) = req.email_sender_name {
                obj.insert("sender_name".to_string(), json!(sn.trim()));
            }
            if let Some(ft) = req.email_footer {
                obj.insert("footer".to_string(), json!(ft.trim()));
            }
        }
        state
            .pb
            .set_account_config(&auth.account.id, EMAIL_BRANDING_KEY, current_eb)
            .await?;
    }

    if let Some(sl) = req.support_links {
        state
            .pb
            .set_account_config(&auth.account.id, SUPPORT_LINKS_KEY, sl)
            .await?;
    } else if req.support_email.is_some()
        || req.support_url.is_some()
        || req.terms_url.is_some()
        || req.privacy_url.is_some()
    {
        let mut current_sl = state
            .pb
            .get_account_config(Some(&auth.account.id), SUPPORT_LINKS_KEY)
            .await?
            .unwrap_or_else(|| json!({}));
        if let Some(obj) = current_sl.as_object_mut() {
            if let Some(se) = req.support_email {
                obj.insert("support_email".to_string(), json!(se.trim()));
            }
            if let Some(su) = req.support_url {
                obj.insert("support_url".to_string(), json!(su.trim()));
            }
            if let Some(tu) = req.terms_url {
                obj.insert("terms_url".to_string(), json!(tu.trim()));
            }
            if let Some(pu) = req.privacy_url {
                obj.insert("privacy_url".to_string(), json!(pu.trim()));
            }
        }
        state
            .pb
            .set_account_config(&auth.account.id, SUPPORT_LINKS_KEY, current_sl)
            .await?;
    }

    let mut updated_account = auth.account.clone();
    if let Some(logo_b64) = req.logo_base64 {
        if !logo_b64.trim().is_empty() {
            let (mime, bytes) = decode_data_uri_or_base64(&logo_b64, "image/svg+xml")?;
            let ext = if mime.contains("svg") { "svg" } else { "png" };
            let blob = store_blob(
                &state,
                &format!("custom-logo.{}", ext),
                &mime,
                &bytes,
                json!({ "type": "logo" }),
            )
            .await?;
            updated_account = state
                .pb
                .update(
                    "accounts",
                    &auth.account.id,
                    &json!({ "logo_file": blob.uuid }),
                )
                .await?;
        }
    }

    if let Some(fav_b64) = req.favicon_base64 {
        if !fav_b64.trim().is_empty() {
            let (mime, bytes) = decode_data_uri_or_base64(&fav_b64, "image/png")?;
            let ext = if mime.contains("svg") { "svg" } else { "png" };
            let blob = store_blob(
                &state,
                &format!("custom-favicon.{}", ext),
                &mime,
                &bytes,
                json!({ "type": "favicon" }),
            )
            .await?;
            updated_account = state
                .pb
                .update(
                    "accounts",
                    &auth.account.id,
                    &json!({ "favicon_file": blob.uuid }),
                )
                .await?;
        }
    }

    let resolved = resolve_branding(&state, Some(&updated_account)).await?;
    Ok(Json(resolved))
}

async fn upload_logo(
    State(state): State<AppState>,
    auth: AuthContext,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(e.to_string()))?
    {
        if let Some(fname) = field.file_name().map(String::from) {
            let ctype = field.content_type().unwrap_or("image/png").to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::bad_request(e.to_string()))?;
            let blob = store_blob(&state, &fname, &ctype, &bytes, json!({ "type": "logo" })).await?;
            let acc: Account = state
                .pb
                .update(
                    "accounts",
                    &auth.account.id,
                    &json!({ "logo_file": blob.uuid }),
                )
                .await?;
            let branding = resolve_branding(&state, Some(&acc)).await?;
            return Ok(Json(branding));
        }
    }
    Err(AppError::bad_request("No logo file uploaded"))
}

async fn delete_logo(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let acc: Account = state
        .pb
        .update("accounts", &auth.account.id, &json!({ "logo_file": "" }))
        .await?;
    let branding = resolve_branding(&state, Some(&acc)).await?;
    Ok(Json(branding))
}

async fn upload_favicon(
    State(state): State<AppState>,
    auth: AuthContext,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(e.to_string()))?
    {
        if let Some(fname) = field.file_name().map(String::from) {
            let ctype = field.content_type().unwrap_or("image/png").to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::bad_request(e.to_string()))?;
            let blob =
                store_blob(&state, &fname, &ctype, &bytes, json!({ "type": "favicon" })).await?;
            let acc: Account = state
                .pb
                .update(
                    "accounts",
                    &auth.account.id,
                    &json!({ "favicon_file": blob.uuid }),
                )
                .await?;
            let branding = resolve_branding(&state, Some(&acc)).await?;
            return Ok(Json(branding));
        }
    }
    Err(AppError::bad_request("No favicon file uploaded"))
}

async fn delete_favicon(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let acc: Account = state
        .pb
        .update("accounts", &auth.account.id, &json!({ "favicon_file": "" }))
        .await?;
    let branding = resolve_branding(&state, Some(&acc)).await?;
    Ok(Json(branding))
}

// ---------------------------------------------------------------------------
// 4. Email SMTP, Templates & Events
// ---------------------------------------------------------------------------
async fn get_email_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let smtp = state
        .pb
        .get_encrypted_config(&state.config, Some(&auth.account.id), EMAIL_SMTP_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    let gmail = state
        .pb
        .get_encrypted_config(&state.config, Some(&auth.account.id), EMAIL_GMAIL_KEY)
        .await?;

    let gmail_connected = is_gmail_connected(&gmail);
    let active_transport = if gmail_connected {
        "gmail_oauth"
    } else if smtp
        .get("address")
        .or_else(|| smtp.get("host"))
        .and_then(|v| v.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        "smtp"
    } else {
        "local_log"
    };

    let ev_filter = format!("account_id = '{}'", pb_escape(&auth.account.id));
    let events = state
        .pb
        .list_all::<EmailEventRecord>("email_events", Some(&ev_filter), Some("-created"))
        .await
        .unwrap_or_default();

    Ok(Json(json!({
        "smtp": smtp,
        "gmail": {
            "configured": gmail.as_ref().and_then(|g| g.get("client_id")).and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false),
            "connected": gmail_connected,
            "client_id": gmail.as_ref().and_then(|g| g.get("client_id")).and_then(|v| v.as_str()).unwrap_or(""),
            "email": gmail.as_ref().and_then(|g| g.get("email")).and_then(|v| v.as_str()).unwrap_or(""),
            "sender_name": gmail.as_ref().and_then(|g| g.get("sender_name")).and_then(|v| v.as_str()).unwrap_or(""),
            "source": gmail.as_ref().and_then(|g| g.get("source")).and_then(|v| v.as_str()).unwrap_or("oauth2"),
            "connected_at": gmail.as_ref().and_then(|g| g.get("connected_at")).and_then(|v| v.as_str()).unwrap_or(""),
        },
        "active_transport": active_transport,
        "events": events,
    })))
}

async fn update_smtp_settings(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<Value>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let smtp_val = req.get("smtp").cloned().unwrap_or(req);
    state
        .pb
        .set_encrypted_config(&state.config, &auth.account.id, EMAIL_SMTP_KEY, &smtp_val)
        .await?;
    get_email_settings(State(state), auth).await
}

async fn delete_smtp_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    state
        .pb
        .delete_encrypted_config(&auth.account.id, EMAIL_SMTP_KEY)
        .await?;
    get_email_settings(State(state), auth).await
}

#[derive(Debug, Deserialize)]
pub struct TestEmailReq {
    pub to: Option<String>,
}

async fn send_test_email(
    State(state): State<AppState>,
    auth: AuthContext,
    body: Option<Json<TestEmailReq>>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let recipient = body
        .and_then(|b| b.0.to)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| auth.user.email.clone());

    let event = send_email(
        &state,
        &auth.account.id,
        &recipient,
        "Signline Test Email Configuration",
        "<p>Your Signline email delivery configuration is working properly.</p>",
        "test_email",
        "Account",
        &auth.account.id,
    )
    .await?;

    Ok(Json(json!({
        "ok": true,
        "recipient": recipient,
        "email_event": event,
    })))
}

async fn list_email_templates(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let filter = format!("account_id = '{}'", pb_escape(&auth.account.id));
    let items = state
        .pb
        .list_all::<EmailMessageRecord>("email_messages", Some(&filter), Some("-created"))
        .await?;
    Ok(Json(json!({ "templates": items })))
}

#[derive(Debug, Deserialize)]
pub struct EmailTemplatePayload {
    pub id: Option<String>,
    pub subject: String,
    pub body: String,
}

async fn upsert_email_template(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<EmailTemplatePayload>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let mut sha = Sha1::new();
    sha.update(format!("{}:{}", req.subject, req.body).as_bytes());
    let sha1_hex = hex::encode(sha.finalize());

    let rec: EmailMessageRecord = if let Some(ref id) = req.id {
        if !id.is_empty() {
            state
                .pb
                .update(
                    "email_messages",
                    id,
                    &json!({
                        "subject": req.subject,
                        "body": req.body,
                        "sha1": sha1_hex,
                    }),
                )
                .await?
        } else {
            state
                .pb
                .create(
                    "email_messages",
                    &json!({
                        "account_id": auth.account.id,
                        "author_id": auth.user.id,
                        "uuid": Uuid::new_v4().to_string(),
                        "subject": req.subject,
                        "body": req.body,
                        "sha1": sha1_hex,
                    }),
                )
                .await?
        }
    } else {
        state
            .pb
            .create(
                "email_messages",
                &json!({
                    "account_id": auth.account.id,
                    "author_id": auth.user.id,
                    "uuid": Uuid::new_v4().to_string(),
                    "subject": req.subject,
                    "body": req.body,
                    "sha1": sha1_hex,
                }),
            )
            .await?
    };

    Ok(Json(json!(rec)))
}

async fn list_email_events(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let filter = format!("account_id = '{}'", pb_escape(&auth.account.id));
    let events = state
        .pb
        .list_all::<EmailEventRecord>("email_events", Some(&filter), Some("-created"))
        .await?;
    Ok(Json(json!({ "events": events })))
}

// ---------------------------------------------------------------------------
// 5. Gmail OAuth 2.0 Integration
// ---------------------------------------------------------------------------
async fn get_gmail_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    get_email_settings(State(state), auth).await
}

#[derive(Debug, Deserialize)]
pub struct SaveGmailCredsReq {
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
}

/// Redirect URI registered with Google for the Gmail OAuth 2.0 authorization code flow.
fn gmail_redirect_uri(state: &AppState) -> String {
    format!(
        "{}/api/settings/email/gmail/callback",
        state.config.public_url.trim_end_matches('/')
    )
}

async fn save_gmail_credentials(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<SaveGmailCredsReq>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    if req.client_id.trim().is_empty() {
        return Err(AppError::bad_request("Gmail Client ID is required"));
    }
    let mut existing = state
        .pb
        .get_encrypted_config(&state.config, Some(&auth.account.id), EMAIL_GMAIL_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    if let Some(obj) = existing.as_object_mut() {
        obj.insert("client_id".to_string(), json!(req.client_id.trim()));
        if !req.client_secret.trim().is_empty() {
            obj.insert("client_secret".to_string(), json!(req.client_secret.trim()));
        }
    }

    state
        .pb
        .set_encrypted_config(&state.config, &auth.account.id, EMAIL_GMAIL_KEY, &existing)
        .await?;

    Ok(get_email_settings(State(state), auth).await?)
}

async fn gmail_connect_url(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let redirect_uri = gmail_redirect_uri(&state);
    let url = begin_gmail_oauth(&state, &auth.account.id, &redirect_uri).await?;

    Ok(Json(json!({
        "authorization_url": url,
        "redirect_uri": redirect_uri,
    })))
}

#[derive(Debug, Deserialize)]
pub struct GmailCallbackQuery {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Google redirects the browser here after consent; the result is passed back to the
/// email settings page as `?gmail=connected` or `?gmail=error&message=...`.
async fn gmail_oauth_callback(
    State(state): State<AppState>,
    Query(query): Query<GmailCallbackQuery>,
) -> Response {
    let settings_url = "/settings/email";
    let fail = |msg: &str| {
        Redirect::to(&format!(
            "{}?gmail=error&message={}",
            settings_url,
            urlencoding::encode(msg)
        ))
        .into_response()
    };

    if let Some(err) = query.error.as_deref().filter(|e| !e.is_empty()) {
        return fail(&format!("Google authorization was not completed: {}", err));
    }
    let Some((account_id, nonce)) = query.state.as_deref().and_then(parse_oauth_state) else {
        return fail("Invalid Gmail OAuth state; start the connection again");
    };
    let code = query.code.as_deref().unwrap_or("");

    let redirect_uri = gmail_redirect_uri(&state);
    match connect_gmail_oauth(&state, account_id, nonce, code, &redirect_uri).await {
        Ok(_) => Redirect::to(&format!("{}?gmail=connected", settings_url)).into_response(),
        Err(e) => {
            tracing::warn!("Gmail OAuth callback failed: {}", e);
            fail(&e.to_string())
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GmailCallbackReq {
    pub code: String,
    pub state: String,
}

async fn gmail_oauth_callback_post(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<GmailCallbackReq>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let (account_id, nonce) = parse_oauth_state(&req.state)
        .filter(|(account_id, _)| *account_id == auth.account.id)
        .ok_or_else(|| AppError::bad_request("Invalid Gmail OAuth state"))?;
    let redirect_uri = gmail_redirect_uri(&state);
    let updated = connect_gmail_oauth(&state, account_id, nonce, &req.code, &redirect_uri).await?;

    Ok(Json(json!({
        "ok": true,
        "connected": true,
        "email": updated.get("email").and_then(|v| v.as_str()).unwrap_or(""),
        "connected_at": updated.get("connected_at").and_then(|v| v.as_str()).unwrap_or(""),
    })))
}

async fn disconnect_gmail(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    state
        .pb
        .delete_encrypted_config(&auth.account.id, EMAIL_GMAIL_KEY)
        .await?;
    get_email_settings(State(state), auth).await
}

// ---------------------------------------------------------------------------
// 6. Notifications & Automated Signature Request Reminders
// ---------------------------------------------------------------------------
async fn get_notifications_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let reminders = state
        .pb
        .get_account_config(Some(&auth.account.id), SUBMITTER_REMINDERS_KEY)
        .await?
        .unwrap_or_else(|| {
            json!({
                "first_duration": "",
                "second_duration": "",
                "third_duration": "",
            })
        });

    let bcc = state
        .pb
        .get_account_config(Some(&auth.account.id), BCC_EMAILS_KEY)
        .await?
        .unwrap_or_else(|| json!({ "emails": "" }));

    Ok(Json(json!({
        "reminders": reminders,
        "bcc_emails": bcc.get("emails").and_then(|v| v.as_str()).unwrap_or(""),
        "available_durations": reminder_presets_json(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotificationsReq {
    pub first_duration: Option<String>,
    pub second_duration: Option<String>,
    pub third_duration: Option<String>,
    pub reminders: Option<Value>,
    pub bcc_emails: Option<String>,
    pub run_now: Option<bool>,
}

async fn update_notifications_settings(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<UpdateNotificationsReq>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;

    let reminder_val = if let Some(r) = req.reminders {
        r
    } else {
        json!({
            "first_duration": req.first_duration.unwrap_or_default(),
            "second_duration": req.second_duration.unwrap_or_default(),
            "third_duration": req.third_duration.unwrap_or_default(),
        })
    };

    state
        .pb
        .set_account_config(&auth.account.id, SUBMITTER_REMINDERS_KEY, reminder_val)
        .await?;

    if let Some(bcc) = req.bcc_emails {
        state
            .pb
            .set_account_config(&auth.account.id, BCC_EMAILS_KEY, json!({ "emails": bcc }))
            .await?;
    }

    if req.run_now == Some(true) {
        let result = process_pending_reminders(&state, true).await?;
        let mut settings = get_notifications_settings(State(state), auth).await?.0;
        if let Some(obj) = settings.as_object_mut() {
            obj.insert(
                "sent_count".to_string(),
                result.get("sent_count").cloned().unwrap_or_else(|| json!(0)),
            );
            obj.insert("reminder_run".to_string(), result);
        }
        return Ok(Json(settings));
    }

    get_notifications_settings(State(state), auth).await
}

async fn trigger_reminders_now(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let result = process_pending_reminders(&state, true).await?;
    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// 7. E-Signature Certificates (X.509 / PKCS#12) & RFC 3161 TSA
// ---------------------------------------------------------------------------
async fn get_esign_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let cert_bundle = ensure_account_cert(&state, &auth.account.id).await?;
    let tsa_cfg = state
        .pb
        .get_account_config(Some(&auth.account.id), TIMESTAMP_SERVER_URL_KEY)
        .await?;
    let tsa_url = tsa_cfg
        .as_ref()
        .and_then(|v| v.get("url"))
        .and_then(|s| s.as_str());

    Ok(Json(json!({
        "certificate": cert_bundle.to_public_json(tsa_url),
        "timestamp_server_url": tsa_url.unwrap_or(""),
    })))
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateEsignReq {
    pub p12_base64: Option<String>,
    pub password: Option<String>,
    pub timestamp_server_url: Option<String>,
    pub regenerate_self_signed: Option<bool>,
    pub action: Option<String>,
}

async fn update_esign_settings(
    State(state): State<AppState>,
    auth: AuthContext,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;

    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if content_type.starts_with("multipart/form-data") {
        let boundary = content_type
            .split("boundary=")
            .nth(1)
            .map(|b| b.split(';').next().unwrap_or(b).trim().trim_matches('"'))
            .unwrap_or("");
        if !boundary.is_empty() {
            // If a PKCS#12 file was uploaded via multipart, parse or regenerate
            let bundle = parse_pkcs12_bundle(&body, "").unwrap_or_else(|_| {
                generate_self_signed_bundle(&auth.account.name).unwrap()
            });
            let val = serde_json::to_value(&bundle)?;
            state
                .pb
                .set_encrypted_config(&state.config, &auth.account.id, ESIGN_CERTS_KEY, &val)
                .await?;
        }
        return get_esign_settings(State(state), auth).await;
    }

    let req: UpdateEsignReq = serde_json::from_slice(&body).unwrap_or_default();

    if let Some(ref tsa) = req.timestamp_server_url {
        state
            .pb
            .set_account_config(
                &auth.account.id,
                TIMESTAMP_SERVER_URL_KEY,
                json!({ "url": tsa.trim() }),
            )
            .await?;
    }

    if req.regenerate_self_signed == Some(true)
        || req.action.as_deref() == Some("regenerate")
    {
        let bundle = generate_self_signed_bundle(&auth.account.name)?;
        let val = serde_json::to_value(&bundle)?;
        state
            .pb
            .set_encrypted_config(&state.config, &auth.account.id, ESIGN_CERTS_KEY, &val)
            .await?;
    } else if let Some(ref p12_b64) = req.p12_base64 {
        if !p12_b64.trim().is_empty() {
            let (_, bytes) = decode_data_uri_or_base64(p12_b64, "application/x-pkcs12")?;
            let pass = req.password.as_deref().unwrap_or("");
            let bundle = parse_pkcs12_bundle(&bytes, pass)?;
            let val = serde_json::to_value(&bundle)?;
            state
                .pb
                .set_encrypted_config(&state.config, &auth.account.id, ESIGN_CERTS_KEY, &val)
                .await?;
        }
    }

    get_esign_settings(State(state), auth).await
}

async fn regenerate_self_signed_esign(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let bundle = generate_self_signed_bundle(&auth.account.name)?;
    let val = serde_json::to_value(&bundle)?;
    state
        .pb
        .set_encrypted_config(&state.config, &auth.account.id, ESIGN_CERTS_KEY, &val)
        .await?;
    get_esign_settings(State(state), auth).await
}

// ---------------------------------------------------------------------------
// 8. Users / Team Management
// ---------------------------------------------------------------------------
async fn list_users(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let filter = format!("account_id = '{}'", pb_escape(&auth.account.id));
    let all_users = state
        .pb
        .list_all::<User>("app_users", Some(&filter), Some("created"))
        .await?;

    let active: Vec<Value> = all_users
        .iter()
        .filter(|u| u.archived_at.is_empty())
        .map(|u| u.to_public_json())
        .collect();
    let archived: Vec<Value> = all_users
        .iter()
        .filter(|u| !u.archived_at.is_empty())
        .map(|u| u.to_public_json())
        .collect();

    Ok(Json(json!({
        "users": active,
        "archived_users": archived,
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateUserReq {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub role: Option<String>,
    pub password: Option<String>,
}

async fn create_user(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<CreateUserReq>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth.user)?;
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::bad_request("Valid email is required"));
    }

    let role = req.role.unwrap_or_else(|| "member".to_string());
    let raw_pw = req
        .password
        .filter(|p| p.len() >= 6)
        .unwrap_or_else(|| format!("Signline!{}", &Uuid::new_v4().simple().to_string()[..8]));
    let pw_hash = hash_password(&raw_pw)?;

    let created: User = state
        .pb
        .create(
            "app_users",
            &json!({
                "uuid": Uuid::new_v4().to_string(),
                "account_id": auth.account.id,
                "email": email,
                "password_hash": pw_hash,
                "first_name": req.first_name.unwrap_or_default().trim(),
                "last_name": req.last_name.unwrap_or_default().trim(),
                "role": role,
                "otp_required_for_login": false,
            }),
        )
        .await?;

    let _: Value = state
        .pb
        .create(
            "account_accesses",
            &json!({
                "account_id": auth.account.id,
                "user_id": created.id,
            }),
        )
        .await?;

    let sign_in_url = format!("{}/sign_in", state.config.public_url.trim_end_matches('/'));
    let welcome_html = format!(
        "<p>Hello {},</p><p>Welcome to <strong>{}</strong> on Signline! Your account has been created with the role <code>{}</code>.</p><p>You can sign in at <a href=\"{}\">{}</a> using your email address (<code>{}</code>) and temporary password: <code>{}</code></p>",
        if created.first_name.is_empty() {
            &created.email
        } else {
            &created.first_name
        },
        auth.account.name,
        created.role,
        sign_in_url,
        sign_in_url,
        created.email,
        raw_pw
    );
    let _ = send_email(
        &state,
        &auth.account.id,
        &created.email,
        &format!("Welcome to {} on Signline", auth.account.name),
        &welcome_html,
        "welcome_email",
        "User",
        &created.id,
    )
    .await;

    Ok((StatusCode::CREATED, Json(created.to_public_json())))
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserReq {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub password: Option<String>,
    pub restore: Option<bool>,
}

async fn update_user(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
    Json(req): Json<UpdateUserReq>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let mut patch = json!({});
    if let Some(obj) = patch.as_object_mut() {
        if let Some(f) = req.first_name {
            obj.insert("first_name".to_string(), json!(f.trim()));
        }
        if let Some(l) = req.last_name {
            obj.insert("last_name".to_string(), json!(l.trim()));
        }
        if let Some(e) = req.email {
            obj.insert("email".to_string(), json!(e.trim().to_lowercase()));
        }
        if let Some(r) = req.role {
            obj.insert("role".to_string(), json!(r.trim()));
        }
        if let Some(pw) = req.password {
            if pw.len() >= 6 {
                obj.insert("password_hash".to_string(), json!(hash_password(&pw)?));
            }
        }
        if req.restore == Some(true) {
            obj.insert("archived_at".to_string(), json!(""));
        }
    }

    let updated: User = state.pb.update("app_users", &id, &patch).await?;
    Ok(Json(updated.to_public_json()))
}

async fn archive_user(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    if id == auth.user.id {
        return Err(AppError::bad_request(
            "You cannot archive your own active administrator account",
        ));
    }
    let now_str = chrono::Utc::now().to_rfc3339();
    let updated: User = state
        .pb
        .update("app_users", &id, &json!({ "archived_at": now_str }))
        .await?;
    Ok(Json(updated.to_public_json()))
}

async fn restore_user(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let updated: User = state
        .pb
        .update("app_users", &id, &json!({ "archived_at": "" }))
        .await?;
    Ok(Json(updated.to_public_json()))
}

// ---------------------------------------------------------------------------
// 9. Webhooks Settings
// ---------------------------------------------------------------------------
async fn list_webhooks(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let filter = format!("account_id = '{}'", pb_escape(&auth.account.id));
    let urls = state
        .pb
        .list_all::<WebhookUrl>("webhook_urls", Some(&filter), Some("-created"))
        .await?;
    let events = state
        .pb
        .list_all::<WebhookEvent>("webhook_events", Some(&filter), Some("-created"))
        .await?;

    let mut events_with_attempts = Vec::new();
    for ev in events.into_iter().take(50) {
        let att_filter = format!("webhook_event_id = '{}'", pb_escape(&ev.id));
        let attempts = state
            .pb
            .list_all::<WebhookAttempt>("webhook_attempts", Some(&att_filter), Some("-attempt"))
            .await
            .unwrap_or_default();
        events_with_attempts.push(json!({
            "id": ev.id,
            "uuid": ev.uuid,
            "webhook_url_id": ev.webhook_url_id,
            "event_type": ev.event_type,
            "record_type": ev.record_type,
            "record_id": ev.record_id,
            "status": ev.status,
            "created_at": ev.created,
            "attempts": attempts,
        }));
    }

    let urls_json: Vec<Value> = urls.iter().map(|u| u.to_api_json()).collect();

    Ok(Json(json!({
        "webhooks": urls_json,
        "events": events_with_attempts,
        "available_events": ALL_WEBHOOK_EVENTS,
    })))
}

#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    pub url: String,
    pub events: Option<Vec<String>>,
    pub secret: Option<Value>,
    pub hmac_secret: Option<String>,
}

async fn create_webhook(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<WebhookPayload>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth.user)?;
    let url = req.url.trim();
    if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
        return Err(AppError::bad_request(
            "Webhook URL must start with http:// or https://",
        ));
    }

    let events = req.events.unwrap_or_else(|| {
        ALL_WEBHOOK_EVENTS
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    });
    let hmac_secret = req
        .hmac_secret
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("whsec_{}", Uuid::new_v4().simple()));

    let mut sha = Sha1::new();
    sha.update(url.as_bytes());
    let sha1_hex = hex::encode(sha.finalize());

    let created: WebhookUrl = state
        .pb
        .create(
            "webhook_urls",
            &json!({
                "account_id": auth.account.id,
                "url": url,
                "events": events,
                "secret": req.secret.unwrap_or_else(|| json!({})),
                "hmac_secret": hmac_secret,
                "sha1": sha1_hex,
            }),
        )
        .await?;

    Ok((StatusCode::CREATED, Json(created.to_api_json())))
}

async fn update_webhook(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
    Json(req): Json<WebhookPayload>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let mut patch = json!({ "url": req.url.trim() });
    if let Some(obj) = patch.as_object_mut() {
        if let Some(ev) = req.events {
            obj.insert("events".to_string(), json!(ev));
        }
        if let Some(sec) = req.secret {
            obj.insert("secret".to_string(), sec);
        }
        if let Some(hmac) = req.hmac_secret {
            obj.insert("hmac_secret".to_string(), json!(hmac));
        }
    }
    let updated: WebhookUrl = state.pb.update("webhook_urls", &id, &patch).await?;
    Ok(Json(updated.to_api_json()))
}

async fn delete_webhook(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    state.pb.delete("webhook_urls", &id).await?;
    Ok(Json(json!({ "ok": true, "id": id })))
}

async fn test_webhook(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let wh: WebhookUrl = state.pb.get_by_id("webhook_urls", &id).await?;
    let events = dispatch_webhook(
        &state,
        &auth.account.id,
        "form.completed",
        "WebhookTest",
        &wh.id,
        json!({
            "test": true,
            "webhook_id": wh.id,
            "message": "Signline test webhook event"
        }),
    )
    .await?;

    Ok(Json(json!({
        "ok": true,
        "dispatched_events": events,
    })))
}

async fn resend_webhook_event(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    let ev: WebhookEvent = state.pb.get_by_id("webhook_events", &id).await?;
    let dispatched = dispatch_webhook(
        &state,
        &auth.account.id,
        &ev.event_type,
        &ev.record_type,
        &ev.record_id,
        json!({ "resent_from_event_id": ev.id }),
    )
    .await?;
    Ok(Json(json!({ "ok": true, "dispatched": dispatched })))
}

// ---------------------------------------------------------------------------
// 10. Storage Settings
// ---------------------------------------------------------------------------
async fn get_storage_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let cfg = state
        .pb
        .get_account_config(Some(&auth.account.id), STORAGE_SETTINGS_KEY)
        .await?
        .unwrap_or_else(|| {
            json!({
                "service": "disk",
                "bucket": "",
                "region": "",
                "endpoint": "",
            })
        });
    Ok(Json(cfg))
}

async fn update_storage_settings(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<Value>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    state
        .pb
        .set_account_config(&auth.account.id, STORAGE_SETTINGS_KEY, req.clone())
        .await?;
    Ok(Json(req))
}

// ---------------------------------------------------------------------------
// 11. REST API & Sandbox Mode Settings
// ---------------------------------------------------------------------------
async fn get_api_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let filter = format!("user_id = '{}'", pb_escape(&auth.user.id));
    let mut token_rec = state
        .pb
        .find_first::<AccessTokenRecord>("access_tokens", &filter, Some("-created"))
        .await?;

    let raw_token = if let Some(ref rec) = token_rec {
        state
            .config
            .decrypt(&rec.token_encrypted)
            .unwrap_or_default()
    } else {
        let new_raw = format!("sl_live_{}", Uuid::new_v4().simple());
        let enc = state.config.encrypt(&new_raw).map_err(AppError::Internal)?;
        token_rec = Some(
            state
                .pb
                .create(
                    "access_tokens",
                    &json!({
                        "user_id": auth.user.id,
                        "token_encrypted": enc,
                        "sha256": sha256_hex(&new_raw),
                    }),
                )
                .await?,
        );
        new_raw
    };

    let testing_acc = resolve_testing_account(&state, &auth.account).await?;

    Ok(Json(json!({
        "token": raw_token,
        "created_at": token_rec.map(|r| r.created).unwrap_or_default(),
        "testing_account": testing_acc,
        "testing": auth.is_testing,
        "api_base_url": format!("{}/api", state.config.public_url.trim_end_matches('/')),
    })))
}

#[derive(Debug, Deserialize, Default)]
pub struct ApiSettingsPostReq {
    pub action: Option<String>,
    pub testing: Option<bool>,
}

async fn rotate_api_token(
    State(state): State<AppState>,
    auth: AuthContext,
    body: Option<Json<ApiSettingsPostReq>>,
) -> AppResult<Json<Value>> {
    let req = body.map(|b| b.0).unwrap_or_default();
    if let Some(testing) = req.testing {
        let testing_acc = resolve_testing_account(&state, &auth.account).await?;
        let mut res = get_api_settings(State(state), auth).await?.0;
        if let Some(obj) = res.as_object_mut() {
            obj.insert("testing".to_string(), json!(testing));
            obj.insert("testing_account".to_string(), json!(testing_acc));
        }
        return Ok(Json(res));
    }

    let filter = format!("user_id = '{}'", pb_escape(&auth.user.id));
    let existing = state
        .pb
        .list_all::<AccessTokenRecord>("access_tokens", Some(&filter), None)
        .await?;
    for old in existing {
        let _ = state.pb.delete("access_tokens", &old.id).await;
    }

    let new_raw = format!("sl_live_{}", Uuid::new_v4().simple());
    let enc = state.config.encrypt(&new_raw).map_err(AppError::Internal)?;
    let rec: AccessTokenRecord = state
        .pb
        .create(
            "access_tokens",
            &json!({
                "user_id": auth.user.id,
                "token_encrypted": enc,
                "sha256": sha256_hex(&new_raw),
            }),
        )
        .await?;

    Ok(Json(json!({
        "token": new_raw,
        "created_at": rec.created,
    })))
}

// ---------------------------------------------------------------------------
// 12. Model Context Protocol (MCP) Settings
// ---------------------------------------------------------------------------
async fn get_mcp_settings(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let enabled_cfg = state
        .pb
        .get_account_config(Some(&auth.account.id), MCP_ENABLED_KEY)
        .await?
        .unwrap_or_else(|| json!({ "enabled": true }));

    let enabled = enabled_cfg
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let filter = format!(
        "user_id = '{}' && archived_at = ''",
        pb_escape(&auth.user.id)
    );
    let tokens = state
        .pb
        .list_all::<McpTokenRecord>("mcp_tokens", Some(&filter), Some("-created"))
        .await?;

    Ok(Json(json!({
        "enabled": enabled,
        "mcp_url": format!("{}/mcp", state.config.public_url.trim_end_matches('/')),
        "tokens": tokens,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ToggleMcpReq {
    pub enabled: bool,
}

async fn toggle_mcp_server(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<ToggleMcpReq>,
) -> AppResult<Json<Value>> {
    require_admin(&auth.user)?;
    state
        .pb
        .set_account_config(
            &auth.account.id,
            MCP_ENABLED_KEY,
            json!({ "enabled": req.enabled }),
        )
        .await?;
    get_mcp_settings(State(state), auth).await
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateMcpTokenReq {
    pub name: Option<String>,
    pub enabled: Option<bool>,
}

async fn create_mcp_token(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<CreateMcpTokenReq>,
) -> AppResult<impl IntoResponse> {
    if let Some(enabled) = req.enabled {
        if req.name.is_none() {
            require_admin(&auth.user)?;
            state
                .pb
                .set_account_config(
                    &auth.account.id,
                    MCP_ENABLED_KEY,
                    json!({ "enabled": enabled }),
                )
                .await?;
            let status = get_mcp_settings(State(state), auth).await?;
            return Ok((StatusCode::OK, status));
        }
    }

    let name = req
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "MCP Client".to_string());
    let raw_token = format!("sl_mcp_{}", Uuid::new_v4().simple());
    let prefix = raw_token[..13].to_string();

    let rec: McpTokenRecord = state
        .pb
        .create(
            "mcp_tokens",
            &json!({
                "user_id": auth.user.id,
                "name": name,
                "token_prefix": prefix,
                "sha256": sha256_hex(&raw_token),
            }),
        )
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": rec.id,
            "name": rec.name,
            "token": raw_token,
            "token_prefix": rec.token_prefix,
            "created_at": rec.created,
        })),
    ))
}

async fn revoke_mcp_token(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let now_str = chrono::Utc::now().to_rfc3339();
    let _: McpTokenRecord = state
        .pb
        .update(
            "mcp_tokens",
            &id,
            &json!({
                "archived_at": now_str,
                "user_id": auth.user.id,
            }),
        )
        .await?;
    Ok(Json(json!({ "ok": true, "id": id })))
}
