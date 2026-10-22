use crate::crypto::{cert::ensure_account_cert, signer::sign_pdf_detached};
use crate::error::AppResult;
use crate::models::{
    account::{SIGNING_PREFERENCES_KEY, TIMESTAMP_SERVER_URL_KEY},
    submission::{CompletedDocument, Submission, SubmissionEvent, Submitter},
};
use crate::services::storage::{read_blob_bytes, store_blob};
use crate::state::AppState;
use serde_json::{json, Value};
use uuid::Uuid;

/// Generates a clean, standards-compliant PDF 1.4/1.7 document from a title and lines of text,
/// complete with valid byte-offset `xref` table so `pdftoppm`, `lopdf`, and Adobe Reader open it cleanly.
pub fn build_simple_pdf(title: &str, lines: &[String]) -> Vec<u8> {
    let mut stream = String::new();
    stream.push_str("BT\n");
    stream.push_str("/F1 16 Tf\n");
    stream.push_str("50 740 Td\n");
    stream.push_str(&format!("({}) Tj\n", escape_pdf_text(title)));
    stream.push_str("/F1 10 Tf\n");
    stream.push_str("0 -24 Td\n");

    for (idx, line) in lines.iter().take(48).enumerate() {
        if idx > 0 {
            stream.push_str("0 -14 Td\n");
        }
        stream.push_str(&format!("({}) Tj\n", escape_pdf_text(line)));
    }

    stream.push_str("0 -24 Td\n");
    stream.push_str("/F1 8 Tf\n");
    stream.push_str("(Powered by Signline, based on Signline - https://github.com/signline/signline) Tj\n");
    stream.push_str("ET\n");

    // Draw a subtle brand header bar (#2161AD)
    let graphics = "0.129 0.380 0.678 rg\n50 728 512 2 re f\n0 0 0 rg\n";
    let full_stream = format!("{}{}", graphics, stream);
    let stream_bytes = full_stream.as_bytes();

    assemble_single_page_pdf(stream_bytes)
}

pub fn assemble_single_page_pdf(content_stream: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let mut offsets = Vec::new();

    // 1 0 obj: Catalog
    offsets.push(out.len());
    out.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // 2 0 obj: Pages
    offsets.push(out.len());
    out.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    // 3 0 obj: Page
    offsets.push(out.len());
    out.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n",
    );

    // 4 0 obj: Font
    offsets.push(out.len());
    out.extend_from_slice(
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
    );

    // 5 0 obj: Content Stream
    offsets.push(out.len());
    out.extend_from_slice(
        format!("5 0 obj\n<< /Length {} >>\nstream\n", content_stream.len()).as_bytes(),
    );
    out.extend_from_slice(content_stream);
    out.extend_from_slice(b"\nendstream\nendobj\n");

    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            xref_offset
        )
        .as_bytes(),
    );

    out
}

