//! Share role enforcement against Postgres (runs when `DATABASE_URL` is set).

use axum::http::StatusCode;
use houston_cloud_control_plane::agent_access;
use houston_cloud_control_plane::agents;
use houston_cloud_control_plane::auth::Principal;
use houston_cloud_control_plane::db::Db;
use houston_cloud_control_plane::shares::{self, UpsertShare};
use uuid::Uuid;

struct ShareFixture {
    db: Db,
    org_id: Uuid,
    owner_id: Uuid,
    viewer_id: Uuid,
    operator_id: Uuid,
    admin_id: Uuid,
    agent_id: Uuid,
}

async fn maybe_db() -> Option<Db> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let db = Db::connect(&url).await.ok()?;
    db.migrate().await.ok()?;
    Some(db)
}

fn principal(user_id: Uuid, org_id: Uuid, org_role: &str) -> Principal {
    Principal {
        user_id,
        email: None,
        org_id,
        org_role: org_role.to_string(),
    }
}

async fn seed_share_fixture(db: &Db) -> ShareFixture {
    let org_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let viewer_id = Uuid::new_v4();
    let operator_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    let agent_id = Uuid::new_v4();

    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
        .bind(org_id)
        .bind(format!("share-test-{org_id}"))
        .execute(db.pool())
        .await
        .expect("org insert");
    for (user_id, role) in [
        (owner_id, "owner"),
        (viewer_id, "member"),
        (operator_id, "member"),
        (admin_id, "member"),
    ] {
        sqlx::query(
            "INSERT INTO organization_members (org_id, user_id, role) VALUES ($1, $2, $3)",
        )
        .bind(org_id)
        .bind(user_id)
        .bind(role)
        .execute(db.pool())
        .await
        .expect("member insert");
    }
    sqlx::query(
        "INSERT INTO cloud_entitlements (org_id, status, max_cloud_agents, max_storage_gb, max_members)
         VALUES ($1, 'active', 10, 10, 10)",
    )
    .bind(org_id)
    .execute(db.pool())
    .await
    .expect("entitlement insert");
    sqlx::query(
        "INSERT INTO cloud_agents (id, org_id, owner_user_id, name, config_id, folder_path)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(agent_id)
    .bind(org_id)
    .bind(owner_id)
    .bind("shared-agent")
    .bind("default")
    .bind("/cloud/shared-agent")
    .execute(db.pool())
    .await
    .expect("agent insert");
    for (user_id, role) in [
        (viewer_id, "viewer"),
        (operator_id, "operator"),
        (admin_id, "admin"),
    ] {
        sqlx::query(
            "INSERT INTO cloud_agent_shares (agent_id, user_id, role) VALUES ($1, $2, $3)",
        )
        .bind(agent_id)
        .bind(user_id)
        .bind(role)
        .execute(db.pool())
        .await
        .expect("share insert");
    }

    ShareFixture {
        db: db.clone(),
        org_id,
        owner_id,
        viewer_id,
        operator_id,
        admin_id,
        agent_id,
    }
}

#[tokio::test]
async fn share_role_enforcement_with_database() {
    let Some(db) = maybe_db().await else {
        eprintln!("skip share_role_enforcement_with_database: DATABASE_URL not set");
        return;
    };
    let fx = seed_share_fixture(&db).await;
    let owner = principal(fx.owner_id, fx.org_id, "owner");
    let viewer = principal(fx.viewer_id, fx.org_id, "member");
    let operator = principal(fx.operator_id, fx.org_id, "member");
    let admin = principal(fx.admin_id, fx.org_id, "member");

    agent_access::assert_agent_access(&fx.db, &viewer, fx.agent_id, "viewer")
        .await
        .expect("viewer can read");
    let viewer_admin = agent_access::assert_agent_access(&fx.db, &viewer, fx.agent_id, "admin").await;
    assert_eq!(viewer_admin.unwrap_err().status, StatusCode::FORBIDDEN);

    agent_access::assert_agent_access(&fx.db, &operator, fx.agent_id, "operator")
        .await
        .expect("operator can operate");
    let operator_admin =
        agent_access::assert_agent_access(&fx.db, &operator, fx.agent_id, "admin").await;
    assert_eq!(operator_admin.unwrap_err().status, StatusCode::FORBIDDEN);

    agent_access::assert_agent_access(&fx.db, &admin, fx.agent_id, "admin")
        .await
        .expect("admin share can admin");

    shares::list_shares(&fx.db, &owner, fx.agent_id)
        .await
        .expect("owner lists shares");
    shares::list_shares(&fx.db, &admin, fx.agent_id)
        .await
        .expect("admin share lists shares");
    let viewer_list = shares::list_shares(&fx.db, &viewer, fx.agent_id).await;
    assert_eq!(viewer_list.unwrap_err().status, StatusCode::FORBIDDEN);

    let listed = agents::list_agents(&fx.db, &viewer).await.expect("list agents");
    assert!(
        listed.iter().any(|a| a.id == fx.agent_id.to_string()),
        "shared agent appears in list_agents"
    );
    let shared_only = agents::list_shared_agents(&fx.db, &viewer)
        .await
        .expect("list shared agents");
    assert_eq!(shared_only.len(), 1);
    assert_eq!(shared_only[0].id, fx.agent_id.to_string());

    let owner_shared = agents::list_shared_agents(&fx.db, &owner)
        .await
        .expect("owner shared list");
    assert!(owner_shared.is_empty(), "owner is not a share recipient");

    shares::revoke_share(&fx.db, &owner, fx.agent_id, fx.viewer_id)
        .await
        .expect("revoke viewer share");
    let after_revoke =
        agent_access::assert_agent_access(&fx.db, &viewer, fx.agent_id, "viewer").await;
    assert_eq!(after_revoke.unwrap_err().status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn cross_org_shared_agent_is_listed() {
    let Some(db) = maybe_db().await else {
        eprintln!("skip cross_org_shared_agent_is_listed: DATABASE_URL not set");
        return;
    };
    let fx = seed_share_fixture(&db).await;
    let other_org = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
        .bind(other_org)
        .bind(format!("other-{other_org}"))
        .execute(db.pool())
        .await
        .expect("other org");
    sqlx::query(
        "INSERT INTO organization_members (org_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(other_org)
    .bind(fx.viewer_id)
    .execute(db.pool())
    .await
    .expect("viewer other org member");

    let viewer = principal(fx.viewer_id, other_org, "owner");
    let listed = agents::list_agents(&db, &viewer).await.expect("list agents");
    assert!(
        listed.iter().any(|a| a.id == fx.agent_id.to_string()),
        "cross-org share still visible in list_agents"
    );
}

#[tokio::test]
async fn upsert_share_rejects_invalid_role() {
    let Some(db) = maybe_db().await else {
        eprintln!("skip upsert_share_rejects_invalid_role: DATABASE_URL not set");
        return;
    };
    let fx = seed_share_fixture(&db).await;
    let owner = principal(fx.owner_id, fx.org_id, "owner");
    let err = shares::upsert_share(
        &fx.db,
        &owner,
        fx.agent_id,
        UpsertShare {
            user_id: Uuid::new_v4(),
            role: "superuser".into(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
}
