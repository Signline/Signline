use crate::error::AppResult;
use crate::models::webhook::{WebhookAttempt, WebhookEvent, WebhookUrl};
use crate::pb::pb_escape;
use crate::state::AppState;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;
use std::time::Duration;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Computes the hex-encoded HMAC-SHA256 signature for `X-Signline-Signature`.
pub fn compute_webhook_hmac(secret: &str, body_bytes: &[u8]) -> String {
    if let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) {
        mac.update(body_bytes);
        hex::encode(mac.finalize().into_bytes())
    } else {
        String::new()
    }
}

/// Dispatches a webhook event (`form.viewed`, `form.started`, `form.completed`, `form.declined`,
/// `form.delegated`, `submission.created`, `submission.completed`, `submission.expired`,
/// `submission.archived`, `template.created`, `template.updated`) to all subscribed `webhook_urls`
/// for the account, attaching the `X-Signline-Signature` HMAC-SHA256 header and recording
/// `webhook_events` and `webhook_attempts`.
pub async fn dispatch_webhook(
    state: &AppState,
    account_id: &str,
    event_type: &str,
    record_type: &str,
    record_id: &str,
    data: Value,
) -> AppResult<Vec<WebhookEvent>> {
    let filter = format!("account_id = '{}'", pb_escape(account_id));
    let urls = state
        .pb
        .list_all::<WebhookUrl>("webhook_urls", Some(&filter), None)
        .await?;

    let mut dispatched = Vec::new();
    let timestamp = chrono::Utc::now().to_rfc3339();

    let payload = json!({
        "event_type": event_type,
        "timestamp": timestamp,
        "data": data,
    });
    let payload_str = serde_json::to_string(&payload)?;

    for wh in urls {
        let subscribed = wh.events_list();
        if !subscribed.is_empty() && !subscribed.iter().any(|e| e == event_type || e == "*") {
            continue;
        }

        let event_uuid = Uuid::new_v4().to_string();
        let mut wh_event: WebhookEvent = state
            .pb
            .create(
                "webhook_events",
                &json!({
                    "uuid": event_uuid,
                    "account_id": account_id,
                    "webhook_url_id": wh.id,
                    "record_id": record_id,
                    "record_type": record_type,
                    "event_type": event_type,
                    "status": "pending",
                }),
            )
            .await?;

        let signature = compute_webhook_hmac(&wh.hmac_secret, payload_str.as_bytes());

        let mut req = state
            .pb
            .http
            .post(&wh.url)
            .timeout(Duration::from_secs(10))
            .header("Content-Type", "application/json")
            .header("User-Agent", "Signline-Webhook/0.1.0b")
            .header("X-Signline-Signature", &signature)
            .header("X-Webhook-Event", event_type);

        // Attach any custom secret headers configured on the webhook URL
        if let Some(secret_obj) = wh.secret.as_object() {
            for (k, v) in secret_obj {
                if let Some(val_str) = v.as_str() {
                    if !k.trim().is_empty() && !val_str.is_empty() {
                        req = req.header(k.as_str(), val_str);
                    }
                }
            }
        }

        let (status_code, resp_body, event_status) = match req.body(payload_str.clone()).send().await
        {
            Ok(resp) => {
                let code = resp.status().as_u16() as i64;
                let ok = resp.status().is_success();
                let txt = resp.text().await.unwrap_or_default();
                (code, txt, if ok { "success" } else { "error" })
            }
            Err(e) => (0, format!("Delivery error: {}", e), "error"),
        };

        let _: WebhookAttempt = state
            .pb
            .create(
                "webhook_attempts",
                &json!({
                    "webhook_event_id": wh_event.id,
                    "attempt": 1,
                    "response_status_code": status_code,
                    "response_body": resp_body,
                }),
            )
            .await?;

        wh_event = state
            .pb
            .update(
                "webhook_events",
                &wh_event.id,
                &json!({
                    "status": event_status,
                }),
            )
            .await?;

        dispatched.push(wh_event);
    }

    Ok(dispatched)
}
