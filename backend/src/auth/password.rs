use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use crate::error::{AppError, AppResult};

/// Hashes a user password with Argon2id (while retaining Bcrypt verification compatibility
/// for existing Bcrypt password hashes).
pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("Password hashing error: {}", e)))
}

/// Verifies a plaintext password against either an Argon2id or Bcrypt hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    if hash.starts_with("$argon2") {
        if let Ok(parsed) = PasswordHash::new(hash) {
            return Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok();
        }
        false
    } else if hash.starts_with("$2") {
        bcrypt::verify(password, hash).unwrap_or(false)
    } else {
        false
    }
}
