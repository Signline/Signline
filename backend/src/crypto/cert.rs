use crate::error::{AppError, AppResult};
use crate::models::account::ESIGN_CERTS_KEY;
use crate::state::AppState;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::process::Command;
use tempfile::TempDir;

/// Metadata and PEM bundle for the active X.509 signing certificate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EsignCertBundle {
    pub cert_pem: String,
    pub key_pem: String,
    pub ca_pem: String,
    pub p12_base64: String,
    pub subject: String,
    pub issuer: String,
    pub common_name: String,
    pub organization: String,
    pub serial: String,
    pub not_before: String,
    pub not_after: String,
    pub fingerprint_sha256: String,
    pub is_self_signed: bool,
    pub created_at: String,
}

impl EsignCertBundle {
    pub fn to_public_json(&self, timestamp_server_url: Option<&str>) -> Value {
        json!({
            "subject": self.subject,
            "issuer": self.issuer,
            "common_name": self.common_name,
            "organization": self.organization,
            "serial": self.serial,
            "not_before": self.not_before,
            "not_after": self.not_after,
            "fingerprint_sha256": self.fingerprint_sha256,
            "is_self_signed": self.is_self_signed,
            "created_at": self.created_at,
            "timestamp_server_url": timestamp_server_url.unwrap_or(""),
            "status": "active"
        })
    }
}

/// Generates a 2048-bit RSA keypair and X.509 signing certificate (`O=Signline`, `CN=signline.local`)
/// along with a PKCS#12 archive using OpenSSL.
pub fn generate_self_signed_bundle(product_name: &str) -> AppResult<EsignCertBundle> {
    let tmp = TempDir::new()?;
    let key_path = tmp.path().join("key.pem");
    let cert_path = tmp.path().join("cert.pem");
    let p12_path = tmp.path().join("bundle.p12");

    let org_clean: String = product_name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
        .collect();
    let org = if org_clean.trim().is_empty() {
        "Signline"
    } else {
        org_clean.trim()
    };
    let subj = format!("/C=US/O={}/CN=signline.local", org);

    let req_status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            key_path.to_str().unwrap(),
            "-out",
            cert_path.to_str().unwrap(),
            "-days",
            "3650",
            "-subj",
            &subj,
            "-sha256",
        ])
        .output()
        .map_err(|e| AppError::Internal(format!("Failed to execute openssl req: {}", e)))?;

    if !req_status.status.success() {
        return Err(AppError::Internal(format!(
            "OpenSSL certificate generation failed: {}",
            String::from_utf8_lossy(&req_status.stderr)
        )));
    }

    let _ = Command::new("openssl")
        .args([
            "pkcs12",
            "-export",
            "-out",
            p12_path.to_str().unwrap(),
            "-inkey",
            key_path.to_str().unwrap(),
            "-in",
            cert_path.to_str().unwrap(),
            "-passout",
            "pass:",
        ])
        .output();

    let cert_pem = std::fs::read_to_string(&cert_path)?;
    let key_pem = std::fs::read_to_string(&key_path)?;
    let p12_bytes = std::fs::read(&p12_path).unwrap_or_default();

    inspect_pem_bundle(&cert_pem, &key_pem, &cert_pem, &BASE64.encode(&p12_bytes), true)
}

/// Parses an uploaded PKCS#12 (`.p12` / `.pfx`) file and password into an `EsignCertBundle`.
pub fn parse_pkcs12_bundle(p12_bytes: &[u8], password: &str) -> AppResult<EsignCertBundle> {
    let tmp = TempDir::new()?;
    let p12_path = tmp.path().join("upload.p12");
    let cert_path = tmp.path().join("cert.pem");
    let key_path = tmp.path().join("key.pem");
    let ca_path = tmp.path().join("ca.pem");

    std::fs::write(&p12_path, p12_bytes)?;
    let pass_arg = format!("pass:{}", password);

    // Extract client certificate
    let cert_out = Command::new("openssl")
        .args([
            "pkcs12",
            "-in",
            p12_path.to_str().unwrap(),
            "-clcerts",
            "-nokeys",
            "-out",
            cert_path.to_str().unwrap(),
            "-passin",
            &pass_arg,
        ])
        .output()?;

    if !cert_out.status.success() {
        // Fallback without -clcerts if bundle has single cert
        let fallback = Command::new("openssl")
            .args([
                "pkcs12",
                "-in",
                p12_path.to_str().unwrap(),
                "-nokeys",
                "-out",
                cert_path.to_str().unwrap(),
                "-passin",
                &pass_arg,
            ])
            .output()?;
        if !fallback.status.success() {
            return Err(AppError::bad_request(
                "Invalid PKCS#12 file or incorrect certificate password",
            ));
        }
    }

    // Extract unencrypted private key
    let key_out = Command::new("openssl")
        .args([
            "pkcs12",
            "-in",
            p12_path.to_str().unwrap(),
            "-nocerts",
            "-nodes",
            "-out",
            key_path.to_str().unwrap(),
            "-passin",
            &pass_arg,
        ])
        .output()?;

    if !key_out.status.success() {
        return Err(AppError::bad_request(
            "Unable to extract private key from PKCS#12 archive",
        ));
    }

    // Extract CA chain if any
    let _ = Command::new("openssl")
        .args([
            "pkcs12",
            "-in",
            p12_path.to_str().unwrap(),
            "-cacerts",
            "-nokeys",
            "-out",
            ca_path.to_str().unwrap(),
            "-passin",
            &pass_arg,
        ])
        .output();

    let cert_pem = std::fs::read_to_string(&cert_path).unwrap_or_default();
    let key_pem = std::fs::read_to_string(&key_path).unwrap_or_default();
    let ca_pem = std::fs::read_to_string(&ca_path).unwrap_or_else(|_| cert_pem.clone());

    if !cert_pem.contains("BEGIN CERTIFICATE") || !key_pem.contains("PRIVATE KEY") {
        return Err(AppError::bad_request(
            "PKCS#12 bundle must contain both an X.509 certificate and a private key",
        ));
    }

    inspect_pem_bundle(
        &cert_pem,
        &key_pem,
        if ca_pem.contains("BEGIN CERTIFICATE") {
            &ca_pem
        } else {
            &cert_pem
        },
        &BASE64.encode(p12_bytes),
        false,
    )
}

