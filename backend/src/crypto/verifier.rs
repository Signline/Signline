use crate::crypto::cert::EsignCertBundle;
use crate::error::AppResult;
use crate::models::submission::{CompletedDocument, Submission, Submitter};
use crate::pb::pb_escape;
use crate::state::AppState;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::process::Command;
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationMessage {
    pub text: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedSignatureInfo {
    pub verified: bool,
    pub trusted: bool,
    pub has_unsigned_changes: bool,
    pub reason: String,
    pub location: String,
    pub signing_time: String,
    pub common_name: String,
    pub organization: String,
    pub issuer: String,
    pub serial: String,
    pub sub_filter: String,
    pub byte_range: Vec<usize>,
    pub certificate_chain: Vec<String>,
    pub messages: Vec<VerificationMessage>,
}

/// Comprehensive PDF verification report returned by `/api/verify` and `/api/tools/verify`.
pub async fn verify_pdf_bytes(
    state: &AppState,
    pdf_bytes: &[u8],
    trusted_cert: Option<&EsignCertBundle>,
) -> AppResult<Value> {
    let mut hasher = Sha256::new();
    hasher.update(pdf_bytes);
    let sha256 = hex::encode(hasher.finalize());

    // 1. Check CompletedDocument registry in PocketBase by SHA-256
    let filter = format!("sha256 = '{}'", pb_escape(&sha256));
    let completed_doc = state
        .pb
        .find_first::<CompletedDocument>("completed_documents", &filter, None)
        .await?;

    let mut matched_submitter: Option<Value> = None;
    let mut matched_submission: Option<Value> = None;

    if let Some(ref cdoc) = completed_doc {
        if let Ok(sub) = state
            .pb
            .get_by_id::<Submitter>("submitters", &cdoc.submitter_id)
            .await
        {
            matched_submitter = Some(json!({
                "id": sub.id,
                "name": sub.name,
                "email": sub.email,
                "slug": sub.slug,
                "completed_at": sub.completed_at,
                "ip": sub.ip,
            }));
            if let Ok(submission) = state
                .pb
                .get_by_id::<Submission>("submissions", &sub.submission_id)
                .await
            {
                matched_submission = Some(json!({
                    "id": submission.id,
                    "name": submission.name,
                    "slug": submission.slug,
                    "completed_at": submission.completed_at,
                }));
            }
        }
    }

    // 2. Parse embedded PDF /ByteRange and /Contents <hex> digital signatures
    let signatures = extract_and_verify_pdf_signatures(pdf_bytes, trusted_cert);

    let any_crypto_verified = signatures.iter().any(|s| s.verified);
    let db_matched = completed_doc.is_some();
    let overall_verified = if !signatures.is_empty() {
        any_crypto_verified
    } else {
        db_matched
    };
    let primary_sig = signatures.first().map(|s| {
        json!({
            "subject": format!("CN={}, O={}", s.common_name, s.organization),
            "common_name": s.common_name,
            "organization": s.organization,
            "issuer": s.issuer,
            "serial": s.serial,
            "verified": s.verified,
            "trusted": s.trusted,
            "signing_time": s.signing_time,
        })
    });

    Ok(json!({
        "verified": overall_verified,
        "integrity_valid": any_crypto_verified,
        "byte_range_valid": any_crypto_verified,
        "sha256": sha256,
        "byte_size": pdf_bytes.len(),
        "database_matched": db_matched,
        "database_match": {
            "matched": db_matched,
            "submitter": matched_submitter,
            "submission": matched_submission,
        },
        "submitter": matched_submitter,
        "submission": matched_submission,
        "signature": primary_sig,
        "signatures": signatures,
    }))
}

/// Scans the PDF for `/ByteRange [offset1 len1 offset2 len2]` and `/Contents <hex>` blocks,
/// extracts the signed data, and verifies the CMS/PKCS#7 detached signature via OpenSSL.
pub fn extract_and_verify_pdf_signatures(
    pdf_bytes: &[u8],
    trusted_cert: Option<&EsignCertBundle>,
) -> Vec<VerifiedSignatureInfo> {
    let text = String::from_utf8_lossy(pdf_bytes);
    let br_re = Regex::new(r"/ByteRange\s*\[\s*(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s*\]").unwrap();
    let reason_re = Regex::new(r"/Reason\s*\(([^)]*)\)").unwrap();
    let loc_re = Regex::new(r"/Location\s*\(([^)]*)\)").unwrap();
    let time_re = Regex::new(r"/M\s*\(([^)]*)\)").unwrap();
    let subfilter_re = Regex::new(r"/SubFilter\s*/([A-Za-z0-9._-]+)").unwrap();

    let mut results = Vec::new();

    for cap in br_re.captures_iter(&text) {
        let m = cap.get(0).unwrap();
        let o1: usize = cap[1].parse().unwrap_or(0);
        let l1: usize = cap[2].parse().unwrap_or(0);
        let o2: usize = cap[3].parse().unwrap_or(0);
        let l2: usize = cap[4].parse().unwrap_or(0);

        if l1 == 0 || o1 + l1 > pdf_bytes.len() || o2 + l2 > pdf_bytes.len() || o2 <= o1 + l1 {
            continue;
        }

        // The `/Contents <...>` lies between `o1 + l1` and `o2`
        let contents_slice = &pdf_bytes[o1 + l1..o2];
        let contents_str = String::from_utf8_lossy(contents_slice);
        let hex_trimmed = contents_str
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim_end_matches('0');

        // Ensure even number of hex digits
        let hex_clean = if hex_trimmed.len() % 2 != 0 {
            format!("{}0", hex_trimmed)
        } else {
            hex_trimmed.to_string()
        };

        let der_bytes = match hex::decode(&hex_clean) {
            Ok(b) if !b.is_empty() => b,
            _ => continue,
        };

        let mut signed_data = Vec::with_capacity(l1 + l2);
        signed_data.extend_from_slice(&pdf_bytes[o1..o1 + l1]);
        signed_data.extend_from_slice(&pdf_bytes[o2..o2 + l2]);

        let has_unsigned_changes = (o2 + l2) < pdf_bytes.len()
            && !pdf_bytes[o2 + l2..]
                .iter()
                .all(|b| *b == b'\n' || *b == b'\r' || *b == b' ');

        // Extract surrounding dictionary context for Reason, Location, M, SubFilter
        let ctx_start = m.start().saturating_sub(512);
        let ctx_end = (m.end() + 256).min(text.len());
        let ctx = &text[ctx_start..ctx_end];

        let reason = reason_re
            .captures(ctx)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Digitally signed by Signline".to_string());
        let location = loc_re
            .captures(ctx)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Signline E-Signature Platform".to_string());
        let signing_time = time_re
            .captures(ctx)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let sub_filter = subfilter_re
            .captures(ctx)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "adbe.pkcs7.detached".to_string());

        let (verified, common_name, organization, issuer, serial, trusted) =
            verify_cms_der(&der_bytes, &signed_data, trusted_cert);

        let mut messages = Vec::new();
        if verified {
            messages.push(VerificationMessage {
                text: "Signature is cryptographically valid and document integrity is intact".to_string(),
                status: "success".to_string(),
            });
            if trusted {
                messages.push(VerificationMessage {
                    text: "Signed with active Signline instance certificate".to_string(),
                    status: "success".to_string(),
                });
            } else {
                messages.push(VerificationMessage {
                    text: "Signed with external or historical X.509 certificate".to_string(),
                    status: "info".to_string(),
                });
            }
        } else {
            messages.push(VerificationMessage {
                text: "Signature verification failed or document bytes were modified".to_string(),
                status: "error".to_string(),
            });
        }

        if has_unsigned_changes {
            messages.push(VerificationMessage {
                text: "Contains unsigned bytes appended after the signature revision".to_string(),
                status: "warning".to_string(),
            });
        }

        messages.push(VerificationMessage {
            text: format!("Certificate chain: {}", common_name),
            status: "info".to_string(),
        });

        results.push(VerifiedSignatureInfo {
            verified,
            trusted,
            has_unsigned_changes,
            reason,
            location,
            signing_time,
            common_name: common_name.clone(),
            organization,
            issuer,
            serial,
            sub_filter,
            byte_range: vec![o1, l1, o2, l2],
            certificate_chain: vec![common_name],
            messages,
        });
    }

    results
}

