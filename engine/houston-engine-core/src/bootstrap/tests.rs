use super::build::build_bootstrap_bundle;
use super::seeds::{filter_seeds, is_activity_seed_path, seeds_from_manifest};
use super::skills::read_packaged_skills;
use super::source::resolve_from_installed;
use crate::workspaces::{self, CreateWorkspace};
use houston_engine_protocol::BuildBootstrapBundleRequest;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn filters_activity_seeds() {
    let mut seeds = HashMap::from([
        (".houston/activity.json".into(), "[]".into()),
        (".houston/board.json".into(), "{}".into()),
    ]);
    seeds = filter_seeds(seeds);
    assert!(!seeds.contains_key(".houston/activity.json"));
    assert_eq!(seeds[".houston/board.json"], "{}");
    assert!(is_activity_seed_path(".houston/activity/activity.json"));
}

#[test]
fn template_mode_copies_store_skills_and_claude_md() {
    let installed = TempDir::new().unwrap();
    let skills = installed.path().join(".agents/skills/demo");
    fs::create_dir_all(&skills).unwrap();
    fs::write(skills.join("SKILL.md"), "skill body").unwrap();
    fs::write(installed.path().join("CLAUDE.md"), "## Store template").unwrap();
    fs::write(
        installed.path().join("houston.json"),
        r#"{"id":"demo","version":"1.2.0","agentSeeds":{".houston/board.json":"{}"}}"#,
    )
    .unwrap();
    fs::write(
        installed.path().join(".source.json"),
        r#"{"source":"houston-store","agent_id":"demo","version":"1.2.0"}"#,
    )
    .unwrap();

    let bundle = build_bootstrap_bundle(BuildBootstrapBundleRequest {
        config_id: "demo".into(),
        name: "Ops".into(),
        color: Some("navy".into()),
        claude_md: None,
        installed_path: Some(installed.path().to_string_lossy().to_string()),
        seeds: None,
        provider: Some("anthropic".into()),
        model: Some("sonnet".into()),
        effort: None,
        agent_path: None,
    })
    .unwrap();

    assert_eq!(bundle.claude_md, "## Store template");
    assert_eq!(bundle.skills.len(), 1);
    assert_eq!(bundle.skills[0].slug, "demo");
    assert_eq!(bundle.skills[0].skill_md, "skill body");
    assert_eq!(bundle.seeds[".houston/board.json"], "{}");
    assert_eq!(bundle.config_patch.as_ref().unwrap().provider.as_deref(), Some("anthropic"));
    assert_eq!(bundle.source.as_ref().unwrap().kind, "houston-store");
    assert_eq!(bundle.source.as_ref().unwrap().version.as_deref(), Some("1.2.0"));
}

#[test]
fn template_mode_includes_routines_from_manifest() {
    let installed = TempDir::new().unwrap();
    fs::write(
        installed.path().join("houston.json"),
        r#"{"id":"demo","agentSeeds":{".houston/routines/routines.json":"[{\"id\":\"r1\",\"name\":\"Daily\",\"schedule\":\"0 9 * * *\"}]"}}"#,
    )
    .unwrap();

    let bundle = build_bootstrap_bundle(BuildBootstrapBundleRequest {
        config_id: "demo".into(),
        name: "Ops".into(),
        color: None,
        claude_md: None,
        installed_path: Some(installed.path().to_string_lossy().to_string()),
        seeds: None,
        provider: None,
        model: None,
        effort: None,
        agent_path: None,
    })
    .unwrap();

    assert!(bundle.seeds.contains_key(".houston/routines/routines.json"));
    assert!(bundle.seeds[".houston/routines/routines.json"].contains("Daily"));
}

