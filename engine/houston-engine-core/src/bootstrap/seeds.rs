use crate::error::{CoreError, CoreResult};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const ACTIVITY_SEED_PATHS: &[&str] = &[
    ".houston/activity.json",
    ".houston/activity/activity.json",
];

pub fn is_activity_seed_path(path: &str) -> bool {
    ACTIVITY_SEED_PATHS.contains(&path)
}

pub fn filter_seeds(seeds: HashMap<String, String>) -> HashMap<String, String> {
    seeds
        .into_iter()
        .filter(|(path, _)| !is_activity_seed_path(path))
        .collect()
}

pub fn seeds_from_manifest(manifest: &serde_json::Value) -> HashMap<String, String> {
    let Some(seeds) = manifest.get("agentSeeds").and_then(|v| v.as_object()) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (path, value) in seeds {
        if let Some(text) = value.as_str() {
            out.insert(path.clone(), text.to_string());
        }
    }
    filter_seeds(out)
}

/// On-disk routines and learnings for **local → cloud migration** only.
///
/// Store → cloud MVP uses `agentSeeds` from the template `houston.json`, not
/// this helper. Callers pass `agentPath` (not `installedPath`) to activate it.
pub fn gather_migration_seeds(agent_root: &Path) -> CoreResult<HashMap<String, String>> {
    let mut out = HashMap::new();
    for rel in [
        ".houston/routines/routines.json",
        ".houston/learnings/learnings.json",
    ] {
        let path = agent_root.join(rel);
        let body = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(CoreError::Internal(format!("read {}: {e}", path.display())));
            }
        };
        if is_empty_json_array(&body) {
            continue;
        }
        out.insert(rel.to_string(), body);
    }
    Ok(out)
}

fn is_empty_json_array(raw: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(raw.trim()) {
        Ok(serde_json::Value::Array(items)) => items.is_empty(),
        Ok(_) => false,
        Err(_) => false,
    }
}
