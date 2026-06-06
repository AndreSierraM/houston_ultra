//! REST proxy from control plane to private cloud engines.

use crate::agent_access::{assert_agent_access_for_proxy, is_credential_route};
use crate::audit;
use crate::auth::Principal;
use crate::db::Db;
use crate::entitlements;
use crate::error::{ApiError, ApiResult};
use crate::runtime_wake;
use axum::{
    body::Body,
    extract::{OriginalUri, Path, State},
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
    Path((agent_id, _)): Path<(Uuid, String)>,
    OriginalUri(original_uri): OriginalUri,
    req: Request<Body>,
) -> ApiResult<Response> {
    let tail = extract_proxy_tail(agent_id, original_uri.path())?;
    entitlements::assert_active(&state.db, &principal).await?;
    let (parts, body) = req.into_parts();
    let method = parts.method.clone();
    let sensitive = is_credential_route(&tail);
    assert_agent_access_for_proxy(&state.db, &principal, agent_id, &tail, &method).await?;
    runtime_wake::ensure_agent_awake(&state.db, state.runtime.as_ref(), agent_id).await?;
    let runtime = load_runtime(&state.db, agent_id).await?;
    // HoustonClient calls `{proxy}/v1/...` — tail already includes `v1/...`.
    let target = append_proxy_query(
        &build_proxy_target(&runtime.internal_url, &tail),
        original_uri.query(),
    );
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
    let resp = forward.body(body_bytes).send().await.map_err(|e| {
        tracing::error!(%agent_id, path = %tail, error = %e, "Engine proxy request failed");
        ApiError::internal("Engine proxy failed")
    })?;
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

/// Join engine base URL with proxied tail path (HoustonClient sends `v1/...` in tail).
pub fn build_proxy_target(internal_url: &str, tail: &str) -> String {
    format!(
        "{}/{}",
        internal_url.trim_end_matches('/'),
        tail.trim_start_matches('/')
    )
}

/// Append the incoming URI query string to a proxied engine URL.
pub fn append_proxy_query(target: &str, query: Option<&str>) -> String {
    match query {
        Some(q) if !q.is_empty() => format!("{target}?{q}"),
        _ => target.to_string(),
    }
}

const PROXY_PREFIX: &str = "/v1/cloud/agents/";

/// Tail path after `/v1/cloud/agents/{id}/proxy/` from the original request URI.
pub fn extract_proxy_tail(agent_id: Uuid, original_path: &str) -> ApiResult<String> {
    let marker = format!("{PROXY_PREFIX}{agent_id}/proxy/");
    let tail = original_path
        .strip_prefix(&marker)
        .ok_or_else(|| ApiError::bad_request("Invalid proxy path"))?
        .trim_start_matches('/');
    if tail.is_empty() {
        return Err(ApiError::bad_request("Invalid proxy path"));
    }
    Ok(repair_decoded_agent_path_in_tail(tail))
}

/// Axum's wildcard tail decodes `%2F` into `/`, splitting one engine path segment
/// into many. Re-encode the agent_path portion for `/v1/agents/{path}/sessions…`.
pub fn repair_decoded_agent_path_in_tail(tail: &str) -> String {
    const PREFIX: &str = "v1/agents/";
    let Some(rest) = tail.strip_prefix(PREFIX) else {
        return tail.to_string();
    };
    let Some((agent_part, suffix)) = rest.split_once("/sessions") else {
        return tail.to_string();
    };
    if !agent_part.contains('/') {
        return tail.to_string();
    }
    let normalized = normalize_absolute_agent_path(agent_part);
    let encoded = encode_path_segment(&normalized);
    format!("{PREFIX}{encoded}/sessions{suffix}")
}

fn normalize_absolute_agent_path(part: &str) -> String {
    let collapsed = part
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    format!("/{collapsed}")
}

fn encode_path_segment(path: &str) -> String {
    path.replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn credential_route_helpers_redact_provider_only() {
        let tail = "v1/providers/anthropic/credential-import";
        assert_eq!(credential_route_provider(tail), Some("anthropic"));
        assert_eq!(credential_route_kind(tail), "import");
    }

    #[test]
    fn append_proxy_query_preserves_engine_path() {
        let base = build_proxy_target("http://engine:7777", "v1/sessions");
        assert_eq!(
            append_proxy_query(&base, Some("limit=10&cursor=abc")),
            "http://engine:7777/v1/sessions?limit=10&cursor=abc"
        );
        assert_eq!(append_proxy_query(&base, None), base);
        assert_eq!(append_proxy_query(&base, Some("")), base);
    }

    #[test]
    fn repair_decoded_session_tail_reencodes_agent_path() {
        let tail = "v1/agents//data/workspace/Cloud/Test2/sessions";
        assert_eq!(
            repair_decoded_agent_path_in_tail(tail),
            "v1/agents/%2Fdata%2Fworkspace%2FCloud%2FTest2/sessions"
        );
    }

    #[test]
    fn extract_proxy_tail_from_original_uri_path() {
        let agent_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let path = format!(
            "/v1/cloud/agents/{agent_id}/proxy/v1/agents/%2Fdata%2Fws%2Fagent/sessions"
        );
        let tail = extract_proxy_tail(agent_id, &path).expect("tail");
        assert_eq!(tail, "v1/agents/%2Fdata%2Fws%2Fagent/sessions");
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
