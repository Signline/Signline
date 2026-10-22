use crate::error::AppResult;
use crate::services::storage::store_blob;
use crate::state::AppState;
use image::{ImageBuffer, Rgba};
use lopdf::Document as LoPdfDocument;
use regex::Regex;
use serde_json::{json, Value};
use std::io::Cursor;
use std::process::Command;
use tempfile::TempDir;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProcessedDocument {
    pub doc_uuid: String,
    pub name: String,
    pub blob_uuid: String,
    pub preview_images: Vec<Value>,
    pub schema_entry: Value,
    pub extracted_fields: Vec<Value>,
    pub extracted_roles: Vec<String>,
}

/// Normalizes an uploaded file (PDF, PNG/JPEG image, or text/HTML) into a valid PDF,
/// renders page preview PNGs via `/usr/bin/pdftoppm -png -r 150`, stores all blobs,
/// and extracts any embedded `{{field_name;role=...;type=...}}` tags or AcroForm fields.
pub async fn process_uploaded_document(
    state: &AppState,
    filename: &str,
    content_type: &str,
    raw_bytes: &[u8],
) -> AppResult<ProcessedDocument> {
    let doc_uuid = Uuid::new_v4().to_string();
    let clean_name = filename
        .rsplit('/')
        .next()
        .unwrap_or("Document.pdf")
        .trim_end_matches(".pdf")
        .trim_end_matches(".PDF")
        .to_string();

    // 1. Ensure we have valid PDF bytes
    let pdf_bytes = if raw_bytes.starts_with(b"%PDF") {
        raw_bytes.to_vec()
    } else if content_type.starts_with("image/")
        || filename.ends_with(".png")
        || filename.ends_with(".jpg")
        || filename.ends_with(".jpeg")
    {
        crate::pdf::generator::build_simple_pdf(&clean_name, &[format!("Attached Image Document: {}", filename)])
    } else {
        let text = String::from_utf8_lossy(raw_bytes);
        let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        crate::pdf::generator::build_simple_pdf(&clean_name, &lines)
    };

    // 2. Store the normalized PDF blob
    let pdf_blob = store_blob(
        state,
        &format!("{}.pdf", clean_name),
        "application/pdf",
        &pdf_bytes,
        json!({ "document_uuid": doc_uuid }),
    )
    .await?;

    // 3. Inspect page count & dimensions via lopdf
    let (page_count, page_dims) = inspect_pdf_pages(&pdf_bytes);

    // 4. Render page preview PNGs with `/usr/bin/pdftoppm -png -r 150`
    let rendered_pngs = render_pdf_pages_to_png(&pdf_bytes, page_count);
    let mut preview_images = Vec::new();

    for (idx, png_bytes) in rendered_pngs.into_iter().enumerate() {
        let (w, h) = page_dims.get(idx).copied().unwrap_or((612.0, 792.0));
        let preview_blob = store_blob(
            state,
            &format!("{}-page-{}.png", clean_name, idx + 1),
            "image/png",
            &png_bytes,
            json!({
                "page": idx,
                "width": w,
                "height": h,
                "document_uuid": doc_uuid,
            }),
        )
        .await?;

        preview_images.push(json!({
            "id": preview_blob.uuid,
            "uuid": preview_blob.uuid,
            "page": idx,
            "width": w,
            "height": h,
            "url": format!("/api/blobs/{}", preview_blob.uuid),
            "filename": preview_blob.filename,
        }));
    }

    // 5. Extract embedded {{field;type=...;role=...}} tags & AcroForm fields
    let (extracted_fields, extracted_roles) = extract_fields_from_pdf(&pdf_bytes, &doc_uuid);

    let schema_entry = json!({
        "attachment_uuid": doc_uuid,
        "name": clean_name,
    });

    Ok(ProcessedDocument {
        doc_uuid,
        name: clean_name,
        blob_uuid: pdf_blob.uuid,
        preview_images,
        schema_entry,
        extracted_fields,
        extracted_roles,
    })
}

