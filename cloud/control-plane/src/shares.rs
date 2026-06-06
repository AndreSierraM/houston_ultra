//! Cloud agent sharing.

use crate::agents;
use crate::audit;
use crate::auth::Principal;
use crate::db::Db;
use crate::error::{ApiError, ApiResult};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ShareRow {
    pub user_id: Uuid,
    pub role: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertShare {
    pub user_id: Uuid,
    pub role: String,
}

pub async fn list_shares(
    db: &Db,
    principal: &Principal,
    agent_id: Uuid,
) -> ApiResult<Vec<ShareRow>> {
    agents::assert_agent_access(db, principal, agent_id, "admin").await?;
    let rows = sqlx::query_as::<_, ShareRow>(
        "SELECT user_id, role FROM cloud_agent_shares
         WHERE agent_id = $1 AND revoked_at IS NULL",
    )
    .bind(agent_id)
    .fetch_all(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(rows)
}

pub async fn upsert_share(
    db: &Db,
    principal: &Principal,
    agent_id: Uuid,
    body: UpsertShare,
) -> ApiResult<ShareRow> {
    agents::assert_agent_access(db, principal, agent_id, "admin").await?;
    if !matches!(body.role.as_str(), "viewer" | "operator" | "admin") {
        return Err(ApiError::bad_request("Invalid share role"));
    }
    sqlx::query(
        "INSERT INTO cloud_agent_shares (agent_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (agent_id, user_id) DO UPDATE
         SET role = EXCLUDED.role, revoked_at = NULL",
    )
    .bind(agent_id)
    .bind(body.user_id)
    .bind(&body.role)
    .execute(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    audit::log(
        db,
        Some(principal.org_id),
        Some(agent_id),
        Some(principal.user_id),
        "agent.share.upsert",
        Some(serde_json::json!({ "target": body.user_id, "role": body.role })),
    )
    .await;
    Ok(ShareRow {
        user_id: body.user_id,
        role: body.role,
    })
}

pub async fn revoke_share(
    db: &Db,
    principal: &Principal,
    agent_id: Uuid,
    user_id: Uuid,
) -> ApiResult<()> {
    agents::assert_agent_access(db, principal, agent_id, "admin").await?;
    sqlx::query(
        "UPDATE cloud_agent_shares SET revoked_at = now()
         WHERE agent_id = $1 AND user_id = $2",
    )
    .bind(agent_id)
    .bind(user_id)
    .execute(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    audit::log(
        db,
        Some(principal.org_id),
        Some(agent_id),
        Some(principal.user_id),
        "agent.share.revoke",
        Some(serde_json::json!({ "target": user_id })),
    )
    .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    fn valid_share_role(role: &str) -> bool {
        matches!(role, "viewer" | "operator" | "admin")
    }

    #[test]
    fn share_roles_are_viewer_operator_admin_only() {
        assert!(valid_share_role("viewer"));
        assert!(valid_share_role("operator"));
        assert!(valid_share_role("admin"));
        assert!(!valid_share_role("superuser"));
        assert!(!valid_share_role("owner"));
    }
}
