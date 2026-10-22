use crate::crypto::{cert::ensure_account_cert, verifier::verify_pdf_bytes};
use crate::error::{AppError, AppResult};
use crate::pdf::generator::merge_pdf_documents;
use crate::services::storage::decode_data_uri_or_base64;
use crate::state::AppState;
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/verify", post(verify_pdf_endpoint))
        .route("/api/tools/verify", post(verify_pdf_endpoint))
        .route("/api/tools/merge", post(merge_pdfs_endpoint))
}

/// Inspects an uploaded PDF (accepted as `multipart/form-data`, `application/json` with base64 `file`,
/// or raw `application/pdf` bytes) and verifies its SHA-256 against `completed_documents` as well as
/// its embedded `/ByteRange` + `/Contents` PKCS#7 / CMS signature and X.509 certificate chain.
async fn verify_pdf_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let pdf_bytes = if content_type.starts_with("application/json") {
        let val: Value = serde_json::from_slice(&body)?;
        let b64_str = val
            .get("file")
            .or_else(|| val.get("pdf_base64"))
            .or_else(|| val.get("document"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                AppError::bad_request("Provide 'file' or 'pdf_base64' in JSON request body")
            })?;
        let (_, decoded) = decode_data_uri_or_base64(b64_str, "application/pdf")?;
        decoded
    } else if content_type.starts_with("multipart/form-data") {
        extract_pdf_from_multipart_bytes(content_type, &body)?
    } else {
        body.to_vec()
    };

    if pdf_bytes.is_empty() {
        return Err(AppError::bad_request("Empty PDF payload provided"));
    }

    let trusted_cert = if let Some(acc) = state.pb.resolve_account(None).await? {
        ensure_account_cert(&state, &acc.id).await.ok()
    } else {
        None
    };

    let report = verify_pdf_bytes(&state, &pdf_bytes, trusted_cert.as_ref()).await?;
    Ok(Json(report))
}

/// Parses a `multipart/form-data` body to extract the uploaded binary file.
fn extract_pdf_from_multipart_bytes(content_type: &str, body: &[u8]) -> AppResult<Vec<u8>> {
    let boundary = content_type
        .split("boundary=")
        .nth(1)
        .map(|b| b.split(';').next().unwrap_or(b).trim().trim_matches('"'))
        .ok_or_else(|| AppError::bad_request("Missing boundary in multipart/form-data"))?;

    let delimiter = format!("--{}", boundary);
    let delim_bytes = delimiter.as_bytes();

    // Locate the first part containing `\r\n\r\n` and extract bytes up to the next `\r\n--boundary`
    let mut pos = 0usize;
    while pos + delim_bytes.len() < body.len() {
        if let Some(rel_start) = find_subsequence(&body[pos..], delim_bytes) {
            let part_start = pos + rel_start + delim_bytes.len();
            if let Some(hdr_end) = find_subsequence(&body[part_start..], b"\r\n\r\n") {
                let content_start = part_start + hdr_end + 4;
                let next_delim = format!("\r\n--{}", boundary);
                if let Some(content_len) =
                    find_subsequence(&body[content_start..], next_delim.as_bytes())
                {
                    let slice = &body[content_start..content_start + content_len];
                    if !slice.is_empty() {
                        return Ok(slice.to_vec());
                    }
                }
            }
            pos = part_start;
        } else {
            break;
        }
    }

    Err(AppError::bad_request(
        "Could not locate file payload in multipart upload",
    ))
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// `POST /api/tools/merge` — Merges multiple PDFs into a single PDF document.
async fn merge_pdfs_endpoint(
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<impl IntoResponse> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let mut pdf_inputs: Vec<Vec<u8>> = Vec::new();
    if content_type.starts_with("application/json") {
        let val: Value = serde_json::from_slice(&body)?;
        if let Some(arr) = val
            .get("files")
            .or_else(|| val.get("documents"))
            .and_then(|v| v.as_array())
        {
            for item in arr {
                if let Some(s) = item.as_str() {
                    if let Ok((_, b)) = decode_data_uri_or_base64(s, "application/pdf") {
                        pdf_inputs.push(b);
                    }
                } else if let Some(s) = item.get("file").and_then(|f| f.as_str()) {
                    if let Ok((_, b)) = decode_data_uri_or_base64(s, "application/pdf") {
                        pdf_inputs.push(b);
                    }
                }
            }
        }
    }

    let merged = merge_pdf_documents(&pdf_inputs);

    if content_type.starts_with("application/json") {
        Ok((
            StatusCode::OK,
            Json(json!({
                "merged": true,
                "document_count": pdf_inputs.len(),
                "byte_size": merged.len(),
                "pdf_base64": BASE64.encode(&merged),
            }))
            .into_response(),
        ))
    } else {
        let mut resp_headers = HeaderMap::new();
        resp_headers.insert(header::CONTENT_TYPE, "application/pdf".parse().unwrap());
        Ok((StatusCode::OK, (resp_headers, merged).into_response()))
    }
}