fn verify_cms_der(
    der_bytes: &[u8],
    signed_data: &[u8],
    trusted_cert: Option<&EsignCertBundle>,
) -> (bool, String, String, String, String, bool) {
    let tmp = match TempDir::new() {
        Ok(t) => t,
        Err(_) => {
            return (
                false,
                "Unknown".into(),
                "Unknown".into(),
                "Unknown".into(),
                "".into(),
                false,
            )
        }
    };

    let sig_path = tmp.path().join("sig.der");
    let data_path = tmp.path().join("data.bin");
    let certs_path = tmp.path().join("signer_certs.pem");

    let _ = std::fs::write(&sig_path, der_bytes);
    let _ = std::fs::write(&data_path, signed_data);

    // 1. Verify CMS signature mathematically against the signed byte ranges
    let verify_out = Command::new("openssl")
        .args([
            "cms",
            "-verify",
            "-binary",
            "-inform",
            "DER",
            "-in",
            sig_path.to_str().unwrap(),
            "-content",
            data_path.to_str().unwrap(),
            "-noverify",
            "-out",
            "/dev/null",
        ])
        .output();

    let verified = verify_out.map(|o| o.status.success()).unwrap_or(false);

    // 2. Extract embedded signer certificates from the PKCS#7 / CMS structure
    let _ = Command::new("openssl")
        .args([
            "pkcs7",
            "-inform",
            "DER",
            "-in",
            sig_path.to_str().unwrap(),
            "-print_certs",
            "-out",
            certs_path.to_str().unwrap(),
        ])
        .output();

    let mut common_name = "signline.local".to_string();
    let mut organization = "Signline".to_string();
    let mut issuer = "Signline".to_string();
    let mut serial = String::new();
    let mut fingerprint = String::new();

    if certs_path.exists() {
        if let Ok(info_out) = Command::new("openssl")
            .args([
                "x509",
                "-in",
                certs_path.to_str().unwrap(),
                "-noout",
                "-subject",
                "-issuer",
                "-serial",
                "-fingerprint",
                "-sha256",
            ])
            .output()
        {
            let txt = String::from_utf8_lossy(&info_out.stdout);
            for line in txt.lines() {
                if let Some(s) = line.strip_prefix("subject=") {
                    for part in s.split(&[',', '/'][..]) {
                        if let Some((k, v)) = part.trim().split_once('=') {
                            if k.trim().eq_ignore_ascii_case("CN") {
                                common_name = v.trim().to_string();
                            } else if k.trim().eq_ignore_ascii_case("O") {
                                organization = v.trim().to_string();
                            }
                        }
                    }
                } else if let Some(s) = line.strip_prefix("issuer=") {
                    issuer = s.trim().to_string();
                } else if let Some(s) = line.strip_prefix("serial=") {
                    serial = s.trim().to_string();
                } else if let Some((_, fp)) = line.split_once("Fingerprint=") {
                    fingerprint = fp.trim().to_string();
                }
            }
        }
    }

    let trusted = trusted_cert
        .map(|tc| !fingerprint.is_empty() && tc.fingerprint_sha256 == fingerprint)
        .unwrap_or(verified);

    (verified, common_name, organization, issuer, serial, trusted)
}
