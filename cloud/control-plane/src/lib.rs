//! Houston Cloud control plane library.

pub mod agent_access;
pub mod agents;
pub mod audit;
pub mod auth;
pub mod config;
pub mod db;
pub mod docker_runtime;
pub mod engine_provision;
pub mod entitlements;
pub mod error;
pub mod proxy;
pub mod routes;
pub mod runtime;
pub mod shares;
pub mod state;
pub mod ws_proxy;

use axum::{middleware, Router};
use axum::http::{HeaderValue, Method};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use config::Config;
pub use state::AppState;

fn cors_layer() -> CorsLayer {
    let raw = std::env::var("HOUSTON_CLOUD_CORS_ORIGINS").unwrap_or_else(|_| "*".into());
    let layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);
    if raw.trim() == "*" {
        layer.allow_origin(Any)
    } else {
        let origins: Vec<HeaderValue> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(HeaderValue::from_str)
            .collect::<Result<_, _>>()
            .expect("invalid HOUSTON_CLOUD_CORS_ORIGINS");
        layer.allow_origin(AllowOrigin::list(origins))
    }
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let cloud = routes::router(state.clone()).layer(middleware::from_fn_with_state(
        state.clone(),
        auth::require_auth,
    ));
    Router::new()
        .nest("/v1/cloud", cloud)
        .route("/health", axum::routing::get(|| async { "ok" }))
        .layer(cors_layer())
        .layer(TraceLayer::new_for_http())
}
