use crate::auth::AuthContext;
use crate::error::AppResult;
use crate::models::{
    submission::{Submission, SubmissionEvent, Submitter},
    template::Template,
};
use crate::pb::pb_escape;
use crate::routes::{
    submissions_routes::{create_submission_internal, CreateSubmissionReq, SubmitterInput},
    templates_routes::{build_template_from_html_internal, find_template_by_id_or_slug, HtmlTemplateReq},
};
use crate::state::AppState;
use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<AppState> {
    Router::new().route("/mcp", get(mcp_info).post(handle_mcp_rpc))
}

async fn mcp_info() -> Json<Value> {
    Json(json!({
        "name": "signline-mcp-server",
        "version": "0.1.0b",
        "protocolVersion": "2024-11-05",
        "status": "ok"
    }))
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: Option<String>,
    pub id: Option<Value>,
    pub method: Option<String>,
    #[serde(default)]
    pub params: Value,
}

/// Implements the Model Context Protocol (MCP) JSON-RPC 2.0 server over `POST /mcp`,
/// supporting `initialize`, `notifications/initialized`, `ping`, `tools/list`, and `tools/call`
/// for all 7 Signline tools (`list_templates`, `get_template`, `create_submission`,
/// `list_submissions`, `get_submission`, `create_template_from_html`, `create_submission_from_html`).
async fn handle_mcp_rpc(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(rpc): Json<JsonRpcRequest>,
) -> AppResult<Json<Value>> {
    let req_id = rpc.id.unwrap_or(Value::Null);
    let method = rpc.method.as_deref().unwrap_or("");

    match method {
        "initialize" => Ok(Json(json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "signline-mcp",
                    "version": "0.1.0b"
                }
            }
        }))),

        "notifications/initialized" | "ping" => Ok(Json(json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {}
        }))),

        "tools/list" => Ok(Json(json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "tools": mcp_tool_schemas()
            }
        }))),

        "tools/call" => {
            let tool_name = rpc
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let args = rpc
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));

            match execute_mcp_tool(&state, &auth, tool_name, args).await {
                Ok(tool_output) => Ok(Json(json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": serde_json::to_string_pretty(&tool_output).unwrap_or_default()
                            }
                        ],
                        "data": tool_output,
                        "isError": false
                    }
                }))),
                Err(e) => Ok(Json(json!({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": format!("Error: {}", e)
                            }
                        ],
                        "isError": true
                    }
                }))),
            }
        }

        _ => Ok(Json(json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "error": {
                "code": -32601,
                "message": format!("Method '{}' not found", method)
            }
        }))),
    }
}

