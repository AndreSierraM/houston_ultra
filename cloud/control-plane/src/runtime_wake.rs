//! Ensure a cloud agent pod is running before proxy traffic reaches it.

use crate::agents;
use crate::db::Db;
use crate::error::{ApiError, ApiResult};
use crate::runtime::RuntimeBackend;
use uuid::Uuid;

pub async fn ensure_agent_awake(
    db: &Db,
    runtime: &dyn RuntimeBackend,
    agent_id: Uuid,
) -> ApiResult<()> {
    let status = runtime
        .status(agent_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    match status.as_str() {
        "running" => Ok(()),
        "stopped" => {
            runtime
                .start(agent_id)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            agents::update_runtime_status(db, agent_id, "running").await?;
            Ok(())
        }
        "provisioning" => {
            runtime
                .start(agent_id)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            agents::update_runtime_status(db, agent_id, "running").await?;
            Ok(())
        }
        "error" => {
            let row = agents::load_runtime_wake_row(db, agent_id).await?;
            runtime
                .reconcile_workload(agent_id, row.org_id, &row.engine_token)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            agents::update_runtime_status(db, agent_id, "running").await?;
            Ok(())
        }
        other => Err(ApiError::internal(format!(
            "cloud agent {agent_id} runtime status {other} is not reachable"
        ))),
    }
}
