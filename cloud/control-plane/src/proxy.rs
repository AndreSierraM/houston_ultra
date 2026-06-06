//! REST proxy from control plane to private cloud engines.

use crate::agent_access::{assert_agent_access_for_proxy, is_credential_route};
use crate::audit;
use crate::auth::Principal;
use crate::db::Db;
use crate::entitlements;
use crate::error::{ApiError, ApiResult};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ProxyState {
    pub http: reqwest::Client,
}

#[derive(sqlx::FromRow)]
struct RuntimeTarget {
    internal_url: String,
    engine_token: String,
}

pub async fn proxy_rest(
    State(state): State<Arc<crate::state::AppState>>,
    principal: Principal,
    Path((agent_id, tail)): Path<(Uuid, String)>,
    req: Request<Body>,
) -> ApiResult<Response> {
    let tail = tail.trim_start_matches('/').to_string();
    if tail.is_empty() {
        return Err(ApiError::bad_request("Invalid proxy path"));
    }
    entitlements::assert_active(&state.db, &principal).await?;
    let (parts, body) = req.into_parts();
    let method = parts.method.clone();
    let sensitive = is_credential_route(&tail);
    assert_agent_access_for_proxy(&state.db, &principal, agent_id, &tail, &method).await?;
    let runtime = load_runtime(&state.db, agent_id).await?;
    // HoustonClient calls `{proxy}/v1/...` — tail already includes `v1/...`.
    let target = build_proxy_target(&runtime.internal_url, &tail);
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut forward = state.proxy.http.request(method.clone(), &target);
    forward = forward.header("Authorization", format!("Bearer {}", runtime.engine_token));
    for (name, value) in parts.headers.iter() {
        let n = name.as_str();
        if n == "host" || n == "authorization" || n == "connection" {
            continue;
        }
        forward = forward.header(name, value);
    }
    let resp = forward
        .body(body_bytes)
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("Engine proxy failed: {e}")))?;
    let status_code = resp.status().as_u16();
    let audit_action = if sensitive {
        "agent.proxy.credentials"
    } else {
        "agent.proxy.rest"
    };
    let audit_detail = if sensitive {
        serde_json::json!({
            "provider": credential_route_provider(&tail),
            "route": credential_route_kind(&tail),
            "statusCode": status_code,
        })
    } else {
        serde_json::json!({ "path": tail })
    };
    audit::log(
        &state.db,
        Some(principal.org_id),
        Some(agent_id),
        Some(principal.user_id),
        audit_action,
        Some(audit_detail),
    )
    .await;
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = Response::builder().status(status);
    if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE) {
        builder = builder.header(axum::http::header::CONTENT_TYPE, ct.as_bytes());
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    builder
        .body(Body::from(bytes))
        .map_err(|e| ApiError::internal(e.to_string()))
}

fn credential_route_provider(tail: &str) -> Option<&str> {
    let rest = tail.strip_prefix("v1/providers/")?;
    rest.split('/').next()
}

fn credential_route_kind(tail: &str) -> &'static str {
    if tail.contains("/credential-import/session") {
        "import-session"
    } else if tail.contains("/credential-import") {
        "import"
    } else if tail.contains("/credential-export") {
        "export"
    } else {
        "unknown"
    }
}

/// Suffix after `/agents/{id}/proxy/` in the incoming URI.
///
/// Accepts the full client path (via [`OriginalUri`]) or the prefix-stripped path
/// that axum passes to nested `fallback` handlers.
pub fn proxy_tail(agent_id: Uuid, path: &str) -> Option<&str> {
    let marker = format!("/agents/{agent_id}/proxy");
    if let Some((_, rest)) = path.split_once(&marker) {
        let tail = rest.trim_start_matches('/');
        return if tail.is_empty() { None } else { Some(tail) };
    }
    let stripped = path.trim_start_matches('/');
    if stripped.is_empty() || stripped.starts_with("agents/") {
        None
    } else {
        Some(stripped)
    }
}

/// Join engine base URL with proxied tail path (HoustonClient sends `v1/...` in tail).
pub fn build_proxy_target(internal_url: &str, tail: &str) -> String {
    format!(
        "{}/{}",
        internal_url.trim_end_matches('/'),
        tail.trim_start_matches('/')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_tail_full_cloud_path() {
        let agent_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let path = format!("/v1/cloud/agents/{agent_id}/proxy/v1/health");
        assert_eq!(proxy_tail(agent_id, &path), Some("v1/health"));
    }

    #[test]
    fn proxy_tail_cloud_router_path() {
        let agent_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let path = format!("/agents/{agent_id}/proxy/v1/sessions/abc");
        assert_eq!(proxy_tail(agent_id, &path), Some("v1/sessions/abc"));
    }

    #[test]
    fn proxy_tail_nested_stripped_path() {
        let agent_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(proxy_tail(agent_id, "/v1/health"), Some("v1/health"));
        assert_eq!(proxy_tail(agent_id, "v1/workspaces"), Some("v1/workspaces"));
    }

    #[test]
    fn proxy_tail_rejects_empty_and_wrong_agent() {
        let agent_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let other = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
        assert_eq!(proxy_tail(agent_id, "/"), None);
        assert_eq!(proxy_tail(agent_id, ""), None);
        let bare = format!("/v1/cloud/agents/{agent_id}/proxy");
        assert_eq!(proxy_tail(agent_id, &bare), None);
        let path = format!("/agents/{other}/proxy/v1/health");
        assert_eq!(proxy_tail(agent_id, &path), None);
    }

    #[test]
    fn credential_route_helpers_redact_provider_only() {
        let tail = "v1/providers/anthropic/credential-import";
        assert_eq!(credential_route_provider(tail), Some("anthropic"));
        assert_eq!(credential_route_kind(tail), "import");
    }
}

async fn load_runtime(db: &Db, agent_id: Uuid) -> ApiResult<RuntimeTarget> {
    sqlx::query_as::<_, RuntimeTarget>(
        "SELECT internal_url, engine_token FROM cloud_agent_runtimes WHERE agent_id = $1",
    )
    .bind(agent_id)
    .fetch_optional(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("Runtime not provisioned"))
}
