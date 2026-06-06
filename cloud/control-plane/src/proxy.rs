//! REST proxy from control plane to private cloud engines.

use crate::agent_access::assert_agent_access_for_method;
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
    entitlements::assert_active(&state.db, &principal).await?;
    let (parts, body) = req.into_parts();
    let method = parts.method.clone();
    assert_agent_access_for_method(&state.db, &principal, agent_id, &method).await?;
    let runtime = load_runtime(&state.db, agent_id).await?;
    // HoustonClient calls `{proxy}/v1/...` — tail already includes `v1/...`.
    let target = build_proxy_target(&runtime.internal_url, &tail);
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut forward = state.proxy.http.request(method, &target);
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
    audit::log(
        &state.db,
        Some(principal.org_id),
        Some(agent_id),
        Some(principal.user_id),
        "agent.proxy.rest",
        Some(serde_json::json!({ "path": tail })),
    )
    .await;
    let status = StatusCode::from_u16(resp.status().as_u16())
        .unwrap_or(StatusCode::BAD_GATEWAY);
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

/// Join engine base URL with proxied tail path (HoustonClient sends `v1/...` in tail).
pub fn build_proxy_target(internal_url: &str, tail: &str) -> String {
    format!(
        "{}/{}",
        internal_url.trim_end_matches('/'),
        tail.trim_start_matches('/')
    )
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