/// Extracts X.509 metadata fields (`subject`, `issuer`, `serial`, `dates`, `fingerprint`) via `openssl x509`.
pub fn inspect_pem_bundle(
    cert_pem: &str,
    key_pem: &str,
    ca_pem: &str,
    p12_base64: &str,
    is_self_signed: bool,
) -> AppResult<EsignCertBundle> {
    let tmp = TempDir::new()?;
    let cert_path = tmp.path().join("cert.pem");
    std::fs::write(&cert_path, cert_pem)?;

    let info_out = Command::new("openssl")
        .args([
            "x509",
            "-in",
            cert_path.to_str().unwrap(),
            "-noout",
            "-subject",
            "-issuer",
            "-serial",
            "-dates",
            "-fingerprint",
            "-sha256",
        ])
        .output()?;

    let info_text = String::from_utf8_lossy(&info_out.stdout);
    let mut subject = String::from("CN=signline.local, O=Signline");
    let mut issuer = String::from("CN=signline.local, O=Signline");
    let mut serial = String::new();
    let mut not_before = String::new();
    let mut not_after = String::new();
    let mut fingerprint = String::new();

    for line in info_text.lines() {
        if let Some(rest) = line.strip_prefix("subject=") {
            subject = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("issuer=") {
            issuer = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("serial=") {
            serial = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("notBefore=") {
            not_before = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("notAfter=") {
            not_after = rest.trim().to_string();
        } else if line.contains("Fingerprint=") {
            if let Some((_, fp)) = line.split_once("Fingerprint=") {
                fingerprint = fp.trim().to_string();
            }
        }
    }

    let common_name = extract_dn_part(&subject, "CN").unwrap_or_else(|| "signline.local".to_string());
    let organization = extract_dn_part(&subject, "O").unwrap_or_else(|| "Signline".to_string());

    Ok(EsignCertBundle {
        cert_pem: cert_pem.to_string(),
        key_pem: key_pem.to_string(),
        ca_pem: ca_pem.to_string(),
        p12_base64: p12_base64.to_string(),
        subject,
        issuer,
        common_name,
        organization,
        serial,
        not_before,
        not_after,
        fingerprint_sha256: fingerprint,
        is_self_signed,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn extract_dn_part(dn: &str, key: &str) -> Option<String> {
    for part in dn.split(&[',', '/'][..]) {
        let trimmed = part.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim().eq_ignore_ascii_case(key) {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

/// Ensures the account has an active `EsignCertBundle` in `encrypted_configs`, generating
/// a default 2048-bit RSA self-signed certificate if none exists yet.
pub async fn ensure_account_cert(state: &AppState, account_id: &str) -> AppResult<EsignCertBundle> {
    if let Some(val) = state
        .pb
        .get_encrypted_config(&state.config, Some(account_id), ESIGN_CERTS_KEY)
        .await?
    {
        if let Ok(bundle) = serde_json::from_value::<EsignCertBundle>(val) {
            if !bundle.cert_pem.is_empty() && !bundle.key_pem.is_empty() {
                return Ok(bundle);
            }
        }
    }

    let bundle = generate_self_signed_bundle("Signline")?;
    let val = serde_json::to_value(&bundle)?;
    state
        .pb
        .set_encrypted_config(&state.config, account_id, ESIGN_CERTS_KEY, &val)
        .await?;

    Ok(bundle)
}
