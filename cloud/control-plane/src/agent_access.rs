//! Cloud agent access control.

use crate::audit;
use crate::auth::Principal;
use crate::db::Db;
use crate::error::{ApiError, ApiResult};
use axum::http::Method;
use uuid::Uuid;

/// Minimum share/member role required for a proxied HTTP method.
pub fn min_role_for_method(method: &Method) -> &'static str {
    if *method == Method::GET || *method == Method::HEAD {
        "viewer"
    } else {
        "operator"
    }
}

pub async fn assert_agent_access_for_method(
    db: &Db,
    principal: &Principal,
    agent_id: Uuid,
    method: &Method,
) -> ApiResult<()> {
    assert_agent_access(db, principal, agent_id, min_role_for_method(method)).await
}

pub async fn assert_agent_access(
    db: &Db,
    principal: &Principal,
    agent_id: Uuid,
    min_role: &str,
) -> ApiResult<()> {
    let row = sqlx::query_as::<_, (bool, Option<String>, Option<String>)>(
        "SELECT (a.owner_user_id = $2) AS is_owner,
                m.role AS member_role,
                s.role AS share_role
         FROM cloud_agents a
         LEFT JOIN organization_members m
           ON m.org_id = a.org_id AND m.user_id = $2
         LEFT JOIN cloud_agent_shares s
           ON s.agent_id = a.id AND s.user_id = $2 AND s.revoked_at IS NULL
         WHERE a.id = $1 AND a.deleted_at IS NULL",
    )
    .bind(agent_id)
    .bind(principal.user_id)
    .fetch_optional(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("Cloud agent not found"))?;
    let effective = effective_role(row.0, row.1.as_deref(), row.2.as_deref());
    if role_at_least(effective.as_deref(), min_role) {
        Ok(())
    } else {
        audit::log(
            db,
            Some(principal.org_id),
            Some(agent_id),
            Some(principal.user_id),
            "agent.access.denied",
            Some(serde_json::json!({ "required": min_role })),
        )
        .await;
        Err(ApiError::forbidden("Insufficient permission for this agent"))
    }
}

fn effective_role(is_owner: bool, member: Option<&str>, share: Option<&str>) -> Option<String> {
    if is_owner {
        return Some("owner".into());
    }
    let mut best = 0u8;
    let mut label = None;
    for r in [member, share].into_iter().flatten() {
        let rank = role_rank(r);
        if rank > best {
            best = rank;
            label = Some(r.to_string());
        }
    }
    label
}

fn role_rank(role: &str) -> u8 {
    match role {
        "owner" | "admin" => 3,
        "operator" => 2,
        "viewer" | "member" => 1,
        _ => 0,
    }
}

fn role_at_least(have: Option<&str>, need: &str) -> bool {
    role_rank(have.unwrap_or("")) >= role_rank(need)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_rank_orders_roles() {
        assert!(role_rank("owner") > role_rank("operator"));
        assert!(role_rank("admin") > role_rank("viewer"));
        assert!(role_rank("operator") > role_rank("member"));
        assert_eq!(role_rank("unknown"), 0);
    }

    #[test]
    fn effective_role_owner_wins() {
        assert_eq!(
            effective_role(true, Some("member"), Some("viewer")).as_deref(),
            Some("owner")
        );
    }

    #[test]
    fn effective_role_picks_highest() {
        assert_eq!(
            effective_role(false, Some("viewer"), Some("admin")).as_deref(),
            Some("admin")
        );
        assert_eq!(
            effective_role(false, Some("owner"), Some("operator")).as_deref(),
            Some("owner")
        );
        assert_eq!(
            effective_role(false, None, Some("viewer")).as_deref(),
            Some("viewer")
        );
        assert!(effective_role(false, None, None).is_none());
    }

    #[test]
    fn role_at_least_requires_minimum_rank() {
        assert!(role_at_least(Some("admin"), "operator"));
        assert!(!role_at_least(Some("viewer"), "operator"));
        assert!(!role_at_least(None, "viewer"));
    }
}
