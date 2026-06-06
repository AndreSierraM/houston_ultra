//! Bootstrap workspace + agent inside a freshly started houston-engine container.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub const CLOUD_WORKSPACE_NAME: &str = "Cloud";

const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const HEALTH_POLL_MAX: u32 = 60;

#[derive(Debug, Clone)]
pub struct AgentBootstrapConfig {
    pub name: String,
    pub config_id: String,
    pub color: Option<String>,
    pub claude_md: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Workspace {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateAgentResult {
    agent: EngineAgent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineAgent {
    folder_path: String,
}

pub async fn bootstrap_engine_agent(
    internal_url: &str,
    engine_token: &str,
    config: &AgentBootstrapConfig,
) -> anyhow::Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let base = internal_url.trim_end_matches('/');
    wait_healthy(&client, base, engine_token).await?;
    let workspace_id = ensure_cloud_workspace(&client, base, engine_token).await?;
    let folder_path = create_agent(&client, base, engine_token, &workspace_id, config).await?;
    seed_agent_content(&client, base, engine_token, &folder_path, config).await?;
    Ok(folder_path)
}

async fn wait_healthy(
    client: &reqwest::Client,
    base: &str,
    token: &str,
) -> anyhow::Result<()> {
    let url = format!("{base}/v1/health");
    for attempt in 1..=HEALTH_POLL_MAX {
        match client.get(&url).bearer_auth(token).send().await {
            Ok(res) if res.status().is_success() => return Ok(()),
            Ok(res) => {
                tracing::debug!(attempt, status = %res.status(), "engine health not ready");
            }
            Err(e) => {
                tracing::debug!(attempt, error = %e, "engine health poll failed");
            }
        }
        tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
    }
    anyhow::bail!("engine did not become healthy at {url}");
}

async fn ensure_cloud_workspace(
    client: &reqwest::Client,
    base: &str,
    token: &str,
) -> anyhow::Result<String> {
    let list_url = format!("{base}/v1/workspaces");
    let workspaces: Vec<Workspace> = client
        .get(&list_url)
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(ws) = workspaces
        .into_iter()
        .find(|w| w.name == CLOUD_WORKSPACE_NAME)
    {
        return Ok(ws.id);
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CreateWorkspace<'a> {
        name: &'a str,
    }
    let created: Workspace = client
        .post(&list_url)
        .bearer_auth(token)
        .json(&CreateWorkspace {
            name: CLOUD_WORKSPACE_NAME,
        })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(created.id)
}

async fn create_agent(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    workspace_id: &str,
    config: &AgentBootstrapConfig,
) -> anyhow::Result<String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CreateAgent<'a> {
        name: &'a str,
        config_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<&'a str>,
    }
    let url = format!("{base}/v1/workspaces/{workspace_id}/agents");
    let body = CreateAgent {
        name: &config.name,
        config_id: &config.config_id,
        color: config.color.as_deref(),
    };
    let created: CreateAgentResult = client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(created.agent.folder_path)
}

async fn seed_agent_content(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    agent_path: &str,
    config: &AgentBootstrapConfig,
) -> anyhow::Result<()> {
    if let Some(content) = config.claude_md.as_deref() {
        write_claude_md(client, base, token, agent_path, content).await?;
    }
    if config.provider.is_some() || config.model.is_some() {
        write_provider_config(client, base, token, agent_path, config).await?;
    }
    Ok(())
}

async fn write_claude_md(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    agent_path: &str,
    content: &str,
) -> anyhow::Result<()> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct WriteBody<'a> {
        agent_path: &'a str,
        rel_path: &'a str,
        content: &'a str,
    }
    let url = format!("{base}/v1/agents/files/write");
    client
        .post(&url)
        .bearer_auth(token)
        .json(&WriteBody {
            agent_path,
            rel_path: "CLAUDE.md",
            content,
        })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn write_provider_config(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    agent_path: &str,
    config: &AgentBootstrapConfig,
) -> anyhow::Result<()> {
    let url = format!("{base}/v1/agents/config");
    let current: Value = client
        .get(&url)
        .query(&[("agent_path", agent_path)])
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let patched = patch_config(
        current,
        config.provider.as_deref(),
        config.model.as_deref(),
    );
    client
        .put(&url)
        .query(&[("agent_path", agent_path)])
        .bearer_auth(token)
        .json(&patched)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

fn patch_config(mut cfg: Value, provider: Option<&str>, model: Option<&str>) -> Value {
    if let Some(p) = provider {
        cfg["provider"] = Value::String(p.to_string());
    }
    if let Some(m) = model {
        cfg["model"] = Value::String(m.to_string());
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::patch_config;
    use serde_json::json;

    #[test]
    fn patch_config_sets_provider_and_model() {
        let out = patch_config(json!({ "name": "alpha" }), Some("anthropic"), Some("sonnet"));
        assert_eq!(out["provider"], "anthropic");
        assert_eq!(out["model"], "sonnet");
        assert_eq!(out["name"], "alpha");
    }

    #[test]
    fn patch_config_preserves_extra_fields() {
        let out = patch_config(
            json!({ "worktreeMode": "always" }),
            Some("openai"),
            None,
        );
        assert_eq!(out["provider"], "openai");
        assert_eq!(out["worktreeMode"], "always");
        assert!(out.get("model").is_none());
    }
}
