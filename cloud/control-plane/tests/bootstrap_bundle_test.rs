//! Wire contract tests for bootstrap bundle and create payload parsing.

use houston_cloud_control_plane::agents::CreateCloudAgent;
use houston_cloud_control_plane::bootstrap_bundle::resolve_bootstrap;
use serde_json::json;

#[test]
fn create_cloud_agent_deserializes_bootstrap_bundle_and_credential_sync() {
    let raw = json!({
        "name": "Research",
        "configId": "default",
        "bootstrapBundle": {
            "configId": "store-alpha",
            "name": "Research",
            "claudeMd": "Instructions",
            "skills": [{ "slug": "draft-email", "skillMd": "skill body" }],
            "seeds": [{ "relPath": ".houston/goals/goals.json", "content": "[]" }],
            "configPatch": { "provider": "anthropic", "model": "sonnet", "effort": "high" },
            "source": { "kind": "store", "id": "alpha" }
        },
        "credentialSync": {
            "provider": "anthropic",
            "importBody": { "sessionId": "sess-1", "ciphertext": "opaque" }
        }
    });
    let body: CreateCloudAgent = serde_json::from_value(raw).expect("deserialize");
    assert_eq!(body.name, "Research");
    assert!(body.bootstrap_bundle.is_some());
    assert!(body.credential_sync.is_some());
    let bundle = body.bootstrap_bundle.as_ref().unwrap();
    assert_eq!(bundle.skills.len(), 1);
    assert_eq!(bundle.seeds.len(), 1);
    let sync = body.credential_sync.as_ref().unwrap();
    assert_eq!(sync.provider, "anthropic");
    assert_eq!(sync.import_body["sessionId"], "sess-1");
}

#[test]
fn resolve_bootstrap_keeps_legacy_fields_without_bundle() {
    let resolved = resolve_bootstrap(
        "Local",
        "default",
        None,
        Some("legacy md".into()),
        Some("openai".into()),
        Some("gpt-4".into()),
        None,
    );
    assert_eq!(resolved.claude_md.as_deref(), Some("legacy md"));
    assert_eq!(resolved.provider.as_deref(), Some("openai"));
    assert!(resolved.skills.is_empty());
}
