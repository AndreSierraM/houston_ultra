//! Kubernetes runtime — Deployment + PVC + Service per cloud agent.

use crate::auth::hash_token;
use crate::engine_provision;
use crate::k8s_specs::{
    agent_deployment_name, agent_pvc_manifest, agent_pvc_name, agent_secret_name,
    agent_workload_manifests, internal_service_url, namespace_manifest, org_namespace,
    org_quota_manifests,
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
const DEPLOYMENT_READY_POLL_INTERVAL: Duration = Duration::from_millis(500);
const DEPLOYMENT_READY_POLL_MAX: u32 = 360;

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

    async fn ensure_namespace_with_quota(
        &self,
        org_id: Uuid,
        max_cloud_agents: i32,
        max_storage_gb: i32,
    ) -> anyhow::Result<()> {
        let ns_yaml = namespace_manifest(org_id);
        if let Err(e) = self.apply_yaml(&ns_yaml).await {
            if !e.to_string().contains("AlreadyExists") {
                return Err(e);
            }
        }
        let quota_yaml = org_quota_manifests(org_id, max_cloud_agents, max_storage_gb);
        self.apply_yaml(&quota_yaml).await?;
        Ok(())
    }

    fn random_token() -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    async fn labeled_namespace(
        &self,
        resources: &str,
        label: &str,
    ) -> anyhow::Result<Option<String>> {
        let out = self
            .kubectl(&[
                "get",
                resources,
                "-A",
                "-l",
                label,
                "-o",
                "jsonpath={range .items[*]}{.metadata.namespace}{end}",
            ])
            .await?;
        if out.is_empty() {
            Ok(None)
        } else {
            Ok(Some(out))
        }
    }

    async fn agent_namespace(&self, agent_id: Uuid) -> anyhow::Result<Option<String>> {
        let label = format!("houston.ai/agent-id={agent_id}");
        self.labeled_namespace("deployment,service,pvc,secret", &label)
            .await
    }

    async fn deployment_namespace(&self, agent_id: Uuid) -> anyhow::Result<String> {
        let label = format!("houston.ai/agent-id={agent_id}");
        self.labeled_namespace("deployment", &label)
            .await?
            .ok_or_else(|| anyhow::anyhow!("deployment for agent {agent_id} not found"))
    }

    async fn deployment_replica_fields(
        &self,
        agent_id: Uuid,
    ) -> anyhow::Result<Option<(String, String)>> {
        let label = format!("houston.ai/agent-id={agent_id}");
        let out = self
            .kubectl(&[
                "get",
                "deployment",
                "-A",
                "-l",
                &label,
                "-o",
                "jsonpath={range .items[*]}{.spec.replicas}{\",\"}{.status.readyReplicas}{end}",
            ])
            .await?;
        parse_deployment_replica_fields(&out)
    }

    async fn scale_agent_deployment(&self, agent_id: Uuid, replicas: u32) -> anyhow::Result<()> {
        let ns = self.deployment_namespace(agent_id).await?;
        let deploy = agent_deployment_name(agent_id);
        self.kubectl(&[
            "scale",
            "deployment",
            &deploy,
            "-n",
            &ns,
            &format!("--replicas={replicas}"),
        ])
        .await?;
        Ok(())
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

    async fn wait_deployment_ready(&self, deploy: &str, ns: &str) -> anyhow::Result<()> {
        for attempt in 1..=DEPLOYMENT_READY_POLL_MAX {
            let ready = self
                .kubectl(&[
                    "get",
                    "deployment",
                    deploy,
                    "-n",
                    ns,
                    "-o",
                    "jsonpath={.status.readyReplicas}",
                ])
                .await?;
            if ready.trim() == "1" {
                return Ok(());
            }
            let unavailable = self
                .kubectl(&[
                    "get",
                    "deployment",
                    deploy,
                    "-n",
                    ns,
                    "-o",
                    "jsonpath={.status.unavailableReplicas}",
                ])
                .await
                .unwrap_or_default();
            tracing::debug!(
                attempt,
                deploy,
                ns,
                ready = %ready,
                unavailable = %unavailable,
                "deployment not ready yet"
            );
            tokio::time::sleep(DEPLOYMENT_READY_POLL_INTERVAL).await;
        }
        let pod_hint = self
            .kubectl(&[
                "get",
                "pods",
                "-n",
                ns,
                "-l",
                &format!("app={deploy}"),
                "-o",
                "jsonpath={.items[0].status.containerStatuses[0].state}",
            ])
            .await
            .unwrap_or_else(|_| "unknown".into());
        anyhow::bail!(
            "deployment {deploy} in {ns} did not become ready within timeout (pod state: {pod_hint})"
        );
    }

    async fn provision_agent_resources(
        &self,
        agent_id: Uuid,
        org_id: Uuid,
        engine_token: &str,
    ) -> anyhow::Result<(String, String)> {
        let ns = org_namespace(org_id);
        let deploy = agent_deployment_name(agent_id);
        // PVC may stay Pending until a pod is scheduled (WaitForFirstConsumer StorageClasses).
        self.apply_yaml(&agent_pvc_manifest(agent_id, org_id))
            .await?;
        let workload =
            agent_workload_manifests(agent_id, org_id, &self.engine_image, engine_token);
        self.apply_yaml(&workload).await?;
        self.wait_deployment_ready(&deploy, &ns).await?;
        self.wait_service_endpoints(&deploy, &ns).await?;
        Ok((deploy, ns))
    }
}

fn service_endpoints_ready(endpoint_ip: &str) -> bool {
    !endpoint_ip.trim().is_empty()
}

fn parse_deployment_replica_fields(out: &str) -> anyhow::Result<Option<(String, String)>> {
    let line = out.trim();
    if line.is_empty() {
        return Ok(None);
    }
    let (spec, ready) = line
        .split_once(',')
        .map(|(s, r)| (s.to_string(), r.to_string()))
        .unwrap_or_else(|| (line.to_string(), String::new()));
    Ok(Some((spec, ready)))
}

/// Maps K8s deployment replica fields to Houston runtime status.
pub(crate) fn map_k8s_agent_status(spec_replicas: &str, ready_replicas: &str) -> String {
    if ready_replicas.trim() == "1" {
        return "running".into();
    }
    if spec_replicas.trim() == "0" {
        return "stopped".into();
    }
    "provisioning".into()
}

#[async_trait]
impl RuntimeBackend for K8sRuntime {
    async fn provision(
        &self,
        agent_id: Uuid,
        org_id: Uuid,
        agent: &AgentProvisionConfig,
    ) -> anyhow::Result<RuntimeRow> {
        self.ensure_namespace_with_quota(
            org_id,
            agent.org_quota.max_cloud_agents,
            agent.org_quota.max_storage_gb,
        )
        .await?;
        let token = Self::random_token();
        if let Err(e) = self
            .provision_agent_resources(agent_id, org_id, &token)
            .await
        {
            if let Err(ce) = self.remove(agent_id).await {
                tracing::warn!(
                    agent_id = %agent_id,
                    error = %ce,
                    "k8s cleanup after provision error"
                );
            }
            return Err(e);
        }
        let deploy = agent_deployment_name(agent_id);
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
        let ns = self.deployment_namespace(agent_id).await?;
        let deploy = agent_deployment_name(agent_id);
        self.kubectl(&["rollout", "restart", "deployment", &deploy, "-n", &ns])
            .await?;
        Ok(())
    }

    async fn stop(&self, agent_id: Uuid) -> anyhow::Result<()> {
        self.scale_agent_deployment(agent_id, 0).await
    }

    async fn start(&self, agent_id: Uuid) -> anyhow::Result<()> {
        self.scale_agent_deployment(agent_id, 1).await?;
        let ns = self.deployment_namespace(agent_id).await?;
        let deploy = agent_deployment_name(agent_id);
        self.wait_deployment_ready(&deploy, &ns).await?;
        self.wait_service_endpoints(&deploy, &ns).await
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
        let Some((spec, ready)) = self.deployment_replica_fields(agent_id).await? else {
            return Ok("error".into());
        };
        Ok(map_k8s_agent_status(&spec, &ready))
    }

    async fn reconcile_workload(
        &self,
        agent_id: Uuid,
        org_id: Uuid,
        engine_token: &str,
    ) -> anyhow::Result<()> {
        if self.deployment_replica_fields(agent_id).await?.is_some() {
            return Ok(());
        }
        tracing::info!(agent_id = %agent_id, "reconciling missing k8s workload");
        self.provision_agent_resources(agent_id, org_id, engine_token)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{map_k8s_agent_status, parse_deployment_replica_fields, service_endpoints_ready};

    #[test]
    fn service_endpoints_ready_accepts_pod_ip() {
        assert!(service_endpoints_ready("10.42.0.22"));
    }

    #[test]
    fn service_endpoints_ready_rejects_empty() {
        assert!(!service_endpoints_ready(""));
        assert!(!service_endpoints_ready("   "));
    }

    #[test]
    fn map_k8s_agent_status_running() {
        assert_eq!(map_k8s_agent_status("1", "1"), "running");
    }

    #[test]
    fn map_k8s_agent_status_stopped_when_scaled_to_zero() {
        assert_eq!(map_k8s_agent_status("0", ""), "stopped");
        assert_eq!(map_k8s_agent_status("0", "0"), "stopped");
    }

    #[test]
    fn map_k8s_agent_status_provisioning_while_scaling_up() {
        assert_eq!(map_k8s_agent_status("1", ""), "provisioning");
        assert_eq!(map_k8s_agent_status("1", "0"), "provisioning");
    }

    #[test]
    fn parse_deployment_replica_fields_handles_empty_and_ready() {
        assert!(parse_deployment_replica_fields("").unwrap().is_none());
        assert_eq!(
            parse_deployment_replica_fields("1,1").unwrap(),
            Some(("1".into(), "1".into()))
        );
        assert_eq!(
            parse_deployment_replica_fields("1,").unwrap(),
            Some(("1".into(), String::new()))
        );
    }
}
