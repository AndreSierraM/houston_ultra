//! HTTP route handlers.

use crate::agents::{self, CreateCloudAgent};
use crate::auth::Principal;
use crate::entitlements;
use crate::error::ApiResult;
use crate::shares::{self, UpsertShare};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MeResponse {
    user_id: String,
    email: Option<String>,
    org_id: String,
    org_role: String,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/me", get(me))
        .route("/entitlements", get(entitlements_route))
        .route("/agents", get(list_agents).post(create_agent))
        .route("/agents/shared", get(list_shared_agents))
        .route("/agents/:id", patch(patch_agent).delete(delete_agent))
        .route("/agents/:id/status", get(agent_status))
        .route("/agents/:id/restart", post(restart_agent))
        .route("/agents/:id/shares", get(list_shares).post(upsert_share))
        .route("/agents/:id/shares/:user_id", delete(revoke_share))
        .route(
            "/agents/:id/proxy/{*tail}",
            axum::routing::any(crate::proxy::proxy_rest),
        )
        .route("/agents/:id/ws", get(crate::ws_proxy::proxy_ws))
        .with_state(state)
}

async fn me(principal: Principal) -> Json<MeResponse> {
    Json(MeResponse {
        user_id: principal.user_id.to_string(),
        email: principal.email,
        org_id: principal.org_id.to_string(),
        org_role: principal.org_role,
    })
}

async fn entitlements_route(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> ApiResult<Json<serde_json::Value>> {
    let ent = entitlements::get_entitlement(&state.db, &principal).await?;
    Ok(Json(serde_json::to_value(ent).unwrap()))
}

async fn list_agents(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> ApiResult<Json<Vec<agents::CloudAgent>>> {
    Ok(Json(agents::list_agents(&state.db, &principal).await?))
}

async fn list_shared_agents(
    State(state): State<Arc<AppState>>,
    principal: Principal,
) -> ApiResult<Json<Vec<agents::CloudAgent>>> {
    Ok(Json(agents::list_shared_agents(&state.db, &principal).await?))
}

async fn create_agent(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Json(body): Json<CreateCloudAgent>,
) -> ApiResult<Json<agents::CloudAgent>> {
    let agent = agents::create_agent(&state.db, state.runtime.as_ref(), &principal, body).await?;
    Ok(Json(agent))
}

async fn patch_agent(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(agent_id): Path<Uuid>,
    Json(body): Json<agents::PatchCloudAgent>,
) -> ApiResult<Json<agents::CloudAgent>> {
    Ok(Json(
        agents::patch_agent(&state.db, &principal, agent_id, body).await?,
    ))
}

async fn delete_agent(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(agent_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    agents::delete_agent(&state.db, state.runtime.as_ref(), &principal, agent_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn agent_status(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(agent_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    agents::assert_agent_access(&state.db, &principal, agent_id, "viewer").await?;
    let status = state.runtime.status(agent_id).await.map_err(|e| {
        crate::error::ApiError::internal(e.to_string())
    })?;
    Ok(Json(serde_json::json!({ "status": status })))
}

async fn restart_agent(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(agent_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    agents::assert_agent_access(&state.db, &principal, agent_id, "admin").await?;
    state
        .runtime
        .restart(agent_id)
        .await
        .map_err(|e| crate::error::ApiError::internal(e.to_string()))?;
    crate::audit::log(
        &state.db,
        Some(principal.org_id),
        Some(agent_id),
        Some(principal.user_id),
        "agent.restart",
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn list_shares(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(agent_id): Path<Uuid>,
) -> ApiResult<Json<Vec<shares::ShareRow>>> {
    Ok(Json(shares::list_shares(&state.db, &principal, agent_id).await?))
}

async fn upsert_share(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path(agent_id): Path<Uuid>,
    Json(body): Json<UpsertShare>,
) -> ApiResult<Json<shares::ShareRow>> {
    Ok(Json(
        shares::upsert_share(&state.db, &principal, agent_id, body).await?,
    ))
}

async fn revoke_share(
    State(state): State<Arc<AppState>>,
    principal: Principal,
    Path((agent_id, user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    shares::revoke_share(&state.db, &principal, agent_id, user_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
