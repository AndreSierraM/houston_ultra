use crate::agents::config;
use crate::agents_crud::AgentMeta;
use crate::error::{CoreError, CoreResult};
use crate::paths::expand_tilde;
use houston_engine_protocol::{
    AgentBootstrapBundle, BootstrapConfigPatch, BuildBootstrapBundleRequest,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::seeds::{filter_seeds, gather_migration_seeds, seeds_from_manifest};
use super::skills::read_packaged_skills;
use super::source::resolve_from_installed;

const DEFAULT_CLAUDE_MD: &str = "## Instructions\n\n## Learnings\n";

pub fn build_bootstrap_bundle(req: BuildBootstrapBundleRequest) -> CoreResult<AgentBootstrapBundle> {
    if let Some(agent_path) = req.agent_path.as_deref() {
        return build_from_agent(&req, &expand_tilde(Path::new(agent_path)));
    }
    build_from_template(&req)
}

fn build_from_template(req: &BuildBootstrapBundleRequest) -> CoreResult<AgentBootstrapBundle> {
    let installed = req
        .installed_path
        .as_ref()
        .map(|p| expand_tilde(Path::new(p)));

    let claude_md = resolve_claude_md(req.claude_md.as_deref(), installed.as_deref())?;
    let skills = match installed.as_deref() {
        Some(path) => read_packaged_skills(&path.join(".agents/skills"))?,
        None => Vec::new(),
    };

    let seeds = resolve_template_seeds(req, installed.as_deref())?;
    let source = match installed.as_deref() {
        Some(path) => resolve_from_installed(path, &req.config_id)?,
        None => None,
    };
    let config_patch = config_patch_from_request(req);

    Ok(AgentBootstrapBundle {
        config_id: req.config_id.clone(),
        name: req.name.clone(),
        color: req.color.clone(),
        claude_md,
        seeds,
        skills,
        config_patch,
        source,
    })
}

fn build_from_agent(
    req: &BuildBootstrapBundleRequest,
    agent_root: &Path,
) -> CoreResult<AgentBootstrapBundle> {
    if !agent_root.is_dir() {
        return Err(CoreError::BadRequest(format!(
            "agent directory does not exist: {}",
            agent_root.display()
        )));
    }

    let meta = read_agent_meta(agent_root)?;
    let claude_md = read_claude_md(agent_root)?;
    let skills = read_packaged_skills(&agent_root.join(".agents/skills"))?;
    let mut seeds = gather_migration_seeds(agent_root)?;
    if let Some(extra) = req.seeds.clone() {
        seeds.extend(filter_seeds(extra));
    }

    let on_disk = config::read(agent_root)?;
    let config_patch = merge_config_patch(
        BootstrapConfigPatch {
            provider: on_disk.provider,
            model: on_disk.model,
            effort: on_disk.effort,
        },
        req,
    );

    let source = resolve_from_installed(agent_root, &meta.config_id)
        .or_else(|_| resolve_from_installed(agent_root, &req.config_id))
        .unwrap_or(None);

    Ok(AgentBootstrapBundle {
        config_id: req.config_id.clone(),
        name: req.name.clone(),
        color: req.color.clone().or(meta.color),
        claude_md,
        seeds,
        skills,
        config_patch,
        source,
    })
}

fn resolve_template_seeds(
    req: &BuildBootstrapBundleRequest,
    installed: Option<&Path>,
) -> CoreResult<HashMap<String, String>> {
    if let Some(seeds) = req.seeds.clone() {
        return Ok(filter_seeds(seeds));
    }
    let Some(path) = installed else {
        return Ok(HashMap::new());
    };
    let manifest_path = path.join("houston.json");
    if !manifest_path.exists() {
        return Ok(HashMap::new());
    }
    let body = fs::read_to_string(&manifest_path).map_err(|e| {
        CoreError::Internal(format!("read {}: {e}", manifest_path.display()))
    })?;
    let manifest: serde_json::Value = serde_json::from_str(&body)?;
    Ok(seeds_from_manifest(&manifest))
}

fn resolve_claude_md(
    explicit: Option<&str>,
    installed: Option<&Path>,
) -> CoreResult<String> {
    if let Some(body) = explicit.filter(|s| !s.is_empty()) {
        return Ok(body.to_string());
    }
    if let Some(path) = installed {
        let claude_path = path.join("CLAUDE.md");
        if claude_path.exists() {
            return fs::read_to_string(&claude_path).map_err(|e| {
                CoreError::Internal(format!("read {}: {e}", claude_path.display()))
            });
        }
    }
    Ok(DEFAULT_CLAUDE_MD.to_string())
}

fn read_claude_md(agent_root: &Path) -> CoreResult<String> {
    let path = agent_root.join("CLAUDE.md");
    match fs::read_to_string(&path) {
        Ok(body) if !body.is_empty() => Ok(body),
        Ok(_) => Ok(DEFAULT_CLAUDE_MD.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DEFAULT_CLAUDE_MD.to_string()),
        Err(e) => Err(CoreError::Internal(format!("read {}: {e}", path.display()))),
    }
}

fn read_agent_meta(agent_root: &Path) -> CoreResult<AgentMeta> {
    let path = agent_root.join(".houston/agent.json");
    let body = fs::read_to_string(&path).map_err(|e| {
        CoreError::BadRequest(format!(
            "agent metadata missing at {}: {e}",
            path.display()
        ))
    })?;
    serde_json::from_str(&body).map_err(Into::into)
}

fn config_patch_from_request(req: &BuildBootstrapBundleRequest) -> Option<BootstrapConfigPatch> {
    merge_config_patch(BootstrapConfigPatch::default(), req)
}

fn merge_config_patch(
    mut patch: BootstrapConfigPatch,
    req: &BuildBootstrapBundleRequest,
) -> Option<BootstrapConfigPatch> {
    if let Some(provider) = req.provider.clone() {
        patch.provider = Some(provider);
    }
    if let Some(model) = req.model.clone() {
        patch.model = Some(model);
    }
    if let Some(effort) = req.effort.clone() {
        patch.effort = Some(effort);
    }
    if patch.provider.is_none() && patch.model.is_none() && patch.effort.is_none() {
        None
    } else {
        Some(patch)
    }
}
