//! Agent bootstrap bundle and credential sync wire types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBootstrapBundle {
    pub config_id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub claude_md: Option<String>,
    #[serde(default)]
    pub seeds: Vec<BootstrapSeed>,
    #[serde(default)]
    pub skills: Vec<BootstrapSkill>,
    #[serde(default)]
    pub config_patch: Option<BootstrapConfigPatch>,
    #[serde(default)]
    pub source: Option<BootstrapSource>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSkill {
    pub slug: String,
    pub skill_md: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSeed {
    pub rel_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapConfigPatch {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapSource {
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSyncOptions {
    pub provider: String,
    /// Opaque JSON forwarded to `POST /v1/providers/{provider}/credential-import`.
    pub import_body: serde_json::Value,
}

/// Merged bootstrap inputs for engine provisioning (legacy fields + optional bundle).
#[derive(Debug, Clone)]
pub struct ResolvedBootstrap {
    pub name: String,
    pub config_id: String,
    pub color: Option<String>,
    pub claude_md: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub seeds: Vec<BootstrapSeed>,
    pub skills: Vec<BootstrapSkill>,
    pub source: Option<BootstrapSource>,
}

pub fn resolve_bootstrap(
    name: &str,
    config_id: &str,
    color: Option<String>,
    claude_md: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    bundle: Option<&AgentBootstrapBundle>,
) -> ResolvedBootstrap {
    let mut resolved = ResolvedBootstrap {
        name: name.to_string(),
        config_id: config_id.to_string(),
        color,
        claude_md,
        provider,
        model,
        effort: None,
        seeds: Vec::new(),
        skills: Vec::new(),
        source: None,
    };
    let Some(bundle) = bundle else {
        return resolved;
    };
    if let Some(md) = &bundle.claude_md {
        resolved.claude_md = Some(md.clone());
    }
    if let Some(patch) = &bundle.config_patch {
        if patch.provider.is_some() {
            resolved.provider = patch.provider.clone();
        }
        if patch.model.is_some() {
            resolved.model = patch.model.clone();
        }
        resolved.effort = patch.effort.clone();
    }
    resolved.seeds = bundle.seeds.clone();
    resolved.skills = bundle.skills.clone();
    resolved.source = bundle.source.clone();
    resolved
}

/// Redacted audit detail for bootstrap apply (no file contents).
pub fn bootstrap_audit_detail(source: &Option<BootstrapSource>, skill_count: usize, seed_count: usize) -> serde_json::Value {
    serde_json::json!({
        "skillCount": skill_count,
        "seedCount": seed_count,
        "sourceKind": source.as_ref().map(|s| s.kind.as_str()),
    })
}

/// Redacted audit detail for credential sync (no ciphertext).
pub fn credential_sync_audit_detail(provider: &str, ok: bool, status: Option<u16>, error: Option<&str>) -> serde_json::Value {
    let mut detail = serde_json::Map::new();
    detail.insert("provider".into(), provider.into());
    detail.insert("ok".into(), ok.into());
    if let Some(code) = status {
        detail.insert("statusCode".into(), code.into());
    }
    if let Some(err) = error {
        detail.insert("error".into(), err.into());
    }
    serde_json::Value::Object(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_bootstrap_merges_bundle_over_legacy() {
        let bundle = AgentBootstrapBundle {
            config_id: "store".into(),
            name: "ignored".into(),
            color: None,
            claude_md: Some("# Cloud".into()),
            seeds: vec![BootstrapSeed {
                rel_path: ".houston/activity/activity.json".into(),
                content: "{}".into(),
            }],
            skills: vec![BootstrapSkill {
                slug: "draft-email".into(),
                skill_md: "---\nname: draft\n---\n".into(),
            }],
            config_patch: Some(BootstrapConfigPatch {
                provider: Some("anthropic".into()),
                model: Some("sonnet".into()),
                effort: Some("high".into()),
            }),
            source: Some(BootstrapSource {
                kind: "store".into(),
                id: Some("alpha".into()),
                version: None,
            }),
        };
        let resolved = resolve_bootstrap(
            "My Agent",
            "default",
            Some("#abc".into()),
            Some("legacy".into()),
            Some("openai".into()),
            Some("gpt".into()),
            Some(&bundle),
        );
        assert_eq!(resolved.name, "My Agent");
        assert_eq!(resolved.claude_md.as_deref(), Some("# Cloud"));
        assert_eq!(resolved.provider.as_deref(), Some("anthropic"));
        assert_eq!(resolved.model.as_deref(), Some("sonnet"));
        assert_eq!(resolved.effort.as_deref(), Some("high"));
        assert_eq!(resolved.skills.len(), 1);
        assert_eq!(resolved.seeds.len(), 1);
    }

    #[test]
    fn credential_sync_audit_detail_never_includes_secrets() {
        let detail = credential_sync_audit_detail("anthropic", false, Some(502), Some("engine unreachable"));
        let s = detail.to_string();
        assert!(!s.contains("ciphertext"));
        assert!(!s.contains("token"));
        assert_eq!(detail["provider"], "anthropic");
        assert_eq!(detail["ok"], false);
    }
}
