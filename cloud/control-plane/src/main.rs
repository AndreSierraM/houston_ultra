//! `houston-cloud-control-plane` binary entry point.

use houston_cloud_control_plane::auth::AuthState;
use houston_cloud_control_plane::config::{generate_local_token, AuthMode, Config};
use houston_cloud_control_plane::db::Db;
use houston_cloud_control_plane::{build_router, AppState};
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,houston_cloud_control_plane=debug".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    let db = Db::connect(&cfg.database_url).await?;
    db.migrate().await?;

    let (local_token, generated) = match cfg.auth_mode {
        AuthMode::Local => {
            if let Some(t) = cfg.local_token.clone() {
                (t, false)
            } else {
                (generate_local_token(), true)
            }
        }
        AuthMode::Jwt => (String::new(), false),
    };

    if generated {
        tracing::warn!(
            token = %local_token,
            "HOUSTON_CLOUD_TOKEN not set — generated ephemeral local token (set env to persist)"
        );
    }

    let auth = AuthState {
        mode: cfg.auth_mode,
        jwt_secret: cfg.jwt_secret.clone(),
        local_token,
        local_user_id: cfg.local_user_id,
        local_email: cfg.local_email.clone(),
        db: db.clone(),
    };

    let runtime = AppState::docker_runtime(cfg.engine_image.clone(), cfg.docker_socket.clone());
    let state = AppState::new(db, auth, runtime);
    let router = build_router(state);
    let addr: SocketAddr = cfg.bind.parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, auth_mode = ?cfg.auth_mode, "houston cloud control plane listening");
    axum::serve(listener, router).await?;
    Ok(())
}
