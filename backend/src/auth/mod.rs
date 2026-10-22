pub mod password;
pub mod rbac;
pub mod totp;

use crate::error::{AppError, AppResult};
use crate::models::{
    account::{Account, AccountLinkedAccountRecord},
    user::{AccessTokenRecord, McpTokenRecord, User},
};
use crate::pb::pb_escape;
use crate::state::AppState;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub account_id: String,
    pub exp: usize,
    pub iat: usize,
}

/// Authenticated request context resolved from `Authorization: Bearer <jwt|mcp>`,
/// `X-Auth-Token: <api_key>`, or `signline_session=<jwt>` cookie.
/// Also handles `X-Account-Testing: true` sandbox mode switching.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user: User,
    pub account: Account,
    pub is_testing: bool,
}

pub fn issue_jwt(user: &User, secret: &str) -> AppResult<String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = JwtClaims {
        sub: user.id.clone(),
        account_id: user.account_id.clone(),
        iat: now,
        exp: now + 60 * 60 * 24 * 30, // 30 days
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("JWT encoding error: {}", e)))
}

pub fn verify_jwt(token: &str, secret: &str) -> Option<JwtClaims> {
    decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Provisions or fetches the linked sandbox testing account for `primary_account` when
/// `X-Account-Testing: true` is passed on API requests.
pub async fn resolve_testing_account(state: &AppState, primary: &Account) -> AppResult<Account> {
    let filter = format!(
        "account_id = '{}' && account_type = 'testing'",
        pb_escape(&primary.id)
    );
    if let Some(linked) = state
        .pb
        .find_first::<AccountLinkedAccountRecord>("account_linked_accounts", &filter, None)
        .await?
    {
        if let Ok(testing_acc) = state
            .pb
            .get_by_id::<Account>("accounts", &linked.linked_account_id)
            .await
        {
            return Ok(testing_acc);
        }
    }

    let testing_acc: Account = state
        .pb
        .create(
            "accounts",
            &json!({
                "uuid": Uuid::new_v4().to_string(),
                "name": format!("{} (Sandbox)", primary.name),
                "timezone": primary.timezone,
                "locale": primary.locale,
            }),
        )
        .await?;

    let _: AccountLinkedAccountRecord = state
        .pb
        .create(
            "account_linked_accounts",
            &json!({
                "account_id": primary.id,
                "linked_account_id": testing_acc.id,
                "account_type": "testing"
            }),
        )
        .await?;

    Ok(testing_acc)
}

/// Resolves an `AuthContext` from HTTP request headers.
pub async fn authenticate_parts(parts: &Parts, state: &AppState) -> AppResult<AuthContext> {
    let mut resolved_user: Option<User> = None;

    // 1. Check X-Auth-Token header (REST API authentication)
    if let Some(api_token) = parts
        .headers
        .get("X-Auth-Token")
        .or_else(|| parts.headers.get("x-auth-token"))
        .and_then(|v| v.to_str().ok())
    {
        let token_trimmed = api_token.trim();
        if !token_trimmed.is_empty() {
            let hash = sha256_hex(token_trimmed);
            let filter = format!("sha256 = '{}'", pb_escape(&hash));
            if let Some(rec) = state
                .pb
                .find_first::<AccessTokenRecord>("access_tokens", &filter, None)
                .await?
            {
                if let Ok(u) = state.pb.get_by_id::<User>("app_users", &rec.user_id).await {
                    if u.archived_at.is_empty() {
                        resolved_user = Some(u);
                    }
                }
            }
        }
    }

    // 2. Check Authorization: Bearer <token> (JWT, AccessToken, or McpToken)
    if resolved_user.is_none() {
        if let Some(auth_header) = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
        {
            let bearer = auth_header
                .strip_prefix("Bearer ")
                .or_else(|| auth_header.strip_prefix("bearer "))
                .unwrap_or(auth_header)
                .trim();

            if !bearer.is_empty() {
                if let Some(claims) = verify_jwt(bearer, &state.config.secret_key_base) {
                    if let Ok(u) = state.pb.get_by_id::<User>("app_users", &claims.sub).await {
                        if u.archived_at.is_empty() {
                            resolved_user = Some(u);
                        }
                    }
                } else {
                    let hash = sha256_hex(bearer);
                    let filter = format!("sha256 = '{}'", pb_escape(&hash));
                    if let Some(mcp_rec) = state
                        .pb
                        .find_first::<McpTokenRecord>("mcp_tokens", &filter, None)
                        .await?
                    {
                        if mcp_rec.archived_at.is_empty() {
                            if let Ok(u) = state
                                .pb
                                .get_by_id::<User>("app_users", &mcp_rec.user_id)
                                .await
                            {
                                if u.archived_at.is_empty() {
                                    resolved_user = Some(u);
                                }
                            }
                        }
                    } else if let Some(acc_rec) = state
                        .pb
                        .find_first::<AccessTokenRecord>("access_tokens", &filter, None)
                        .await?
                    {
                        if let Ok(u) = state
                            .pb
                            .get_by_id::<User>("app_users", &acc_rec.user_id)
                            .await
                        {
                            if u.archived_at.is_empty() {
                                resolved_user = Some(u);
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Check Cookie: signline_session=<jwt>
    if resolved_user.is_none() {
        if let Some(cookie_hdr) = parts
            .headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
        {
            for part in cookie_hdr.split(';') {
                let kv = part.trim();
                if let Some(jwt) = kv.strip_prefix("signline_session=") {
                    if let Some(claims) = verify_jwt(jwt.trim(), &state.config.secret_key_base) {
                        if let Ok(u) = state.pb.get_by_id::<User>("app_users", &claims.sub).await {
                            if u.archived_at.is_empty() {
                                resolved_user = Some(u);
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let user = resolved_user.ok_or_else(|| {
        AppError::unauthorized("Authentication required. Provide X-Auth-Token, Bearer token, or session cookie.")
    })?;

    let primary_account = state
        .pb
        .resolve_account(Some(&user.account_id))
        .await?
        .ok_or_else(|| AppError::not_found("Account not found"))?;

    let is_testing = parts
        .headers
        .get("X-Account-Testing")
        .or_else(|| parts.headers.get("x-account-testing"))
        .and_then(|v| v.to_str().ok())
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let account = if is_testing {
        resolve_testing_account(state, &primary_account).await?
    } else {
        primary_account
    };

    Ok(AuthContext {
        user,
        account,
        is_testing,
    })
}

#[async_trait]
impl FromRequestParts<AppState> for AuthContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        authenticate_parts(parts, state).await
    }
}
