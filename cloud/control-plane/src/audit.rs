//! Audit event persistence.

use crate::db::Db;
use serde_json::Value;
use uuid::Uuid;

pub async fn log(
    db: &Db,
    org_id: Option<Uuid>,
    agent_id: Option<Uuid>,
    user_id: Option<Uuid>,
    action: &str,
    detail: Option<Value>,
) {
    let result = sqlx::query(
        "INSERT INTO audit_events (org_id, agent_id, user_id, action, detail)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(org_id)
    .bind(agent_id)
    .bind(user_id)
    .bind(action)
    .bind(detail)
    .execute(db.pool())
    .await;
    if let Err(e) = result {
        tracing::error!(action, error = %e, "failed to write audit event");
    }
}
