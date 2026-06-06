use crate::error::{CoreError, CoreResult};
use houston_engine_protocol::BootstrapSource;
use std::fs;
use std::path::Path;

pub fn resolve_from_installed(installed: &Path, config_id: &str) -> CoreResult<Option<BootstrapSource>> {
    let source_path = installed.join(".source.json");
    if source_path.exists() {
        return parse_source_file(&source_path, config_id);
    }
    let manifest_path = installed.join("houston.json");
    if manifest_path.exists() {
        let body = fs::read_to_string(&manifest_path).map_err(|e| {
            CoreError::Internal(format!("read {}: {e}", manifest_path.display()))
        })?;
        let manifest: serde_json::Value = serde_json::from_str(&body)?;
        let version = manifest
            .get("version")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        return Ok(Some(BootstrapSource {
            kind: "custom".into(),
            id: config_id.to_string(),
            version,
        }));
    }
    Ok(None)
}

fn parse_source_file(path: &Path, fallback_id: &str) -> CoreResult<Option<BootstrapSource>> {
    let body = fs::read_to_string(path)
        .map_err(|e| CoreError::Internal(format!("read {}: {e}", path.display())))?;
    let source: serde_json::Value = serde_json::from_str(&body)?;
    if source["source"].as_str() == Some("houston-store") {
        let id = source["agent_id"]
            .as_str()
            .unwrap_or(fallback_id)
            .to_string();
        let version = source["version"].as_str().map(str::to_string);
        return Ok(Some(BootstrapSource {
            kind: "houston-store".into(),
            id,
            version,
        }));
    }
    if let Some(repo) = source["repo"].as_str() {
        return Ok(Some(BootstrapSource {
            kind: "github".into(),
            id: repo.to_string(),
            version: None,
        }));
    }
    Ok(None)
}
