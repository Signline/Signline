use crate::auth::{
    issue_jwt,
    password::{hash_password, verify_password},
     sha256_hex,
    totp::{generate_totp_secret, provisioning_uri, verify_totp},
    AuthContext,
};
use crate::crypto::cert::ensure_account_cert;
use crate::error::{AppError, AppResult};
use crate::models::{
    account::Account,
    template::TemplateFolder,
    user::{AccessTokenRecord, User},
};
use crate::pb::pb_escape;
use crate::services::branding::resolve_branding;
use crate::state::AppState;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/setup/status", get(setup_status))
        .route("/api/setup", post(perform_setup))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(current_user))
        .route("/api/auth/password/reset", post(request_password_reset))
        .route("/api/auth/password/reset", put(confirm_password_reset))
        .route("/api/auth/2fa/setup", post(setup_2fa))
        .route("/api/auth/2fa/verify", post(verify_2fa))
        .route("/api/auth/2fa", delete(disable_2fa))
}

async fn setup_status(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let has_user = state.pb.has_any_user().await?;
    let branding = resolve_branding(&state, None).await?;
    Ok(Json(json!({
        "needs_setup": !has_user,
        "branding": branding,
        "version": "0.1.0b"
    })))
}

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
    #[serde(default)]
    pub account_name: String,
    #[serde(default)]
    pub timezone: String,
    #[serde(default)]
    pub locale: String,
}

