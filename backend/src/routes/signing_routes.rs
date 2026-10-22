use crate::error::{AppError, AppResult};
use crate::models::{
    account::SIGNING_PREFERENCES_KEY,
    submission::{Submission, SubmissionEvent, Submitter, SubmitterVersion},
    template::Template,
    user::User,
};
use crate::pb::pb_escape;
use crate::pdf::generator::finalize_submission_documents;
use crate::routes::submissions_routes::{
    create_submission_internal, load_submission_pdf_attachment, CreateSubmissionReq, SubmitterInput,
};
use crate::services::{
    branding::resolve_branding,
    email::{send_email, send_email_with_attachments, EmailAttachment},
    storage::read_blob_bytes,
    webhooks::dispatch_webhook,
};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/s/:slug", get(get_signing_session).put(submit_signing_form))
        .route(
            "/api/s/:slug/values",
            put(save_draft_values).post(save_draft_values),
        )
        .route("/api/s/:slug/decline", post(decline_signing))
        .route("/api/s/:slug/delegate", post(delegate_signing))
        .route(
            "/api/s/:slug/verify_email",
            post(send_email_2fa_code).put(verify_email_2fa_code),
        )
        .route("/api/d/:slug", get(get_shared_template).post(start_shared_template))
        .route("/api/e/:slug", get(get_completed_preview))
}

fn extract_client_meta(headers: &HeaderMap) -> (String, String) {
    let ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("127.0.0.1")
        .to_string();

    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Signline-Browser/1.0")
        .to_string();

    (ip, ua)
}

async fn find_submitter_by_slug(state: &AppState, slug: &str) -> AppResult<Submitter> {
    let filter = format!("slug = '{}'", pb_escape(slug));
    state
        .pb
        .find_first::<Submitter>("submitters", &filter, None)
        .await?
        .ok_or_else(|| AppError::not_found("Signing link not found or expired"))
}

/// `GET /api/s/:slug` — Loads the public submitter signing session, records `view_form`,
/// and returns all fields, documents, prefilled values, account signing preferences, and branding.
async fn get_signing_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
) -> AppResult<Json<Value>> {
    let mut submitter = find_submitter_by_slug(&state, &slug).await?;
    let submission: Submission = state
        .pb
        .get_by_id("submissions", &submitter.submission_id)
        .await?;

    if !submission.archived_at.is_empty() {
        return Err(AppError::bad_request(
            "This signature request has been archived",
        ));
    }

    let (ip, ua) = extract_client_meta(&headers);
    let now_str = chrono::Utc::now().to_rfc3339();

    if submitter.opened_at.is_empty() {
        submitter = state
            .pb
            .update(
                "submitters",
                &submitter.id,
                &json!({
                    "opened_at": now_str,
                    "ip": ip,
                    "ua": ua,
                }),
            )
            .await?;

        let _: SubmissionEvent = state
            .pb
            .create(
                "submission_events",
                &json!({
                    "account_id": submitter.account_id,
                    "submission_id": submission.id,
                    "submitter_id": submitter.id,
                    "event_type": "view_form",
                    "event_timestamp": now_str,
                    "data": { "ip": ip, "ua": ua }
                }),
            )
            .await?;

        let _ = dispatch_webhook(
            &state,
            &submitter.account_id,
            "form.viewed",
            "Submitter",
            &submitter.id,
            submitter.to_api_json(&state.config.public_url),
        )
        .await;
    }

    // Load all sibling submitters to check signing order & shared field values
    let sub_filter = format!("submission_id = '{}'", pb_escape(&submission.id));
    let all_submitters = state
        .pb
        .list_all::<Submitter>("submitters", Some(&sub_filter), Some("created"))
        .await?;

    // Check if preserved signing order requires an earlier submitter to complete first
    let mut awaiting_previous_signer = false;
    if submission.submitters_order == "preserved" {
        for s in &all_submitters {
            if s.id == submitter.id {
                break;
            }
            if s.completed_at.is_empty() {
                awaiting_previous_signer = true;
                break;
            }
        }
    }

    let require_email_2fa = submitter
        .preferences
        .get("require_email_2fa")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let email_verified = submitter
        .preferences
        .get("email_verified")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let account = state
        .pb
        .resolve_account(Some(&submitter.account_id))
        .await?;
    let branding = resolve_branding(&state, account.as_ref()).await?;
    let signing_prefs = state
        .pb
        .get_account_config(Some(&submitter.account_id), SIGNING_PREFERENCES_KEY)
        .await?
        .unwrap_or_else(|| {
            json!({
                "with_signature_id": true,
                "allow_to_decline": true,
                "allow_typed_signature": true,
            })
        });

    let all_submitters_json: Vec<Value> = all_submitters
        .iter()
        .map(|s| s.to_api_json(&state.config.public_url))
        .collect();

    Ok(Json(json!({
        "submitter": submitter.to_api_json(&state.config.public_url),
        "submission": {
            "id": submission.id,
            "slug": submission.slug,
            "name": submission.name,
            "status": submission.status(&all_submitters),
            "submitters_order": submission.submitters_order,
            "documents": submission.documents,
            "template_fields": submission.template_fields,
            "template_schema": submission.template_schema,
            "template_submitters": submission.template_submitters,
            "variables": submission.variables,
            "completed_at": if submission.completed_at.is_empty() { Value::Null } else { json!(submission.completed_at) },
            "audit_trail_url": if submission.audit_trail_file.is_empty() { Value::Null } else { json!(format!("/api/submissions/{}/download?audit=true", submission.id)) },
            "combined_document_url": if submission.combined_document_file.is_empty() { Value::Null } else { json!(format!("/api/submissions/{}/download", submission.id)) },
        },
        "all_submitters": all_submitters_json,
        "awaiting_previous_signer": awaiting_previous_signer,
        "require_email_2fa": require_email_2fa && !email_verified,
        "signing_preferences": signing_prefs,
        "branding": branding,
    })))
}

