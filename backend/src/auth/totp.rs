use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha1 = Hmac<Sha1>;

const BASE32_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Encodes raw bytes into RFC 4648 Base32 (without padding) for TOTP authenticator apps.
pub fn base32_encode(data: &[u8]) -> String {
    let mut result = String::new();
    let mut buffer: u32 = 0;
    let mut bits_left: usize = 0;

    for &byte in data {
        buffer = (buffer << 8) | (byte as u32);
        bits_left += 8;
        while bits_left >= 5 {
            let index = ((buffer >> (bits_left - 5)) & 0x1F) as usize;
            result.push(BASE32_ALPHABET[index] as char);
            bits_left -= 5;
        }
    }

    if bits_left > 0 {
        let index = ((buffer << (5 - bits_left)) & 0x1F) as usize;
        result.push(BASE32_ALPHABET[index] as char);
    }

    result
}

/// Decodes an RFC 4648 Base32 string into raw secret bytes.
pub fn base32_decode(encoded: &str) -> Option<Vec<u8>> {
    let mut buffer: u32 = 0;
    let mut bits_left: usize = 0;
    let mut result = Vec::new();

    for c in encoded.trim().chars() {
        if c == '=' || c == ' ' || c == '-' {
            continue;
        }
        let val = match c.to_ascii_uppercase() {
            'A'..='Z' => (c.to_ascii_uppercase() as u8 - b'A') as u32,
            '2'..='7' => (c as u8 - b'2' + 26) as u32,
            _ => return None,
        };
        buffer = (buffer << 5) | val;
        bits_left += 5;
        if bits_left >= 8 {
            let byte = ((buffer >> (bits_left - 8)) & 0xFF) as u8;
            result.push(byte);
            bits_left -= 8;
        }
    }

    Some(result)
}

/// Generates a new 160-bit random TOTP secret encoded in Base32.
pub fn generate_totp_secret() -> String {
    let mut bytes = [0u8; 20];
    rand::thread_rng().fill_bytes(&mut bytes);
    base32_encode(&bytes)
}

/// Builds a standard `otpauth://totp/` provisioning URI for QR code scanners.
pub fn provisioning_uri(secret: &str, email: &str, issuer: &str) -> String {
    let encoded_issuer = urlencoding::encode(issuer);
    let encoded_email = urlencoding::encode(email);
    format!(
        "otpauth://totp/{}:{}?secret={}&issuer={}&algorithm=SHA1&digits=6&period=30",
        encoded_issuer, encoded_email, secret, encoded_issuer
    )
}

/// Computes the 6-digit RFC 6238 TOTP code for a specific 30-second timestep.
pub fn compute_totp_at(secret: &str, timestep: u64) -> Option<String> {
    let key = base32_decode(secret)?;
    let counter = timestep.to_be_bytes();
    let mut mac = HmacSha1::new_from_slice(&key).ok()?;
    mac.update(&counter);
    let result = mac.finalize().into_bytes();
    let offset = (result[19] & 0x0f) as usize;
    let binary = ((result[offset] as u32 & 0x7f) << 24)
        | ((result[offset + 1] as u32) << 16)
        | ((result[offset + 2] as u32) << 8)
        | (result[offset + 3] as u32);
    let otp = binary % 1_000_000;
    Some(format!("{:06}", otp))
}

/// Verifies a 6-digit TOTP code allowing +/- 1 timestep drift (30s window).
/// Returns the matched timestep so callers can update `consumed_timestep` against replay.
pub fn verify_totp(secret: &str, code: &str, last_consumed: i64) -> Option<i64> {
    let clean_code = code.trim().replace(' ', "");
    if clean_code.len() != 6 {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let current_step = now / 30;

    for offset in [-1i64, 0, 1] {
        let step = (current_step as i64) + offset;
        if step <= last_consumed {
            continue;
        }
        if let Some(expected) = compute_totp_at(secret, step as u64) {
            if expected == clean_code {
                return Some(step);
            }
        }
    }
    None
}
