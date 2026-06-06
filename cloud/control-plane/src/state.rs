//! Shared application state.

use crate::auth::AuthState;
use crate::db::Db;
use crate::docker_runtime::DockerRuntime;
use crate::proxy::ProxyState;
use crate::runtime::RuntimeBackend;
use std::sync::Arc;

pub struct AppState {
    pub db: Db,
    pub auth: AuthState,
    pub runtime: Arc<dyn RuntimeBackend>,
    pub proxy: ProxyState,
}

impl AppState {
    pub fn new(db: Db, auth: AuthState, runtime: Arc<dyn RuntimeBackend>) -> Arc<Self> {
        Arc::new(Self {
            db,
            auth,
            runtime,
            proxy: ProxyState {
                http: reqwest::Client::new(),
            },
        })
    }

    pub fn docker_runtime(engine_image: String, docker_socket: String) -> Arc<dyn RuntimeBackend> {
        Arc::new(DockerRuntime {
            engine_image,
            docker_socket,
        })
    }
}