#[derive(Debug, Deserialize)]
pub struct SubmitValuesReq {
    #[serde(default)]
    pub values: Value,
    #[serde(default)]
    pub completed: Option<bool>,
    #[serde(default)]
    pub timezone: Option<String>,
}

async fn save_draft_values(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json( mut req): Json<SubmitValuesReq>,
) -> AppResult<Json<Value>> {
    if req.completed.is_none() {
        req.completed = Some(false);
    }
    submit_signing_form(State(state), headers, Path(slug), Json(req)).await
}

/// `PUT /api/s/:slug` — Saves submitted field values, records `start_form` / `complete_form`,
/// unlocks the next signer if `preserved` order is active, and when all submitters finish,
/// stamps & cryptographically signs the PDF + Audit Trail PDF.
async fn submit_signing_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(req): Json<SubmitValuesReq>,
) -> AppResult<Json<Value>> {
    let submitter = find_submitter_by_slug(&state, &slug).await?;
    if !submitter.declined_at.is_empty() {
        return Err(AppError::bad_request(
            "Cannot submit values for a declined signature request",
        ));
    }

    let mut submission: Submission = state
        .pb
        .get_by_id("submissions", &submitter.submission_id)
        .await?;

    let (ip, ua) = extract_client_meta(&headers);
    let now_str = chrono::Utc::now().to_rfc3339();
    let is_completing = req.completed.unwrap_or(true);

    // Merge existing values with newly submitted values
    let mut merged_values = if submitter.values.is_object() {
        submitter.values.clone()
    } else {
        json!({})
    };
    if let (Some(target), Some(incoming)) = (merged_values.as_object_mut(), req.values.as_object())
    {
        for (k, v) in incoming {
            target.insert(k.clone(), v.clone());
        }
    }

    // Check if this is the first time the submitter started filling the form
    let start_ev_filter = format!(
        "submitter_id = '{}' && event_type = 'start_form'",
        pb_escape(&submitter.id)
    );
    let has_started = state
        .pb
        .find_first::<SubmissionEvent>("submission_events", &start_ev_filter, None)
        .await?
        .is_some();

    if !has_started {
        let _: SubmissionEvent = state
            .pb
            .create(
                "submission_events",
                &json!({
                    "account_id": submitter.account_id,
                    "submission_id": submission.id,
                    "submitter_id": submitter.id,
                    "event_type": "start_form",
                    "event_timestamp": now_str,
                    "data": { "ip": ip, "ua": ua }
                }),
            )
            .await?;

        let _ = dispatch_webhook(
            &state,
            &submitter.account_id,
            "form.started",
            "Submitter",
            &submitter.id,
            submitter.to_api_json(&state.config.public_url),
        )
        .await;
    }

    let completed_at = if is_completing {
        now_str.clone()
    } else {
        submitter.completed_at.clone()
    };

    let updated_submitter: Submitter = state
        .pb
        .update(
            "submitters",
            &submitter.id,
            &json!({
                "values": merged_values,
                "ip": ip,
                "ua": ua,
                "timezone": req.timezone.unwrap_or_else(|| {
                    if submitter.timezone.is_empty() { "UTC".to_string() } else { submitter.timezone.clone() }
                }),
                "completed_at": completed_at,
            }),
        )
        .await?;

    if is_completing && submitter.completed_at.is_empty() {
        let _: SubmissionEvent = state
            .pb
            .create(
                "submission_events",
                &json!({
                    "account_id": submitter.account_id,
                    "submission_id": submission.id,
                    "submitter_id": submitter.id,
                    "event_type": "complete_form",
                    "event_timestamp": now_str,
                    "data": { "ip": ip, "ua": ua }
                }),
            )
            .await?;

        let _ = state
            .pb
            .create::<Value>(
                "completed_submitters",
                &json!({
                    "account_id": submitter.account_id,
                    "submission_id": submission.id,
                    "submitter_id": submitter.id,
                    "template_id": submission.template_id,
                    "source": submission.source,
                    "sms_count": 0,
                    "completed_at": now_str,
                    "is_first": true,
                    "verification_method": "email",
                }),
            )
            .await;

        let _ = dispatch_webhook(
            &state,
            &submitter.account_id,
            "form.completed",
            "Submitter",
            &updated_submitter.id,
            updated_submitter.to_api_json(&state.config.public_url),
        )
        .await;
    }

    // Check all submitters on this submission
    let sub_filter = format!("submission_id = '{}'", pb_escape(&submission.id));
    let mut all_submitters = state
        .pb
        .list_all::<Submitter>("submitters", Some(&sub_filter), Some("created"))
        .await?;

    // If preserved signing order is enabled, invite the next awaiting submitter
    if is_completing && submission.submitters_order == "preserved" {
        let pdf_attachments = load_submission_pdf_attachment(&state, &submission).await;
        for s in &mut all_submitters {
            if s.completed_at.is_empty() && s.declined_at.is_empty() {
                if s.sent_at.is_empty() && !s.email.is_empty() {
                    let sign_url = format!(
                        "{}/s/{}",
                        state.config.public_url.trim_end_matches('/'),
                        s.slug
                    );
                    let _ = send_email_with_attachments(
                        &state,
                        &submission.account_id,
                        &s.email,
                        &format!("You are invited to sign {}", submission.name),
                        &format!(
                            "<p>Hello {},</p><p>The previous party has completed signing <strong>{}</strong>. It is now your turn to sign:</p><p><a href=\"{}\">{}</a></p>",
                            if s.name.is_empty() { &s.email } else { &s.name },
                            submission.name,
                            sign_url,
                            sign_url
                        ),
                        "submitter_invitation",
                        "Submitter",
                        &s.id,
                        &pdf_attachments,
                    )
                    .await;

                    *s = state
                        .pb
                        .update("submitters", &s.id, &json!({ "sent_at": now_str }))
                        .await?;

                    let _: SubmissionEvent = state
                        .pb
                        .create(
                            "submission_events",
                            &json!({
                                "account_id": submission.account_id,
                                "submission_id": submission.id,
                                "submitter_id": s.id,
                                "event_type": "send_email",
                                "event_timestamp": now_str,
                                "data": { "recipient": s.email }
                            }),
                        )
                        .await?;
                }
                break;
            }
        }
    }

    // If all submitters have completed, finalize the submission PDFs, sign with X.509/PKCS#7, and dispatch submission.completed
    let all_done = !all_submitters.is_empty()
        && all_submitters.iter().all(|s| !s.completed_at.is_empty());

    let events = state
        .pb
        .list_all::<SubmissionEvent>("submission_events", Some(&sub_filter), Some("created"))
        .await?;

    if all_done && submission.completed_at.is_empty() {
        let (combined_uuid, audit_uuid, sha256) =
            finalize_submission_documents(&state, &submission, &all_submitters, &events).await?;

        submission = state
            .pb
            .update(
                "submissions",
                &submission.id,
                &json!({
                    "completed_at": now_str,
                    "combined_document_file": combined_uuid,
                    "audit_trail_file": audit_uuid,
                }),
            )
            .await?;

        let mut completed_attachments = Vec::new();
        let safe_name = submission
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect::<String>();
        let clean_name = safe_name.trim_matches('-');
        if let Ok((_, signed_bytes)) = read_blob_bytes(&state, &combined_uuid).await {
            completed_attachments.push(EmailAttachment {
                filename: format!("{}-signed.pdf", clean_name),
                content_type: "application/pdf".to_string(),
                data: signed_bytes,
            });
        }
        if let Ok((_, audit_bytes)) = read_blob_bytes(&state, &audit_uuid).await {
            completed_attachments.push(EmailAttachment {
                filename: format!("{}-audit-trail.pdf", clean_name),
                content_type: "application/pdf".to_string(),
                data: audit_bytes,
            });
        }

        let download_url = format!(
            "{}/api/submissions/{}/download",
            state.config.public_url.trim_end_matches('/'),
            submission.id
        );
        let audit_url = format!("{}?audit=true", download_url);

        // Update each submitter's documents array with the signed document reference
        let signed_doc_entry = json!([{
            "name": format!("{}-signed.pdf", submission.name),
            "url": download_url,
            "sha256": sha256,
        }]);
        for s in &mut all_submitters {
            *s = state
                .pb
                .update(
                    "submitters",
                    &s.id,
                    &json!({ "documents": signed_doc_entry }),
                )
                .await?;
            if !s.email.is_empty() {
                let _ = send_email_with_attachments(
                    &state,
                    &submission.account_id,
                    &s.email,
                    &format!("Completed: {}", submission.name),
                    &format!(
                        "<p>All parties have completed signing <strong>{}</strong>. The cryptographically signed PDF and Audit Trail Certificate PDF are attached to this email.</p><p>Download Signed PDF: <a href=\"{}\">{}</a><br/>Download Audit Trail PDF: <a href=\"{}\">{}</a></p><p>Document SHA-256: <code>{}</code></p>",
                        submission.name, download_url, download_url, audit_url, audit_url, sha256
                    ),
                    "submission_completed",
                    "Submission",
                    &submission.id,
                    &completed_attachments,
                )
                .await;
            }
        }

        // Also send a completed document copy with signed PDF & Audit Trail PDF to the submission author / account admin
        if !submission.created_by_user_id.is_empty() {
            if let Ok(author) = state.pb.get_by_id::<User>("app_users", &submission.created_by_user_id).await {
                let already_emailed = all_submitters.iter().any(|s| s.email.eq_ignore_ascii_case(&author.email));
                if !author.email.is_empty() && !already_emailed {
                    let _ = send_email_with_attachments(
                        &state,
                        &submission.account_id,
                        &author.email,
                        &format!("Completed: {}", submission.name),
                        &format!(
                            "<p>All recipients have completed signing <strong>{}</strong>. The cryptographically signed PDF and Audit Trail Certificate PDF are attached.</p><p>Document SHA-256: <code>{}</code></p>",
                            submission.name, sha256
                        ),
                        "submission_completed",
                        "Submission",
                        &submission.id,
                        &completed_attachments,
                    )
                    .await;
                }
            }
        }

        let api_payload =
            submission.to_api_json(&all_submitters, &events, &state.config.public_url);
        let _ = dispatch_webhook(
            &state,
            &submission.account_id,
            "submission.completed",
            "Submission",
            &submission.id,
            api_payload,
        )
        .await;
    }

    Ok(Json(json!({
        "submitter": updated_submitter.to_api_json(&state.config.public_url),
        "submission": submission.to_api_json(&all_submitters, &events, &state.config.public_url),
    })))
}

