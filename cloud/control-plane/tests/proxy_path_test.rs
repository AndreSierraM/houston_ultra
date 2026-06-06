//! Integration-style contract tests for cloud engine proxy paths and RBAC.

use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::{from_fn, Next},
    response::Response,
    routing::{any, get},
    Router,
};
use houston_cloud_control_plane::agent_access::{
    assert_agent_access_for_method, assert_agent_access_for_proxy, is_credential_route,
    min_role_for_method, min_role_for_proxy_path,
};
use houston_cloud_control_plane::auth::Principal;
use houston_cloud_control_plane::db::Db;
use houston_cloud_control_plane::proxy::{append_proxy_query, build_proxy_target};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn catch_all_proxy_route_extracts_id_and_tail() {
    let agent_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    async fn handler(
        axum::extract::Path((id, tail)): axum::extract::Path<(Uuid, String)>,
    ) -> String {
        format!("{id}:{tail}")
    }

    let app = Router::new().route("/agents/:id/proxy/*tail", any(handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let base = format!("http://{addr}/agents/{agent_id}/proxy");
    for (suffix, want_tail) in [
        ("/v1/health", "v1/health"),
        ("/v1/sessions/abc", "v1/sessions/abc"),
    ] {
        let url = format!("{base}{suffix}");
        let resp = reqwest::get(&url).await.expect("catch-all proxy route");
        assert_eq!(resp.status(), 200, "url={url}");
        assert_eq!(
            resp.text().await.expect("body"),
            format!("{agent_id}:{want_tail}"),
            "url={url}"
        );
    }
}

/// Mirrors `build_router`: the cloud router (with a layer, like `require_auth`)
/// mounts the proxy as a wildcard route, then the whole thing is nested under
/// `/v1/cloud`. A `nest(...).fallback(...)` inner router silently misses under
/// this double-nest + layer combo and returned a bare 404 ("Engine error 404")
/// in production. The wildcard route must resolve the full client path.
#[tokio::test]
async fn double_nested_proxy_route_matches_full_client_path() {
    let agent_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    async fn handler(
        axum::extract::Path((id, tail)): axum::extract::Path<(Uuid, String)>,
    ) -> String {
        format!("{id}:{tail}")
    }
    async fn passthrough(req: Request, next: Next) -> Response {
        next.run(req).await
    }

    let cloud = Router::new()
        .route("/agents/:id/proxy/*tail", any(handler))
        .layer(from_fn(passthrough));
    let app = Router::new().nest("/v1/cloud", cloud);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let url = format!("http://{addr}/v1/cloud/agents/{agent_id}/proxy/v1/health");
    let resp = reqwest::get(&url).await.expect("proxy route");
    assert_eq!(resp.status(), 200, "double-nested wildcard proxy must match");
    assert_eq!(resp.text().await.expect("body"), format!("{agent_id}:v1/health"));

    let sessions = format!(
        "http://{addr}/v1/cloud/agents/{agent_id}/proxy/v1/agents/%2Fdata%2Fws%2Fagent/sessions"
    );
    let resp = reqwest::Client::new()
        .post(&sessions)
        .body("{}")
        .send()
        .await
        .expect("proxy route with encoded agent_path segment");
    assert_eq!(
        resp.status(),
        200,
        "wildcard proxy must match engine session paths (encoded slashes in one segment)"
    );
}

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

#[test]
fn credential_proxy_paths_require_admin_role() {
    let tail = "v1/providers/anthropic/credential-import";
    assert!(is_credential_route(tail));
    assert_eq!(min_role_for_proxy_path(tail, &Method::POST), "admin");
    assert_eq!(min_role_for_proxy_path(tail, &Method::GET), "admin");
}

#[tokio::test]
async fn assert_agent_access_for_proxy_blocks_operator_on_credential_import() {
    let db = match test_db().await {
        Some(db) => db,
        None => return,
    };
    let org_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();
    let operator_id = Uuid::new_v4();
    seed_operator_share(&db, org_id, agent_id, operator_id).await;

    let principal = Principal {
        user_id: operator_id,
        email: Some("operator@example.com".into()),
        org_id,
        org_role: "member".into(),
    };

    assert_agent_access_for_method(&db, &principal, agent_id, &Method::POST)
        .await
        .expect("operator may POST non-credential routes");

    let err = assert_agent_access_for_proxy(
        &db,
        &principal,
        agent_id,
        "v1/providers/openai/credential-import",
        &Method::POST,
    )
    .await
    .expect_err("operator must not proxy credential import");
    assert_eq!(err.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn proxy_target_forwards_query_string_to_engine() {
    let seen = Arc::new(Mutex::new(String::new()));
    let seen_capture = seen.clone();
    let app = Router::new().route(
        "/*path",
        get(move |req: Request| {
            let seen_capture = seen_capture.clone();
            async move {
                let uri = req.uri().to_string();
                *seen_capture.lock().expect("lock") = uri;
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
    let target = append_proxy_query(
        &build_proxy_target(&internal_url, "v1/sessions"),
        Some("limit=5&cursor=xyz"),
    );
    assert_eq!(target, format!("{internal_url}/v1/sessions?limit=5&cursor=xyz"));

    let resp = reqwest::get(&target).await.expect("forward");
    assert_eq!(resp.status(), 200);
    assert_eq!(
        *seen.lock().expect("lock"),
        "/v1/sessions?limit=5&cursor=xyz",
        "engine must receive proxied query string"
    );
}

#[tokio::test]
async fn assert_agent_access_for_proxy_blocks_unrelated_user() {
    let db = match test_db().await {
        Some(db) => db,
        None => return,
    };
    let org_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let stranger_id = Uuid::new_v4();
    seed_agent_without_share(&db, org_id, agent_id, owner_id).await;

    let principal = Principal {
        user_id: stranger_id,
        email: Some("stranger@example.com".into()),
        org_id,
        org_role: "member".into(),
    };

    let err = assert_agent_access_for_proxy(
        &db,
        &principal,
        agent_id,
        "v1/health",
        &Method::GET,
    )
    .await
    .expect_err("unrelated user must not proxy to engine");
    assert_eq!(err.status, StatusCode::FORBIDDEN);
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

async fn seed_agent_without_share(db: &Db, org_id: Uuid, agent_id: Uuid, owner_id: Uuid) {
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
    .bind(owner_id)
    .execute(db.pool())
    .await
    .expect("agent");
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

async fn seed_operator_share(db: &Db, org_id: Uuid, agent_id: Uuid, operator_id: Uuid) {
    let owner_id = Uuid::new_v4();
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
    .bind(owner_id)
    .execute(db.pool())
    .await
    .expect("agent");
    sqlx::query(
        "INSERT INTO cloud_agent_shares (agent_id, user_id, role)
         VALUES ($1, $2, 'operator')",
    )
    .bind(agent_id)
    .bind(operator_id)
    .execute(db.pool())
    .await
    .expect("share");
}
