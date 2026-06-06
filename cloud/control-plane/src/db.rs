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
        let sql = include_str!("../migrations/001_init.sql");
        for stmt in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            sqlx::query(stmt).execute(&self.pool).await?;
        }
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
        sqlx::query(
            "INSERT INTO cloud_entitlements (org_id, status, max_cloud_agents, max_storage_gb, max_members)
             VALUES ($1, 'active', 4, 10, 5)",
        )
        .bind(org_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        Ok((org_id, "owner".into()))
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
