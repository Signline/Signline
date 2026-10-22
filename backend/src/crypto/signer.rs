use crate::crypto::cert::EsignCertBundle;
use crate::error::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::process::Command;
use tempfile::TempDir;

/// Reserved size in bytes (8192 bytes -> 16384 hex chars) for the detached PKCS#7 / CMS
/// `/Contents <...>` hex string inside the incremental PDF signature dictionary.
const SIGNATURE_CONTENTS_RESERVE_BYTES: usize = 8192;

/// Appends an incremental PDF signature dictionary (`/Type /Sig`, `/SubFilter /adbe.pkcs7.detached`,
/// `/ByteRange [...]`, `/Contents <...>`) and signs the exact byte ranges using OpenSSL CMS / PKCS#7.
/// Returns `(signed_pdf_bytes, sha256_hex)`.
pub fn sign_pdf_detached(
    pdf_bytes: &[u8],
    cert_bundle: &EsignCertBundle,
    reason: &str,
    location: &str,
    tsa_url: Option<&str>,
) -> AppResult<(Vec<u8>, String)> {
    let now = chrono::Utc::now();
    let pdf_date = format!("D:{}Z", now.format("%Y%m%d%H%M%S"));
    let tsa_comment = tsa_url.unwrap_or("");

    let hex_placeholder_len = SIGNATURE_CONTENTS_RESERVE_BYTES * 2;
    let byte_range_placeholder = "[0 0000000000 0000000000 0000000000]";

    // Build an incremental PDF signature block appended cleanly after the original PDF bytes.
    let prefix = format!(
        "\n%Signline-Digital-Signature\n99990 0 obj\n<<\n/Type /Sig\n/Filter /Adobe.PPKLite\n/SubFilter /adbe.pkcs7.detached\n/Name ({})\n/Reason ({})\n/Location ({})\n/M ({})\n/Prop_Build << /App << /Name /Signline >> >>\n/TSA ({})\n/ByteRange {}\n/Contents <",
        escape_pdf_str(&cert_bundle.common_name),
        escape_pdf_str(reason),
        escape_pdf_str(location),
        pdf_date,
        escape_pdf_str(tsa_comment),
        byte_range_placeholder,
    );

    let suffix = ">\n>>\nendobj\n%%EOF\n";

    let offset1 = pdf_bytes.len() + prefix.len() - 1; // Position of '<'
    let offset2 = offset1 + 1 + hex_placeholder_len + 1; // Position right after '>'
    let length2 = suffix.len() - 1; // Remaining bytes after '>'

    let actual_byte_range = format!(
        "[0 {:010} {:010} {:010}]",
        offset1, offset2, length2
    );

    let prefix_final = prefix.replace(byte_range_placeholder, &actual_byte_range);
    assert_eq!(prefix_final.len(), prefix.len());

    let mut output = Vec::with_capacity(pdf_bytes.len() + prefix_final.len() + hex_placeholder_len + suffix.len());
    output.extend_from_slice(pdf_bytes);
    output.extend_from_slice(prefix_final.as_bytes());
    let contents_start = output.len();
    output.extend(std::iter::repeat(b'0').take(hex_placeholder_len));
    output.extend_from_slice(suffix.as_bytes());

    // Extract the two signed byte ranges: [0..offset1] and [offset2..offset2+length2]
    let mut data_to_sign = Vec::with_capacity(offset1 + length2);
    data_to_sign.extend_from_slice(&output[0..offset1]);
    data_to_sign.extend_from_slice(&output[offset2..offset2 + length2]);

    let cms_der = create_detached_cms_signature(&data_to_sign, cert_bundle)?;
    let cms_hex = hex::encode_upper(&cms_der);

    if cms_hex.len() > hex_placeholder_len {
        return Err(AppError::Internal(format!(
            "CMS signature size ({}) exceeds reserved placeholder ({})",
            cms_hex.len(),
            hex_placeholder_len
        )));
    }

    output[contents_start..contents_start + cms_hex.len()].copy_from_slice(cms_hex.as_bytes());

    let mut sha = Sha256::new();
    sha.update(&output);
    let final_sha256 = hex::encode(sha.finalize());

    Ok((output, final_sha256))
}

/// Calls `openssl cms -sign -binary -outform DER` to produce an RFC 5652 / PKCS#7 detached signature.
fn create_detached_cms_signature(
    data_to_sign: &[u8],
    cert_bundle: &EsignCertBundle,
) -> AppResult<Vec<u8>> {
    let tmp = TempDir::new()?;
    let data_path = tmp.path().join("data.bin");
    let cert_path = tmp.path().join("cert.pem");
    let key_path = tmp.path().join("key.pem");
    let out_path = tmp.path().join("sig.der");

    std::fs::write(&data_path, data_to_sign)?;
    std::fs::write(&cert_path, &cert_bundle.cert_pem)?;
    std::fs::write(&key_path, &cert_bundle.key_pem)?;

    let status = Command::new("openssl")
        .args([
            "cms",
            "-sign",
            "-binary",
            "-in",
            data_path.to_str().unwrap(),
            "-signer",
            cert_path.to_str().unwrap(),
            "-inkey",
            key_path.to_str().unwrap(),
            "-outform",
            "DER",
            "-out",
            out_path.to_str().unwrap(),
            "-md",
            "sha256",
        ])
        .output()
        .map_err(|e| AppError::Internal(format!("Failed to run openssl cms -sign: {}", e)))?;

    if !status.status.success() {
        return Err(AppError::Internal(format!(
            "OpenSSL CMS signing failed: {}",
            String::from_utf8_lossy(&status.stderr)
        )));
    }

    let der = std::fs::read(&out_path)?;
    Ok(der)
}

fn escape_pdf_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}
