//! Runtime backend trait — Docker now, K3s later.

use crate::bootstrap_bundle::ResolvedBootstrap;
use async_trait::async_trait;
use serde::Serialize;
use uuid::Uuid;

/// Org-level K8s ResourceQuota inputs (from `cloud_entitlements`).
#[derive(Debug, Clone, Copy)]
pub struct OrgResourceQuota {
    pub max_cloud_agents: i32,
    pub max_storage_gb: i32,
}

#[derive(Debug, Clone)]
pub struct AgentProvisionConfig {
    pub bootstrap: ResolvedBootstrap,
    pub org_quota: OrgResourceQuota,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeRow {
    pub container_name: String,
    pub internal_url: String,
    pub token_hash: String,
    pub engine_token: String,
    pub status: String,
    pub folder_path: String,
}

#[async_trait]
pub trait RuntimeBackend: Send + Sync {
    async fn provision(
        &self,
        agent_id: Uuid,
        org_id: Uuid,
        agent: &AgentProvisionConfig,
    ) -> anyhow::Result<RuntimeRow>;

    async fn restart(&self, agent_id: Uuid) -> anyhow::Result<()>;

    async fn stop(&self, agent_id: Uuid) -> anyhow::Result<()>;

    async fn start(&self, agent_id: Uuid) -> anyhow::Result<()>;

    async fn remove(&self, agent_id: Uuid) -> anyhow::Result<()>;

    async fn status(&self, agent_id: Uuid) -> anyhow::Result<String>;
}
