use crate::auth::AuthContext;
use crate::error::{AppError, AppResult};
use crate::models::{
    submission::{Submission, SubmissionEvent, Submitter},
    template::Template,
};
use crate::pb::pb_escape;
use crate::pdf::generator::{build_simple_pdf, finalize_submission_documents};
use crate::routes::templates_routes::{
    build_template_from_html_internal, find_template_by_id_or_slug, HtmlTemplateReq,
};
use crate::services::{
    email::{send_email_with_attachments, EmailAttachment},
    search::index_search_entry,
    storage::read_blob_bytes,
    webhooks::dispatch_webhook,
};
use crate::state::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/submissions",
            get(list_submissions).post(create_submission),
        )
        .route("/api/submissions/init", post(create_submission))
        .route("/api/submissions/html", post(create_submission_from_html))
        .route("/api/submissions/export", get(export_submissions_csv))
        .route(
            "/api/submissions/:id",
            get(get_submission).delete(archive_submission),
        )
        .route("/api/submissions/:id/unarchive", post(unarchive_submission))
        .route(
            "/api/submissions/:id/resend_email",
            post(resend_submission_email),
        )
        .route("/api/submissions/:id/download", get(download_submission))
        .route("/api/submitters", get(list_submitters))
        .route(
            "/api/submitters/:id",
            get(get_submitter).put(update_submitter),
        )
}

fn generate_slug() -> String {
    let raw = Uuid::new_v4().simple().to_string();
    raw[..14].to_string()
}

#[derive(Debug, Deserialize, Default)]
pub struct SubmissionListQuery {
    pub template_id: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
    pub slug: Option<String>,
    pub archived: Option<bool>,
    pub limit: Option<usize>,
    pub page: Option<usize>,
}

