//! houston-agent-files — generic file-level access to an agent's `.houston/` directory.
//!
//! Each data "type" lives in its own folder with a co-located JSON Schema:
//!   .houston/<type>/<type>.json
//!   .houston/<type>/<type>.schema.json
//!
//! Types currently in use:
//!   - activity
//!   - routines
//!   - routine_runs
//!   - config
//!   - learnings
//!
//! The crate exposes two generic functions (`read_file` / `write_file_atomic`)
//! plus helpers to seed embedded schemas and migrate from the legacy flat layout.
//!
//! Safety: all relative paths are canonicalised against the agent root before
//! read/write — any attempt to escape the root via `..` or symlink is rejected.

use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

pub mod schemas;

#[derive(Debug, Error)]
pub enum AgentFilesError {
    #[error("invalid relative path: {0}")]
    InvalidPath(String),
    #[error("path escapes agent root")]
    PathEscapesRoot,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, AgentFilesError>;

/// Sanitise a caller-supplied relative path so it cannot escape the agent root.
///
/// Rules:
///   * must be relative
///   * no `..` components
///   * no absolute prefixes, drive letters, or root components
fn safe_relative(rel: &str) -> Result<PathBuf> {
    let p = Path::new(rel);
    if p.is_absolute() {
        return Err(AgentFilesError::InvalidPath(rel.to_string()));
    }
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::Normal(s) => out.push(s),
            Component::CurDir => {}
            Component::ParentDir => return Err(AgentFilesError::PathEscapesRoot),
            Component::Prefix(_) | Component::RootDir => {
                return Err(AgentFilesError::InvalidPath(rel.to_string()));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(AgentFilesError::InvalidPath(rel.to_string()));
    }
    Ok(out)
}

/// Resolve `<agent_root>/<rel>` with traversal protection.
pub fn resolve(agent_root: &Path, rel: &str) -> Result<PathBuf> {
    let safe = safe_relative(rel)?;
    Ok(agent_root.join(safe))
}

/// Read raw file contents (UTF-8 string).
pub fn read_file(agent_root: &Path, rel: &str) -> Result<String> {
    let path = resolve(agent_root, rel)?;
    match fs::read_to_string(&path) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

/// Write file atomically: write to `<path>.tmp` then rename into place.
/// Creates parent directories as needed.
pub fn write_file_atomic(agent_root: &Path, rel: &str, content: &str) -> Result<()> {
    let path = resolve(agent_root, rel)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = unique_tmp_path(&path);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn unique_tmp_path(path: &Path) -> PathBuf {
    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    path.with_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()))
}

/// Classify a relative path to the matching event type name.
/// Returns the first path component of `.houston/<type>/...` — e.g. "activity".
pub fn classify(rel: &str) -> Option<String> {
    let p = Path::new(rel);
    let mut it = p.components();
    // Expect first component to be ".houston"
    let first = it.next()?;
    let s = match first {
        Component::Normal(s) => s.to_str()?,
        _ => return None,
    };
    if s != ".houston" {
        return None;
    }
    let next = it.next()?;
    match next {
        Component::Normal(s) => s.to_str().map(|s| s.to_string()),
        _ => None,
    }
}

/// Seed the five embedded JSON Schemas under `.houston/<type>/<type>.schema.json`.
/// Idempotent: overwrites if present (schemas are compile-time constants, always authoritative).
pub fn seed_schemas(agent_root: &Path) -> Result<()> {
    for (name, content) in schemas::ALL {
        let rel = format!(".houston/{name}/{name}.schema.json");
        write_file_atomic(agent_root, &rel, content)?;
    }
    Ok(())
}

/// Migrate an agent from the legacy flat layout to the per-type folder layout.
///
/// Legacy:
///   .houston/activity.json
///   .houston/routines.json
///   .houston/routine_runs.json
///   .houston/config.json
///   .houston/memory/learnings.md
///
/// New:
///   .houston/activity/activity.json
///   .houston/routines/routines.json
///   .houston/routine_runs/routine_runs.json
///   .houston/config/config.json
///   .houston/learnings/learnings.json
///
/// Idempotent: if the old file is missing or the new one already exists, the step is skipped.
/// Old files are left in place to act as a rollback safety net — callers may remove them
/// after verifying the new layout works.
pub fn migrate_agent_data(agent_root: &Path) -> Result<()> {
    // JSON files → move to per-type folder (copy + leave original).
    for name in ["activity", "routines", "routine_runs", "config"] {
        let old_rel = format!(".houston/{name}.json");
        let new_rel = format!(".houston/{name}/{name}.json");
        let old_path = agent_root.join(&old_rel);
        let new_path = agent_root.join(&new_rel);
        if old_path.exists() && !new_path.exists() {
            let content = fs::read_to_string(&old_path)?;
            write_file_atomic(agent_root, &new_rel, &content)?;
            tracing::info!(agent_root = %agent_root.display(), name, "migrated legacy agent file");
        }
    }

    // Rewrite legacy Claude model aliases (`opus`/`sonnet`) stored in the
    // per-agent config to explicit version IDs, now that the model catalog
    // pins versions (see `app/src/lib/providers.ts`). Runs after the layout
    // migration above so the config has reached its
    // `.houston/config/config.json` home.
    migrate_config_model_aliases(agent_root)?;
    migrate_config_provider_gemini(agent_root)?;

    // learnings.md → learnings.json (parse markdown bullet list into JSON objects).
    let learnings_md = agent_root.join(".houston/memory/learnings.md");
    let learnings_new = agent_root.join(".houston/learnings/learnings.json");
    if learnings_md.exists() && !learnings_new.exists() {
        let md = fs::read_to_string(&learnings_md)?;
        let now = chrono::Utc::now().to_rfc3339();
        let entries: Vec<serde_json::Value> = md
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                let stripped = t
                    .strip_prefix("- ")
                    .or_else(|| t.strip_prefix("* "))
                    .unwrap_or(t);
                let stripped = stripped.trim();
                if stripped.is_empty() {
                    None
                } else {
                    Some(serde_json::json!({
                        "id": uuid::Uuid::new_v4().to_string(),
                        "text": stripped,
                        "created_at": now,
                    }))
                }
            })
            .collect();
        let body = serde_json::to_string_pretty(&entries)?;
        write_file_atomic(agent_root, ".houston/learnings/learnings.json", &body)?;
        tracing::info!(agent_root = %agent_root.display(), count = entries.len(), "migrated learnings.md → learnings.json");
    }

    // Retire product-layer prompt files that earlier versions seeded under
    // `.houston/prompts/`. These were never user-editable through any UI —
    // the Houston product prompt lives in the app process now. Deleting is
    // safe: no real user edits to preserve. User's mode overrides in
    // `modes/` are untouched.
    for legacy in [
        ".houston/prompts/system.md",
        ".houston/prompts/self-improvement.md",
    ] {
        let path = agent_root.join(legacy);
        if path.exists() {
            match fs::remove_file(&path) {
                Ok(()) => tracing::info!(
                    agent_root = %agent_root.display(),
                    file = legacy,
                    "removed legacy product prompt file"
                ),
                Err(e) => tracing::warn!(
                    agent_root = %agent_root.display(),
                    file = legacy,
                    error = %e,
                    "failed to remove legacy product prompt file"
                ),
            }
        }
    }

    // Seed schemas at the end so every migrated agent has them available.
    seed_schemas(agent_root)?;
    Ok(())
}

