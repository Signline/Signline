use crate::error::{AppError, AppResult};
use crate::models::StorageBlob;
use crate::pb::pb_escape;
use crate::state::AppState;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Stores a binary file on disk under `storage_dir/<key>` and registers its metadata
/// in the `storage_blobs` PocketBase collection.
pub async fn store_blob(
    state: &AppState,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
    metadata: Value,
) -> AppResult<StorageBlob> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let checksum = hex::encode(hasher.finalize());

    let uuid = Uuid::new_v4().to_string();
    let safe_ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let key = format!("{}.{}", uuid, safe_ext);

    std::fs::create_dir_all(&state.config.storage_dir)?;
    let file_path = state.config.storage_dir.join(&key);
    std::fs::write(&file_path, bytes)?;

    let blob: StorageBlob = state
        .pb
        .create(
            "storage_blobs",
            &json!({
                "uuid": uuid,
                "key": key,
                "filename": filename,
                "content_type": content_type,
                "byte_size": bytes.len() as i64,
                "checksum": checksum,
                "metadata": metadata,
                "service_name": "local",
            }),
        )
        .await?;

    Ok(blob)
}

/// Resolves a `StorageBlob` by either its `uuid`, `key`, or PocketBase `id`, and reads its raw bytes.
pub async fn read_blob_bytes(
    state: &AppState,
    identifier: &str,
) -> AppResult<(StorageBlob, Vec<u8>)> {
    let id_clean = identifier.trim();
    if id_clean.is_empty() {
        return Err(AppError::not_found("Empty blob identifier"));
    }

    let filter = format!(
        "uuid = '{}' || key = '{}' || id = '{}'",
        pb_escape(id_clean),
        pb_escape(id_clean),
        pb_escape(id_clean)
    );
    let blob = state
        .pb
        .find_first::<StorageBlob>("storage_blobs", &filter, None)
        .await?
        .ok_or_else(|| AppError::not_found(format!("Storage blob '{}' not found", id_clean)))?;

    let file_path = state.config.storage_dir.join(&blob.key);
    let bytes = std::fs::read(&file_path).map_err(|e| {
        AppError::not_found(format!("Blob file '{}' missing on disk: {}", blob.key, e))
    })?;

    Ok((blob, bytes))
}

/// Decodes a `data:image/...;base64,...` data URI or raw base64 string into `(content_type, bytes)`.
pub fn decode_data_uri_or_base64(input: &str, default_mime: &str) -> AppResult<(String, Vec<u8>)> {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("data:") {
        if let Some((meta, b64)) = rest.split_once(',') {
            let mime = meta
                .split(';')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(default_mime)
                .to_string();
            let decoded = BASE64
                .decode(b64.trim())
                .map_err(|e| AppError::bad_request(format!("Invalid base64 data URI: {}", e)))?;
            return Ok((mime, decoded));
        }
    }
    let decoded = BASE64
        .decode(trimmed)
        .map_err(|e| AppError::bad_request(format!("Invalid base64 payload: {}", e)))?;
    Ok((default_mime.to_string(), decoded))
}