#[derive(Debug, Deserialize)]
pub struct DeclineReq {
    #[serde(default)]
    pub reason: String,
}

/// `POST /api/s/:slug/decline` — Declines the signature request with a reason.
async fn decline_signing(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(req): Json<DeclineReq>,
) -> AppResult<Json<Value>> {
    let submitter = find_submitter_by_slug(&state, &slug).await?;
    let (ip, ua) = extract_client_meta(&headers);
    let now_str = chrono::Utc::now().to_rfc3339();

    let updated: Submitter = state
        .pb
        .update(
            "submitters",
            &submitter.id,
            &json!({
                "declined_at": now_str,
                "ip": ip,
                "ua": ua,
            }),
        )
        .await?;

    let _: SubmissionEvent = state
        .pb
        .create(
            "submission_events",
            &json!({
                "account_id": submitter.account_id,
                "submission_id": submitter.submission_id,
                "submitter_id": submitter.id,
                "event_type": "decline_form",
                "event_timestamp": now_str,
                "data": {
                    "reason": req.reason,
                    "ip": ip,
                    "ua": ua,
                }
            }),
        )
        .await?;

    let payload = updated.to_api_json(&state.config.public_url);
    let _ = dispatch_webhook(
        &state,
        &submitter.account_id,
        "form.declined",
        "Submitter",
        &updated.id,
        payload.clone(),
    )
    .await;

    // Notify submission author / account admin of declined signature request
    if let Ok(sub_rec) = state
        .pb
        .get_by_id::<Submission>("submissions", &submitter.submission_id)
        .await
    {
        if !sub_rec.created_by_user_id.is_empty() {
            if let Ok(author) = state
                .pb
                .get_by_id::<User>("app_users", &sub_rec.created_by_user_id)
                .await
            {
                if !author.email.is_empty() {
                    let _ = send_email(
                        &state,
                        &submitter.account_id,
                        &author.email,
                        &format!("Declined: {}", sub_rec.name),
                        &format!(
                            "<p>Recipient <strong>{}</strong> ({}) declined to sign <strong>{}</strong>.</p><p>Reason: <em>{}</em></p>",
                            if updated.name.is_empty() {
                                &updated.email
                            } else {
                                &updated.name
                            },
                            updated.email,
                            sub_rec.name,
                            if req.reason.is_empty() {
                                "No reason provided"
                            } else {
                                &req.reason
                            }
                        ),
                        "submission_declined",
                        "Submission",
                        &sub_rec.id,
                    )
                    .await;
                }
            }
        }
    }

    Ok(Json(json!({
        "submitter": payload,
        "declined": true,
        "reason": req.reason,
    })))
}

