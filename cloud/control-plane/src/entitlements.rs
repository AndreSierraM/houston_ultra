//! Subscription entitlement checks.

use crate::auth::Principal;
use crate::db::Db;
use crate::error::{ApiError, ApiResult};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Entitlement {
    pub org_id: Uuid,
    pub status: String,
    pub max_cloud_agents: i32,
    pub max_storage_gb: i32,
    pub max_members: i32,
}

pub async fn get_entitlement(db: &Db, principal: &Principal) -> ApiResult<Entitlement> {
    sqlx::query_as::<_, Entitlement>(
        "SELECT org_id, status, max_cloud_agents, max_storage_gb, max_members
         FROM cloud_entitlements WHERE org_id = $1",
    )
    .bind(principal.org_id)
    .fetch_optional(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("Entitlement not found"))
}

pub async fn assert_active(db: &Db, principal: &Principal) -> ApiResult<Entitlement> {
    let ent = get_entitlement(db, principal).await?;
    assert_subscription_active(&ent)?;
    Ok(ent)
}

/// Pure subscription gate used by [`assert_active`].
pub(crate) fn assert_subscription_active(ent: &Entitlement) -> ApiResult<()> {
    if ent.status == "active" {
        return Ok(());
    }
    Err(ApiError::forbidden(inactive_subscription_message(&ent.status)))
}

fn inactive_subscription_message(status: &str) -> &'static str {
    match status {
        "canceled" => "Subscription canceled; cloud agent access is unavailable",
        "past_due" => "Subscription past due; cloud agent access is unavailable",
        _ => "Cloud agents require an active subscription",
    }
}

pub async fn assert_can_create(db: &Db, principal: &Principal) -> ApiResult<Entitlement> {
    let ent = assert_active(db, principal).await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cloud_agents WHERE org_id = $1 AND deleted_at IS NULL",
    )
    .bind(principal.org_id)
    .fetch_one(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    assert_agent_quota(&ent, count)?;
    Ok(ent)
}

/// Pure quota gate used by [`assert_can_create`].
pub(crate) fn assert_agent_quota(ent: &Entitlement, active_agent_count: i64) -> ApiResult<()> {
    if active_agent_count >= ent.max_cloud_agents as i64 {
        return Err(ApiError::forbidden(format!(
            "Cloud agent limit reached ({})",
            ent.max_cloud_agents
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use houston_engine_protocol::ErrorCode;
    use uuid::Uuid;

    fn sample_entitlement(status: &str, max: i32) -> Entitlement {
        Entitlement {
            org_id: Uuid::new_v4(),
            status: status.into(),
            max_cloud_agents: max,
            max_storage_gb: 10,
            max_members: 5,
        }
    }

    #[test]
    fn assert_agent_quota_allows_under_limit() {
        let ent = sample_entitlement("active", 3);
        assert!(assert_agent_quota(&ent, 2).is_ok());
    }

    #[test]
    fn assert_agent_quota_blocks_at_limit() {
        let ent = sample_entitlement("active", 2);
        let err = assert_agent_quota(&ent, 2).unwrap_err();
        assert_eq!(err.status, StatusCode::FORBIDDEN);
        assert_eq!(err.code, ErrorCode::Forbidden);
        assert!(err.message.contains("limit reached"));
    }

    #[test]
    fn assert_subscription_active_blocks_past_due() {
        let ent = sample_entitlement("past_due", 5);
        let err = assert_subscription_active(&ent).unwrap_err();
        assert_eq!(err.status, StatusCode::FORBIDDEN);
        assert!(err.message.contains("past due"));
    }
}