async fn perform_setup(
    State(state): State<AppState>,
    Json(req): Json<SetupRequest>,
) -> AppResult<impl IntoResponse> {
    if state.pb.has_any_user().await? {
        return Err(AppError::bad_request(
            "Initial setup has already been completed. Please sign in.",
        ));
    }

    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::bad_request("A valid email address is required"));
    }
    if req.password.len() < 6 {
        return Err(AppError::bad_request(
            "Password must be at least 6 characters long",
        ));
    }

    let account_name = if req.account_name.trim().is_empty() {
        "Signline Organization".to_string()
    } else {
        req.account_name.trim().to_string()
    };

    let timezone = if req.timezone.trim().is_empty() {
        "UTC".to_string()
    } else {
        req.timezone.trim().to_string()
    };

    let locale = if req.locale.trim().is_empty() {
        "en".to_string()
    } else {
        req.locale.trim().to_string()
    };

    // 1. Create initial Account
    let account: Account = state
        .pb
        .create(
            "accounts",
            &json!({
                "uuid": Uuid::new_v4().to_string(),
                "name": account_name,
                "timezone": timezone,
                "locale": locale,
            }),
        )
        .await?;

    // 2. Create initial Admin User
    let password_hash = hash_password(&req.password)?;
    let now_str = chrono::Utc::now().to_rfc3339();
    let user: User = state
        .pb
        .create(
            "app_users",
            &json!({
                "uuid": Uuid::new_v4().to_string(),
                "account_id": account.id,
                "email": email,
                "password_hash": password_hash,
                "first_name": if req.first_name.trim().is_empty() { "Admin" } else { req.first_name.trim() },
                "last_name": req.last_name.trim(),
                "role": "admin",
                "otp_required_for_login": false,
                "sign_in_count": 1,
                "current_sign_in_at": now_str,
                "last_sign_in_at": now_str,
                "current_sign_in_ip": "127.0.0.1",
            }),
        )
        .await?;

    // 3. Create AccountAccess link
    let _: Value = state
        .pb
        .create(
            "account_accesses",
            &json!({
                "account_id": account.id,
                "user_id": user.id,
            }),
        )
        .await?;

    // 4. Create Default TemplateFolder
    let default_folder: TemplateFolder = state
        .pb
        .create(
            "template_folders",
            &json!({
                "account_id": account.id,
                "author_id": user.id,
                "name": "Default",
                "parent_folder_id": "",
            }),
        )
        .await?;

    // 5. Generate initial REST API AccessToken (`X-Auth-Token`)
    let raw_api_token = format!("sl_live_{}", Uuid::new_v4().simple());
    let token_enc = state
        .config
        .encrypt(&raw_api_token)
        .map_err(AppError::Internal)?;
    let _: AccessTokenRecord = state
        .pb
        .create(
            "access_tokens",
            &json!({
                "user_id": user.id,
                "token_encrypted": token_enc,
                "sha256": sha256_hex(&raw_api_token),
            }),
        )
        .await?;

    // 6. Provision default 2048-bit RSA X.509 signing certificate
    let cert_bundle = ensure_account_cert(&state, &account.id).await?;

    // 7. Send Welcome Email to the initial administrator
    let _ = crate::services::email::send_email(
        &state,
        &account.id,
        &user.email,
        &format!("Welcome to {} on Signline!", account.name),
        &format!(
            "<p>Hello {},</p><p>Welcome to <strong>{}</strong> on Signline! Your electronic document signing workspace and 2048-bit RSA X.509 signing certificate have been provisioned.</p><p>Dashboard URL: <a href=\"{}\">{}</a></p>",
            if user.first_name.is_empty() { &user.email } else { &user.first_name },
            account.name,
            state.config.public_url,
            state.config.public_url
        ),
        "welcome_email",
        "User",
        &user.id,
    )
    .await;

    let jwt = issue_jwt(&user, &state.config.secret_key_base)?;
    let branding = resolve_branding(&state, Some(&account)).await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        format!(
            "signline_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
            jwt
        )
        .parse()
        .unwrap(),
    );

    Ok((
        StatusCode::CREATED,
        headers,
        Json(json!({
            "token": jwt,
            "api_token": raw_api_token,
            "user": user.to_public_json(),
            "account": account,
            "default_folder": default_folder,
            "certificate": cert_bundle.to_public_json(None),
            "branding": branding,
        })),
    ))
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub otp_attempt: Option<String>,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<impl IntoResponse> {
    let email = req.email.trim().to_lowercase();
    let filter = format!("email = '{}' && archived_at = ''", pb_escape(&email));
    let user = state
        .pb
        .find_first::<User>("app_users", &filter, None)
        .await?
        .ok_or_else(|| AppError::unauthorized("Invalid email or password"))?;

    if !verify_password(&req.password, &user.password_hash) {
        let _ = state
            .pb
            .update::<User>(
                "app_users",
                &user.id,
                &json!({ "failed_attempts": user.failed_attempts + 1 }),
            )
            .await;
        return Err(AppError::unauthorized("Invalid email or password"));
    }

    // Verify 2FA TOTP if enabled on this user
    let mut consumed_step = user.consumed_timestep;
    if user.otp_required_for_login && !user.otp_secret.is_empty() {
        let otp_code = req.otp_attempt.as_deref().unwrap_or("").trim();
        if otp_code.is_empty() {
            return Ok((
                StatusCode::OK,
                HeaderMap::new(),
                Json(json!({
                    "otp_required": true,
                    "message": "Two-factor authentication code required"
                })),
            ));
        }
        if let Some(step) = verify_totp(&user.otp_secret, otp_code, user.consumed_timestep) {
            consumed_step = step;
        } else {
            return Err(AppError::unauthorized("Invalid two-factor authentication code"));
        }
    }

    let now_str = chrono::Utc::now().to_rfc3339();
    let updated_user: User = state
        .pb
        .update(
            "app_users",
            &user.id,
            &json!({
                "failed_attempts": 0,
                "consumed_timestep": consumed_step,
                "sign_in_count": user.sign_in_count + 1,
                "last_sign_in_at": if user.current_sign_in_at.is_empty() { &now_str } else { &user.current_sign_in_at },
                "current_sign_in_at": now_str,
            }),
        )
        .await?;

    let account = state
        .pb
        .resolve_account(Some(&updated_user.account_id))
        .await?
        .ok_or_else(|| AppError::not_found("Account not found"))?;

    let jwt = issue_jwt(&updated_user, &state.config.secret_key_base)?;
    let branding = resolve_branding(&state, Some(&account)).await?;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        format!(
            "signline_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
            jwt
        )
        .parse()
        .unwrap(),
    );

    Ok((
        StatusCode::OK,
        headers,
        Json(json!({
            "token": jwt,
            "user": updated_user.to_public_json(),
            "account": account,
            "branding": branding,
        })),
    ))
}

async fn logout() -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        "signline_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            .parse()
            .unwrap(),
    );
    (StatusCode::OK, headers, Json(json!({ "ok": true })))
}

async fn current_user(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let branding = resolve_branding(&state, Some(&auth.account)).await?;
    Ok(Json(json!({
        "user": auth.user.to_public_json(),
        "account": auth.account,
        "is_testing": auth.is_testing,
        "branding": branding,
    })))
}

#[derive(Debug, Deserialize)]
pub struct PasswordResetReq {
    pub email: String,
}

