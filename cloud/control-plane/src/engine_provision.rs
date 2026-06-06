//! Bootstrap workspace + agent inside a freshly started houston-engine container.

use crate::bootstrap_bundle::ResolvedBootstrap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub const CLOUD_WORKSPACE_NAME: &str = "Cloud";

const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);
const HEALTH_POLL_MAX: u32 = 240;
const HEALTH_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HEALTH_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const HEALTH_STABLE_SUCCESSES: u32 = 2;
const BOOTSTRAP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CREDENTIAL_SYNC_TIMEOUT: Duration = Duration::from_secs(30);

fn health_check_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(HEALTH_CONNECT_TIMEOUT)
        .timeout(HEALTH_REQUEST_TIMEOUT)
        .build()?)
}

fn bootstrap_client() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(BOOTSTRAP_REQUEST_TIMEOUT)
        .build()?)
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
    config: &ResolvedBootstrap,
) -> anyhow::Result<String> {
    let health_client = health_check_client()?;
    let client = bootstrap_client()?;
    let base = internal_url.trim_end_matches('/');
    wait_healthy(&health_client, base, engine_token).await?;
    let workspace_id = ensure_cloud_workspace(&client, base, engine_token).await?;
    let folder_path =
        create_agent(&client, base, engine_token, &workspace_id, config).await?;
    apply_bootstrap_bundle(&client, base, engine_token, &folder_path, config).await?;
    Ok(folder_path)
}

/// Proxy-passthrough credential import to the private engine (control plane never decrypts).
pub async fn sync_provider_credentials(
    internal_url: &str,
    engine_token: &str,
    provider: &str,
    import_body: &Value,
) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(CREDENTIAL_SYNC_TIMEOUT)
        .build()?;
    let base = internal_url.trim_end_matches('/');
    let url = format!("{base}/v1/providers/{provider}/credential-import");
    client
        .post(&url)
        .bearer_auth(engine_token)
        .json(import_body)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn wait_healthy(
    client: &reqwest::Client,
    base: &str,
    token: &str,
) -> anyhow::Result<()> {
    let url = format!("{base}/v1/health");
    let mut stable = 0u32;
    for attempt in 1..=HEALTH_POLL_MAX {
        match client.get(&url).bearer_auth(token).send().await {
            Ok(res) if res.status().is_success() => {
                stable += 1;
                if stable >= HEALTH_STABLE_SUCCESSES {
                    return Ok(());
                }
            }
            Ok(res) => {
                stable = 0;
                tracing::debug!(attempt, status = %res.status(), "engine health not ready");
            }
            Err(e) => {
                stable = 0;
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
    config: &ResolvedBootstrap,
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

/// Agent instruction filenames written from bootstrap `claude_md` (Codex reads AGENTS.md).
pub fn agent_instruction_filenames() -> [&'static str; 2] {
    ["CLAUDE.md", "AGENTS.md"]
}

async fn apply_bootstrap_bundle(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    agent_path: &str,
    config: &ResolvedBootstrap,
) -> anyhow::Result<()> {
    if let Some(content) = config.claude_md.as_deref() {
        for rel_path in agent_instruction_filenames() {
            write_agent_file(client, base, token, agent_path, rel_path, content).await?;
        }
    }
    for seed in &config.seeds {
        write_agent_file(client, base, token, agent_path, &seed.rel_path, &seed.content)
            .await?;
    }
    for skill in &config.skills {
        install_skill(client, base, token, agent_path, skill).await?;
    }
    if config.provider.is_some() || config.model.is_some() || config.effort.is_some() {
        write_provider_config(client, base, token, agent_path, config).await?;
    }
    seed_schemas_and_migrate(client, base, token, agent_path).await?;
    Ok(())
}

async fn write_agent_file(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    agent_path: &str,
    rel_path: &str,
    content: &str,
) -> anyhow::Result<()> {
    #[derive(Serialize)]
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
            rel_path,
            content,
        })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

async fn install_skill(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    agent_path: &str,
    skill: &crate::bootstrap_bundle::BootstrapSkill,
) -> anyhow::Result<()> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CreateSkill<'a> {
        workspace_path: &'a str,
        name: &'a str,
        description: &'a str,
        content: &'a str,
    }
    let url = format!("{base}/v1/skills");
    client
        .post(&url)
        .bearer_auth(token)
        .json(&CreateSkill {
            workspace_path: agent_path,
            name: &skill.slug,
            description: "",
            content: &skill.skill_md,
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
    config: &ResolvedBootstrap,
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
        config.effort.as_deref(),
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

async fn seed_schemas_and_migrate(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    agent_path: &str,
) -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct AgentPathBody<'a> {
        agent_path: &'a str,
    }
    let body = AgentPathBody { agent_path };
    for path in ["seed-schemas", "migrate"] {
        let url = format!("{base}/v1/agents/files/{path}");
        client
            .post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
    }
    Ok(())
}

pub fn patch_config(
    mut cfg: Value,
    provider: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
) -> Value {
    if let Some(p) = provider {
        cfg["provider"] = Value::String(p.to_string());
    }
    if let Some(m) = model {
        cfg["model"] = Value::String(m.to_string());
    }
    if let Some(e) = effort {
        cfg["effort"] = Value::String(e.to_string());
    }
    cfg
}

#[cfg(test)]
mod tests {
    use super::patch_config;
    use serde_json::json;

    #[test]
    fn write_body_uses_snake_case_for_engine() {
        #[derive(serde::Serialize)]
        struct WriteBody<'a> {
            agent_path: &'a str,
            rel_path: &'a str,
            content: &'a str,
        }
        let body = WriteBody {
            agent_path: "/data/agents/test",
            rel_path: "CLAUDE.md",
            content: "# hi",
        };
        let json = serde_json::to_value(body).unwrap();
        assert_eq!(json["agent_path"], "/data/agents/test");
        assert_eq!(json["rel_path"], "CLAUDE.md");
        assert!(json.get("agentPath").is_none());
        assert!(json.get("relPath").is_none());
    }

    #[test]
    fn patch_config_sets_provider_model_and_effort() {
        let out = patch_config(
            json!({ "name": "alpha" }),
            Some("anthropic"),
            Some("sonnet"),
            Some("high"),
        );
        assert_eq!(out["provider"], "anthropic");
        assert_eq!(out["model"], "sonnet");
        assert_eq!(out["effort"], "high");
    }

    #[test]
    fn agent_instruction_filenames_includes_claude_and_agents_md() {
        let files = super::agent_instruction_filenames();
        assert_eq!(files, ["CLAUDE.md", "AGENTS.md"]);
    }

    #[test]
    fn patch_config_preserves_extra_fields() {
        let out = patch_config(
            json!({ "worktreeMode": "always" }),
            Some("openai"),
            None,
            None,
        );
        assert_eq!(out["provider"], "openai");
        assert_eq!(out["worktreeMode"], "always");
        assert!(out.get("model").is_none());
    }
}
