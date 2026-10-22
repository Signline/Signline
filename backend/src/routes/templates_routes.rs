use crate::auth::{rbac::require_template_write, AuthContext};
use crate::error::{AppError, AppResult};
use crate::models::template::{DynamicDocument, Template, TemplateFolder, TemplateVersion};
use crate::pb::pb_escape;
use crate::pdf::{
    html_to_pdf::render_html_to_pdf_and_fields,
    processor::{auto_detect_fields_for_template, process_uploaded_document},
};
use crate::services::{
    search::index_search_entry,
    storage::{decode_data_uri_or_base64, read_blob_bytes},
    webhooks::dispatch_webhook,
};
use crate::state::AppState;
use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha1::{Digest as _, Sha1};
use std::collections::HashMap;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/folders", get(list_folders).post(create_folder))
        .route(
            "/api/folders/:id",
            put(update_folder).delete(delete_folder),
        )
        .route("/api/templates", get(list_templates).post(create_template))
        .route("/api/templates/upload", post(upload_template_multipart))
        .route("/api/templates/pdf", post(create_template_from_pdf_json))
        .route("/api/templates/docx", post(create_template_from_pdf_json))
        .route("/api/templates/html", post(create_template_from_html))
        .route(
            "/api/templates/:id",
            get(get_template)
                .put(update_template)
                .delete(archive_template),
        )
        .route("/api/templates/:id/clone", post(clone_template))
        .route("/api/templates/:id/restore", post(restore_template))
        .route("/api/templates/:id/detect_fields", post(detect_fields))
        .route("/api/templates/:id/versions", get(list_template_versions))
}

fn generate_slug() -> String {
    let raw = Uuid::new_v4().simple().to_string();
    raw[..14].to_string()
}

async fn list_folders(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let filter = format!(
        "account_id = '{}' && archived_at = ''",
        pb_escape(&auth.account.id)
    );
    let folders = state
        .pb
        .list_all::<TemplateFolder>("template_folders", Some(&filter), Some("name"))
        .await?;

    Ok(Json(json!({
        "data": folders,
        "folders": folders,
    })))
}

#[derive(Debug, Deserialize)]
pub struct FolderPayload {
    pub name: String,
    #[serde(default)]
    pub parent_folder_id: Option<String>,
}

async fn create_folder(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<FolderPayload>,
) -> AppResult<impl IntoResponse> {
    require_template_write(&auth.user)?;
    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("Folder name cannot be empty"));
    }

    let folder: TemplateFolder = state
        .pb
        .create(
            "template_folders",
            &json!({
                "account_id": auth.account.id,
                "author_id": auth.user.id,
                "name": name,
                "parent_folder_id": req.parent_folder_id.unwrap_or_default(),
            }),
        )
        .await?;

    Ok((StatusCode::CREATED, Json(json!(folder))))
}

async fn update_folder(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
    Json(req): Json<FolderPayload>,
) -> AppResult<Json<Value>> {
    require_template_write(&auth.user)?;
    let folder: TemplateFolder = state
        .pb
        .update(
            "template_folders",
            &id,
            &json!({
                "name": req.name.trim(),
                "parent_folder_id": req.parent_folder_id.unwrap_or_default(),
            }),
        )
        .await?;
    Ok(Json(json!(folder)))
}

async fn delete_folder(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_template_write(&auth.user)?;
    let now_str = chrono::Utc::now().to_rfc3339();
    let _: TemplateFolder = state
        .pb
        .update("template_folders", &id, &json!({ "archived_at": now_str }))
        .await?;
    Ok(Json(json!({ "id": id, "archived": true })))
}

#[derive(Debug, Deserialize, Default)]
pub struct TemplateListQuery {
    pub q: Option<String>,
    pub folder: Option<String>,
    pub folder_id: Option<String>,
    pub archived: Option<bool>,
    pub limit: Option<usize>,
    pub page: Option<usize>,
    pub external_id: Option<String>,
}