/// Legacy Claude model aliases → explicit version IDs.
///
/// Houston used to store the Claude CLI shorthand (`"opus"`, `"sonnet"`) as an
/// agent's model. The shorthand resolves to *whatever the CLI calls latest*, so
/// it can't distinguish Opus 4.7 from 4.8 once both ship. The model catalog
/// (`app/src/lib/providers.ts`) now pins explicit version IDs, so each alias is
/// rewritten to the version it denoted at the time of the switch. Mapping
/// `"opus"` → 4.7 is deliberate: it preserves the exact model existing users
/// were implicitly running rather than silently bumping them to the new
/// flagship (they can opt into 4.8 from the picker).
const LEGACY_MODEL_ALIASES: &[(&str, &str)] = &[
    ("opus", "claude-opus-4-7"),
    ("sonnet", "claude-sonnet-4-6"),
];

/// Rewrite a legacy Claude model alias in `.houston/config/config.json` to its
/// explicit version ID. Idempotent: explicit IDs and unknown values pass
/// through untouched, and a missing / empty / non-object config is a no-op so
/// hand-edited or foreign-format files are never clobbered.
///
/// The model has historically lived under `"model"` (current) or
/// `"claude_model"` (pre-multi-provider, still read via a serde alias), so both
/// keys are normalized in place.
fn migrate_config_model_aliases(agent_root: &Path) -> Result<()> {
    let rel = ".houston/config/config.json";
    let path = agent_root.join(rel);
    let Ok(raw) = fs::read_to_string(&path) else {
        return Ok(()); // no per-agent config — nothing to rewrite
    };
    if raw.trim().is_empty() {
        return Ok(());
    }
    let Ok(serde_json::Value::Object(mut obj)) = serde_json::from_str::<serde_json::Value>(&raw)
    else {
        return Ok(()); // not a JSON object — leave it untouched
    };

    let mut changed = false;
    for key in ["model", "claude_model"] {
        let replacement = match obj.get(key) {
            Some(serde_json::Value::String(current)) => LEGACY_MODEL_ALIASES
                .iter()
                .find(|(alias, _)| *alias == current.as_str())
                .map(|(_, repl)| (*repl).to_string()),
            _ => None,
        };
        if let Some(repl) = replacement {
            obj.insert(key.to_string(), serde_json::Value::String(repl));
            changed = true;
        }
    }

    if changed {
        // Downgrade safety net: save the pre-migration content as a sibling
        // backup BEFORE the atomic overwrite. The catalog now pins explicit
        // version IDs (claude-opus-4-7 / claude-sonnet-4-6), so a user who
        // rolls back to an older Houston build that only knows the bare
        // aliases can restore the original config with
        // `mv config.json.pre-opus48 config.json`. Only the FIRST rewrite
        // writes a backup; idempotent re-runs (or any later migration that
        // touches the model field again) find the backup already present
        // and leave the original pre-opus48 content intact, so the rollback
        // target never drifts forward over time.
        let backup_rel = ".houston/config/config.json.pre-opus48";
        if !agent_root.join(backup_rel).exists() {
            write_file_atomic(agent_root, backup_rel, &raw)?;
        }
        let body = serde_json::to_string_pretty(&serde_json::Value::Object(obj))?;
        write_file_atomic(agent_root, rel, &body)?;
        tracing::info!(
            agent_root = %agent_root.display(),
            "migrated legacy Claude model alias → explicit version id (backup: config.json.pre-opus48)"
        );
    }
    Ok(())
}

