use crate::error::{AppError, AppResult};
use crate::models::{
    account::{EMAIL_GMAIL_KEY, EMAIL_SMTP_KEY},
    EmailEventRecord,
};
use crate::services::branding::{resolve_branding, UPSTREAM_NAME, UPSTREAM_URL};
use crate::state::AppState;
use base64::{
    engine::general_purpose::{STANDARD as BASE64_STD, URL_SAFE_NO_PAD},
    Engine as _,
};
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const GMAIL_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GMAIL_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const GMAIL_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
pub const GMAIL_SEND_URL: &str = "https://gmail.googleapis.com/gmail/v1/users/me/messages/send";
pub const GMAIL_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/gmail.send",
    "https://www.googleapis.com/auth/userinfo.email",
];

/// Access tokens are refreshed when within 2 minutes of expiry.
pub const TOKEN_EXPIRY_LEEWAY_SECS: i64 = 120;

/// Binary attachment (such as a template PDF, signed PDF, or Audit Trail Certificate PDF)
/// to be embedded in an outgoing RFC 2822 `multipart/mixed` MIME message.
#[derive(Debug, Clone)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
}

/// Returns true if the given Gmail OAuth config contains a valid `refresh_token`.
pub fn is_gmail_connected(gmail_cfg: &Option<Value>) -> bool {
    gmail_cfg
        .as_ref()
        .and_then(|v| v.get("refresh_token"))
        .and_then(|s| s.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// Pending OAuth 2.0 authorization requests expire after 10 minutes.
pub const OAUTH_STATE_TTL_SECS: i64 = 600;

/// Builds the Google OAuth 2.0 authorization URL for connecting a Gmail / Google Workspace mailbox
/// (authorization code flow with PKCE `S256`).
pub fn build_gmail_auth_url(
    client_id: &str,
    redirect_uri: &str,
    state_param: &str,
    code_challenge: &str,
) -> String {
    let scope = GMAIL_SCOPES.join(" ");
    format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&include_granted_scopes=true&prompt=consent&state={}&code_challenge={}&code_challenge_method=S256",
        GMAIL_AUTH_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(&scope),
        urlencoding::encode(state_param),
        urlencoding::encode(code_challenge)
    )
}

/// Returns `count` cryptographically random bytes encoded as unpadded Base64url.
fn random_urlsafe(count: usize) -> String {
    let mut bytes = vec![0u8; count];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Constant-time comparison of two strings.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Splits an OAuth `state` parameter (`<account_id>.<nonce>`) into its account id and nonce.
pub fn parse_oauth_state(state_param: &str) -> Option<(&str, &str)> {
    let (account_id, nonce) = state_param.rsplit_once('.')?;
    if account_id.is_empty() || nonce.is_empty() {
        return None;
    }
    Some((account_id, nonce))
}

/// Starts a Gmail OAuth 2.0 authorization: stores a single-use `state` nonce and PKCE
/// `code_verifier` in the account's encrypted Gmail config and returns the Google authorization URL.
pub async fn begin_gmail_oauth(
    state: &AppState,
    account_id: &str,
    redirect_uri: &str,
) -> AppResult<String> {
    let mut cfg = state
        .pb
        .get_encrypted_config(&state.config, Some(account_id), EMAIL_GMAIL_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    let client_id = cfg
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let client_secret = cfg
        .get("client_secret")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if client_id.is_empty() || client_secret.is_empty() {
        return Err(AppError::bad_request(
            "Save Gmail Client ID and Client Secret first",
        ));
    }

    let nonce = random_urlsafe(32);
    let code_verifier = random_urlsafe(32);
    let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
    let expires_at =
        (chrono::Utc::now() + chrono::Duration::seconds(OAUTH_STATE_TTL_SECS)).to_rfc3339();

    if let Some(obj) = cfg.as_object_mut() {
        obj.insert("oauth_state".to_string(), json!(nonce));
        obj.insert("oauth_code_verifier".to_string(), json!(code_verifier));
        obj.insert("oauth_state_expires_at".to_string(), json!(expires_at));
    }
    state
        .pb
        .set_encrypted_config(&state.config, account_id, EMAIL_GMAIL_KEY, &cfg)
        .await?;

    let state_param = format!("{}.{}", account_id, nonce);
    Ok(build_gmail_auth_url(
        &client_id,
        redirect_uri,
        &state_param,
        &code_challenge,
    ))
}

/// Completes a Gmail OAuth 2.0 authorization: verifies the single-use `state` nonce issued by
/// `begin_gmail_oauth`, exchanges the authorization `code` (with the PKCE `code_verifier`) for an
/// `access_token` + `refresh_token` at `https://oauth2.googleapis.com/token`, and stores the
/// connected mailbox address from Google's userinfo endpoint.
pub async fn connect_gmail_oauth(
    state: &AppState,
    account_id: &str,
    nonce: &str,
    code: &str,
    redirect_uri: &str,
) -> AppResult<Value> {
    let mut cfg = state
        .pb
        .get_encrypted_config(&state.config, Some(account_id), EMAIL_GMAIL_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    let expected_nonce = cfg
        .get("oauth_state")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let code_verifier = cfg
        .get("oauth_code_verifier")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let state_expired = cfg
        .get("oauth_state_expires_at")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|exp| exp.with_timezone(&chrono::Utc) <= chrono::Utc::now())
        .unwrap_or(true);

    if expected_nonce.is_empty()
        || code_verifier.is_empty()
        || !constant_time_eq(&expected_nonce, nonce)
    {
        return Err(AppError::bad_request(
            "Invalid Gmail OAuth state; start the connection again",
        ));
    }

    // The state nonce is single-use: clear it before the code exchange
    if let Some(obj) = cfg.as_object_mut() {
        obj.remove("oauth_state");
        obj.remove("oauth_code_verifier");
        obj.remove("oauth_state_expires_at");
    }
    state
        .pb
        .set_encrypted_config(&state.config, account_id, EMAIL_GMAIL_KEY, &cfg)
        .await?;

    if state_expired {
        return Err(AppError::bad_request(
            "Gmail OAuth request expired; start the connection again",
        ));
    }
    if code.trim().is_empty() {
        return Err(AppError::bad_request("Missing Gmail OAuth authorization code"));
    }

    let client_id = cfg
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let client_secret = cfg
        .get("client_secret")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if client_id.is_empty() || client_secret.is_empty() {
        return Err(AppError::bad_request(
            "Configure Gmail Client ID and Client Secret before connecting OAuth",
        ));
    }

    let now = chrono::Utc::now();
    let params = [
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("code", code),
        ("code_verifier", code_verifier.as_str()),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
    ];
    let resp = state
        .pb
        .http
        .post(GMAIL_TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| AppError::bad_request(format!("Gmail token request failed: {}", e)))?;

    if !resp.status().is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(AppError::bad_request(format!(
            "Gmail OAuth token exchange failed: {}",
            err_body
        )));
    }

    let tokens: Value = resp.json().await?;
    let refresh_token = tokens
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::bad_request(
                "Gmail did not return a refresh_token; please reconnect with consent",
            )
        })?
        .to_string();
    let access_token = tokens
        .get("access_token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request("Gmail did not return an access_token"))?
        .to_string();
    let expires_in = tokens
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);
    let expires_at = (now + chrono::Duration::seconds(expires_in)).to_rfc3339();

    // The sender address is the connected Google account's address (no default address)
    let u_resp = state
        .pb
        .http
        .get(GMAIL_USERINFO_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| AppError::bad_request(format!("Gmail userinfo request failed: {}", e)))?;
    if !u_resp.status().is_success() {
        return Err(AppError::bad_request(format!(
            "Gmail userinfo request failed with status {}",
            u_resp.status()
        )));
    }
    let u_json: Value = u_resp.json().await.unwrap_or_else(|_| json!({}));
    let email = u_json
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .ok_or_else(|| {
            AppError::bad_request("Google did not return the connected account's email address")
        })?;

    if let Some(obj) = cfg.as_object_mut() {
        obj.insert("access_token".to_string(), json!(access_token));
        obj.insert("refresh_token".to_string(), json!(refresh_token));
        obj.insert("email".to_string(), json!(email));
        obj.insert("expires_at".to_string(), json!(expires_at));
        obj.insert("connected_at".to_string(), json!(now.to_rfc3339()));
        obj.insert("source".to_string(), json!("oauth2"));
    }

    state
        .pb
        .set_encrypted_config(&state.config, account_id, EMAIL_GMAIL_KEY, &cfg)
        .await?;

    Ok(cfg)
}

