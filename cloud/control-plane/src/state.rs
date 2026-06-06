//! Shared application state.

use crate::auth::AuthState;
use crate::db::Db;
use crate::config::RuntimeKind;
use crate::docker_runtime::DockerRuntime;
use crate::k8s_runtime::K8sRuntime;
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

    pub fn k8s_runtime(engine_image: String, kubectl_bin: String) -> Arc<dyn RuntimeBackend> {
        Arc::new(K8sRuntime {
            engine_image,
            kubectl_bin,
        })
    }

    pub fn runtime_for(
        kind: RuntimeKind,
        engine_image: String,
        docker_socket: String,
        kubectl_bin: String,
    ) -> Arc<dyn RuntimeBackend> {
        match kind {
            RuntimeKind::Docker => Self::docker_runtime(engine_image, docker_socket),
            RuntimeKind::K8s => Self::k8s_runtime(engine_image, kubectl_bin),
        }
    }
}
