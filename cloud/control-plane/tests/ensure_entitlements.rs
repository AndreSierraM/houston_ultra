//! Org entitlement backfill when membership exists without a subscription row.

use houston_cloud_control_plane::db::Db;
use uuid::Uuid;

async fn maybe_db() -> Option<Db> {
    let url = std::env::var("DATABASE_URL").ok()?;
    let db = Db::connect(&url).await.ok()?;
    db.migrate().await.ok()?;
    Some(db)
}

#[tokio::test]
async fn ensure_user_org_backfills_missing_entitlements() {
    let Some(db) = maybe_db().await else {
        eprintln!("skip ensure_user_org_backfills_missing_entitlements: DATABASE_URL unset");
        return;
    };

    let org_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
        .bind(org_id)
        .bind(format!("entitlement-backfill-{org_id}"))
        .execute(db.pool())
        .await
        .expect("org insert");
    sqlx::query(
        "INSERT INTO organization_members (org_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(org_id)
    .bind(user_id)
    .execute(db.pool())
    .await
    .expect("member insert");

    let (resolved_org, role) = db
        .ensure_user_org(user_id, Some("backfill@test.local"))
        .await
        .expect("ensure_user_org");
    assert_eq!(resolved_org, org_id);
    assert_eq!(role, "owner");

    let status: String = sqlx::query_scalar(
        "SELECT status FROM cloud_entitlements WHERE org_id = $1",
    )
    .bind(org_id)
    .fetch_one(db.pool())
    .await
    .expect("entitlement row");
    assert_eq!(status, "active");
}