#[derive(Debug, Deserialize)]
pub struct DelegateReq {
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub reason: String,
}

/// `POST /api/s/:slug/delegate` — Backs up current signer into `submitter_versions` and delegates
/// signing to a new recipient email & name.
async fn delegate_signing(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(req): Json<DelegateReq>,
) -> AppResult<Json<Value>> {
    let submitter = find_submitter_by_slug(&state, &slug).await?;
    let new_email = req.email.trim().to_lowercase();
    if new_email.is_empty() || !new_email.contains('@') {
        return Err(AppError::bad_request(
            "A valid delegate email address is required",
        ));
    }

    let (ip, ua) = extract_client_meta(&headers);
    let now_str = chrono::Utc::now().to_rfc3339();

    // Record historical signer version
    let version: SubmitterVersion = state
        .pb
        .create(
            "submitter_versions",
            &json!({
                "submitter_id": submitter.id,
                "slug": submitter.slug,
                "email": submitter.email,
                "name": submitter.name,
                "phone": submitter.phone,
            }),
        )
        .await?;

    let updated: Submitter = state
        .pb
        .update(
            "submitters",
            &submitter.id,
            &json!({
                "email": new_email,
                "name": req.name.trim(),
                "sent_at": now_str,
                "opened_at": "",
            }),
        )
        .await?;

    let _: SubmissionEvent = state
        .pb
        .create(
            "submission_events",
            &json!({
                "account_id": submitter.account_id,
                "submission_id": submitter.submission_id,
                "submitter_id": submitter.id,
                "event_type": "delegate_form",
                "event_timestamp": now_str,
                "data": {
                    "previous_email": submitter.email,
                    "previous_name": submitter.name,
                    "delegated_to_email": updated.email,
                    "delegated_to_name": updated.name,
                    "reason": req.reason,
                    "ip": ip,
                    "ua": ua,
                }
            }),
        )
        .await?;

    let sign_url = format!(
        "{}/s/{}",
        state.config.public_url.trim_end_matches('/'),
        updated.slug
    );
    let delegated_attachments = if let Ok(sub_rec) = state
        .pb
        .get_by_id::<Submission>("submissions", &updated.submission_id)
        .await
    {
        load_submission_pdf_attachment(&state, &sub_rec).await
    } else {
        Vec::new()
    };
    let _ = send_email_with_attachments(
        &state,
        &updated.account_id,
        &updated.email,
        "Delegated Signature Request",
        &format!(
            "<p>Hello {},</p><p>{} has delegated a signature request to you. Please review the attached document and sign online:</p><p><a href=\"{}\">{}</a></p>",
            if updated.name.is_empty() { &updated.email } else { &updated.name },
            if submitter.name.is_empty() { &submitter.email } else { &submitter.name },
            sign_url,
            sign_url
        ),
        "submitter_delegated",
        "Submitter",
        &updated.id,
        &delegated_attachments,
    )
    .await;

    let payload = updated.to_api_json(&state.config.public_url);
    let _ = dispatch_webhook(
        &state,
        &updated.account_id,
        "form.delegated",
        "Submitter",
        &updated.id,
        payload.clone(),
    )
    .await;

    Ok(Json(json!({
        "submitter": payload,
        "previous_version": version,
        "delegated": true,
    })))
}

