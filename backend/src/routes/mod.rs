pub mod auth_routes;
pub mod mcp_routes;
pub mod settings_routes;
pub mod signing_routes;
pub mod submissions_routes;
pub mod templates_routes;
pub mod verify_routes;

use crate::auth::AuthContext;
use crate::error::{AppError, AppResult};
use crate::models::{
    submission::{Submission, Submitter},
    template::Template,
};
use crate::pb::pb_escape;
use crate::services::{branding::resolve_branding, storage::read_blob_bytes};
use crate::state::AppState;
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::{Any, CorsLayer};

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/branding", get(get_public_branding))
        .route("/branding/logo", get(serve_branding_logo))
        .route("/branding/favicon", get(serve_branding_favicon))
        .route("/manifest.json", get(serve_manifest_json))
        .route("/api/blobs/:id", get(serve_storage_blob))
        .route("/api/search", get(unified_search))
        .merge(auth_routes::router())
        .merge(templates_routes::router())
        .merge(submissions_routes::router())
        .merge(signing_routes::router())
        .merge(settings_routes::router())
        .merge(verify_routes::router())
        .merge(mcp_routes::router())
        .fallback(spa_or_public_fallback)
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50 MB upload limit
        .layer(cors)
        .with_state(state)
}

async fn health_check(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "signline-backend",
        "version": "0.1.0b",
        "pocketbase_url": state.config.pb_url,
    }))
}

async fn get_public_branding(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let branding = resolve_branding(&state, None).await?;
    Ok(Json(branding))
}

async fn serve_branding_logo(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    if let Some(acc) = state.pb.resolve_account(None).await? {
        if !acc.logo_file.is_empty() {
            if let Ok((blob, bytes)) = read_blob_bytes(&state, &acc.logo_file).await {
                let mut headers = HeaderMap::new();
                headers.insert(
                    header::CONTENT_TYPE,
                    blob.content_type
                        .parse()
                        .unwrap_or_else(|_| "image/svg+xml".parse().unwrap()),
                );
                return Ok((StatusCode::OK, headers, bytes));
            }
        }
    }

    let default_logo = state.config.public_dir.join("logo.svg");
    let bytes = std::fs::read(&default_logo).unwrap_or_else(|_| {
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><rect width=\"100\" height=\"100\" rx=\"18\" fill=\"#2161AD\"/><text x=\"50\" y=\"65\" font-size=\"48\" text-anchor=\"middle\" fill=\"#fff\" font-family=\"sans-serif\">S</text></svg>".to_vec()
    });
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/svg+xml".parse().unwrap());
    Ok((StatusCode::OK, headers, bytes))
}

async fn serve_branding_favicon(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    if let Some(acc) = state.pb.resolve_account(None).await? {
        if !acc.favicon_file.is_empty() {
            if let Ok((blob, bytes)) = read_blob_bytes(&state, &acc.favicon_file).await {
                let mut headers = HeaderMap::new();
                headers.insert(
                    header::CONTENT_TYPE,
                    blob.content_type
                        .parse()
                        .unwrap_or_else(|_| "image/png".parse().unwrap()),
                );
                return Ok((StatusCode::OK, headers, bytes));
            }
        }
    }

    let default_fav = state.config.public_dir.join("favicon.svg");
    let bytes = std::fs::read(&default_fav).unwrap_or_default();
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/svg+xml".parse().unwrap());
    Ok((StatusCode::OK, headers, bytes))
}

async fn serve_manifest_json(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let branding = resolve_branding(&state, None).await?;
    let name = branding["product_name"].as_str().unwrap_or("Signline");
    let primary = branding["primary_color"].as_str().unwrap_or("#2161AD");

    Ok(Json(json!({
        "name": name,
        "short_name": name,
        "start_url": "/",
        "display": "standalone",
        "background_color": "#F5F8FC",
        "theme_color": primary,
        "icons": [
            {
                "src": "/branding/favicon",
                "sizes": "any",
                "type": "image/svg+xml"
            },
            {
                "src": "/favicon-96x96.png",
                "sizes": "96x96",
                "type": "image/png"
            }
        ]
    })))
}