async fn request_password_reset(
    State(state): State<AppState>,
    Json(req): Json<PasswordResetReq>,
) -> AppResult<Json<Value>> {
    let email = req.email.trim().to_lowercase();
    let filter = format!("email = '{}' && archived_at = ''", pb_escape(&email));
    let mut generated_token = None;
    if let Some(user) = state.pb.find_first::<User>("app_users", &filter, None).await? {
        let reset_token = Uuid::new_v4().simple().to_string();
        let now_str = chrono::Utc::now().to_rfc3339();
        let _ = state
            .pb
            .update::<User>(
                "app_users",
                &user.id,
                &json!({
                    "reset_password_token": reset_token,
                    "reset_password_sent_at": now_str,
                }),
            )
            .await;

        let _ = crate::services::email::send_email(
            &state,
            &user.account_id,
            &user.email,
            "Reset your Signline password",
            &format!(
                "<p>Use the following password reset token to set a new password:</p><p><code>{}</code></p>",
                reset_token
            ),
            "password_reset",
            "User",
            &user.id,
        )
        .await;
        generated_token = Some(reset_token);
    }

    Ok(Json(json!({
        "ok": true,
        "reset_token": generated_token,
        "message": "If that email exists, password reset instructions have been sent."
    })))
}

#[derive(Debug, Deserialize)]
pub struct ConfirmPasswordResetReq {
    pub token: String,
    pub password: String,
}

async fn confirm_password_reset(
    State(state): State<AppState>,
    Json(req): Json<ConfirmPasswordResetReq>,
) -> AppResult<Json<Value>> {
    if req.token.trim().is_empty() || req.password.len() < 6 {
        return Err(AppError::bad_request(
            "Valid reset token and password (min 6 chars) are required",
        ));
    }
    let filter = format!("reset_password_token = '{}'", pb_escape(req.token.trim()));
    let user = state
        .pb
        .find_first::<User>("app_users", &filter, None)
        .await?
        .ok_or_else(|| AppError::bad_request("Invalid or expired password reset token"))?;

    let password_hash = hash_password(&req.password)?;
    let _: User = state
        .pb
        .update(
            "app_users",
            &user.id,
            &json!({
                "password_hash": password_hash,
                "reset_password_token": "",
            }),
        )
        .await?;

    let _ = crate::services::email::send_email(
        &state,
        &user.account_id,
        &user.email,
        "Your Signline password has been changed",
        "<p>Your Signline account password was recently reset. If you did not perform this action, please contact your administrator immediately.</p>",
        "password_reset_confirmed",
        "User",
        &user.id,
    )
    .await;

    Ok(Json(json!({ "ok": true })))
}

async fn setup_2fa(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let secret = generate_totp_secret();
    let _: User = state
        .pb
        .update(
            "app_users",
            &auth.user.id,
            &json!({
                "otp_secret": secret,
            }),
        )
        .await?;

    let uri = provisioning_uri(&secret, &auth.user.email, "Signline");
    Ok(Json(json!({
        "otp_secret": secret,
        "provisioning_uri": uri,
    })))
}

#[derive(Debug, Deserialize)]
pub struct Verify2faReq {
    pub code: String,
}

async fn verify_2fa(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(req): Json<Verify2faReq>,
) -> AppResult<Json<Value>> {
    let fresh_user: User = state.pb.get_by_id("app_users", &auth.user.id).await?;
    if fresh_user.otp_secret.is_empty() {
        return Err(AppError::bad_request(
            "Call /api/auth/2fa/setup first to generate a TOTP secret",
        ));
    }

    let step = verify_totp(&fresh_user.otp_secret, &req.code, fresh_user.consumed_timestep)
        .ok_or_else(|| AppError::bad_request("Invalid 6-digit verification code"))?;

    let updated: User = state
        .pb
        .update(
            "app_users",
            &fresh_user.id,
            &json!({
                "otp_required_for_login": true,
                "consumed_timestep": step,
            }),
        )
        .await?;

    Ok(Json(json!({
        "ok": true,
        "otp_required_for_login": updated.otp_required_for_login,
        "user": updated.to_public_json()
    })))
}

async fn disable_2fa(
    State(state): State<AppState>,
    auth: AuthContext,
) -> AppResult<Json<Value>> {
    let updated: User = state
        .pb
        .update(
            "app_users",
            &auth.user.id,
            &json!({
                "otp_required_for_login": false,
                "otp_secret": "",
            }),
        )
        .await?;

    Ok(Json(json!({
        "ok": true,
        "otp_required_for_login": false,
        "user": updated.to_public_json()
    })))
}
