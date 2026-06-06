//! Postgres access layer.

use crate::error::{ApiError, ApiResult};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

impl Db {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        for sql in [
            include_str!("../migrations/001_init.sql"),
            include_str!("../migrations/002_worker_agents.sql"),
        ] {
            for stmt in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(stmt).execute(&self.pool).await?;
            }
        }
        Ok(())
    }

    /// Ensures every org has a subscription row. New orgs get active defaults; existing
    /// orgs missing a row (partial seed, manual DB, pre-entitlements schema) are backfilled.
    pub async fn ensure_org_entitlements(&self, org_id: Uuid) -> ApiResult<()> {
        sqlx::query(
            "INSERT INTO cloud_entitlements (org_id, status, max_cloud_agents, max_storage_gb, max_members)
             VALUES ($1, 'active', 100000, 10, 5)
             ON CONFLICT (org_id) DO NOTHING",
        )
        .bind(org_id)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        // Lift legacy dev defaults (4/8) even when migrate already ran before this org existed.
        sqlx::query(
            "UPDATE cloud_entitlements SET max_cloud_agents = 100000, updated_at = now()
             WHERE org_id = $1 AND max_cloud_agents IN (4, 8)",
        )
        .bind(org_id)
        .execute(&self.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        Ok(())
    }

    pub async fn ensure_user_org(
        &self,
        user_id: Uuid,
        email: Option<&str>,
    ) -> ApiResult<(Uuid, String)> {
        let existing = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT org_id, role FROM organization_members WHERE user_id = $1 LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        if let Some((org_id, role)) = existing {
            self.ensure_org_entitlements(org_id).await?;
            return Ok((org_id, role));
        }
        let org_name = email
            .map(|e| format!("{e}'s org"))
            .unwrap_or_else(|| format!("org-{user_id}"));
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let org_id: Uuid = sqlx::query_scalar(
            "INSERT INTO organizations (name) VALUES ($1) RETURNING id",
        )
        .bind(&org_name)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        sqlx::query(
            "INSERT INTO organization_members (org_id, user_id, role) VALUES ($1, $2, 'owner')",
        )
        .bind(org_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        self.ensure_org_entitlements(org_id).await?;
        Ok((org_id, "owner".into()))
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
