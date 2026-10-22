use crate::error::AppResult;
use crate::models::{
    account::{Account, SUBMITTER_REMINDERS_KEY},
    submission::{Submission, SubmissionEvent, Submitter},
};
use crate::pb::pb_escape;
use crate::services::email::send_email;
use crate::state::AppState;
use serde_json::{json, Value};

/// All 16 supported reminder duration presets.
pub const REMINDER_DURATION_PRESETS: &[(&str, &str, i64)] = &[
    ("one_hour", "1 hour", 3600),
    ("two_hours", "2 hours", 2 * 3600),
    ("four_hours", "4 hours", 4 * 3600),
    ("eight_hours", "8 hours", 8 * 3600),
    ("twelve_hours", "12 hours", 12 * 3600),
    ("twenty_four_hours", "24 hours", 24 * 3600),
    ("one_day", "1 day", 24 * 3600),
    ("two_days", "2 days", 2 * 86400),
    ("three_days", "3 days", 3 * 86400),
    ("four_days", "4 days", 4 * 86400),
    ("five_days", "5 days", 5 * 86400),
    ("six_days", "6 days", 6 * 86400),
    ("seven_days", "7 days", 7 * 86400),
    ("eight_days", "8 days", 8 * 86400),
    ("fifteen_days", "15 days", 15 * 86400),
    ("twenty_one_days", "21 days", 21 * 86400),
    ("thirty_days", "30 days", 30 * 86400),
];

pub fn duration_seconds_for_key(key: &str) -> Option<i64> {
    let trimmed = key.trim();
    REMINDER_DURATION_PRESETS
        .iter()
        .find(|(k, _, _)| *k == trimmed)
        .map(|(_, _, secs)| *secs)
}

pub fn reminder_presets_json() -> Value {
    let items: Vec<Value> = REMINDER_DURATION_PRESETS
        .iter()
        .map(|(k, label, secs)| {
            json!({
                "key": k,
                "label": label,
                "seconds": secs
            })
        })
        .collect();
    json!(items)
}

/// Evaluates pending submitters across accounts and dispatches due signature request reminder emails.
/// When `force_due` is true (used by the explicit `/api/settings/notifications/run_reminders` trigger),
/// reminders configured on the account are evaluated immediately for pending submitters that have not
/// yet received that reminder slot (`first_duration`, `second_duration`, `third_duration`).
pub async fn process_pending_reminders(state: &AppState, force_due: bool) -> AppResult<Value> {
    let accounts = state
        .pb
        .list_all::<Account>("accounts", Some("archived_at = ''"), None)
        .await?;

    let mut sent_reminders = Vec::new();
    let now = chrono::Utc::now();

    for acc in accounts {
        let cfg = state
            .pb
            .get_account_config(Some(&acc.id), SUBMITTER_REMINDERS_KEY)
            .await?
            .unwrap_or_else(|| json!({}));

        let slots = ["first_duration", "second_duration", "third_duration"];
        let mut active_slots: Vec<(usize, String, String, i64)> = Vec::new();
        let mut cumulative_secs = 0i64;

        for (idx, slot_name) in slots.iter().enumerate() {
            if let Some(key) = cfg
                .get(*slot_name)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                if let Some(dur) = duration_seconds_for_key(key) {
                    cumulative_secs += dur;
                    active_slots.push((idx, (*slot_name).to_string(), key.to_string(), cumulative_secs));
                }
            }
        }

        if active_slots.is_empty() {
            continue;
        }

        let filter = format!(
            "account_id = '{}' && completed_at = '' && declined_at = '' && email != ''",
            pb_escape(&acc.id)
        );
        let pending_submitters = state
            .pb
            .list_all::<Submitter>("submitters", Some(&filter), None)
            .await?;

        for sub in pending_submitters {
            if sub
                .preferences
                .get("send_email")
                .and_then(|v| v.as_bool())
                == Some(false)
            {
                continue;
            }

            // Verify submission is neither archived nor expired
            let submission = match state
                .pb
                .get_by_id::<Submission>("submissions", &sub.submission_id)
                .await
            {
                Ok(s) => s,
                Err(_) => continue,
            };

            if !submission.archived_at.is_empty() || !submission.completed_at.is_empty() {
                continue;
            }
            if !submission.expire_at.is_empty() {
                if let Ok(exp) = chrono::DateTime::parse_from_rfc3339(&submission.expire_at) {
                    if exp < now {
                        continue;
                    }
                }
            }

            // Load existing send_reminder_email events for this submitter
            let ev_filter = format!(
                "submitter_id = '{}' && event_type = 'send_reminder_email'",
                pb_escape(&sub.id)
            );
            let existing_events = state
                .pb
                .list_all::<SubmissionEvent>("submission_events", Some(&ev_filter), None)
                .await?;

            let base_time = chrono::DateTime::parse_from_rfc3339(&sub.sent_at)
                .or_else(|_| chrono::DateTime::parse_from_rfc3339(&sub.created))
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or(now);

            for (slot_idx, slot_field, duration_key, offset_secs) in &active_slots {
                let already_sent = existing_events.iter().any(|ev| {
                    ev.data.get("reminder").and_then(|v| v.as_str()) == Some(slot_field.as_str())
                        || ev.data.get("reminder_index").and_then(|v| v.as_u64())
                            == Some(*slot_idx as u64)
                });

                if already_sent {
                    continue;
                }

                let due_at = base_time + chrono::Duration::seconds(*offset_secs);
                if force_due || now >= due_at {
                    let sign_url = format!(
                        "{}/s/{}",
                        state.config.public_url.trim_end_matches('/'),
                        sub.slug
                    );
                    let subject = format!("Reminder: Please sign {}", submission.name);
                    let body = format!(
                        "<p>Hello {},</p><p>This is a friendly reminder that you have a pending document waiting for your signature: <strong>{}</strong>.</p><p><a href=\"{}\" style=\"display:inline-block;padding:10px 20px;background:#2161ad;color:#ffffff;text-decoration:none;border-radius:6px;\">Review &amp; Sign Document</a></p>",
                        if sub.name.is_empty() { &sub.email } else { &sub.name },
                        submission.name,
                        sign_url
                    );

                    let _ = send_email(
                        state,
                        &acc.id,
                        &sub.email,
                        &subject,
                        &body,
                        "submitter_invitation_reminder",
                        "Submitter",
                        &sub.id,
                    )
                    .await;

                    let ev: SubmissionEvent = state
                        .pb
                        .create(
                            "submission_events",
                            &json!({
                                "account_id": acc.id,
                                "submission_id": sub.submission_id,
                                "submitter_id": sub.id,
                                "event_type": "send_reminder_email",
                                "event_timestamp": now.to_rfc3339(),
                                "data": {
                                    "reminder": slot_field,
                                    "reminder_index": slot_idx,
                                    "duration_key": duration_key,
                                }
                            }),
                        )
                        .await?;

                    sent_reminders.push(json!({
                        "submitter_id": sub.id,
                        "email": sub.email,
                        "reminder": slot_field,
                        "reminder_index": slot_idx,
                        "duration_key": duration_key,
                        "event_id": ev.id,
                    }));

                    // In normal or forced mode, send one reminder stage per run per submitter
                    break;
                }
            }
        }
    }

    Ok(json!({
        "processed_at": now.to_rfc3339(),
        "sent_count": sent_reminders.len(),
        "sent_reminders": sent_reminders,
    }))
}
