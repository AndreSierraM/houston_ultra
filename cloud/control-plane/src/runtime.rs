//! Runtime backend trait — Docker now, K3s later.

use async_trait::async_trait;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AgentProvisionConfig {
    pub name: String,
    pub config_id: String,
    pub color: Option<String>,
    pub claude_md: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
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

    async fn remove(&self, agent_id: Uuid) -> anyhow::Result<()>;

    async fn status(&self, agent_id: Uuid) -> anyhow::Result<String>;
}
