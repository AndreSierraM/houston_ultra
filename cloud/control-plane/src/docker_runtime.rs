//! Docker runtime — one private houston-engine container per cloud agent.

use crate::auth::hash_token;
use crate::engine_provision::{self, AgentBootstrapConfig};
use crate::runtime::{AgentProvisionConfig, RuntimeBackend, RuntimeRow};
use async_trait::async_trait;
use rand::RngCore;
use std::process::Stdio;
use tokio::process::Command;
use uuid::Uuid;

#[derive(Clone)]
pub struct DockerRuntime {
    pub engine_image: String,
    pub docker_socket: String,
}

impl DockerRuntime {
    fn container_name(agent_id: Uuid) -> String {
        format!("hou-cloud-agent-{agent_id}")
    }

    fn volume_name(agent_id: Uuid) -> String {
        format!("hou-cloud-agent-{agent_id}-home")
    }

    fn network_name(org_id: Uuid) -> String {
        format!("hou-org-{org_id}")
    }

    async fn docker(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = Command::new("docker")
            .env("DOCKER_HOST", &self.docker_socket)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("docker {} failed: {stderr}", args.join(" "));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn ensure_network(&self, org_id: Uuid) -> anyhow::Result<()> {
        let name = Self::network_name(org_id);
        match self.docker(&["network", "create", &name]).await {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("already exists") => Ok(()),
            Err(e) => {
                tracing::error!(org_id = %org_id, network = %name, error = %e, "docker network create failed");
                Err(e)
            }
        }
    }

    fn random_token() -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[async_trait]
impl RuntimeBackend for DockerRuntime {
    async fn provision(
        &self,
        agent_id: Uuid,
        org_id: Uuid,
        agent: &AgentProvisionConfig,
    ) -> anyhow::Result<RuntimeRow> {
        self.ensure_network(org_id).await?;
        let container = Self::container_name(agent_id);
        let volume = Self::volume_name(agent_id);
        let network = Self::network_name(org_id);
        let token = Self::random_token();
        let _ = self.docker(&["volume", "create", &volume]).await?;
        self.docker(&[
            "run", "-d", "--name", &container, "--restart", "unless-stopped",
            "--network", &network, "-v", &format!("{volume}:/data/.houston"),
            "-e", "HOUSTON_HOME=/data/.houston", "-e", "HOUSTON_DOCS=/data/workspace",
            "-e", &format!("HOUSTON_ENGINE_TOKEN={token}"),
            "-e", "HOUSTON_BIND=0.0.0.0:7777", "-e", "HOUSTON_BIND_ALL=1",
            "-e", "HOUSTON_NO_PARENT_WATCHDOG=1", &self.engine_image,
        ]).await?;
        let internal_url = format!("http://{container}:7777");
        let folder_path = match engine_provision::bootstrap_engine_agent(
            &internal_url,
            &token,
            &AgentBootstrapConfig {
                name: agent.name.clone(),
                config_id: agent.config_id.clone(),
                color: agent.color.clone(),
                claude_md: agent.claude_md.clone(),
                provider: agent.provider.clone(),
                model: agent.model.clone(),
            },
        )
        .await
        {
            Ok(path) => path,
            Err(e) => {
                if let Err(ce) = self.remove(agent_id).await {
                    tracing::warn!(agent_id = %agent_id, error = %ce, "provision cleanup failed after bootstrap error");
                }
                return Err(e);
            }
        };
        Ok(RuntimeRow {
            container_name: container,
            internal_url,
            token_hash: hash_token(&token),
            engine_token: token,
            status: "running".into(),
            folder_path,
        })
    }

    async fn restart(&self, agent_id: Uuid) -> anyhow::Result<()> {
        let container = Self::container_name(agent_id);
        self.docker(&["restart", &container]).await?;
        Ok(())
    }

    async fn remove(&self, agent_id: Uuid) -> anyhow::Result<()> {
        let container = Self::container_name(agent_id);
        let volume = Self::volume_name(agent_id);
        match self.docker(&["rm", "-f", &container]).await {
            Ok(_) => {}
            Err(e) if e.to_string().contains("No such container") => {}
            Err(e) => return Err(e),
        }
        match self.docker(&["volume", "rm", &volume]).await {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("No such volume") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn status(&self, agent_id: Uuid) -> anyhow::Result<String> {
        let container = Self::container_name(agent_id);
        let state = self
            .docker(&["inspect", "-f", "{{.State.Status}}", &container])
            .await?;
        Ok(state)
    }
}
