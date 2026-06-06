//! Integration tests for `POST /v1/agents/bootstrap-bundle`.

use houston_engine_server::{build_router, ServerConfig, ServerState};
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;

async fn spawn_server() -> (SocketAddr, String, TempDir, TempDir) {
    let token = "bootstrap-token".to_string();
    let docs = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let cfg = ServerConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        token: token.clone(),
        home_dir: home.path().to_path_buf(),
        docs_dir: docs.path().to_path_buf(),
        app_system_prompt: String::new(),
        app_onboarding_prompt: String::new(),
        tunnel_url: "http://test.invalid".into(),
    };
    let listener = TcpListener::bind(cfg.bind).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = Arc::new(ServerState::new_in_memory(cfg).await.unwrap());
    let app = build_router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, token, home, docs)
}

#[tokio::test]
async fn bootstrap_bundle_from_installed_template() {
    let (addr, tok, home, _docs) = spawn_server().await;
    let installed = home.path().join("agents/demo");
    fs::create_dir_all(installed.join(".agents/skills/demo")).unwrap();
    fs::write(installed.join("CLAUDE.md"), "## Demo").unwrap();
    fs::write(
        installed.join("houston.json"),
        r#"{"id":"demo","version":"2.0.0","agentSeeds":{"AGENTS.md":"agents file"}}"#,
    )
    .unwrap();
    fs::write(installed.join(".agents/skills/demo/SKILL.md"), "skill").unwrap();
    fs::write(
        installed.join(".source.json"),
        r#"{"source":"houston-store","agent_id":"demo","version":"2.0.0"}"#,
    )
    .unwrap();

    let bundle: serde_json::Value = reqwest::Client::new()
        .post(format!("http://{addr}/v1/agents/bootstrap-bundle"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "configId": "demo",
            "name": "Ops",
            "installedPath": installed.to_string_lossy(),
            "provider": "anthropic",
            "model": "sonnet"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(bundle["claudeMd"], "## Demo");
    assert_eq!(bundle["skills"][0]["slug"], "demo");
    assert_eq!(bundle["seeds"]["AGENTS.md"], "agents file");
    assert_eq!(bundle["configPatch"]["provider"], "anthropic");
    assert_eq!(bundle["source"]["kind"], "houston-store");
}

#[tokio::test]
async fn bootstrap_bundle_rejects_missing_agent_path() {
    let (addr, tok, _home, _docs) = spawn_server().await;
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/v1/agents/bootstrap-bundle"))
        .bearer_auth(&tok)
        .json(&serde_json::json!({
            "configId": "demo",
            "name": "Ops",
            "agentPath": "/tmp/does-not-exist"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}
