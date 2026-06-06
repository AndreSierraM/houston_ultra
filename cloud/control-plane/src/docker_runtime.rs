//! Docker runtime — one private houston-engine container per cloud agent.

use crate::auth::hash_token;
use crate::engine_provision;
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

    /// Maps Docker container state to Houston runtime status.
    pub(crate) fn map_docker_agent_status(raw: &str) -> String {
        match raw.trim() {
            "running" => "running".into(),
            "exited" => "stopped".into(),
            "created" | "restarting" => "provisioning".into(),
            other => other.into(),
        }
    }

    /// Optional override: read product prompt from host path (entrypoint baked-in file is enough).
    fn product_prompt_env_arg() -> Option<String> {
        let path = std::env::var("HOUSTON_CLOUD_PRODUCT_PROMPT_PATH").ok()?;
        let content = std::fs::read_to_string(&path).ok()?;
        if content.trim().is_empty() {
            return None;
        }
        Some(format!("HOUSTON_APP_SYSTEM_PROMPT={content}"))
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
        let mut run_args: Vec<String> = vec![
            "run".into(),
            "-d".into(),
            "--name".into(),
            container.clone(),
            "--restart".into(),
            "unless-stopped".into(),
            "--network".into(),
            network.clone(),
            "-v".into(),
            format!("{volume}:/data"),
            "-e".into(),
            "HOME=/data".into(),
            "-e".into(),
            "HOUSTON_HOME=/data/.houston".into(),
            "-e".into(),
            "HOUSTON_DOCS=/data/workspace".into(),
            "-e".into(),
            format!("HOUSTON_ENGINE_TOKEN={token}"),
            "-e".into(),
            "HOUSTON_BIND=0.0.0.0:7777".into(),
            "-e".into(),
            "HOUSTON_BIND_ALL=1".into(),
            "-e".into(),
            "HOUSTON_NO_PARENT_WATCHDOG=1".into(),
            "-e".into(),
            "HOUSTON_TUNNEL_URL=http://127.0.0.1:1".into(),
        ];
        if let Some(prompt_env) = Self::product_prompt_env_arg() {
            run_args.push("-e".into());
            run_args.push(prompt_env);
        }
        run_args.push(self.engine_image.clone());
        let run_refs: Vec<&str> = run_args.iter().map(String::as_str).collect();
        self.docker(&run_refs).await?;
        let internal_url = format!("http://{container}:7777");
        let folder_path = match engine_provision::bootstrap_engine_agent(
            &internal_url,
            &token,
            &agent.bootstrap,
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

    async fn stop(&self, agent_id: Uuid) -> anyhow::Result<()> {
        let container = Self::container_name(agent_id);
        self.docker(&["stop", &container]).await?;
        Ok(())
    }

    async fn start(&self, agent_id: Uuid) -> anyhow::Result<()> {
        let container = Self::container_name(agent_id);
        self.docker(&["start", &container]).await?;
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
        Ok(Self::map_docker_agent_status(&state))
    }
}

#[cfg(test)]
mod tests {
    use super::DockerRuntime;

    #[test]
    fn map_docker_agent_status_normalizes_states() {
        assert_eq!(DockerRuntime::map_docker_agent_status("running"), "running");
        assert_eq!(DockerRuntime::map_docker_agent_status("exited"), "stopped");
        assert_eq!(
            DockerRuntime::map_docker_agent_status("restarting"),
            "provisioning"
        );
    }
}