async fn serve_storage_blob(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let (blob, bytes) = read_blob_bytes(&state, &id).await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        blob.content_type
            .parse()
            .unwrap_or_else(|_| "application/octet-stream".parse().unwrap()),
    );
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600".parse().unwrap(),
    );
    Ok((StatusCode::OK, headers, bytes))
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

async fn unified_search(
    State(state): State<AppState>,
    auth: AuthContext,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Value>> {
    let q = query.q.unwrap_or_default();
    let qt = q.trim();
    if qt.is_empty() {
        return Ok(Json(json!({
            "templates": [],
            "submissions": [],
            "submitters": [],
        })));
    }

    let tpl_filter = format!(
        "account_id = '{}' && archived_at = '' && name ~ '{}'",
        pb_escape(&auth.account.id),
        pb_escape(qt)
    );
    let templates = state
        .pb
        .list_all::<Template>("templates", Some(&tpl_filter), Some("-created"))
        .await?;

    let sub_filter = format!(
        "account_id = '{}' && archived_at = '' && name ~ '{}'",
        pb_escape(&auth.account.id),
        pb_escape(qt)
    );
    let submissions = state
        .pb
        .list_all::<Submission>("submissions", Some(&sub_filter), Some("-created"))
        .await?;

    let submitter_filter = format!(
        "account_id = '{}' && (name ~ '{}' || email ~ '{}')",
        pb_escape(&auth.account.id),
        pb_escape(qt),
        pb_escape(qt)
    );
    let submitters = state
        .pb
        .list_all::<Submitter>("submitters", Some(&submitter_filter), Some("-created"))
        .await?;

    Ok(Json(json!({
        "templates": templates.into_iter().take(10).map(|t| t.to_api_json(None)).collect::<Vec<_>>(),
        "submissions": submissions.into_iter().take(10).collect::<Vec<_>>(),
        "submitters": submitters.into_iter().take(10).map(|s| s.to_api_json(&state.config.public_url)).collect::<Vec<_>>(),
    })))
}

/// Serves static files from `/public` or `/frontend/dist`, falling back to `index.html` for SPA routes.
async fn spa_or_public_fallback(State(state): State<AppState>, uri: Uri) -> Response {
    let req_path = uri.path().trim_start_matches('/');

    // Return JSON 404 for unknown `/api/*` routes
    if req_path.starts_with("api/") {
        return AppError::not_found(format!("API endpoint '/{}' not found", req_path))
            .into_response();
    }

    if !req_path.is_empty() && !req_path.contains("..") {
        let dist_candidate = state.config.frontend_dist_dir.join(req_path);
        if dist_candidate.is_file() {
            if let Ok(bytes) = std::fs::read(&dist_candidate) {
                let mime = mime_guess::from_path(&dist_candidate).first_or_octet_stream();
                let mut headers = HeaderMap::new();
                headers.insert(header::CONTENT_TYPE, mime.as_ref().parse().unwrap());
                return (StatusCode::OK, headers, bytes).into_response();
            }
        }

        let public_candidate = state.config.public_dir.join(req_path);
        if public_candidate.is_file() {
            if let Ok(bytes) = std::fs::read(&public_candidate) {
                let mime = mime_guess::from_path(&public_candidate).first_or_octet_stream();
                let mut headers = HeaderMap::new();
                headers.insert(header::CONTENT_TYPE, mime.as_ref().parse().unwrap());
                return (StatusCode::OK, headers, bytes).into_response();
            }
        }
    }

    // Serve SPA index.html if built
    let index_path = state.config.frontend_dist_dir.join("index.html");
    if index_path.is_file() {
        if let Ok(html) = std::fs::read_to_string(&index_path) {
            return Html(html).into_response();
        }
    }

    // Fallback HTML shell with mandatory Signline attribution
    Html(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>Signline — Electronic Document Signing</title>
  <link rel="icon" href="/branding/favicon" />
</head>
<body style="font-family: sans-serif; background: #F5F8FC; color: #333333; padding: 40px;">
  <h1>Signline v0.1.0b</h1>
  <p>Backend API &amp; PocketBase persistence engine active.</p>
  <footer style="margin-top: 40px; font-size: 13px; color: #64748b;">
    Powered by <strong>Signline</strong>, based on <a href="https://github.com/signline/signline" target="_blank" rel="noopener noreferrer">Signline</a>
  </footer>
</body>
</html>"#,
    )
    .into_response()
}
