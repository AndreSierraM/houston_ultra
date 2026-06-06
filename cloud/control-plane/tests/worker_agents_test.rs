//! Worker burst wire contract and runtime mode parsing.

use houston_cloud_control_plane::agents::{CreateCloudAgent, CreateWorkerAgent};
use serde_json::json;

#[test]
fn create_cloud_agent_deserializes_worker_fields() {
    let raw = json!({
        "name": "Coordinator",
        "configId": "bookkeeping",
        "parentAgentId": "550e8400-e29b-41d4-a716-446655440000",
        "workerTtlSeconds": 3600,
        "runtimeMode": "cloud_worker"
    });
    let body: CreateCloudAgent = serde_json::from_value(raw).expect("deserialize");
    assert_eq!(body.config_id, "bookkeeping");
    assert!(body.parent_agent_id.is_some());
    assert_eq!(body.worker_ttl_seconds, Some(3600));
    assert_eq!(body.runtime_mode.as_deref(), Some("cloud_worker"));
}

#[test]
fn create_worker_agent_deserializes_optional_body() {
    let raw = json!({
        "name": "Burst worker A",
        "workerTtlSeconds": 900
    });
    let body: CreateWorkerAgent = serde_json::from_value(raw).expect("deserialize");
    assert_eq!(body.name.as_deref(), Some("Burst worker A"));
    assert_eq!(body.worker_ttl_seconds, Some(900));
}

#[test]
fn create_worker_agent_defaults_empty_body() {
    let body: CreateWorkerAgent = serde_json::from_value(json!({})).expect("deserialize");
    assert!(body.name.is_none());
    assert!(body.worker_ttl_seconds.is_none());
}