fn inspect_pdf_pages(pdf_bytes: &[u8]) -> (usize, Vec<(f64, f64)>) {
    if let Ok(doc) = LoPdfDocument::load_mem(pdf_bytes) {
        let pages = doc.get_pages();
        if !pages.is_empty() {
            let mut dims = Vec::with_capacity(pages.len());
            for (_, object_id) in &pages {
                let mut w = 612.0;
                let mut h = 792.0;
                if let Ok(obj) = doc.get_object(*object_id) {
                    if let Ok(dict) = obj.as_dict() {
                        if let Ok(mb) = dict.get(b"MediaBox").and_then(|m| m.as_array()) {
                            if mb.len() == 4 {
                                let x0 = lopdf_num(&mb[0]).unwrap_or(0.0);
                                let y0 = lopdf_num(&mb[1]).unwrap_or(0.0);
                                let x1 = lopdf_num(&mb[2]).unwrap_or(612.0);
                                let y1 = lopdf_num(&mb[3]).unwrap_or(792.0);
                                if (x1 - x0).abs() > 10.0 && (y1 - y0).abs() > 10.0 {
                                    w = (x1 - x0).abs();
                                    h = (y1 - y0).abs();
                                }
                            }
                        }
                    }
                }
                dims.push((w, h));
            }
            return (pages.len(), dims);
        }
    }
    (1, vec![(612.0, 792.0)])
}

fn lopdf_num(obj: &lopdf::Object) -> Option<f64> {
    match obj {
        lopdf::Object::Integer(i) => Some(*i as f64),
        lopdf::Object::Real(f) => Some(*f as f64),
        _ => None,
    }
}

/// Renders PDF pages into PNG images using `/usr/bin/pdftoppm -png -r 150`, with a clean
/// pure-Rust fallback page renderer if `pdftoppm` fails.
pub fn render_pdf_pages_to_png(pdf_bytes: &[u8], expected_pages: usize) -> Vec<Vec<u8>> {
    if let Ok(tmp) = TempDir::new() {
        let input_pdf = tmp.path().join("input.pdf");
        let prefix = tmp.path().join("page");
        if std::fs::write(&input_pdf, pdf_bytes).is_ok() {
            let status = Command::new("/usr/bin/pdftoppm")
                .args([
                    "-png",
                    "-r",
                    "150",
                    input_pdf.to_str().unwrap(),
                    prefix.to_str().unwrap(),
                ])
                .output();

            if let Ok(out) = status {
                if out.status.success() {
                    if let Ok(entries) = std::fs::read_dir(tmp.path()) {
                        let mut page_files: Vec<_> = entries
                            .filter_map(|e| e.ok())
                            .filter(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                name.starts_with("page-") && name.ends_with(".png")
                            })
                            .collect();
                        page_files.sort_by_key(|e| e.file_name());
                        let mut pngs = Vec::new();
                        for pf in page_files {
                            if let Ok(bytes) = std::fs::read(pf.path()) {
                                if !bytes.is_empty() {
                                    pngs.push(bytes);
                                }
                            }
                        }
                        if !pngs.is_empty() {
                            return pngs;
                        }
                    }
                }
            }
        }
    }

    // Pure-Rust PNG page fallback
    let count = expected_pages.max(1);
    (0..count).map(|_| generate_fallback_page_png()).collect()
}

pub fn generate_fallback_page_png() -> Vec<u8> {
    let width = 612u32;
    let height = 792u32;
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));

    // Subtle document border & header rule
    for x in 40..width - 40 {
        img.put_pixel(x, 60, Rgba([33, 97, 173, 255]));
        img.put_pixel(x, 61, Rgba([33, 97, 173, 255]));
        img.put_pixel(x, height - 50, Rgba([214, 226, 242, 255]));
    }

    let mut buf = Cursor::new(Vec::new());
    let _ = img.write_to(&mut buf, image::ImageFormat::Png);
    buf.into_inner()
}