async fn execute_mcp_tool(
    state: &AppState,
    auth: &AuthContext,
    tool_name: &str,
    args: Value,
) -> AppResult<Value> {
    match tool_name {
        "list_templates" | "search_templates" => {
            let q = args.get("q").and_then(|v| v.as_str()).unwrap_or("");
            let mut filter = format!(
                "account_id = '{}' && archived_at = ''",
                pb_escape(&auth.account.id)
            );
            if !q.trim().is_empty() {
                filter.push_str(&format!(" && name ~ '{}'", pb_escape(q.trim())));
            }
            let list = state
                .pb
                .list_all::<Template>("templates", Some(&filter), Some("-created"))
                .await?;
            let items: Vec<Value> = list.into_iter().map(|t| t.to_api_json(None)).collect();
            Ok(json!({ "templates": items }))
        }

        "get_template" | "load_template" => {
            let id = args
                .get("id")
                .or_else(|| args.get("template_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let tpl = find_template_by_id_or_slug(state, &auth.account.id, id).await?;
            Ok(tpl.to_api_json(None))
        }

        "create_submission" | "send_documents" => {
            let template_id = args
                .get("template_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tpl = find_template_by_id_or_slug(state, &auth.account.id, &template_id).await?;

            let submitters_val = args
                .get("submitters")
                .cloned()
                .unwrap_or_else(|| json!([]));
            let submitters: Vec<SubmitterInput> =
                serde_json::from_value(submitters_val).unwrap_or_default();

            let (sub, subs, evs) = create_submission_internal(
                state,
                &auth.account.id,
                &auth.user.id,
                &tpl,
                CreateSubmissionReq {
                    template_id: tpl.id.clone(),
                    submitters,
                    send_email: args.get("send_email").and_then(|v| v.as_bool()),
                    order: args
                        .get("order")
                        .and_then(|v| v.as_str())
                        .map(String::from),
                    submitters_order: None,
                    expire_at: None,
                    variables: None,
                    preferences: None,
                    source: Some("mcp".to_string()),
                },
            )
            .await?;

            Ok(sub.to_api_json(&subs, &evs, &state.config.public_url))
        }

        "list_submissions" | "search_documents" => {
            let filter = format!(
                "account_id = '{}' && archived_at = ''",
                pb_escape(&auth.account.id)
            );
            let list = state
                .pb
                .list_all::<Submission>("submissions", Some(&filter), Some("-created"))
                .await?;
            let mut out = Vec::new();
            for s in list.into_iter().take(25) {
                let s_filter = format!("submission_id = '{}'", pb_escape(&s.id));
                let subs = state
                    .pb
                    .list_all::<Submitter>("submitters", Some(&s_filter), Some("created"))
                    .await?;
                out.push(s.to_api_json(&subs, &[], &state.config.public_url));
            }
            Ok(json!({ "submissions": out }))
        }

        "get_submission" => {
            let id = args
                .get("id")
                .or_else(|| args.get("submission_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let sub: Submission = state.pb.get_by_id("submissions", id).await?;
            let s_filter = format!("submission_id = '{}'", pb_escape(&sub.id));
            let subs = state
                .pb
                .list_all::<Submitter>("submitters", Some(&s_filter), Some("created"))
                .await?;
            let evs = state
                .pb
                .list_all::<SubmissionEvent>("submission_events", Some(&s_filter), Some("created"))
                .await?;
            Ok(sub.to_api_json(&subs, &evs, &state.config.public_url))
        }

        "create_template_from_html" | "create_template" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from);
            let html = args
                .get("html")
                .and_then(|v| v.as_str())
                .unwrap_or("<p>Document</p>")
                .to_string();
            let tpl = build_template_from_html_internal(
                state,
                auth,
                HtmlTemplateReq {
                    name,
                    html,
                    html_header: None,
                    folder_id: None,
                    external_id: None,
                },
            )
            .await?;
            Ok(tpl.to_api_json(None))
        }

        "create_submission_from_html" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .map(String::from);
            let html = args
                .get("html")
                .and_then(|v| v.as_str())
                .unwrap_or("<p>Agreement</p>")
                .to_string();
            let tpl = build_template_from_html_internal(
                state,
                auth,
                HtmlTemplateReq {
                    name,
                    html,
                    html_header: None,
                    folder_id: None,
                    external_id: None,
                },
            )
            .await?;

            let submitters_val = args
                .get("submitters")
                .cloned()
                .unwrap_or_else(|| json!([{ "email": auth.user.email, "name": auth.user.full_name() }]));
            let submitters: Vec<SubmitterInput> =
                serde_json::from_value(submitters_val).unwrap_or_default();

            let (sub, subs, evs) = create_submission_internal(
                state,
                &auth.account.id,
                &auth.user.id,
                &tpl,
                CreateSubmissionReq {
                    template_id: tpl.id.clone(),
                    submitters,
                    send_email: args.get("send_email").and_then(|v| v.as_bool()),
                    order: None,
                    submitters_order: None,
                    expire_at: None,
                    variables: None,
                    preferences: None,
                    source: Some("mcp".to_string()),
                },
            )
            .await?;
            Ok(sub.to_api_json(&subs, &evs, &state.config.public_url))
        }

        other => Err(crate::error::AppError::bad_request(format!(
            "Unknown MCP tool '{}'",
            other
        ))),
    }
}

fn mcp_tool_schemas() -> Vec<Value> {
    vec![
        json!({
            "name": "list_templates",
            "description": "List and search document templates in Signline",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "q": { "type": "string", "description": "Optional search query" }
                }
            }
        }),
        json!({
            "name": "get_template",
            "description": "Retrieve full schema, fields, and roles for a specific template",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Template ID or slug" }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "create_submission",
            "description": "Create and send a signature request submission from an existing template",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "template_id": { "type": "string" },
                    "submitters": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "email": { "type": "string" },
                                "role": { "type": "string" }
                            }
                        }
                    }
                },
                "required": ["template_id", "submitters"]
            }
        }),
        json!({
            "name": "list_submissions",
            "description": "List recent signature request submissions and their statuses",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "get_submission",
            "description": "Get detailed status, submitter values, and audit events for a submission",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Submission ID" }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "create_template_from_html",
            "description": "Create a new fillable PDF template from dynamic HTML content",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "html": { "type": "string" }
                },
                "required": ["name", "html"]
            }
        }),
        json!({
            "name": "create_submission_from_html",
            "description": "Create a template from HTML and immediately send a signature request",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "html": { "type": "string" },
                    "submitters": { "type": "array" }
                },
                "required": ["html", "submitters"]
            }
        }),
    ]
}
