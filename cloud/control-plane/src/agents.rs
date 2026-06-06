//! Cloud agent metadata and access control.

use crate::audit;
use crate::auth::Principal;
use crate::bootstrap_bundle::{
    self, resolve_bootstrap, AgentBootstrapBundle, CredentialSyncOptions,
};
use crate::db::Db;
use crate::entitlements;
use crate::engine_provision;
use crate::error::{ApiError, ApiResult};
use crate::runtime::{AgentProvisionConfig, RuntimeBackend};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

pub use crate::agent_access::assert_agent_access;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CloudAgentRow {
    pub id: Uuid,
    pub name: String,
    pub config_id: String,
    pub color: Option<String>,
    pub folder_path: String,
    pub created_at: DateTime<Utc>,
    pub last_opened_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAgent {
    pub id: String,
    pub name: String,
    pub folder_path: String,
    pub config_id: String,
    pub color: Option<String>,
    pub created_at: String,
    pub last_opened_at: Option<String>,
    pub runtime: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCloudAgent {
    pub name: String,
    pub config_id: String,
    pub color: Option<String>,
    pub claude_md: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub bootstrap_bundle: Option<AgentBootstrapBundle>,
    #[serde(default)]
    pub credential_sync: Option<CredentialSyncOptions>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchCloudAgent {
    pub name: Option<String>,
    pub color: Option<String>,
}

fn to_wire(row: CloudAgentRow) -> CloudAgent {
    CloudAgent {
        id: row.id.to_string(),
        name: row.name,
        folder_path: row.folder_path,
        config_id: row.config_id,
        color: row.color,
        created_at: row.created_at.to_rfc3339(),
        last_opened_at: row.last_opened_at.map(|t| t.to_rfc3339()),
        runtime: "cloud_24_7",
    }
}

const AGENT_SELECT: &str =
    "a.id, a.name, a.config_id, a.color, a.folder_path, a.created_at, a.last_opened_at";

pub async fn list_agents(db: &Db, principal: &Principal) -> ApiResult<Vec<CloudAgent>> {
    let query = format!(
        "SELECT {AGENT_SELECT}
         FROM cloud_agents a
         LEFT JOIN cloud_agent_shares s
           ON s.agent_id = a.id AND s.user_id = $2 AND s.revoked_at IS NULL
         WHERE a.deleted_at IS NULL
           AND (
             (a.org_id = $1 AND a.owner_user_id = $2)
             OR s.user_id IS NOT NULL
           )
         ORDER BY a.created_at DESC"
    );
    let rows = sqlx::query_as::<_, CloudAgentRow>(&query)
        .bind(principal.org_id)
        .bind(principal.user_id)
        .fetch_all(db.pool())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(rows.into_iter().map(to_wire).collect())
}

pub async fn list_shared_agents(db: &Db, principal: &Principal) -> ApiResult<Vec<CloudAgent>> {
    let query = format!(
        "SELECT {AGENT_SELECT}
         FROM cloud_agents a
         INNER JOIN cloud_agent_shares s
           ON s.agent_id = a.id AND s.user_id = $1 AND s.revoked_at IS NULL
         WHERE a.deleted_at IS NULL
           AND a.owner_user_id != $1
         ORDER BY a.created_at DESC"
    );
    let rows = sqlx::query_as::<_, CloudAgentRow>(&query)
        .bind(principal.user_id)
        .fetch_all(db.pool())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(rows.into_iter().map(to_wire).collect())
}

pub async fn create_agent(
    db: &Db,
    runtime: &dyn RuntimeBackend,
    principal: &Principal,
    body: CreateCloudAgent,
) -> ApiResult<CloudAgent> {
    entitlements::assert_can_create(db, principal).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("Agent name is required"));
    }
    let agent_id = Uuid::new_v4();
    let mut tx = db
        .pool()
        .begin()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    sqlx::query(
        "INSERT INTO cloud_agents (id, org_id, owner_user_id, name, config_id, color)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(agent_id)
    .bind(principal.org_id)
    .bind(principal.user_id)
    .bind(name)
    .bind(&body.config_id)
    .bind(&body.color)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    let bootstrap = resolve_bootstrap(
        name,
        &body.config_id,
        body.color.clone(),
        body.claude_md.clone(),
        body.provider.clone(),
        body.model.clone(),
        body.bootstrap_bundle.as_ref(),
    );
    let provision_config = AgentProvisionConfig { bootstrap };
    match runtime
        .provision(agent_id, principal.org_id, &provision_config)
        .await
    {
        Ok(runtime_row) => {
            if let Err(e) = sqlx::query("UPDATE cloud_agents SET folder_path = $2 WHERE id = $1")
                .bind(agent_id)
                .bind(&runtime_row.folder_path)
                .execute(&mut *tx)
                .await
            {
                if let Err(ce) = runtime.remove(agent_id).await {
                    tracing::warn!(
                        agent_id = %agent_id,
                        error = %ce,
                        "runtime cleanup after db persist failure"
                    );
                }
                tx.rollback()
                    .await
                    .map_err(|re| ApiError::internal(re.to_string()))?;
                return Err(ApiError::internal(e.to_string()));
            }
            if let Err(e) = sqlx::query(
                "INSERT INTO cloud_agent_runtimes
                 (agent_id, container_name, internal_url, token_hash, engine_token, status)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(agent_id)
            .bind(&runtime_row.container_name)
            .bind(&runtime_row.internal_url)
            .bind(&runtime_row.token_hash)
            .bind(&runtime_row.engine_token)
            .bind(&runtime_row.status)
            .execute(&mut *tx)
            .await
            {
                if let Err(ce) = runtime.remove(agent_id).await {
                    tracing::warn!(
                        agent_id = %agent_id,
                        error = %ce,
                        "runtime cleanup after db persist failure"
                    );
                }
                tx.rollback()
                    .await
                    .map_err(|re| ApiError::internal(re.to_string()))?;
                return Err(ApiError::internal(e.to_string()));
            }
            if let Some(sync) = &body.credential_sync {
                if let Err(e) = run_credential_sync(
                    db,
                    principal,
                    agent_id,
                    &runtime_row.internal_url,
                    &runtime_row.engine_token,
                    sync,
                )
                .await
                {
                    if let Err(ce) = runtime.remove(agent_id).await {
                        tracing::warn!(
                            agent_id = %agent_id,
                            error = %ce,
                            "runtime cleanup after credential sync failure"
                        );
                    }
                    tx.rollback()
                        .await
                        .map_err(|re| ApiError::internal(re.to_string()))?;
                    return Err(e);
                }
            }
            if let Err(e) = tx.commit().await {
                if let Err(ce) = runtime.remove(agent_id).await {
                    tracing::warn!(
                        agent_id = %agent_id,
                        error = %ce,
                        "runtime cleanup after db persist failure"
                    );
                }
                return Err(ApiError::internal(e.to_string()));
            }
            audit::log(
                db,
                Some(principal.org_id),
                Some(agent_id),
                Some(principal.user_id),
                "agent.bootstrap.applied",
                Some(bootstrap_bundle::bootstrap_audit_detail(
                    &provision_config.bootstrap.source,
                    provision_config.bootstrap.skills.len(),
                    provision_config.bootstrap.seeds.len(),
                )),
            )
            .await;
            audit::log(
                db,
                Some(principal.org_id),
                Some(agent_id),
                Some(principal.user_id),
                "agent.create",
                None,
            )
            .await;
            return Ok(CloudAgent {
                id: agent_id.to_string(),
                name: name.to_string(),
                folder_path: runtime_row.folder_path,
                config_id: body.config_id,
                color: body.color,
                created_at: Utc::now().to_rfc3339(),
                last_opened_at: None,
                runtime: "cloud_24_7",
            });
        }
        Err(e) => {
            tx.rollback()
                .await
                .map_err(|re| ApiError::internal(re.to_string()))?;
            audit::log(
                db,
                Some(principal.org_id),
                Some(agent_id),
                Some(principal.user_id),
                "agent.create.failed",
                Some(serde_json::json!({ "error": e.to_string() })),
            )
            .await;
            return Err(ApiError::internal(e.to_string()));
        }
    }
}

pub async fn patch_agent(
    db: &Db,
    principal: &Principal,
    agent_id: Uuid,
    body: PatchCloudAgent,
) -> ApiResult<CloudAgent> {
    assert_agent_access(db, principal, agent_id, "admin").await?;
    let name_update = match &body.name {
        None => None,
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(ApiError::bad_request("Agent name cannot be empty"));
            }
            Some(trimmed.to_string())
        }
    };
    let patched_name = body.name.is_some();
    let patched_color = body.color.is_some();
    if name_update.is_none() && body.color.is_none() {
        return Err(ApiError::bad_request("At least one of name or color is required"));
    }
    // Display metadata only: engine folder rename needs workspace + engine agent id
    // lookup and is not synced here. folder_path stays the engine path from provision.
    let row = sqlx::query_as::<_, CloudAgentRow>(
        "UPDATE cloud_agents SET
            name = COALESCE($2, name),
            color = COALESCE($3, color)
         WHERE id = $1 AND deleted_at IS NULL
         RETURNING id, name, config_id, color, folder_path, created_at, last_opened_at",
    )
    .bind(agent_id)
    .bind(name_update)
    .bind(body.color)
    .fetch_optional(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("Cloud agent not found"))?;
    audit::log(
        db,
        Some(principal.org_id),
        Some(agent_id),
        Some(principal.user_id),
        "agent.patch",
        Some(serde_json::json!({
            "name": patched_name,
            "color": patched_color,
        })),
    )
    .await;
    Ok(to_wire(row))
}

async fn run_credential_sync(
    db: &Db,
    principal: &Principal,
    agent_id: Uuid,
    internal_url: &str,
    engine_token: &str,
    sync: &CredentialSyncOptions,
) -> ApiResult<()> {
    assert_agent_access(db, principal, agent_id, "admin").await?;
    let provider = sync.provider.trim();
    if provider.is_empty() {
        return Err(ApiError::bad_request("credentialSync.provider is required"));
    }
    audit::log(
        db,
        Some(principal.org_id),
        Some(agent_id),
        Some(principal.user_id),
        "provider.credentials.sync.requested",
        Some(serde_json::json!({ "provider": provider })),
    )
    .await;
    match engine_provision::sync_provider_credentials(
        internal_url,
        engine_token,
        provider,
        &sync.import_body,
    )
    .await
    {
        Ok(()) => {
            audit::log(
                db,
                Some(principal.org_id),
                Some(agent_id),
                Some(principal.user_id),
                "provider.credentials.sync.ok",
                Some(bootstrap_bundle::credential_sync_audit_detail(
                    provider, true, Some(200), None,
                )),
            )
            .await;
            Ok(())
        }
        Err(e) => {
            let msg = e.to_string();
            audit::log(
                db,
                Some(principal.org_id),
                Some(agent_id),
                Some(principal.user_id),
                "provider.credentials.sync.failed",
                Some(bootstrap_bundle::credential_sync_audit_detail(
                    provider, false, None, Some(&msg),
                )),
            )
            .await;
            Err(ApiError::internal(msg))
        }
    }
}

pub async fn delete_agent(
    db: &Db,
    runtime: &dyn RuntimeBackend,
    principal: &Principal,
    agent_id: Uuid,
) -> ApiResult<()> {
    assert_agent_access(db, principal, agent_id, "admin").await?;
    runtime
        .remove(agent_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let updated = sqlx::query(
        "UPDATE cloud_agents SET deleted_at = now()
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(agent_id)
    .execute(db.pool())
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::not_found("Cloud agent not found"));
    }
    audit::log(
        db,
        Some(principal.org_id),
        Some(agent_id),
        Some(principal.user_id),
        "agent.delete",
        None,
    )
    .await;
    Ok(())
}