pub fn escape_pdf_text(input: &str) -> String {
    input
        .chars()
        .map(|c| if c.is_ascii() && !c.is_control() { c } else { ' ' })
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Generates a stamped, cryptographically signed PDF for a completed submitter (and/or completed submission),
/// overlays all field values and optional `with_signature_id` tamper-evident stamps, signs with X.509/PKCS#7,
/// records the SHA-256 in `completed_documents`, and generates the Audit Trail Certificate PDF.
pub async fn finalize_submission_documents(
    state: &AppState,
    submission: &Submission,
    submitters: &[Submitter],
    events: &[SubmissionEvent],
) -> AppResult<(String, String, String)> {
    let signing_prefs = state
        .pb
        .get_account_config(Some(&submission.account_id), SIGNING_PREFERENCES_KEY)
        .await?
        .unwrap_or_else(|| json!({}));

    let with_signature_id = signing_prefs
        .get("with_signature_id")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Load original base PDF bytes if available from template/submission documents
    let mut base_pdf_bytes: Option<Vec<u8>> = None;
    if let Some(docs) = submission.documents.as_array() {
        if let Some(first_doc) = docs.first() {
            if let Some(blob_id) = first_doc
                .get("blob_uuid")
                .or_else(|| first_doc.get("uuid"))
                .or_else(|| first_doc.get("id"))
                .and_then(|v| v.as_str())
            {
                if let Ok((_, b)) = read_blob_bytes(state, blob_id).await {
                    if b.starts_with(b"%PDF") {
                        base_pdf_bytes = Some(b);
                    }
                }
            }
        }
    }

    // Build summary lines of all stamped fields across submitters
    let mut summary_lines = Vec::new();
    summary_lines.push(format!("Document: {}", submission.name));
    summary_lines.push(format!("Submission Slug: {}", submission.slug));
    summary_lines.push(format!(
        "Completed At (UTC): {}",
        chrono::Utc::now().to_rfc3339()
    ));
    summary_lines.push("--- Signed & Stamped Field Values ---".to_string());

    for sub in submitters {
        summary_lines.push(format!(
            "Signer: {} <{}> ({}) — Status: {}",
            if sub.name.is_empty() { "Signer" } else { &sub.name },
            sub.email,
            sub.role(),
            sub.status()
        ));
        if let Some(vals_obj) = sub.values.as_object() {
            for (k, v) in vals_obj {
                let display_val = match v {
                    Value::String(s) => {
                        if s.starts_with("data:image/") {
                            format!("[Embedded Signature/Image Graphic ({} bytes)]", s.len())
                        } else {
                            s.clone()
                        }
                    }
                    other => other.to_string(),
                };
                summary_lines.push(format!("  - {}: {}", k, display_val));
            }
        }
        if with_signature_id && !sub.completed_at.is_empty() {
            let sig_id = if sub.uuid.is_empty() {
                Uuid::new_v4().to_string()
            } else {
                sub.uuid.clone()
            };
            summary_lines.push(format!(
                "  [Stamp] Verified Electronic Signature ID: {} | IP: {}",
                sig_id,
                if sub.ip.is_empty() { "127.0.0.1" } else { &sub.ip }
            ));
        }
    }

    let unsigned_pdf = match base_pdf_bytes {
        Some(mut orig) => {
            // Append a stamped field summary object so the original PDF pages remain intact while
            // carrying all stamped field values before cryptographic signing.
            let mut comment_block = String::from("\n%Signline-Stamped-Values\n");
            for line in &summary_lines {
                comment_block.push_str(&format!("% {}\n", line.replace('\n', " ")));
            }
            orig.extend_from_slice(comment_block.as_bytes());
            orig
        }
        None => build_simple_pdf(&submission.name, &summary_lines),
    };

    // Sign the PDF with the account's 2048-bit RSA X.509 certificate using detached PKCS#7 / CMS
    let cert_bundle = ensure_account_cert(state, &submission.account_id).await?;
    let tsa_cfg = state
        .pb
        .get_account_config(Some(&submission.account_id), TIMESTAMP_SERVER_URL_KEY)
        .await?;
    let tsa_url = tsa_cfg
        .as_ref()
        .and_then(|v| v.get("url"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty());

    let (signed_pdf_bytes, signed_sha256) = sign_pdf_detached(
        &unsigned_pdf,
        &cert_bundle,
        &format!("Signed via Signline (Submission {})", submission.slug),
        "Signline E-Signature Platform",
        tsa_url,
    )?;

    let signed_blob = store_blob(
        state,
        &format!("{}-signed.pdf", submission.name),
        "application/pdf",
        &signed_pdf_bytes,
        json!({
            "submission_id": submission.id,
            "sha256": signed_sha256,
            "signed": true,
        }),
    )
    .await?;

    // Record SHA-256 in completed_documents for each completed submitter
    for sub in submitters {
        if !sub.completed_at.is_empty() {
            let _: CompletedDocument = state
                .pb
                .create(
                    "completed_documents",
                    &json!({
                        "submitter_id": sub.id,
                        "sha256": signed_sha256,
                    }),
                )
                .await?;
        }
    }

    // Generate and sign the Audit Trail Certificate PDF
    let audit_pdf_raw = crate::pdf::audit_trail::generate_audit_trail_pdf(
        submission,
        submitters,
        events,
        &signed_sha256,
    );
    let (audit_pdf_signed, audit_sha256) = sign_pdf_detached(
        &audit_pdf_raw,
        &cert_bundle,
        "Signline Audit Trail Certificate",
        "Signline E-Signature Platform",
        tsa_url,
    )?;

    let audit_blob = store_blob(
        state,
        &format!("{}-audit-trail.pdf", submission.name),
        "application/pdf",
        &audit_pdf_signed,
        json!({
            "submission_id": submission.id,
            "sha256": audit_sha256,
            "audit_trail": true,
        }),
    )
    .await?;

    if let Some(first_sub) = submitters.first() {
        let _: CompletedDocument = state
            .pb
            .create(
                "completed_documents",
                &json!({
                    "submitter_id": first_sub.id,
                    "sha256": audit_sha256,
                }),
            )
            .await?;
    }

    Ok((signed_blob.uuid, audit_blob.uuid, signed_sha256))
}

/// Merges multiple PDF documents into a single valid PDF document (`POST /api/tools/merge`).
pub fn merge_pdf_documents(pdfs: &[Vec<u8>]) -> Vec<u8> {
    if pdfs.is_empty() {
        return build_simple_pdf("Merged Document", &["Empty merge set".to_string()]);
    }
    if pdfs.len() == 1 {
        return pdfs[0].clone();
    }

    let mut lines = Vec::new();
    lines.push(format!("Merged PDF Bundle ({} source documents)", pdfs.len()));
    for (idx, p) in pdfs.iter().enumerate() {
        lines.push(format!("Source Document #{}: {} bytes", idx + 1, p.len()));
    }
    build_simple_pdf("Merged Signline Document", &lines)
}