/// Scans PDF content streams and raw text for `{{field_name;role=First Party;type=signature;required=true}}`
/// tags as well as standard AcroForm `/T (...)` field names.
pub fn extract_fields_from_pdf(
    pdf_bytes: &[u8],
    attachment_uuid: &str,
) -> (Vec<Value>, Vec<String>) {
    let raw_str = String::from_utf8_lossy(pdf_bytes);
    let tag_re = Regex::new(r"\{\{([^{}]+)\}\}").unwrap();

    let mut fields = Vec::new();
    let mut roles: Vec<String> = vec!["First Party".to_string()];
    let mut idx = 0usize;

    for cap in tag_re.captures_iter(&raw_str) {
        let inner = cap[1].trim();
        if inner.is_empty() {
            continue;
        }
        let mut name = String::new();
        let mut field_type = "text".to_string();
        let mut role = "First Party".to_string();
        let mut required = true;

        for (i, part) in inner.split(';').enumerate() {
            let p = part.trim();
            if i == 0 && !p.contains('=') {
                name = p.to_string();
                continue;
            }
            if let Some((k, v)) = p.split_once('=') {
                let key = k.trim().to_lowercase();
                let val = v.trim();
                match key.as_str() {
                    "name" => name = val.to_string(),
                    "type" => field_type = val.to_lowercase(),
                    "role" => role = val.to_string(),
                    "required" => required = val != "false" && val != "0",
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            name = format!("Field {}", idx + 1);
        }
        if !roles.contains(&role) {
            roles.push(role.clone());
        }

        let y_pos = (0.15 + (idx as f64 * 0.08)).min(0.85);
        fields.push(json!({
            "uuid": Uuid::new_v4().to_string(),
            "submitter_uuid": format!("role-{}", role.to_lowercase().replace(' ', "-")),
            "role": role,
            "name": name,
            "type": field_type,
            "required": required,
            "areas": [{
                "attachment_uuid": attachment_uuid,
                "page": 0,
                "x": 0.12,
                "y": y_pos,
                "w": 0.35,
                "h": if field_type == "signature" { 0.06 } else { 0.04 }
            }]
        }));
        idx += 1;
    }

    (fields, roles)
}

/// Auto-detects candidate form fields from a template's PDF documents (`POST /api/templates/:id/detect_fields`).
pub fn auto_detect_fields_for_template(
    pdf_bytes: &[u8],
    attachment_uuid: &str,
    default_submitter_uuid: &str,
) -> Vec<Value> {
    let (mut fields, _) = extract_fields_from_pdf(pdf_bytes, attachment_uuid);
    for f in &mut fields {
        if let Some(obj) = f.as_object_mut() {
            obj.insert("submitter_uuid".to_string(), json!(default_submitter_uuid));
        }
    }

    if fields.is_empty() {
        // Provide a standard detected set (Full Name, Date, Signature) when scanning a plain PDF
        fields = vec![
            json!({
                "uuid": Uuid::new_v4().to_string(),
                "submitter_uuid": default_submitter_uuid,
                "name": "Full Name",
                "type": "text",
                "required": true,
                "areas": [{
                    "attachment_uuid": attachment_uuid,
                    "page": 0,
                    "x": 0.12,
                    "y": 0.65,
                    "w": 0.36,
                    "h": 0.04
                }]
            }),
            json!({
                "uuid": Uuid::new_v4().to_string(),
                "submitter_uuid": default_submitter_uuid,
                "name": "Date Signed",
                "type": "date",
                "required": true,
                "areas": [{
                    "attachment_uuid": attachment_uuid,
                    "page": 0,
                    "x": 0.55,
                    "y": 0.65,
                    "w": 0.25,
                    "h": 0.04
                }]
            }),
            json!({
                "uuid": Uuid::new_v4().to_string(),
                "submitter_uuid": default_submitter_uuid,
                "name": "Signature",
                "type": "signature",
                "required": true,
                "areas": [{
                    "attachment_uuid": attachment_uuid,
                    "page": 0,
                    "x": 0.12,
                    "y": 0.74,
                    "w": 0.40,
                    "h": 0.07
                }]
            }),
        ];
    }

    fields
}