async fn list_templates(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(query): Query<TemplateListQuery>,
) -> AppResult<Json<Value>> {
    let mut clauses = vec![format!("account_id = '{}'", pb_escape(&auth.account.id))];

    if query.archived.unwrap_or(false) {
        clauses.push("archived_at != ''".to_string());
    } else {
        clauses.push("archived_at = ''".to_string());
    }

    if let Some(ref fid) = query.folder_id {
        if !fid.is_empty() {
            clauses.push(format!("folder_id = '{}'", pb_escape(fid)));
        }
    }

    if let Some(ref ext) = query.external_id {
        if !ext.is_empty() {
            clauses.push(format!("external_id = '{}'", pb_escape(ext)));
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

    let folder_list = state
        .pb
        .list_all::<TemplateFolder>(
            "template_folders",
            Some(&format!("account_id = '{}'", pb_escape(&auth.account.id))),
            None,
        )
        .await?;

    let mut folder_map = HashMap::new();
    for f in folder_list {
        if let Some(ref folder_name_filter) = query.folder {
            if f.name.eq_ignore_ascii_case(folder_name_filter) && query.folder_id.is_none() {
                // also support filtering by folder name
            }
        }
        folder_map.insert(f.id.clone(), f.name);
    }

    let pb_res = state
        .pb
        .list::<Template>("templates", page, per_page, Some(&filter), Some("-created"))
        .await?;

    let items: Vec<Value> = pb_res
        .items
        .into_iter()
        .filter(|t| {
            if let Some(ref fn_filter) = query.folder {
                if !fn_filter.is_empty() {
                    let fname = folder_map
                        .get(&t.folder_id)
                        .map(|s| s.as_str())
                        .unwrap_or("Default");
                    return fname.eq_ignore_ascii_case(fn_filter);
                }
            }
            true
        })
        .map(|t| {
            let fname = folder_map
                .get(&t.folder_id)
                .map(|s| s.as_str())
                .unwrap_or("Default");
            t.to_api_json(Some(fname))
        })
        .collect();

    Ok(Json(json!({
        "data": items,
        "templates": items,
        "pagination": {
            "count": pb_res.total_items,
            "page": pb_res.page,
            "per_page": pb_res.per_page,
            "pages": pb_res.total_pages,
        }
    })))
}

async fn resolve_default_folder_id(state: &AppState, account_id: &str, user_id: &str) -> AppResult<String> {
    let filter = format!(
        "account_id = '{}' && name = 'Default' && archived_at = ''",
        pb_escape(account_id)
    );
    if let Some(f) = state
        .pb
        .find_first::<TemplateFolder>("template_folders", &filter, None)
        .await?
    {
        return Ok(f.id);
    }

    let created: TemplateFolder = state
        .pb
        .create(
            "template_folders",
            &json!({
                "account_id": account_id,
                "author_id": user_id,
                "name": "Default",
                "parent_folder_id": "",
            }),
        )
        .await?;
    Ok(created.id)
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateReq {
    pub name: Option<String>,
    pub folder_id: Option<String>,
    pub folder_name: Option<String>,
    pub external_id: Option<String>,
    pub shared_link: Option<bool>,
    pub schema: Option<Value>,
    pub fields: Option<Value>,
    pub submitters: Option<Value>,
    pub preferences: Option<Value>,
    pub documents: Option<Value>,
}

async fn create_template(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<CreateTemplateReq>,
) -> AppResult<impl IntoResponse> {
    require_template_write(&auth.user)?;

    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Untitled Document")
        .to_string();

    let folder_id = match req.folder_id {
        Some(ref id) if !id.is_empty() => id.clone(),
        _ => resolve_default_folder_id(&state, &auth.account.id, &auth.user.id).await?,
    };

    let default_sub_uuid = format!("sub-{}", Uuid::new_v4().simple());
    let submitters = req.submitters.unwrap_or_else(|| {
        json!([
            {
                "uuid": default_sub_uuid,
                "name": "First Party",
            }
        ])
    });

    let mut documents = req.documents.unwrap_or_else(|| json!([]));
    let mut schema = req.schema.unwrap_or_else(|| json!([]));
    let mut fields = req.fields.unwrap_or_else(|| json!([]));

    // If no documents were uploaded yet, generate a starter document PDF so the builder is immediately usable
    if documents.as_array().map(|a| a.is_empty()).unwrap_or(true) {
        let starter_pdf = crate::pdf::generator::build_simple_pdf(
            &name,
            &[
                format!("Template: {}", name),
                "Please review the terms and provide your signature below.".to_string(),
            ],
        );
        let processed = process_uploaded_document(
            &state,
            &format!("{}.pdf", name),
            "application/pdf",
            &starter_pdf,
        )
        .await?;

        documents = json!([{
            "id": processed.doc_uuid,
            "uuid": processed.doc_uuid,
            "blob_uuid": processed.blob_uuid,
            "name": processed.name,
            "url": format!("/api/blobs/{}", processed.blob_uuid),
            "preview_images": processed.preview_images,
        }]);
        schema = json!([processed.schema_entry]);
        if fields.as_array().map(|a| a.is_empty()).unwrap_or(true) {
            fields = json!([{
                "uuid": Uuid::new_v4().to_string(),
                "submitter_uuid": submitters[0]["uuid"].as_str().unwrap_or("submitter-1"),
                "name": "Signature",
                "type": "signature",
                "required": true,
                "areas": [{
                    "attachment_uuid": processed.doc_uuid,
                    "page": 0,
                    "x": 0.12,
                    "y": 0.70,
                    "w": 0.38,
                    "h": 0.07
                }]
            }]);
        }
    }

    let tpl: Template = state
        .pb
        .create(
            "templates",
            &json!({
                "account_id": auth.account.id,
                "author_id": auth.user.id,
                "folder_id": folder_id,
                "slug": generate_slug(),
                "name": name,
                "source": "native",
                "external_id": req.external_id.unwrap_or_default(),
                "shared_link": req.shared_link.unwrap_or(false),
                "schema": schema,
                "fields": fields,
                "submitters": submitters,
                "preferences": req.preferences.unwrap_or_else(|| json!({})),
                "variables_schema": json!({}),
                "documents": documents,
            }),
        )
        .await?;

    save_template_version(&state, &tpl, &auth.user.id).await;
    let _ = index_search_entry(&state, &auth.account.id, "Template", &tpl.id, &tpl.name).await;
    let _ = dispatch_webhook(
        &state,
        &auth.account.id,
        "template.created",
        "Template",
        &tpl.id,
        tpl.to_api_json(None),
    )
    .await;

    Ok((StatusCode::CREATED, Json(tpl.to_api_json(None))))
}

/// Handles `POST /api/templates/upload` (multipart form upload of PDF / PNG / JPG / HTML files).
async fn upload_template_multipart(
    State(state): State<AppState>,
    auth: AuthContext,
    mut multipart: Multipart,
) -> AppResult<impl IntoResponse> {
    require_template_write(&auth.user)?;

    let mut template_name = String::new();
    let mut folder_id = String::new();
    let mut uploaded_files: Vec<(String, String, Vec<u8>)> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("Multipart error: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "name" {
            template_name = field.text().await.unwrap_or_default();
        } else if field_name == "folder_id" {
            folder_id = field.text().await.unwrap_or_default();
        } else if let Some(fname) = field.file_name().map(|s| s.to_string()) {
            let ctype = field
                .content_type()
                .unwrap_or("application/pdf")
                .to_string();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::bad_request(format!("Failed reading upload bytes: {}", e)))?
                .to_vec();
            if !bytes.is_empty() {
                uploaded_files.push((fname, ctype, bytes));
            }
        }
    }

    if uploaded_files.is_empty() {
        return Err(AppError::bad_request(
            "At least one document file must be provided in the multipart upload",
        ));
    }

    if folder_id.is_empty() {
        folder_id = resolve_default_folder_id(&state, &auth.account.id, &auth.user.id).await?;
    }

    if template_name.trim().is_empty() {
        template_name = uploaded_files[0]
            .0
            .trim_end_matches(".pdf")
            .trim_end_matches(".PDF")
            .to_string();
    }

    let mut documents = Vec::new();
    let mut schema = Vec::new();
    let mut fields = Vec::new();
    let mut role_names = vec!["First Party".to_string()];

    for (fname, ctype, bytes) in uploaded_files {
        let proc = process_uploaded_document(&state, &fname, &ctype, &bytes).await?;
        for r in &proc.extracted_roles {
            if !role_names.contains(r) {
                role_names.push(r.clone());
            }
        }
        schema.push(proc.schema_entry);
        fields.extend(proc.extracted_fields);
        documents.push(json!({
            "id": proc.doc_uuid,
            "uuid": proc.doc_uuid,
            "blob_uuid": proc.blob_uuid,
            "name": proc.name,
            "url": format!("/api/blobs/{}", proc.blob_uuid),
            "preview_images": proc.preview_images,
        }));
    }

    let submitters: Vec<Value> = role_names
        .iter()
        .map(|r| {
            json!({
                "uuid": format!("role-{}", r.to_lowercase().replace(' ', "-")),
                "name": r,
            })
        })
        .collect();

    let tpl: Template = state
        .pb
        .create(
            "templates",
            &json!({
                "account_id": auth.account.id,
                "author_id": auth.user.id,
                "folder_id": folder_id,
                "slug": generate_slug(),
                "name": template_name,
                "source": "native",
                "shared_link": false,
                "schema": schema,
                "fields": fields,
                "submitters": submitters,
                "preferences": json!({}),
                "variables_schema": json!({}),
                "documents": documents,
            }),
        )
        .await?;

    save_template_version(&state, &tpl, &auth.user.id).await;
    let _ = index_search_entry(&state, &auth.account.id, "Template", &tpl.id, &tpl.name).await;
    let _ = dispatch_webhook(
        &state,
        &auth.account.id,
        "template.created",
        "Template",
        &tpl.id,
        tpl.to_api_json(None),
    )
    .await;

    Ok((StatusCode::CREATED, Json(tpl.to_api_json(None))))
}

/// Handles REST API `POST /api/templates/pdf` (accepting base64 documents in JSON).
#[derive(Debug, Deserialize)]
pub struct ApiPdfTemplateReq {
    pub name: Option<String>,
    pub folder_name: Option<String>,
    pub external_id: Option<String>,
    pub documents: Option<Vec<ApiPdfDocInput>>,
    pub fields: Option<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
pub struct ApiPdfDocInput {
    pub name: Option<String>,
    pub file: String,
    pub fields: Option<Vec<Value>>,
}

async fn create_template_from_pdf_json(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<ApiPdfTemplateReq>,
) -> AppResult<impl IntoResponse> {
    require_template_write(&auth.user)?;
    let folder_id = resolve_default_folder_id(&state, &auth.account.id, &auth.user.id).await?;
    let name = req
        .name
        .unwrap_or_else(|| "API Uploaded Template".to_string());

    let mut documents = Vec::new();
    let mut schema = Vec::new();
    let mut fields = req.fields.unwrap_or_default();
    let mut role_names = vec!["First Party".to_string()];

    if let Some(doc_inputs) = req.documents {
        for d in doc_inputs {
            let doc_name = d.name.unwrap_or_else(|| format!("{}.pdf", name));
            let (mime, bytes) = decode_data_uri_or_base64(&d.file, "application/pdf")?;
            let proc = process_uploaded_document(&state, &doc_name, &mime, &bytes).await?;
            for r in &proc.extracted_roles {
                if !role_names.contains(r) {
                    role_names.push(r.clone());
                }
            }
            schema.push(proc.schema_entry);
            fields.extend(proc.extracted_fields);
            if let Some(extra_fields) = d.fields {
                fields.extend(extra_fields);
            }
            documents.push(json!({
                "id": proc.doc_uuid,
                "uuid": proc.doc_uuid,
                "blob_uuid": proc.blob_uuid,
                "name": proc.name,
                "url": format!("/api/blobs/{}", proc.blob_uuid),
                "preview_images": proc.preview_images,
            }));
        }
    }

    let submitters: Vec<Value> = role_names
        .iter()
        .map(|r| {
            json!({
                "uuid": format!("role-{}", r.to_lowercase().replace(' ', "-")),
                "name": r,
            })
        })
        .collect();

    let tpl: Template = state
        .pb
        .create(
            "templates",
            &json!({
                "account_id": auth.account.id,
                "author_id": auth.user.id,
                "folder_id": folder_id,
                "slug": generate_slug(),
                "name": name,
                "source": "api",
                "external_id": req.external_id.unwrap_or_default(),
                "shared_link": false,
                "schema": schema,
                "fields": fields,
                "submitters": submitters,
                "preferences": json!({}),
                "variables_schema": json!({}),
                "documents": documents,
            }),
        )
        .await?;

    save_template_version(&state, &tpl, &auth.user.id).await;
    let _ = index_search_entry(&state, &auth.account.id, "Template", &tpl.id, &tpl.name).await;
    let _ = dispatch_webhook(
        &state,
        &auth.account.id,
        "template.created",
        "Template",
        &tpl.id,
        tpl.to_api_json(None),
    )
    .await;

    Ok((StatusCode::CREATED, Json(tpl.to_api_json(None))))
}

#[derive(Debug, Deserialize)]
pub struct HtmlTemplateReq {
    pub name: Option<String>,
    pub html: String,
    #[serde(default)]
    pub html_header: Option<String>,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
}

pub async fn build_template_from_html_internal(
    state: &AppState,
    auth: &AuthContext,
    req: HtmlTemplateReq,
) -> AppResult<Template> {
    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Dynamic HTML Template")
        .to_string();

    let folder_id = match req.folder_id {
        Some(ref f) if !f.is_empty() => f.clone(),
        _ => resolve_default_folder_id(state, &auth.account.id, &auth.user.id).await?,
    };

    let temp_doc_uuid = Uuid::new_v4().to_string();
    let default_sub_uuid = "role-first-party";
    let (pdf_bytes, extracted_fields) =
        render_html_to_pdf_and_fields(&name, &req.html, &temp_doc_uuid, default_sub_uuid);

    let proc = process_uploaded_document(
        state,
        &format!("{}.pdf", name),
        "application/pdf",
        &pdf_bytes,
    )
    .await?;

    // Re-bind extracted field attachment_uuid to proc.doc_uuid
    let fields: Vec<Value> = extracted_fields
        .into_iter()
        .map(|mut f| {
            if let Some(areas) = f.get_mut("areas").and_then(|a| a.as_array_mut()) {
                for area in areas {
                    if let Some(obj) = area.as_object_mut() {
                        obj.insert("attachment_uuid".to_string(), json!(proc.doc_uuid));
                    }
                }
            }
            f
        })
        .collect();

    let submitters = json!([
        {
            "uuid": default_sub_uuid,
            "name": "First Party"
        }
    ]);

    let documents = json!([{
        "id": proc.doc_uuid,
        "uuid": proc.doc_uuid,
        "blob_uuid": proc.blob_uuid,
        "name": proc.name,
        "url": format!("/api/blobs/{}", proc.blob_uuid),
        "preview_images": proc.preview_images,
    }]);

    let tpl: Template = state
        .pb
        .create(
            "templates",
            &json!({
                "account_id": auth.account.id,
                "author_id": auth.user.id,
                "folder_id": folder_id,
                "slug": generate_slug(),
                "name": name,
                "source": "native",
                "external_id": req.external_id.unwrap_or_default(),
                "shared_link": false,
                "schema": [proc.schema_entry],
                "fields": fields,
                "submitters": submitters,
                "preferences": json!({}),
                "variables_schema": json!({}),
                "documents": documents,
            }),
        )
        .await?;

    let mut sha = Sha1::new();
    sha.update(req.html.as_bytes());
    let sha1_hex = hex::encode(sha.finalize());

    let dyn_doc: DynamicDocument = state
        .pb
        .create(
            "dynamic_documents",
            &json!({
                "template_id": tpl.id,
                "uuid": proc.doc_uuid,
                "sha1": sha1_hex,
                "head": req.html_header.unwrap_or_default(),
                "body": req.html,
            }),
        )
        .await?;

    let _: Value = state
        .pb
        .create(
            "dynamic_document_versions",
            &json!({
                "dynamic_document_id": dyn_doc.id,
                "sha1": sha1_hex,
                "areas": tpl.fields,
                "document_file": proc.blob_uuid,
            }),
        )
        .await?;

    save_template_version(state, &tpl, &auth.user.id).await;
    let _ = index_search_entry(state, &auth.account.id, "Template", &tpl.id, &tpl.name).await;
    let _ = dispatch_webhook(
        state,
        &auth.account.id,
        "template.created",
        "Template",
        &tpl.id,
        tpl.to_api_json(None),
    )
    .await;

    Ok(tpl)
}

async fn create_template_from_html(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<HtmlTemplateReq>,
) -> AppResult<impl IntoResponse> {
    require_template_write(&auth.user)?;
    let tpl = build_template_from_html_internal(&state, &auth, req).await?;
    Ok((StatusCode::CREATED, Json(tpl.to_api_json(None))))
}

async fn get_template(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let tpl = find_template_by_id_or_slug(&state, &auth.account.id, &id).await?;
    Ok(Json(tpl.to_api_json(None)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateTemplateReq {
    pub name: Option<String>,
    pub folder_id: Option<String>,
    pub external_id: Option<String>,
    pub shared_link: Option<bool>,
    pub schema: Option<Value>,
    pub fields: Option<Value>,
    pub submitters: Option<Value>,
    pub preferences: Option<Value>,
    pub documents: Option<Value>,
    pub archived: Option<bool>,
}

async fn update_template(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
    Json(req): Json<UpdateTemplateReq>,
) -> AppResult<Json<Value>> {
    require_template_write(&auth.user)?;
    let existing = find_template_by_id_or_slug(&state, &auth.account.id, &id).await?;

    let mut patch = json!({});
    if let Some(obj) = patch.as_object_mut() {
        if let Some(name) = req.name {
            obj.insert("name".to_string(), json!(name.trim()));
        }
        if let Some(fid) = req.folder_id {
            obj.insert("folder_id".to_string(), json!(fid));
        }
        if let Some(ext) = req.external_id {
            obj.insert("external_id".to_string(), json!(ext));
        }
        if let Some(sl) = req.shared_link {
            obj.insert("shared_link".to_string(), json!(sl));
        }
        if let Some(sc) = req.schema {
            obj.insert("schema".to_string(), sc);
        }
        if let Some(f) = req.fields {
            obj.insert("fields".to_string(), f);
        }
        if let Some(sub) = req.submitters {
            obj.insert("submitters".to_string(), sub);
        }
        if let Some(pref) = req.preferences {
            obj.insert("preferences".to_string(), pref);
        }
        if let Some(docs) = req.documents {
            obj.insert("documents".to_string(), docs);
        }
        if let Some(arch) = req.archived {
            obj.insert(
                "archived_at".to_string(),
                json!(if arch {
                    chrono::Utc::now().to_rfc3339()
                } else {
                    String::new()
                }),
            );
        }
    }

    let updated: Template = state.pb.update("templates", &existing.id, &patch).await?;
    save_template_version(&state, &updated, &auth.user.id).await;
    let _ = index_search_entry(
        &state,
        &auth.account.id,
        "Template",
        &updated.id,
        &updated.name,
    )
    .await;
    let _ = dispatch_webhook(
        &state,
        &auth.account.id,
        "template.updated",
        "Template",
        &updated.id,
        updated.to_api_json(None),
    )
    .await;

    Ok(Json(updated.to_api_json(None)))
}

async fn archive_template(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_template_write(&auth.user)?;
    let existing = find_template_by_id_or_slug(&state, &auth.account.id, &id).await?;
    let now_str = chrono::Utc::now().to_rfc3339();
    let updated: Template = state
        .pb
        .update(
            "templates",
            &existing.id,
            &json!({ "archived_at": now_str }),
        )
        .await?;
    Ok(Json(json!({
        "id": updated.id,
        "archived_at": updated.archived_at,
    })))
}

async fn restore_template(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_template_write(&auth.user)?;
    let existing = find_template_by_id_or_slug(&state, &auth.account.id, &id).await?;
    let updated: Template = state
        .pb
        .update("templates", &existing.id, &json!({ "archived_at": "" }))
        .await?;
    Ok(Json(updated.to_api_json(None)))
}

#[derive(Debug, Deserialize, Default)]
pub struct CloneTemplateReq {
    pub name: Option<String>,
    pub folder_id: Option<String>,
    pub external_id: Option<String>,
}

async fn clone_template(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
    body: Option<Json<CloneTemplateReq>>,
) -> AppResult<impl IntoResponse> {
    require_template_write(&auth.user)?;
    let source = find_template_by_id_or_slug(&state, &auth.account.id, &id).await?;
    let req = body.map(|b| b.0).unwrap_or_default();

    let cloned_name = req
        .name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{} (Copy)", source.name));

    let cloned: Template = state
        .pb
        .create(
            "templates",
            &json!({
                "account_id": auth.account.id,
                "author_id": auth.user.id,
                "folder_id": req.folder_id.unwrap_or(source.folder_id),
                "slug": generate_slug(),
                "name": cloned_name,
                "source": "native",
                "external_id": req.external_id.unwrap_or_default(),
                "shared_link": false,
                "schema": source.schema,
                "fields": source.fields,
                "submitters": source.submitters,
                "preferences": source.preferences,
                "variables_schema": source.variables_schema,
                "documents": source.documents,
            }),
        )
        .await?;

    save_template_version(&state, &cloned, &auth.user.id).await;
    let _ = index_search_entry(&state, &auth.account.id, "Template", &cloned.id, &cloned.name).await;

    Ok((StatusCode::CREATED, Json(cloned.to_api_json(None))))
}

async fn detect_fields(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_template_write(&auth.user)?;
    let tpl = find_template_by_id_or_slug(&state, &auth.account.id, &id).await?;

    let default_sub_uuid = tpl
        .submitters
        .as_array()
        .and_then(|a| a.first())
        .and_then(|s| s.get("uuid"))
        .and_then(|u| u.as_str())
        .unwrap_or("submitter-1")
        .to_string();

    let mut attachment_uuid = String::from("doc-1");
    let mut pdf_bytes = Vec::new();

    if let Some(docs) = tpl.documents.as_array() {
        if let Some(first) = docs.first() {
            attachment_uuid = first
                .get("uuid")
                .or_else(|| first.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("doc-1")
                .to_string();
            if let Some(blob_id) = first
                .get("blob_uuid")
                .or_else(|| first.get("uuid"))
                .and_then(|v| v.as_str())
            {
                if let Ok((_, b)) = read_blob_bytes(&state, blob_id).await {
                    pdf_bytes = b;
                }
            }
        }
    }

    let detected = auto_detect_fields_for_template(&pdf_bytes, &attachment_uuid, &default_sub_uuid);
    let mut combined_fields = tpl.fields.as_array().cloned().unwrap_or_default();
    combined_fields.extend(detected.clone());

    let updated: Template = state
        .pb
        .update(
            "templates",
            &tpl.id,
            &json!({
                "fields": combined_fields,
            }),
        )
        .await?;

    Ok(Json(json!({
        "fields": combined_fields,
        "detected_fields": detected,
        "template": updated.to_api_json(None),
    })))
}

async fn list_template_versions(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let tpl = find_template_by_id_or_slug(&state, &auth.account.id, &id).await?;
    let filter = format!("template_id = '{}'", pb_escape(&tpl.id));
    let versions = state
        .pb
        .list_all::<TemplateVersion>("template_versions", Some(&filter), Some("-created"))
        .await?;
    Ok(Json(json!({ "versions": versions })))
}

pub async fn find_template_by_id_or_slug(
    state: &AppState,
    account_id: &str,
    id_or_slug: &str,
) -> AppResult<Template> {
    if let Ok(t) = state.pb.get_by_id::<Template>("templates", id_or_slug).await {
        if t.account_id == account_id {
            return Ok(t);
        }
    }
    let filter = format!(
        "account_id = '{}' && (slug = '{}' || external_id = '{}')",
        pb_escape(account_id),
        pb_escape(id_or_slug),
        pb_escape(id_or_slug)
    );
    state
        .pb
        .find_first::<Template>("templates", &filter, None)
        .await?
        .ok_or_else(|| AppError::not_found(format!("Template '{}' not found", id_or_slug)))
}

async fn save_template_version(state: &AppState, tpl: &Template, author_id: &str) {
    let snapshot = json!({
        "name": tpl.name,
        "schema": tpl.schema,
        "fields": tpl.fields,
        "submitters": tpl.submitters,
    });
    let serialized = serde_json::to_string(&snapshot).unwrap_or_default();
    let mut sha = Sha1::new();
    sha.update(serialized.as_bytes());
    let sha1_hex = hex::encode(sha.finalize());

    let _ = state
        .pb
        .create::<TemplateVersion>(
            "template_versions",
            &json!({
                "template_id": tpl.id,
                "account_id": tpl.account_id,
                "author_id": author_id,
                "sha1": sha1_hex,
                "data": snapshot,
            }),
        )
        .await;
}