async fn list_submissions(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(query): Query<SubmissionListQuery>,
) -> AppResult<Json<Value>> {
    let mut clauses = vec![format!("account_id = '{}'", pb_escape(&auth.account.id))];

    if query.archived.unwrap_or(false) {
        clauses.push("archived_at != ''".to_string());
    } else {
        clauses.push("archived_at = ''".to_string());
    }

    if let Some(ref tid) = query.template_id {
        if !tid.is_empty() {
            clauses.push(format!("template_id = '{}'", pb_escape(tid)));
        }
    }

    if let Some(ref slug) = query.slug {
        if !slug.is_empty() {
            clauses.push(format!("slug = '{}'", pb_escape(slug)));
        }
    }

    if let Some(ref q) = query.q {
        let q_trim = q.trim();
        if !q_trim.is_empty() {
            clauses.push(format!("name ~ '{}'", pb_escape(q_trim)));
        }
    }

    let filter = clauses.join(" && ");
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.limit.unwrap_or(50).clamp(1, 200);

    let pb_res = state
        .pb
        .list::<Submission>("submissions", page, per_page, Some(&filter), Some("-created"))
        .await?;

    let mut items = Vec::new();
    for sub in pb_res.items {
        let sub_filter = format!("submission_id = '{}'", pb_escape(&sub.id));
        let submitters = state
            .pb
            .list_all::<Submitter>("submitters", Some(&sub_filter), Some("created"))
            .await?;
        let computed_status = sub.status(&submitters);

        if let Some(ref status_filter) = query.status {
            if !status_filter.is_empty()
                && status_filter != "all"
                && !computed_status.eq_ignore_ascii_case(status_filter)
            {
                continue;
            }
        }

        let events = state
            .pb
            .list_all::<SubmissionEvent>("submission_events", Some(&sub_filter), Some("created"))
            .await?;

        items.push(sub.to_api_json(&submitters, &events, &state.config.public_url));
    }

    Ok(Json(json!({
        "data": items,
        "submissions": items,
        "pagination": {
            "count": pb_res.total_items,
            "page": pb_res.page,
            "per_page": pb_res.per_page,
            "pages": pb_res.total_pages,
        }
    })))
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubmitterInput {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub role: Option<String>,
    pub external_id: Option<String>,
    pub values: Option<Value>,
    pub metadata: Option<Value>,
    pub send_email: Option<bool>,
    pub require_email_2fa: Option<bool>,
    pub completed: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubmissionReq {
    pub template_id: String,
    pub submitters: Vec<SubmitterInput>,
    #[serde(default)]
    pub send_email: Option<bool>,
    #[serde(default)]
    pub order: Option<String>,
    #[serde(default)]
    pub submitters_order: Option<String>,
    #[serde(default)]
    pub expire_at: Option<String>,
    #[serde(default)]
    pub variables: Option<Value>,
    #[serde(default)]
    pub preferences: Option<Value>,
    #[serde(default)]
    pub source: Option<String>,
}

pub async fn create_submission_internal(
    state: &AppState,
    account_id: &str,
    user_id: &str,
    tpl: &Template,
    req: CreateSubmissionReq,
) -> AppResult<(Submission, Vec<Submitter>, Vec<SubmissionEvent>)> {
    if req.submitters.is_empty() {
        return Err(AppError::bad_request(
            "At least one submitter recipient is required",
        ));
    }

    let order = req
        .order
        .or(req.submitters_order)
        .unwrap_or_else(|| "preserved".to_string());
    let send_email_default = req.send_email.unwrap_or(true);
    let now_str = chrono::Utc::now().to_rfc3339();

    let submission: Submission = state
        .pb
        .create(
            "submissions",
            &json!({
                "account_id": account_id,
                "template_id": tpl.id,
                "created_by_user_id": user_id,
                "slug": generate_slug(),
                "name": tpl.name,
                "source": req.source.unwrap_or_else(|| "invite".to_string()),
                "submitters_order": order,
                "template_fields": tpl.fields,
                "template_schema": tpl.schema,
                "template_submitters": tpl.submitters,
                "preferences": req.preferences.unwrap_or_else(|| tpl.preferences.clone()),
                "variables": req.variables.unwrap_or_else(|| json!({})),
                "variables_schema": tpl.variables_schema,
                "documents": tpl.documents,
                "expire_at": req.expire_at.unwrap_or_default(),
            }),
        )
        .await?;

    let tpl_roles: Vec<String> = tpl
        .submitters
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.get("name").and_then(|n| n.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_else(|| vec!["First Party".to_string()]);

    let mut created_submitters = Vec::new();
    let mut created_events = Vec::new();

    for (idx, sub_in) in req.submitters.into_iter().enumerate() {
        let role = sub_in
            .role
            .filter(|r| !r.trim().is_empty())
            .or_else(|| tpl_roles.get(idx).cloned())
            .unwrap_or_else(|| "First Party".to_string());

        let email = sub_in
            .email
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        let name = sub_in.name.unwrap_or_default().trim().to_string();
        let should_send_now = send_email_default
            && sub_in.send_email.unwrap_or(true)
            && !email.is_empty()
            && (order == "random" || idx == 0);

        let sent_at = if should_send_now {
            now_str.clone()
        } else {
            String::new()
        };

        let completed_at = if sub_in.completed.unwrap_or(false) {
            now_str.clone()
        } else {
            String::new()
        };

        let sub_prefs = json!({
            "role": role,
            "send_email": sub_in.send_email.unwrap_or(send_email_default),
            "require_email_2fa": sub_in.require_email_2fa.unwrap_or(false),
        });

        let created_sub: Submitter = state
            .pb
            .create(
                "submitters",
                &json!({
                    "account_id": account_id,
                    "submission_id": submission.id,
                    "uuid": Uuid::new_v4().to_string(),
                    "slug": generate_slug(),
                    "name": name,
                    "email": email,
                    "phone": sub_in.phone.unwrap_or_default(),
                    "external_id": sub_in.external_id.unwrap_or_default(),
                    "values": sub_in.values.unwrap_or_else(|| json!({})),
                    "metadata": sub_in.metadata.unwrap_or_else(|| json!({})),
                    "preferences": sub_prefs,
                    "documents": json!([]),
                    "attachments": json!([]),
                    "sent_at": sent_at,
                    "completed_at": completed_at,
                }),
            )
            .await?;

        if should_send_now {
            let sign_url = format!(
                "{}/s/{}",
                state.config.public_url.trim_end_matches('/'),
                created_sub.slug
            );
            let subject = format!("You are invited to sign {}", submission.name);
            let html = format!(
                "<p>Hello {},</p><p>You have been invited to review and electronically sign <strong>{}</strong>. A copy of the document is attached for your review, or you can sign online immediately:</p><p><a href=\"{}\" style=\"display:inline-block;padding:10px 20px;background:#2161ad;color:#ffffff;text-decoration:none;border-radius:6px;\">Review &amp; Sign Document</a></p>",
                if created_sub.name.is_empty() {
                    &created_sub.email
                } else {
                    &created_sub.name
                },
                submission.name,
                sign_url
            );
            let pdf_attachments = load_submission_pdf_attachment(state, &submission).await;
            let _ = send_email_with_attachments(
                state,
                account_id,
                &created_sub.email,
                &subject,
                &html,
                "submitter_invitation",
                "Submitter",
                &created_sub.id,
                &pdf_attachments,
            )
            .await;

            let ev: SubmissionEvent = state
                .pb
                .create(
                    "submission_events",
                    &json!({
                        "account_id": account_id,
                        "submission_id": submission.id,
                        "submitter_id": created_sub.id,
                        "event_type": "send_email",
                        "event_timestamp": now_str,
                        "data": { "recipient": created_sub.email }
                    }),
                )
                .await?;
            created_events.push(ev);
        }

        let search_txt = format!(
            "{} {} {} {}",
            submission.name, created_sub.name, created_sub.email, created_sub.slug
        );
        let _ = index_search_entry(state, account_id, "Submitter", &created_sub.id, &search_txt).await;
        created_submitters.push(created_sub);
    }

    let _ = index_search_entry(
        state,
        account_id,
        "Submission",
        &submission.id,
        &submission.name,
    )
    .await;

    let api_payload =
        submission.to_api_json(&created_submitters, &created_events, &state.config.public_url);
    let _ = dispatch_webhook(
        state,
        account_id,
        "submission.created",
        "Submission",
        &submission.id,
        api_payload,
    )
    .await;

    Ok((submission, created_submitters, created_events))
}

async fn create_submission(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<CreateSubmissionReq>,
) -> AppResult<impl IntoResponse> {
    let tpl = find_template_by_id_or_slug(&state, &auth.account.id, &req.template_id).await?;
    let (submission, submitters, events) =
        create_submission_internal(&state, &auth.account.id, &auth.user.id, &tpl, req).await?;

    let payload = submission.to_api_json(&submitters, &events, &state.config.public_url);
    Ok((StatusCode::CREATED, Json(payload)))
}

#[derive(Debug, Deserialize)]
pub struct CreateSubmissionFromHtmlReq {
    pub name: Option<String>,
    pub html: String,
    pub submitters: Vec<SubmitterInput>,
    #[serde(default)]
    pub send_email: Option<bool>,
    #[serde(default)]
    pub order: Option<String>,
}

async fn create_submission_from_html(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<CreateSubmissionFromHtmlReq>,
) -> AppResult<impl IntoResponse> {
    let tpl = build_template_from_html_internal(
        &state,
        &auth,
        HtmlTemplateReq {
            name: req.name.clone(),
            html: req.html,
            html_header: None,
            folder_id: None,
            external_id: None,
        },
    )
    .await?;

    let (submission, submitters, events) = create_submission_internal(
        &state,
        &auth.account.id,
        &auth.user.id,
        &tpl,
        CreateSubmissionReq {
            template_id: tpl.id.clone(),
            submitters: req.submitters,
            send_email: req.send_email,
            order: req.order,
            submitters_order: None,
            expire_at: None,
            variables: None,
            preferences: None,
            source: Some("api".to_string()),
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(submission.to_api_json(&submitters, &events, &state.config.public_url)),
    ))
}

async fn get_submission(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let sub = find_submission_by_id_or_slug(&state, &auth.account.id, &id).await?;
    let filter = format!("submission_id = '{}'", pb_escape(&sub.id));
    let submitters = state
        .pb
        .list_all::<Submitter>("submitters", Some(&filter), Some("created"))
        .await?;
    let events = state
        .pb
        .list_all::<SubmissionEvent>("submission_events", Some(&filter), Some("created"))
        .await?;

    Ok(Json(sub.to_api_json(
        &submitters,
        &events,
        &state.config.public_url,
    )))
}

async fn archive_submission(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let sub = find_submission_by_id_or_slug(&state, &auth.account.id, &id).await?;
    let now_str = chrono::Utc::now().to_rfc3339();
    let updated: Submission = state
        .pb
        .update(
            "submissions",
            &sub.id,
            &json!({ "archived_at": now_str }),
        )
        .await?;

    let filter = format!("submission_id = '{}'", pb_escape(&updated.id));
    let submitters = state
        .pb
        .list_all::<Submitter>("submitters", Some(&filter), Some("created"))
        .await?;
    let events = state
        .pb
        .list_all::<SubmissionEvent>("submission_events", Some(&filter), Some("created"))
        .await?;

    let payload = updated.to_api_json(&submitters, &events, &state.config.public_url);
    let _ = dispatch_webhook(
        &state,
        &auth.account.id,
        "submission.archived",
        "Submission",
        &updated.id,
        payload,
    )
    .await;

    Ok(Json(json!({
        "id": updated.id,
        "archived_at": updated.archived_at,
    })))
}

async fn unarchive_submission(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let sub = find_submission_by_id_or_slug(&state, &auth.account.id, &id).await?;
    let updated: Submission = state
        .pb
        .update("submissions", &sub.id, &json!({ "archived_at": "" }))
        .await?;
    Ok(Json(json!({
        "id": updated.id,
        "archived_at": null,
    })))
}

#[derive(Debug, Deserialize, Default)]
pub struct ResendEmailReq {
    pub submitter_id: Option<String>,
}

async fn resend_submission_email(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
    body: Option<Json<ResendEmailReq>>,
) -> AppResult<Json<Value>> {
    let sub = find_submission_by_id_or_slug(&state, &auth.account.id, &id).await?;
    let filter = format!("submission_id = '{}'", pb_escape(&sub.id));
    let submitters = state
        .pb
        .list_all::<Submitter>("submitters", Some(&filter), Some("created"))
        .await?;

    let target_sub_id = body.and_then(|b| b.0.submitter_id);
    let now_str = chrono::Utc::now().to_rfc3339();
    let mut sent_to = Vec::new();
    let pdf_attachments = load_submission_pdf_attachment(&state, &sub).await;

    for s in submitters {
        if let Some(ref tid) = target_sub_id {
            if &s.id != tid {
                continue;
            }
        }
        if !s.completed_at.is_empty() || s.email.is_empty() {
            continue;
        }
        let sign_url = format!(
            "{}/s/{}",
            state.config.public_url.trim_end_matches('/'),
            s.slug
        );
        let _ = send_email_with_attachments(
            &state,
            &auth.account.id,
            &s.email,
            &format!("Reminder: Please sign {}", sub.name),
            &format!(
                "<p>Please review and sign <strong>{}</strong>:</p><p><a href=\"{}\">{}</a></p>",
                sub.name, sign_url, sign_url
            ),
            "submitter_invitation_resend",
            "Submitter",
            &s.id,
            &pdf_attachments,
        )
        .await;

        let _: Submitter = state
            .pb
            .update("submitters", &s.id, &json!({ "sent_at": now_str }))
            .await?;

        let _: SubmissionEvent = state
            .pb
            .create(
                "submission_events",
                &json!({
                    "account_id": auth.account.id,
                    "submission_id": sub.id,
                    "submitter_id": s.id,
                    "event_type": "send_email",
                    "event_timestamp": now_str,
                    "data": { "resent": true, "recipient": s.email }
                }),
            )
            .await?;
        sent_to.push(s.email);
    }

    Ok(Json(json!({
        "ok": true,
        "sent_to": sent_to,
    })))
}

/// Resolves the PDF document attachment for a submission (from its stored template document blob,
/// or generates a clean PDF document representation if none is stored yet).
pub async fn load_submission_pdf_attachment(
    state: &AppState,
    sub: &Submission,
) -> Vec<EmailAttachment> {
    let safe_name = sub
        .name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>();
    let filename = format!("{}.pdf", safe_name.trim_matches('-'));

    if let Some(docs) = sub.documents.as_array() {
        for doc in docs {
            if let Some(blob_id) = doc
                .get("uuid")
                .or_else(|| doc.get("blob_id"))
                .or_else(|| doc.get("id"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                if let Ok((_, bytes)) = read_blob_bytes(state, blob_id).await {
                    if bytes.starts_with(b"%PDF") {
                        return vec![EmailAttachment {
                            filename,
                            content_type: "application/pdf".to_string(),
                            data: bytes,
                        }];
                    }
                }
            }
        }
    }

    let fallback_bytes = build_simple_pdf(
        &sub.name,
        &[
            format!("Document for Signature: {}", sub.name),
            format!("Submission Slug: {}", sub.slug),
        ],
    );
    vec![EmailAttachment {
        filename,
        content_type: "application/pdf".to_string(),
        data: fallback_bytes,
    }]
}

#[derive(Debug, Deserialize, Default)]
pub struct DownloadQuery {
    pub audit: Option<bool>,
}

async fn download_submission(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<DownloadQuery>,
) -> AppResult<impl IntoResponse> {
    let mut sub: Submission = if let Ok(s) = state.pb.get_by_id("submissions", &id).await {
        s
    } else {
        let filter = format!("slug = '{}'", pb_escape(&id));
        state
            .pb
            .find_first::<Submission>("submissions", &filter, None)
            .await?
            .ok_or_else(|| AppError::not_found("Submission not found"))?
    };

    if sub.combined_document_file.is_empty() || sub.audit_trail_file.is_empty() {
        let filter = format!("submission_id = '{}'", pb_escape(&sub.id));
        let submitters = state
            .pb
            .list_all::<Submitter>("submitters", Some(&filter), Some("created"))
            .await?;
        let events = state
            .pb
            .list_all::<SubmissionEvent>("submission_events", Some(&filter), Some("created"))
            .await?;
        let (combined_uuid, audit_uuid, _) =
            finalize_submission_documents(&state, &sub, &submitters, &events).await?;
        sub = state
            .pb
            .update(
                "submissions",
                &sub.id,
                &json!({
                    "combined_document_file": combined_uuid,
                    "audit_trail_file": audit_uuid,
                }),
            )
            .await?;
    }

    let blob_id = if query.audit.unwrap_or(false) {
        &sub.audit_trail_file
    } else {
        &sub.combined_document_file
    };

    let (blob, bytes) = read_blob_bytes(&state, blob_id).await?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/pdf".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", blob.filename)
            .parse()
            .unwrap(),
    );

    Ok((StatusCode::OK, headers, bytes))
}

async fn export_submissions_csv(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<impl IntoResponse> {
    let filter = format!(
        "account_id = '{}' && archived_at = ''",
        pb_escape(&auth.account.id)
    );
    let submissions = state
        .pb
        .list_all::<Submission>("submissions", Some(&filter), Some("-created"))
        .await?;

    let mut wtr = csv::Writer::from_writer( vec![]);
    let _ = wtr.write_record([
        "submission_id",
        "submission_name",
        "status",
        "submitter_name",
        "submitter_email",
        "submitter_role",
        "completed_at",
        "created_at",
    ]);

    for sub in submissions {
        let s_filter = format!("submission_id = '{}'", pb_escape(&sub.id));
        let submitters = state
            .pb
            .list_all::<Submitter>("submitters", Some(&s_filter), Some("created"))
            .await?;
        let status = sub.status(&submitters);
        for s in submitters {
            let _ = wtr.write_record([
                &sub.id,
                &sub.name,
                status,
                &s.name,
                &s.email,
                &s.role(),
                &s.completed_at,
                &sub.created,
            ]);
        }
    }

    let csv_bytes = wtr.into_inner().unwrap_or_default();
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/csv; charset=utf-8".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"submissions-export.csv\""
            .parse()
            .unwrap(),
    );
    Ok((StatusCode::OK, headers, csv_bytes))
}

#[derive(Debug, Deserialize, Default)]
pub struct SubmitterListQuery {
    pub submission_id: Option<String>,
    pub q: Option<String>,
    pub slug: Option<String>,
    pub limit: Option<usize>,
    pub page: Option<usize>,
}

async fn list_submitters(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(query): Query<SubmitterListQuery>,
) -> AppResult<Json<Value>> {
    let mut clauses = vec![format!("account_id = '{}'", pb_escape(&auth.account.id))];
    if let Some(ref sid) = query.submission_id {
        if !sid.is_empty() {
            clauses.push(format!("submission_id = '{}'", pb_escape(sid)));
        }
    }
    if let Some(ref slug) = query.slug {
        if !slug.is_empty() {
            clauses.push(format!("slug = '{}'", pb_escape(slug)));
        }
    }
    if let Some(ref q) = query.q {
        let qt = q.trim();
        if !qt.is_empty() {
            clauses.push(format!(
                "(name ~ '{}' || email ~ '{}')",
                pb_escape(qt),
                pb_escape(qt)
            ));
        }
    }

    let filter = clauses.join(" && ");
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.limit.unwrap_or(50).clamp(1, 200);

    let res = state
        .pb
        .list::<Submitter>("submitters", page, per_page, Some(&filter), Some("-created"))
        .await?;

    let items: Vec<Value> = res
        .items
        .into_iter()
        .map(|s| s.to_api_json(&state.config.public_url))
        .collect();

    Ok(Json(json!({
        "data": items,
        "submitters": items,
        "pagination": {
            "count": res.total_items,
            "page": res.page,
            "per_page": res.per_page,
            "pages": res.total_pages,
        }
    })))
}

async fn get_submitter(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let sub: Submitter = state.pb.get_by_id("submitters", &id).await?;
    if sub.account_id != auth.account.id {
        return Err(AppError::not_found("Submitter not found"));
    }
    Ok(Json(sub.to_api_json(&state.config.public_url)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubmitterReq {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub values: Option<Value>,
    pub metadata: Option<Value>,
    pub external_id: Option<String>,
    pub completed: Option<bool>,
}

async fn update_submitter(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
    Json(req): Json<UpdateSubmitterReq>,
) -> AppResult<Json<Value>> {
    let existing: Submitter = state.pb.get_by_id("submitters", &id).await?;
    if existing.account_id != auth.account.id {
        return Err(AppError::not_found("Submitter not found"));
    }

    let mut patch = json!({});
    if let Some(obj) = patch.as_object_mut() {
        if let Some(n) = req.name {
            obj.insert("name".to_string(), json!(n.trim()));
        }
        if let Some(e) = req.email {
            obj.insert("email".to_string(), json!(e.trim().to_lowercase()));
        }
        if let Some(p) = req.phone {
            obj.insert("phone".to_string(), json!(p));
        }
        if let Some(v) = req.values {
            obj.insert("values".to_string(), v);
        }
        if let Some(m) = req.metadata {
            obj.insert("metadata".to_string(), m);
        }
        if let Some(ext) = req.external_id {
            obj.insert("external_id".to_string(), json!(ext));
        }
        if req.completed == Some(true) && existing.completed_at.is_empty() {
            obj.insert(
                "completed_at".to_string(),
                json!(chrono::Utc::now().to_rfc3339()),
            );
        }
    }

    let updated: Submitter = state.pb.update("submitters", &existing.id, &patch).await?;
    Ok(Json(updated.to_api_json(&state.config.public_url)))
}

pub async fn find_submission_by_id_or_slug(
    state: &AppState,
    account_id: &str,
    id_or_slug: &str,
) -> AppResult<Submission> {
    if let Ok(s) = state.pb.get_by_id::<Submission>("submissions", id_or_slug).await {
        if s.account_id == account_id {
            return Ok(s);
        }
    }
    let filter = format!(
        "account_id = '{}' && slug = '{}'",
        pb_escape(account_id),
        pb_escape(id_or_slug)
    );
    state
        .pb
        .find_first::<Submission>("submissions", &filter, None)
        .await?
        .ok_or_else(|| AppError::not_found(format!("Submission '{}' not found", id_or_slug)))
}