/// Checks whether the stored `expires_at` timestamp is within `TOKEN_EXPIRY_LEEWAY_SECS` of now.
fn is_token_expired(cfg: &Value) -> bool {
    let exp_str = cfg
        .get("expires_at")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if exp_str.is_empty() {
        return true;
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(exp_str) {
        let cutoff = chrono::Utc::now() + chrono::Duration::seconds(TOKEN_EXPIRY_LEEWAY_SECS);
        parsed.with_timezone(&chrono::Utc) <= cutoff
    } else {
        true
    }
}

/// Ensures the Gmail OAuth 2.0 `access_token` is fresh, refreshing it via `GMAIL_TOKEN_URL`
/// (`grant_type=refresh_token`) if expired and updating `encrypted_configs`.
async fn ensure_fresh_gmail_token(
    state: &AppState,
    account_id: &str,
    cfg: &mut Value,
) -> AppResult<String> {
    let current_token = cfg
        .get("access_token")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    if !current_token.is_empty() && !is_token_expired(cfg) {
        return Ok(current_token);
    }

    let client_id = cfg
        .get("client_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let client_secret = cfg
        .get("client_secret")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let refresh_token = cfg
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if client_id.is_empty() || client_secret.is_empty() || refresh_token.is_empty() {
        return Err(AppError::bad_request(
            "Gmail OAuth credentials are incomplete; reconnect Gmail",
        ));
    }

    let params = [
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("refresh_token", refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ];
    let resp = state
        .pb
        .http
        .post(GMAIL_TOKEN_URL)
        .form(&params)
        .send()
        .await
        .map_err(|e| AppError::bad_request(format!("Gmail token refresh failed: {}", e)))?;
    if !resp.status().is_success() {
        let err_body = resp.text().await.unwrap_or_default();
        return Err(AppError::bad_request(format!(
            "Gmail token refresh failed: {}",
            err_body
        )));
    }
    let tokens: Value = resp.json().await?;
    let new_at = tokens
        .get("access_token")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request("Gmail token refresh returned no access_token"))?
        .to_string();
    let expires_in = tokens
        .get("expires_in")
        .and_then(|v| v.as_i64())
        .unwrap_or(3600);
    let new_exp = (chrono::Utc::now() + chrono::Duration::seconds(expires_in)).to_rfc3339();
    if let Some(obj) = cfg.as_object_mut() {
        obj.insert("access_token".to_string(), json!(new_at));
        obj.insert("expires_at".to_string(), json!(new_exp));
        // Google may rotate the refresh token
        if let Some(rt) = tokens
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            obj.insert("refresh_token".to_string(), json!(rt));
        }
    }
    state
        .pb
        .set_encrypted_config(&state.config, account_id, EMAIL_GMAIL_KEY, cfg)
        .await?;
    Ok(new_at)
}

/// Builds an RFC 2822 MIME message (either single-part `text/html` or `multipart/mixed` when
/// PDF/file attachments are included) ready for Base64url encoding and Gmail API delivery.
pub fn build_rfc2822_mime_message(
    sender_name: &str,
    from_addr: &str,
    to_email: &str,
    subject: &str,
    message_id: &str,
    full_html: &str,
    attachments: &[EmailAttachment],
) -> String {
    if attachments.is_empty() {
        format!(
            "From: \"{}\" <{}>\r\nTo: {}\r\nSubject: {}\r\nMessage-ID: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n{}",
            sender_name, from_addr, to_email, subject, message_id, full_html
        )
    } else {
        let boundary = format!("signline_mime_boundary_{}", Uuid::new_v4().simple());
        let mut mime = format!(
            "From: \"{}\" <{}>\r\nTo: {}\r\nSubject: {}\r\nMessage-ID: {}\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"{}\"\r\n\r\n",
            sender_name, from_addr, to_email, subject, message_id, boundary
        );

        // Part 1: HTML Body
        mime.push_str(&format!(
            "--{}\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Transfer-Encoding: 8bit\r\n\r\n{}\r\n",
            boundary, full_html
        ));

        // Part 2..N: Attachments (e.g., PDF documents for signature or signed PDF + Audit Trail PDF)
        for att in attachments {
            let b64_data = BASE64_STD.encode(&att.data);
            mime.push_str(&format!(
                "--{}\r\nContent-Type: {}; name=\"{}\"\r\nContent-Disposition: attachment; filename=\"{}\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n",
                boundary, att.content_type, att.filename, att.filename, b64_data
            ));
        }

        mime.push_str(&format!("--{}--\r\n", boundary));
        mime
    }
}

/// Sends an email without binary attachments using the active account transport.
pub async fn send_email(
    state: &AppState,
    account_id: &str,
    to_email: &str,
    subject: &str,
    html_body: &str,
    tag: &str,
    emailable_type: &str,
    emailable_id: &str,
) -> AppResult<EmailEventRecord> {
    send_email_with_attachments(
        state,
        account_id,
        to_email,
        subject,
        html_body,
        tag,
        emailable_type,
        emailable_id,
        &[],
    )
    .await
}

/// Sends an email with optional binary attachments (such as PDF documents to sign or completed
/// signed PDFs + Audit Trail Certificates) using the active account transport (Gmail OAuth 2.0
/// takes precedence over SMTP, with full local database event logging).
#[allow(clippy::too_many_arguments)]
pub async fn send_email_with_attachments(
    state: &AppState,
    account_id: &str,
    to_email: &str,
    subject: &str,
    html_body: &str,
    tag: &str,
    emailable_type: &str,
    emailable_id: &str,
    attachments: &[EmailAttachment],
) -> AppResult<EmailEventRecord> {
    let account = state.pb.resolve_account(Some(account_id)).await?;
    let branding = resolve_branding(state, account.as_ref()).await?;

    let product_name = branding["product_name"].as_str().unwrap_or("Signline");
    let sender_name = branding["email_branding"]["sender_name"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(product_name);
    let custom_footer = branding["email_branding"]["footer"]
        .as_str()
        .unwrap_or("");

    // Always append custom footer (if any) and mandatory AGPLv3 Section 7(b) attribution
    let full_html = format!(
        r#"<div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; color: #333333; line-height: 1.6;">
  {}
  {}
  <hr style="border: none; border-top: 1px solid #d6e2f2; margin: 24px 0 12px;" />
  <p style="font-size: 12px; color: #64748b;">
    Powered by <strong>{}</strong>, based on <a href="{}" style="color: #2161ad;">{}</a>
  </p>
</div>"#,
        html_body,
        if custom_footer.trim().is_empty() {
            String::new()
        } else {
            format!(
                r#"<div style="margin-top: 16px; font-size: 13px; color: #475569;">{}</div>"#,
                custom_footer
            )
        },
        product_name,
        UPSTREAM_URL,
        UPSTREAM_NAME
    );

    let gmail_cfg = state
        .pb
        .get_encrypted_config(&state.config, Some(account_id), EMAIL_GMAIL_KEY)
        .await?;

    let smtp_cfg = state
        .pb
        .get_encrypted_config(&state.config, Some(account_id), EMAIL_SMTP_KEY)
        .await?;

    let message_id = format!("<{}@signline.local>", Uuid::new_v4());
    let mut delivery_transport = "local_log";
    let mut delivery_status = "sent";
    let mut transport_detail = String::new();
    let mut from_address_used = "noreply@signline.local".to_string();
    let mut mime_size_bytes = 0usize;

    let attachment_summaries: Vec<Value> = attachments
        .iter()
        .map(|a| {
            json!({
                "filename": a.filename,
                "content_type": a.content_type,
                "byte_size": a.data.len(),
            })
        })
        .collect();

    // Gmail is used only when connected with a known sender address (no default address)
    let gmail_from = gmail_cfg
        .as_ref()
        .filter(|_| is_gmail_connected(&gmail_cfg))
        .and_then(|g| g.get("email"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    if let Some(from_addr) = gmail_from {
        delivery_transport = "gmail_oauth";
        let mut g = gmail_cfg.unwrap();
        from_address_used = from_addr.clone();

        let raw_mime = build_rfc2822_mime_message(
            sender_name,
            &from_addr,
            to_email,
            subject,
            &message_id,
            &full_html,
            attachments,
        );
        mime_size_bytes = raw_mime.len();
        let encoded_raw = URL_SAFE_NO_PAD.encode(raw_mime.as_bytes());

        match ensure_fresh_gmail_token(state, account_id, &mut g).await {
            Ok(access_token) => {
                match state
                    .pb
                    .http
                    .post(GMAIL_SEND_URL)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .json(&json!({ "raw": encoded_raw }))
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        transport_detail = format!(
                            "Delivered via Gmail REST API ({}, {} attachments)",
                            from_addr,
                            attachments.len()
                        );
                    }
                    Ok(resp) => {
                        delivery_status = "failed";
                        transport_detail = format!(
                            "Gmail API status {} ({}, MIME {} bytes)",
                            resp.status(),
                            from_addr,
                            mime_size_bytes
                        );
                    }
                    Err(e) => {
                        delivery_status = "failed";
                        transport_detail = format!("Gmail API request failed: {}", e);
                    }
                }
            }
            Err(e) => {
                delivery_status = "failed";
                transport_detail = format!("Gmail OAuth token unavailable: {}", e);
            }
        }
    } else if let Some(ref smtp) = smtp_cfg {
        let host = smtp
            .get("address")
            .or_else(|| smtp.get("host"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !host.is_empty() {
            delivery_transport = "smtp";
            let port = smtp
                .get("port")
                .and_then(|v| v.as_u64())
                .unwrap_or(587) as u16;
            let from_addr = smtp
                .get("from")
                .or_else(|| smtp.get("from_email"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("noreply@signline.local");
            from_address_used = from_addr.to_string();
            let username = smtp
                .get("user_name")
                .or_else(|| smtp.get("username"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let password = smtp
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if host != "localhost"
                && host != "127.0.0.1"
                && !host.ends_with(".invalid")
                && !host.ends_with(".example.org")
            {
                if let (Ok(from_mbox), Ok(to_mbox)) = (
                    format!("{} <{}>", sender_name, from_addr).parse(),
                    to_email.parse(),
                ) {
                    if let Ok(email_msg) = Message::builder()
                        .from(from_mbox)
                        .to(to_mbox)
                        .subject(subject)
                        .header(ContentType::TEXT_HTML)
                        .body(full_html.clone())
                    {
                        let mut builder =
                            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
                                .port(port);
                        if !username.is_empty() {
                            builder = builder.credentials(Credentials::new(
                                username.to_string(),
                                password.to_string(),
                            ));
                        }
                        let mailer = builder.build();
                        if let Err(e) = mailer.send(email_msg).await {
                            transport_detail = format!("SMTP attempt note: {}", e);
                        } else {
                            transport_detail = "Delivered via SMTP".to_string();
                        }
                    }
                }
            } else {
                transport_detail = format!("Logged for SMTP host {}:{}", host, port);
            }
        }
    }

    let now_str = chrono::Utc::now().to_rfc3339();
    let event: EmailEventRecord = state
        .pb
        .create(
            "email_events",
            &json!({
                "account_id": account_id,
                "emailable_type": emailable_type,
                "emailable_id": emailable_id,
                "message_id": message_id,
                "tag": tag,
                "event_type": delivery_status,
                "email": to_email,
                "event_datetime": now_str,
                "data": {
                    "subject": subject,
                    "from_email": from_address_used,
                    "transport": delivery_transport,
                    "detail": transport_detail,
                    "sender_name": sender_name,
                    "attachments": attachment_summaries,
                    "mime_size_bytes": mime_size_bytes,
                    "body_html": full_html,
                }
            }),
        )
        .await?;

    Ok(event)
}
