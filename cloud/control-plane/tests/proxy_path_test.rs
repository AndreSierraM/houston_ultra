//! Integration-style contract tests for cloud engine proxy paths and RBAC.

use axum::{
    extract::Request,
    http::{Method, StatusCode},
    routing::get,
    Router,
};
use houston_cloud_control_plane::agent_access::{
    assert_agent_access_for_method, min_role_for_method,
};
use houston_cloud_control_plane::auth::Principal;
use houston_cloud_control_plane::db::Db;
use houston_cloud_control_plane::proxy::build_proxy_target;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use uuid::Uuid;

#[test]
fn build_proxy_target_v1_health_no_double_v1() {
    let cases = [
        ("http://127.0.0.1:7777", "v1/health", "http://127.0.0.1:7777/v1/health"),
        (
            "http://127.0.0.1:7777/",
            "/v1/health",
            "http://127.0.0.1:7777/v1/health",
        ),
        (
            "http://engine:7777",
            "v1/sessions",
            "http://engine:7777/v1/sessions",
        ),
    ];
    for (base, tail, want) in cases {
        let got = build_proxy_target(base, tail);
        assert_eq!(got, want, "base={base} tail={tail}");
        assert!(
            !got.contains("/v1/v1/"),
            "must not double-prefix v1: {got}"
        );
    }
}

#[test]
fn min_role_for_method_get_vs_post() {
    assert_eq!(min_role_for_method(&Method::GET), "viewer");
    assert_eq!(min_role_for_method(&Method::HEAD), "viewer");
    assert_eq!(min_role_for_method(&Method::POST), "operator");
    assert_eq!(min_role_for_method(&Method::PUT), "operator");
    assert_eq!(min_role_for_method(&Method::PATCH), "operator");
    assert_eq!(min_role_for_method(&Method::DELETE), "operator");
}

#[tokio::test]
async fn v1_health_tail_forwards_to_engine_without_double_v1() {
    let seen = Arc::new(Mutex::new(String::new()));
    let seen_capture = seen.clone();
    let app = Router::new().route(
        "/*path",
        get(move |req: Request| {
            let seen_capture = seen_capture.clone();
            async move {
                let path = req.uri().path().to_string();
                *seen_capture.lock().expect("lock") = path;
                "ok"
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let internal_url = format!("http://{addr}");
    let tail = "v1/health";
    let target = build_proxy_target(&internal_url, tail);
    assert!(!target.contains("/v1/v1/"));
    assert_eq!(target, format!("{internal_url}/v1/health"));

    let resp = reqwest::get(&target).await.expect("forward");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.expect("body"), "ok");
    assert_eq!(
        *seen.lock().expect("lock"),
        "/v1/health",
        "engine must receive /v1/health once"
    );
}

#[tokio::test]
async fn assert_agent_access_for_method_get_vs_post_roles() {
    let db = match test_db().await {
        Some(db) => db,
        None => return,
    };
    let org_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();
    let viewer_id = Uuid::new_v4();
    seed_viewer_share(&db, org_id, agent_id, viewer_id).await;

    let principal = Principal {
        user_id: viewer_id,
        email: Some("viewer@example.com".into()),
        org_id,
        org_role: "member".into(),
    };

    assert_agent_access_for_method(&db, &principal, agent_id, &Method::GET)
        .await
        .expect("viewer may GET");

    let err = assert_agent_access_for_method(&db, &principal, agent_id, &Method::POST)
        .await
        .expect_err("viewer must not POST");
    assert_eq!(err.status, StatusCode::FORBIDDEN);
}

async fn test_db() -> Option<Db> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let db = Db::connect(&url).await.ok()?;
    db.migrate().await.ok()?;
    Some(db)
}

async fn seed_viewer_share(db: &Db, org_id: Uuid, agent_id: Uuid, viewer_id: Uuid) {
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, 'test-org')")
        .bind(org_id)
        .execute(db.pool())
        .await
        .expect("org");
    sqlx::query(
        "INSERT INTO cloud_agents (id, org_id, owner_user_id, name, config_id, folder_path)
         VALUES ($1, $2, $3, 'test', 'default', '/data/workspace')",
    )
    .bind(agent_id)
    .bind(org_id)
    .bind(viewer_id)
    .execute(db.pool())
    .await
    .expect("agent");
    sqlx::query(
        "INSERT INTO cloud_agent_shares (agent_id, user_id, role)
         VALUES ($1, $2, 'viewer')",
    )
    .bind(agent_id)
    .bind(viewer_id)
    .execute(db.pool())
    .await
    .expect("share");
}