async fn send_email_2fa_code(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Value>> {
    let submitter = find_submitter_by_slug(&state, &slug).await?;
    let code = format!("{:06}", (Uuid::new_v4().as_u128() % 900_000) + 100_000);

    let mut prefs = if submitter.preferences.is_object() {
        submitter.preferences.clone()
    } else {
        json!({})
    };
    if let Some(obj) = prefs.as_object_mut() {
        obj.insert("email_otp_code".to_string(), json!(code));
    }

    let _: Submitter = state
        .pb
        .update(
            "submitters",
            &submitter.id,
            &json!({ "preferences": prefs }),
        )
        .await?;

    let _ = send_email(
        &state,
        &submitter.account_id,
        &submitter.email,
        "Your Signline Verification Code",
        &format!(
            "<p>Your 6-digit verification code to access the signing document is: <strong>{}</strong></p>",
            code
        ),
        "submitter_email_2fa",
        "Submitter",
        &submitter.id,
    )
    .await;

    Ok(Json(json!({
        "ok": true,
        "sent": true,
        "verification_code_preview": code,
    })))
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailOtpReq {
    pub code: String,
}

async fn verify_email_2fa_code(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Json(req): Json<VerifyEmailOtpReq>,
) -> AppResult<Json<Value>> {
    let submitter = find_submitter_by_slug(&state, &slug).await?;
    let expected = submitter
        .preferences
        .get("email_otp_code")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if expected.is_empty() || expected != req.code.trim() {
        return Err(AppError::bad_request("Invalid verification code"));
    }

    let mut prefs = submitter.preferences.clone();
    if let Some(obj) = prefs.as_object_mut() {
        obj.insert("email_verified".to_string(), json!(true));
        obj.remove("email_otp_code");
    }

    let updated: Submitter = state
        .pb
        .update(
            "submitters",
            &submitter.id,
            &json!({ "preferences": prefs }),
        )
        .await?;

    let _: SubmissionEvent = state
        .pb
        .create(
            "submission_events",
            &json!({
                "account_id": submitter.account_id,
                "submission_id": submitter.submission_id,
                "submitter_id": submitter.id,
                "event_type": "email_verified",
                "event_timestamp": chrono::Utc::now().to_rfc3339(),
                "data": { "email": submitter.email }
            }),
        )
        .await?;

    Ok(Json(json!({
        "ok": true,
        "verified": true,
        "submitter": updated.to_api_json(&state.config.public_url)
    })))
}

/// `GET /api/d/:slug` — Loads a public shared template (`/d/:slug`) for self-serve signing.
async fn get_shared_template(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Value>> {
    let filter = format!("slug = '{}' && archived_at = ''", pb_escape(&slug));
    let tpl = state
        .pb
        .find_first::<Template>("templates", &filter, None)
        .await?
        .ok_or_else(|| AppError::not_found("Shared template link not found"))?;

    let account = state.pb.resolve_account(Some(&tpl.account_id)).await?;
    let branding = resolve_branding(&state, account.as_ref()).await?;

    Ok(Json(json!({
        "template": tpl.to_api_json(None),
        "branding": branding,
    })))
}

#[derive(Debug, Deserialize)]
pub struct StartSharedFormReq {
    pub email: Option<String>,
    pub name: Option<String>,
    pub phone: Option<String>,
    #[serde(default)]
    pub values: Option<Value>,
    #[serde(default)]
    pub completed: Option<bool>,
}

/// `POST /api/d/:slug` — Self-initiates a submission from a public shared template link (`/d/:slug`)
/// and returns the newly created submitter signing session (`/s/:submitter_slug`).
async fn start_shared_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Json(req): Json<StartSharedFormReq>,
) -> AppResult<Json<Value>> {
    let filter = format!("slug = '{}' && archived_at = ''", pb_escape(&slug));
    let tpl = state
        .pb
        .find_first::<Template>("templates", &filter, None)
        .await?
        .ok_or_else(|| AppError::not_found("Shared template link not found"))?;

    let email = req
        .email
        .unwrap_or_else(|| "signer@example.com".to_string());
    let name = req.name.unwrap_or_else(|| "Self-Serve Signer".to_string());

    let (submission, submitters, _) = create_submission_internal(
        &state,
        &tpl.account_id,
        &tpl.author_id,
        &tpl,
        CreateSubmissionReq {
            template_id: tpl.id.clone(),
            submitters: vec![SubmitterInput {
                name: Some(name),
                email: Some(email),
                phone: req.phone,
                role: None,
                external_id: None,
                values: req.values.clone(),
                metadata: None,
                send_email: Some(false),
                require_email_2fa: Some(false),
                completed: Some(false),
            }],
            send_email: Some(false),
            order: Some("preserved".to_string()),
            submitters_order: None,
            expire_at: None,
            variables: None,
            preferences: None,
            source: Some("link".to_string()),
        },
    )
    .await?;

    let first_sub = submitters
        .first()
        .ok_or_else(|| AppError::Internal("No submitter created".to_string()))?;

    // If values and `completed: true` were supplied directly, complete immediately
    if req.completed == Some(true) {
        return submit_signing_form(
            State(state),
            headers,
            Path(first_sub.slug.clone()),
            Json(SubmitValuesReq {
                values: req.values.unwrap_or_else(|| json!({})),
                completed: Some(true),
                timezone: None,
            }),
        )
        .await;
    }

    Ok(Json(json!({
        "submission_id": submission.id,
        "submitter_slug": first_sub.slug,
        "submitter": first_sub.to_api_json(&state.config.public_url),
        "redirect_url": format!("/s/{}", first_sub.slug),
    })))
}

/// `GET /api/e/:slug` — Public completed submission preview and download metadata.
async fn get_completed_preview(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> AppResult<Json<Value>> {
    let filter = format!("slug = '{}'", pb_escape(&slug));
    let submission = state
        .pb
        .find_first::<Submission>("submissions", &filter, None)
        .await?
        .ok_or_else(|| AppError::not_found("Submission preview not found"))?;

    let sub_filter = format!("submission_id = '{}'", pb_escape(&submission.id));
    let submitters = state
        .pb
        .list_all::<Submitter>("submitters", Some(&sub_filter), Some("created"))
        .await?;
    let events = state
        .pb
        .list_all::<SubmissionEvent>("submission_events", Some(&sub_filter), Some("created"))
        .await?;

    let account = state
        .pb
        .resolve_account(Some(&submission.account_id))
        .await?;
    let branding = resolve_branding(&state, account.as_ref()).await?;

    Ok(Json(json!({
        "submission": submission.to_api_json(&submitters, &events, &state.config.public_url),
        "branding": branding,
    })))
}
