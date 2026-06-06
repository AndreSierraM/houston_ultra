//! `POST /v1/agents/bootstrap-bundle` — export cloud bootstrap payload.
//!
//! MVP callers use `installedPath` (Store template). `agentPath` is reserved
//! for future local → cloud migration and is not used by the desktop UI today.

use crate::routes::error::ApiError;
use crate::state::ServerState;
use axum::{routing::post, Json, Router};
use houston_engine_core::bootstrap::build_bootstrap_bundle;
use houston_engine_protocol::{AgentBootstrapBundle, BuildBootstrapBundleRequest};
use std::sync::Arc;

pub fn router() -> Router<Arc<ServerState>> {
    Router::new().route("/agents/bootstrap-bundle", post(build))
}

async fn build(
    Json(req): Json<BuildBootstrapBundleRequest>,
) -> Result<Json<AgentBootstrapBundle>, ApiError> {
    Ok(Json(build_bootstrap_bundle(req)?))
}