/// Rewrite retired `provider: "gemini"` in per-agent config to `anthropic`.
/// Idempotent: other values pass through untouched.
fn migrate_config_provider_gemini(agent_root: &Path) -> Result<()> {
    let rel = ".houston/config/config.json";
    let path = agent_root.join(rel);
    let Ok(raw) = fs::read_to_string(&path) else {
        return Ok(());
    };
    if raw.trim().is_empty() {
        return Ok(());
    }
    let Ok(serde_json::Value::Object(mut obj)) = serde_json::from_str::<serde_json::Value>(&raw)
    else {
        return Ok(());
    };
    let is_gemini = obj
        .get("provider")
        .and_then(|v| v.as_str())
        .is_some_and(|p| p == "gemini");
    if !is_gemini {
        return Ok(());
    }
    obj.insert(
        "provider".to_string(),
        serde_json::Value::String("anthropic".to_string()),
    );
    let body = serde_json::to_string_pretty(&serde_json::Value::Object(obj))?;
    write_file_atomic(agent_root, rel, &body)?;
    tracing::info!(
        agent_root = %agent_root.display(),
        "migrated retired provider gemini → anthropic"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_config(agent_root: &Path, body: &str) {
        let dir = agent_root.join(".houston/config");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.json"), body).unwrap();
    }

    fn read_config(agent_root: &Path) -> serde_json::Value {
        let raw = fs::read_to_string(agent_root.join(".houston/config/config.json")).unwrap();
        serde_json::from_str(&raw).unwrap()
    }

    #[test]
    fn migrates_opus_alias_to_4_7_and_preserves_siblings() {
        let dir = TempDir::new().unwrap();
        write_config(
            dir.path(),
            r#"{"provider":"anthropic","model":"opus","effort":"high"}"#,
        );
        migrate_config_model_aliases(dir.path()).unwrap();
        let cfg = read_config(dir.path());
        assert_eq!(cfg["model"], "claude-opus-4-7");
        assert_eq!(cfg["provider"], "anthropic");
        assert_eq!(cfg["effort"], "high");
    }

    #[test]
    fn migrates_sonnet_alias_to_4_6() {
        let dir = TempDir::new().unwrap();
        write_config(dir.path(), r#"{"model":"sonnet"}"#);
        migrate_config_model_aliases(dir.path()).unwrap();
        assert_eq!(read_config(dir.path())["model"], "claude-sonnet-4-6");
    }

    #[test]
    fn migrates_legacy_claude_model_key() {
        let dir = TempDir::new().unwrap();
        write_config(dir.path(), r#"{"claude_model":"opus"}"#);
        migrate_config_model_aliases(dir.path()).unwrap();
        assert_eq!(read_config(dir.path())["claude_model"], "claude-opus-4-7");
    }

    #[test]
    fn leaves_explicit_ids_and_unknown_values_untouched() {
        let dir = TempDir::new().unwrap();
        write_config(dir.path(), r#"{"model":"claude-opus-4-8"}"#);
        migrate_config_model_aliases(dir.path()).unwrap();
        assert_eq!(read_config(dir.path())["model"], "claude-opus-4-8");

        let dir2 = TempDir::new().unwrap();
        write_config(dir2.path(), r#"{"provider":"openai","model":"gpt-5.5"}"#);
        migrate_config_model_aliases(dir2.path()).unwrap();
        assert_eq!(read_config(dir2.path())["model"], "gpt-5.5");
    }

    #[test]
    fn migration_is_idempotent_and_noop_without_config() {
        // No config file at all — must not error or create one.
        let dir = TempDir::new().unwrap();
        migrate_config_model_aliases(dir.path()).unwrap();
        assert!(!dir.path().join(".houston/config/config.json").exists());

        // Running twice over an alias config is stable.
        let dir2 = TempDir::new().unwrap();
        write_config(dir2.path(), r#"{"model":"opus"}"#);
        migrate_config_model_aliases(dir2.path()).unwrap();
        migrate_config_model_aliases(dir2.path()).unwrap();
        assert_eq!(read_config(dir2.path())["model"], "claude-opus-4-7");
    }

    #[test]
    fn migration_does_not_clobber_non_object_config() {
        let dir = TempDir::new().unwrap();
        write_config(dir.path(), "not json at all");
        migrate_config_model_aliases(dir.path()).unwrap();
        let raw =
            fs::read_to_string(dir.path().join(".houston/config/config.json")).unwrap();
        assert_eq!(raw, "not json at all");
    }

    // --- Pre-migration backup (downgrade safety net) ---

    fn backup_path(agent_root: &Path) -> std::path::PathBuf {
        agent_root.join(".houston/config/config.json.pre-opus48")
    }

    #[test]
    fn first_alias_rewrite_writes_pre_opus48_backup_with_original_content() {
        let dir = TempDir::new().unwrap();
        let original = r#"{"provider":"anthropic","model":"opus","effort":"high"}"#;
        write_config(dir.path(), original);

        migrate_config_model_aliases(dir.path()).unwrap();

        let backup = fs::read_to_string(backup_path(dir.path())).unwrap();
        assert_eq!(backup, original);
        // The live file is the rewritten one.
        assert_eq!(read_config(dir.path())["model"], "claude-opus-4-7");
    }

    #[test]
    fn second_run_preserves_the_original_backup() {
        // Rollback target must NEVER drift forward. After the first rewrite, any
        // subsequent migration that touches the model field again (idempotent
        // re-run, or a later migration extending LEGACY_MODEL_ALIASES) must not
        // overwrite the existing backup, or the user's path back to the pre-
        // opus48 build is lost.
        let dir = TempDir::new().unwrap();
        let original = r#"{"model":"opus"}"#;
        write_config(dir.path(), original);

        migrate_config_model_aliases(dir.path()).unwrap();
        let first_backup = fs::read_to_string(backup_path(dir.path())).unwrap();

        // Simulate a later mutation that triggers another rewrite path: rewrite
        // the live config back to a bare alias and re-run. The backup must
        // still reflect the very first pre-migration content, not this newer
        // intermediate state.
        fs::write(
            dir.path().join(".houston/config/config.json"),
            r#"{"model":"sonnet"}"#,
        )
        .unwrap();
        migrate_config_model_aliases(dir.path()).unwrap();

        let second_backup = fs::read_to_string(backup_path(dir.path())).unwrap();
        assert_eq!(second_backup, first_backup);
        assert_eq!(second_backup, original);
    }

    #[test]
    fn no_op_migration_does_not_create_backup() {
        // No legacy alias present → nothing rewritten → backup must not appear
        // (we only want backups for agents the migration actually touched).
        let dir = TempDir::new().unwrap();
        write_config(dir.path(), r#"{"model":"claude-opus-4-8"}"#);

        migrate_config_model_aliases(dir.path()).unwrap();

        assert!(!backup_path(dir.path()).exists());
    }

    #[test]
    fn missing_or_non_object_config_does_not_create_backup() {
        // Empty/missing/non-object configs bail before the rewrite branch, so
        // they must NOT leave a backup turd behind.
        let dir1 = TempDir::new().unwrap();
        migrate_config_model_aliases(dir1.path()).unwrap();
        assert!(!backup_path(dir1.path()).exists());

        let dir2 = TempDir::new().unwrap();
        write_config(dir2.path(), "");
        migrate_config_model_aliases(dir2.path()).unwrap();
        assert!(!backup_path(dir2.path()).exists());

        let dir3 = TempDir::new().unwrap();
        write_config(dir3.path(), "not json at all");
        migrate_config_model_aliases(dir3.path()).unwrap();
        assert!(!backup_path(dir3.path()).exists());
    }

    #[test]
    fn rejects_parent_dir() {
        let err = safe_relative("../etc/passwd").unwrap_err();
        matches!(err, AgentFilesError::PathEscapesRoot);
    }

    #[test]
    fn rejects_absolute() {
        let err = safe_relative("/etc/passwd").unwrap_err();
        matches!(err, AgentFilesError::InvalidPath(_));
    }

    #[test]
    fn roundtrip_write_read() {
        let dir = TempDir::new().unwrap();
        write_file_atomic(dir.path(), ".houston/activity/activity.json", "[]").unwrap();
        let got = read_file(dir.path(), ".houston/activity/activity.json").unwrap();
        assert_eq!(got, "[]");
    }

    #[test]
    fn missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let got = read_file(dir.path(), ".houston/activity/activity.json").unwrap();
        assert_eq!(got, "");
    }

    #[test]
    fn classify_activity() {
        assert_eq!(
            classify(".houston/activity/activity.json"),
            Some("activity".to_string())
        );
        assert_eq!(classify(".houston/routines/routines.json"), Some("routines".to_string()));
        assert_eq!(classify("CLAUDE.md"), None);
    }

    #[test]
    fn seed_schemas_writes_all() {
        let dir = TempDir::new().unwrap();
        seed_schemas(dir.path()).unwrap();
        for (name, _) in schemas::ALL {
            assert!(dir.path().join(format!(".houston/{name}/{name}.schema.json")).exists());
        }
    }

    #[test]
    fn migrate_moves_legacy_files() {
        let dir = TempDir::new().unwrap();
        let legacy = dir.path().join(".houston/activity.json");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, "[{\"id\":\"a\"}]").unwrap();

        migrate_agent_data(dir.path()).unwrap();

        let new = dir.path().join(".houston/activity/activity.json");
        assert!(new.exists());
        assert_eq!(fs::read_to_string(&new).unwrap(), "[{\"id\":\"a\"}]");
    }

    // --- Full upgrade-path coverage through migrate_agent_data ---
    // The alias unit tests above call migrate_config_model_aliases directly on a
    // pre-seeded FOLDER config. These drive the REAL upgrade sequence a user
    // hits: a legacy FLAT `.houston/config.json` that the layout step copies to
    // the per-type folder, which the alias rewrite then runs against. This also
    // pins the load-bearing ordering (layout copy BEFORE alias rewrite).

    fn write_flat_config(agent_root: &Path, body: &str) {
        let flat = agent_root.join(".houston/config.json");
        fs::create_dir_all(flat.parent().unwrap()).unwrap();
        fs::write(&flat, body).unwrap();
    }

    #[test]
    fn migrate_agent_data_rewrites_alias_via_flat_to_folder_and_preserves_siblings() {
        let dir = TempDir::new().unwrap();
        write_flat_config(
            dir.path(),
            r#"{"provider":"anthropic","model":"opus","effort":"high","worktreeMode":true}"#,
        );

        migrate_agent_data(dir.path()).unwrap();

        let cfg = read_config(dir.path());
        assert_eq!(cfg["model"], "claude-opus-4-7");
        assert_eq!(cfg["provider"], "anthropic");
        assert_eq!(cfg["effort"], "high");
        // An unknown sibling key survives the raw-Object rewrite.
        assert_eq!(cfg["worktreeMode"], true);
    }

    #[test]
    fn migrate_agent_data_leaves_stale_flat_config_untouched() {
        // The flat file is a deliberate rollback safety net; no step rewrites it.
        let dir = TempDir::new().unwrap();
        write_flat_config(dir.path(), r#"{"model":"opus"}"#);

        migrate_agent_data(dir.path()).unwrap();

        let flat = fs::read_to_string(dir.path().join(".houston/config.json")).unwrap();
        assert!(flat.contains("opus"), "flat config must stay as a rollback net: {flat}");
    }

    #[test]
    fn migrate_agent_data_rewrites_both_keys_via_full_path() {
        let dir = TempDir::new().unwrap();
        write_flat_config(dir.path(), r#"{"model":"opus","claude_model":"sonnet"}"#);

        migrate_agent_data(dir.path()).unwrap();

        let cfg = read_config(dir.path());
        assert_eq!(cfg["model"], "claude-opus-4-7");
        assert_eq!(cfg["claude_model"], "claude-sonnet-4-6");
    }

    #[test]
    fn migrate_agent_data_is_idempotent_on_config_alias() {
        let dir = TempDir::new().unwrap();
        write_flat_config(dir.path(), r#"{"model":"opus"}"#);

        migrate_agent_data(dir.path()).unwrap();
        migrate_agent_data(dir.path()).unwrap();

        assert_eq!(read_config(dir.path())["model"], "claude-opus-4-7");
    }

    #[test]
    fn migrate_removes_legacy_product_prompts() {
        let dir = TempDir::new().unwrap();
        let prompts = dir.path().join(".houston/prompts");
        fs::create_dir_all(prompts.join("modes")).unwrap();
        fs::write(prompts.join("system.md"), "stale product prompt").unwrap();
        fs::write(prompts.join("self-improvement.md"), "stale guidance").unwrap();
        fs::write(prompts.join("modes/execution.md"), "user's mode — keep").unwrap();

        migrate_agent_data(dir.path()).unwrap();

        assert!(!prompts.join("system.md").exists());
        assert!(!prompts.join("self-improvement.md").exists());
        // User's mode override must survive.
        assert!(prompts.join("modes/execution.md").exists());
        assert_eq!(
            fs::read_to_string(prompts.join("modes/execution.md")).unwrap(),
            "user's mode — keep"
        );

        // Running again must be idempotent (no-op, no error).
        migrate_agent_data(dir.path()).unwrap();
    }

    #[test]
    fn migrate_rewrites_gemini_provider_to_anthropic() {
        let dir = TempDir::new().unwrap();
        let config_dir = dir.path().join(".houston/config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.json"),
            r#"{"provider":"gemini","model":"gemini-3.1-flash-lite"}"#,
        )
        .unwrap();

        migrate_agent_data(dir.path()).unwrap();

        let raw = fs::read_to_string(config_dir.join("config.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["provider"], "anthropic");
        assert_eq!(v["model"], "gemini-3.1-flash-lite");

        migrate_agent_data(dir.path()).unwrap();
        let raw2 = fs::read_to_string(config_dir.join("config.json")).unwrap();
        assert_eq!(raw2, raw);
    }

    #[test]
    fn migrate_learnings_md_to_json() {
        let dir = TempDir::new().unwrap();
        let md = dir.path().join(".houston/memory/learnings.md");
        fs::create_dir_all(md.parent().unwrap()).unwrap();
        fs::write(&md, "- first learning\n- second learning\n").unwrap();

        migrate_agent_data(dir.path()).unwrap();

        let json = fs::read_to_string(dir.path().join(".houston/learnings/learnings.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
        assert_eq!(parsed[0]["text"].as_str().unwrap(), "first learning");
    }
}
