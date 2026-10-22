pub mod auth;
pub mod config;
pub mod crypto;
pub mod error;
pub mod models;
pub mod pb;
pub mod pdf;
pub mod routes;
pub mod services;
pub mod state;

use crate::config::AppConfig;
use crate::pb::PocketBaseClient;
use crate::state::AppState;
use std::time::Duration;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn load_env_file_if_present(path: &str) {
    if let Ok(contents) = std::fs::read_to_string(path) {
        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"').trim_matches('\'');
                if std::env::var(key).is_err() {
                    std::env::set_var(key, val);
                }
            }
        }
    }
}

async fn handle_cli_command(args: &[String]) -> anyhow::Result<bool> {
    if args.len() < 2 {
        return Ok(false);
    }

    match args[1].as_str() {
        "--list-users" => {
            load_env_file_if_present("/etc/signline/signline.env");
            let config = AppConfig::from_env();
            let pb = PocketBaseClient::connect_or_spawn(&config).await?;
            let users = pb
                .list_all::<crate::models::user::User>("app_users", None, Some("created"))
                .await?;
            let out: Vec<serde_json::Value> = users
                .into_iter()
                .map(|u| {
                    serde_json::json!({
                        "id": u.id,
                        "email": u.email,
                        "first_name": u.first_name,
                        "last_name": u.last_name,
                        "role": u.role,
                        "archived_at": u.archived_at,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string(&out)?);
            Ok(true)
        }
        "--verify-login" => {
            if args.len() < 4 {
                eprintln!("Usage: signline-server --verify-login <email> <password>");
                std::process::exit(2);
            }
            let email = args[2].trim().to_lowercase();
            let password = &args[3];
            load_env_file_if_present("/etc/signline/signline.env");
            let config = AppConfig::from_env();
            let pb = PocketBaseClient::connect_or_spawn(&config).await?;
            let filter = format!("email = '{}'", crate::pb::pb_escape(&email));
            let user_opt = pb
                .find_first::<crate::models::user::User>("app_users", &filter, None)
                .await?;
            if let Some(user) = user_opt {
                if crate::auth::password::verify_password(password, &user.password_hash) {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "email": user.email,
                            "role": user.role,
                            "name": format!("{} {}", user.first_name, user.last_name).trim()
                        })
                    );
                    std::process::exit(0);
                }
            }
            eprintln!("Invalid email or password");
            std::process::exit(1);
        }
        "--reset-password" => {
            if args.len() < 4 {
                eprintln!("Usage: signline-server --reset-password <email> <new_password>");
                std::process::exit(2);
            }
            let email = args[2].trim().to_lowercase();
            let new_password = &args[3];
            if email.is_empty() || new_password.len() < 6 {
                eprintln!("Error: Valid email and password (at least 6 characters) required.");
                std::process::exit(2);
            }
            load_env_file_if_present("/etc/signline/signline.env");
            let config = AppConfig::from_env();
            let pb = PocketBaseClient::connect_or_spawn(&config).await?;
            let hashed = crate::auth::password::hash_password(new_password)
                .map_err(|e| anyhow::anyhow!("Password hash error: {}", e))?;

            let filter = format!("email = '{}'", crate::pb::pb_escape(&email));
            if let Some(existing) = pb
                .find_first::<crate::models::user::User>("app_users", &filter, None)
                .await?
            {
                let _: serde_json::Value = pb
                    .update(
                        "app_users",
                        &existing.id,
                        &serde_json::json!({
                            "password_hash": hashed,
                            "failed_attempts": 0,
                            "locked_at": "",
                        }),
                    )
                    .await?;
                println!("Password successfully reset for existing user '{}'", email);
            } else {
                // Ensure an account exists, or create one
                let accounts = pb
                    .list_all::<crate::models::account::Account>("accounts", None, Some("created"))
                    .await?;
                let account_id = if let Some(acc) = accounts.first() {
                    acc.id.clone()
                } else {
                    let org_name = std::env::var("INITIAL_ORG_NAME")
                        .unwrap_or_else(|_| "Signline Organization".to_string());
                    let created_acc: crate::models::account::Account = pb
                        .create(
                            "accounts",
                            &serde_json::json!({
                                "name": org_name,
                                "timezone": "UTC",
                                "locale": "en",
                                "uuid": uuid::Uuid::new_v4().to_string(),
                            }),
                        )
                        .await?;
                    created_acc.id
                };
                let local_part = email.split('@').next().unwrap_or("Admin");
                let first_name = {
                    let mut c = local_part.chars();
                    match c.next() {
                        None => "Admin".to_string(),
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    }
                };
                let _: serde_json::Value = pb
                    .create(
                        "app_users",
                        &serde_json::json!({
                            "account_id": account_id,
                            "first_name": first_name,
                            "last_name": "Admin",
                            "email": email,
                            "role": "admin",
                            "password_hash": hashed,
                            "uuid": uuid::Uuid::new_v4().to_string(),
                            "sign_in_count": 0,
                            "failed_attempts": 0,
                            "otp_required_for_login": false,
                        }),
                    )
                    .await?;
                println!(
                    "Created new administrator user '{}' and set password",
                    email
                );
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if handle_cli_command(&args).await? {
        return Ok(());
    }

    load_env_file_if_present("/etc/signline/signline.env");

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "signline_backend=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();
    info!("Starting Signline v0.1.0b Rust Backend...");

    let pb = PocketBaseClient::connect_or_spawn(&config).await?;
    let state = AppState::new(config.clone(), pb);

    // Spawn periodic background worker for Automated Signature Request Reminders
    let worker_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) =
                crate::services::reminders::process_pending_reminders(&worker_state, false).await
            {
                error!("Reminder scheduler background tick error: {}", e);
            }
        }
    });

    let app = routes::build_router(state);
    let bind_addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("Signline v0.1.0b listening on http://{}", bind_addr);

    // Also listen on port 80 when running on a non-80 port if port 80 is available,
    // allowing reverse proxies / Cloudflare tunnels targeting :80 to reach Signline seamlessly.
    if config.port != 80 && std::env::var("DISABLE_PORT_80").unwrap_or_default() != "1" {
        let addr_80 = format!("{}:80", config.host);
        if let Ok(listener_80) = tokio::net::TcpListener::bind(&addr_80).await {
            info!("Signline v0.1.0b secondary HTTP listener active on http://{}", addr_80);
            let app_80 = app.clone();
            tokio::spawn(async move {
                if let Err(e) = axum::serve(listener_80, app_80).await {
                    error!("Secondary port 80 listener stopped: {}", e);
                }
            });
        }
    }

    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{
        password::{hash_password, verify_password},
        totp::{compute_totp_at, generate_totp_secret, verify_totp},
    };
    use crate::crypto::{
        cert::generate_self_signed_bundle,
        signer::sign_pdf_detached,
        verifier::extract_and_verify_pdf_signatures,
    };
    use crate::pdf::generator::build_simple_pdf;
    use crate::services::{
        branding::{content_color, hex_to_hsl, is_valid_hex_color},
        reminders::duration_seconds_for_key,
        webhooks::compute_webhook_hmac,
    };

    #[test]
    fn test_aes_gcm_encryption_roundtrip() {
        let cfg = AppConfig::from_env();
        let secret_json = serde_json::json!({
            "client_id": "test-client.apps.googleusercontent.com",
            "refresh_token": "1//test-refresh-token"
        });
        let encrypted = cfg.encrypt_json(&secret_json).expect("encrypt");
        let decrypted = cfg.decrypt_json(&encrypted).expect("decrypt");
        assert_eq!(decrypted, secret_json);
    }

    #[test]
    fn test_password_hashing_and_totp() {
        let hash = hash_password("SignlinePass123!").expect("hash");
        assert!(verify_password("SignlinePass123!", &hash));
        assert!(!verify_password("WrongPassword", &hash));

        let secret = generate_totp_secret();
        let now_step = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs())
            / 30;
        let code = compute_totp_at(&secret, now_step).expect("totp code");
        assert_eq!(code.len(), 6);
        let matched = verify_totp(&secret, &code, 0);
        assert!(matched.is_some());
    }

    #[test]
    fn test_branding_hsl_and_luminance_contrast() {
        assert!(is_valid_hex_color("#2161AD"));
        assert!(is_valid_hex_color("#fff"));
        assert!(!is_valid_hex_color("not-a-color"));

        // Dark brand blue (#2161AD) requires light foreground (#ffffff)
        assert_eq!(content_color("#2161AD"), "#ffffff");
        // Light pastel yellow (#FEF08A) requires dark foreground (#1f2937)
        assert_eq!(content_color("#FEF08A"), "#1f2937");

        let hsl = hex_to_hsl("#2161AD");
        assert!(hsl.contains('%'));
    }

    #[test]
    fn test_reminder_durations_and_webhook_hmac() {
        assert_eq!(duration_seconds_for_key("one_hour"), Some(3600));
        assert_eq!(duration_seconds_for_key("two_days"), Some(2 * 86400));
        assert_eq!(duration_seconds_for_key("thirty_days"), Some(30 * 86400));
        assert_eq!(duration_seconds_for_key("invalid_key"), None);

        let sig = compute_webhook_hmac("whsec_test", b"{\"event_type\":\"form.completed\"}");
        assert_eq!(sig.len(), 64);
    }

    #[test]
    fn test_pdf_generation_and_x509_pkcs7_signature_verification() {
        let cert = generate_self_signed_bundle("Signline").expect("generate RSA 2048 X.509 cert");
        assert!(cert.cert_pem.contains("BEGIN CERTIFICATE"));
        assert_eq!(cert.common_name, "signline.local");

        let raw_pdf = build_simple_pdf(
            "Master Services Agreement",
            &["Party A and Party B agree to the terms.".to_string()],
        );
        assert!(raw_pdf.starts_with(b"%PDF"));

        let (signed_pdf, sha256) = sign_pdf_detached(
            &raw_pdf,
            &cert,
            "Test E-Signature",
            "Signline Test Suite",
            None,
        )
        .expect("sign pdf with detached PKCS#7");
        assert_eq!(sha256.len(), 64);

        let verified_list = extract_and_verify_pdf_signatures(&signed_pdf, Some(&cert));
        assert_eq!(verified_list.len(), 1);
        assert!(verified_list[0].verified);
        assert!(verified_list[0].trusted);
        assert!(!verified_list[0].has_unsigned_changes);
    }
}