#[test]
fn template_mode_merges_manifest_and_explicit_seeds() {
    let installed = TempDir::new().unwrap();
    fs::write(
        installed.path().join("houston.json"),
        r#"{"id":"demo","agentSeeds":{".houston/board.json":"{}",".houston/routines/routines.json":"[]"}}"#,
    )
    .unwrap();

    let bundle = build_bootstrap_bundle(BuildBootstrapBundleRequest {
        config_id: "demo".into(),
        name: "Ops".into(),
        color: None,
        claude_md: None,
        installed_path: Some(installed.path().to_string_lossy().to_string()),
        seeds: Some(HashMap::from([
            (".houston/board.json".into(), r#"{"columns":[]}"#.into()),
            (".houston/activity.json".into(), "[]".into()),
        ])),
        provider: None,
        model: None,
        effort: None,
        agent_path: None,
    })
    .unwrap();

    assert_eq!(bundle.seeds[".houston/board.json"], r#"{"columns":[]}"#);
    assert!(bundle.seeds.contains_key(".houston/routines/routines.json"));
    assert!(!bundle.seeds.contains_key(".houston/activity.json"));
}

#[test]
fn template_mode_drops_activity_seeds_from_manifest() {
    let installed = TempDir::new().unwrap();
    fs::write(
        installed.path().join("houston.json"),
        r#"{"id":"demo","agentSeeds":{".houston/activity/activity.json":"[]","README.md":"hello"}}"#,
    )
    .unwrap();

    let bundle = build_bootstrap_bundle(BuildBootstrapBundleRequest {
        config_id: "demo".into(),
        name: "Blank".into(),
        color: None,
        claude_md: None,
        installed_path: Some(installed.path().to_string_lossy().to_string()),
        seeds: None,
        provider: None,
        model: None,
        effort: None,
        agent_path: None,
    })
    .unwrap();

    assert!(!bundle.seeds.contains_key(".houston/activity/activity.json"));
    assert_eq!(bundle.seeds["README.md"], "hello");
}

#[test]
fn template_mode_drops_activity_seeds_from_request() {
    let bundle = build_bootstrap_bundle(BuildBootstrapBundleRequest {
        config_id: "blank".into(),
        name: "Blank".into(),
        color: None,
        claude_md: Some("custom".into()),
        installed_path: None,
        seeds: Some(HashMap::from([
            (".houston/activity.json".into(), "[]".into()),
            ("AGENTS.md".into(), "# agents".into()),
        ])),
        provider: None,
        model: None,
        effort: None,
        agent_path: None,
    })
    .unwrap();

    assert!(!bundle.seeds.contains_key(".houston/activity.json"));
    assert_eq!(bundle.seeds.get("AGENTS.md").map(String::as_str), Some("# agents"));
}

#[test]
fn template_mode_resolves_bundled_store_without_installed_path() {
    let store_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../store");
    if !store_root.join("catalog.json").exists() {
        return;
    }
    let bookkeeping = store_root.join("agents/bookkeeping");
    if !bookkeeping.join("houston.json").exists() {
        return;
    }

    std::env::set_var("HOUSTON_STORE_DIR", store_root.to_string_lossy().as_ref());

    let bundle = build_bootstrap_bundle(BuildBootstrapBundleRequest {
        config_id: "bookkeeping".into(),
        name: "Bookkeeping burst".into(),
        color: None,
        claude_md: None,
        installed_path: None,
        seeds: None,
        provider: None,
        model: None,
        effort: None,
        agent_path: None,
    })
    .unwrap();

    std::env::remove_var("HOUSTON_STORE_DIR");

    assert!(!bundle.skills.is_empty(), "bookkeeping must ship skills");
    assert!(
        bundle.skills.iter().any(|s| s.slug == "log-an-expense"),
        "expected log-an-expense skill"
    );
    assert_eq!(bundle.source.as_ref().unwrap().kind, "custom");
}

#[test]
fn agent_path_mode_reads_disk_state() {
    let docs = TempDir::new().unwrap();
    let ws = workspaces::create(
        docs.path(),
        CreateWorkspace {
            name: "Acme".into(),
        },
    )
    .unwrap();
    let agent_dir = docs.path().join("Acme/Ops");
    fs::create_dir_all(agent_dir.join(".houston")).unwrap();
    fs::create_dir_all(agent_dir.join(".agents/skills/demo")).unwrap();
    fs::write(agent_dir.join("CLAUDE.md"), "## Live agent").unwrap();
    fs::write(agent_dir.join(".agents/skills/demo/SKILL.md"), "live skill").unwrap();
    fs::write(
        agent_dir.join(".houston/agent.json"),
        r#"{"id":"agent-1","config_id":"demo","color":"green","created_at":"2026-01-01T00:00:00Z"}"#,
    )
    .unwrap();
    fs::create_dir_all(agent_dir.join(".houston/config")).unwrap();
    fs::write(
        agent_dir.join(".houston/config/config.json"),
        r#"{"provider":"openai","model":"gpt-5"}"#,
    )
    .unwrap();
    fs::create_dir_all(agent_dir.join(".houston/routines")).unwrap();
    fs::write(
        agent_dir.join(".houston/routines/routines.json"),
        r#"[{"id":"r1","name":"Daily","description":"","prompt":"hi","schedule":"0 9 * * *","enabled":true,"suppress_when_silent":true,"chat_mode":"shared","integrations":[],"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}]"#,
    )
    .unwrap();

    let bundle = build_bootstrap_bundle(BuildBootstrapBundleRequest {
        config_id: "demo".into(),
        name: "Ops".into(),
        color: None,
        claude_md: None,
        installed_path: None,
        seeds: None,
        provider: None,
        model: None,
        effort: None,
        agent_path: Some(agent_dir.to_string_lossy().to_string()),
    })
    .unwrap();

    let _ = ws;
    assert_eq!(bundle.claude_md, "## Live agent");
    assert_eq!(bundle.skills[0].skill_md, "live skill");
    assert_eq!(bundle.color.as_deref(), Some("green"));
    assert_eq!(
        bundle.config_patch.as_ref().unwrap().provider.as_deref(),
        Some("openai")
    );
    assert!(bundle.seeds.contains_key(".houston/routines/routines.json"));
}

#[test]
fn seeds_from_manifest_skips_activity() {
    let manifest: serde_json::Value = serde_json::from_str(
        r#"{"agentSeeds":{".houston/activity.json":"[]","README.md":"hello"}}"#,
    )
    .unwrap();
    let seeds = seeds_from_manifest(&manifest);
    assert!(!seeds.contains_key(".houston/activity.json"));
    assert_eq!(seeds["README.md"], "hello");
}

#[test]
fn resolve_github_source() {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join(".source.json"),
        r#"{"repo":"owner/repo","installed_at":"2026-01-01T00:00:00Z"}"#,
    )
    .unwrap();
    let source = resolve_from_installed(dir.path(), "demo").unwrap().unwrap();
    assert_eq!(source.kind, "github");
    assert_eq!(source.id, "owner/repo");
}

#[test]
fn read_packaged_skills_sorts_slugs() {
    let dir = TempDir::new().unwrap();
    for slug in ["beta", "alpha"] {
        let skill_dir = dir.path().join(slug);
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), slug).unwrap();
    }
    let skills = read_packaged_skills(dir.path()).unwrap();
    assert_eq!(skills[0].slug, "alpha");
    assert_eq!(skills[1].slug, "beta");
}
