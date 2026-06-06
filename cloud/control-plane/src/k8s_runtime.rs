//! Kubernetes runtime — Deployment + PVC + Service per cloud agent.

use crate::auth::hash_token;
use crate::engine_provision;
use crate::k8s_specs::{
    agent_deployment_name, agent_manifests, agent_pvc_name, agent_secret_name,
    internal_service_url, namespace_manifest, org_namespace,
};
use crate::runtime::{AgentProvisionConfig, RuntimeBackend, RuntimeRow};
use async_trait::async_trait;
use rand::RngCore;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use uuid::Uuid;

const ENDPOINT_POLL_INTERVAL: Duration = Duration::from_millis(500);
const ENDPOINT_POLL_MAX: u32 = 60;

#[derive(Clone)]
pub struct K8sRuntime {
    pub engine_image: String,
    pub kubectl_bin: String,
}

impl K8sRuntime {
    async fn kubectl(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = Command::new(&self.kubectl_bin)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("kubectl {} failed: {stderr}", args.join(" "));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    async fn apply_yaml(&self, yaml: &str) -> anyhow::Result<()> {
        let mut child = Command::new(&self.kubectl_bin)
            .args(["apply", "-f", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(yaml.as_bytes()).await?;
        }
        let output = child.wait_with_output().await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("kubectl apply failed: {stderr}");
        }
        Ok(())
    }

    async fn ensure_namespace(&self, org_id: Uuid) -> anyhow::Result<()> {
        let yaml = namespace_manifest(org_id);
        if let Err(e) = self.apply_yaml(&yaml).await {
            if !e.to_string().contains("AlreadyExists") {
                return Err(e);
            }
        }
        Ok(())
    }

    fn random_token() -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    async fn agent_namespace(&self, agent_id: Uuid) -> anyhow::Result<Option<String>> {
        let label = format!("houston.ai/agent-id={agent_id}");
        let out = self
            .kubectl(&[
                "get",
                "deployment,service,pvc,secret",
                "-A",
                "-l",
                &label,
                "-o",
                "jsonpath={.items[0].metadata.namespace}",
            ])
            .await;
        match out {
            Ok(ns) if !ns.is_empty() => Ok(Some(ns)),
            _ => Ok(None),
        }
    }

    async fn wait_service_endpoints(&self, deploy: &str, ns: &str) -> anyhow::Result<()> {
        for attempt in 1..=ENDPOINT_POLL_MAX {
            match self
                .kubectl(&[
                    "get",
                    "endpoints",
                    deploy,
                    "-n",
                    ns,
                    "-o",
                    "jsonpath={.subsets[0].addresses[0].ip}",
                ])
                .await
            {
                Ok(ip) if service_endpoints_ready(&ip) => return Ok(()),
                Ok(_) => {
                    tracing::debug!(attempt, deploy, ns, "service endpoints not ready yet");
                }
                Err(e) => return Err(e),
            }
            tokio::time::sleep(ENDPOINT_POLL_INTERVAL).await;
        }
        anyhow::bail!("service {deploy} in {ns} has no endpoints after {ENDPOINT_POLL_MAX} polls");
    }
}

fn service_endpoints_ready(endpoint_ip: &str) -> bool {
    !endpoint_ip.trim().is_empty()
}

#[async_trait]
impl RuntimeBackend for K8sRuntime {
    async fn provision(
        &self,
        agent_id: Uuid,
        org_id: Uuid,
        agent: &AgentProvisionConfig,
    ) -> anyhow::Result<RuntimeRow> {
        self.ensure_namespace(org_id).await?;
        let token = Self::random_token();
        let yaml = agent_manifests(agent_id, org_id, &self.engine_image, &token);
        if let Err(e) = self.apply_yaml(&yaml).await {
            if let Err(ce) = self.remove(agent_id).await {
                tracing::warn!(
                    agent_id = %agent_id,
                    error = %ce,
                    "k8s cleanup after apply error"
                );
            }
            return Err(e);
        }
        let deploy = agent_deployment_name(agent_id);
        let ns = org_namespace(org_id);
        if let Err(e) = self
            .kubectl(&[
                "rollout",
                "status",
                "deployment",
                &deploy,
                "-n",
                &ns,
                "--timeout=120s",
            ])
            .await
        {
            if let Err(ce) = self.remove(agent_id).await {
                tracing::warn!(
                    agent_id = %agent_id,
                    error = %ce,
                    "k8s cleanup after rollout error"
                );
            }
            return Err(e);
        }
        self.wait_service_endpoints(&deploy, &ns).await?;
        let internal_url = internal_service_url(agent_id, org_id);
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
                    tracing::warn!(agent_id = %agent_id, error = %ce, "k8s cleanup after bootstrap error");
                }
                return Err(e);
            }
        };
        Ok(RuntimeRow {
            container_name: deploy,
            internal_url,
            token_hash: hash_token(&token),
            engine_token: token,
            status: "running".into(),
            folder_path,
        })
    }

    async fn restart(&self, agent_id: Uuid) -> anyhow::Result<()> {
        let ns = self
            .kubectl(&[
                "get",
                "deployment",
                "-A",
                "-l",
                &format!("houston.ai/agent-id={agent_id}"),
                "-o",
                "jsonpath={.items[0].metadata.namespace}",
            ])
            .await?;
        let deploy = agent_deployment_name(agent_id);
        self.kubectl(&["rollout", "restart", "deployment", &deploy, "-n", &ns])
            .await?;
        Ok(())
    }

    async fn remove(&self, agent_id: Uuid) -> anyhow::Result<()> {
        let Some(ns) = self.agent_namespace(agent_id).await? else {
            return Ok(());
        };
        let deploy = agent_deployment_name(agent_id);
        let pvc = agent_pvc_name(agent_id);
        let secret = agent_secret_name(agent_id);
        let _ = self
            .kubectl(&[
                "delete",
                "deployment,service",
                &deploy,
                "-n",
                &ns,
                "--ignore-not-found",
            ])
            .await;
        let _ = self
            .kubectl(&["delete", "pvc", &pvc, "-n", &ns, "--ignore-not-found"])
            .await;
        let _ = self
            .kubectl(&["delete", "secret", &secret, "-n", &ns, "--ignore-not-found"])
            .await;
        Ok(())
    }

    async fn status(&self, agent_id: Uuid) -> anyhow::Result<String> {
        let out = self
            .kubectl(&[
                "get",
                "deployment",
                "-A",
                "-l",
                &format!("houston.ai/agent-id={agent_id}"),
                "-o",
                "jsonpath={.items[0].status.readyReplicas}",
            ])
            .await?;
        if out == "1" {
            Ok("running".into())
        } else {
            Ok("provisioning".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::service_endpoints_ready;

    #[test]
    fn service_endpoints_ready_accepts_pod_ip() {
        assert!(service_endpoints_ready("10.42.0.22"));
    }

    #[test]
    fn service_endpoints_ready_rejects_empty() {
        assert!(!service_endpoints_ready(""));
        assert!(!service_endpoints_ready("   "));
    }
}
