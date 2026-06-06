//! Environment-driven control plane configuration.

use std::env;
use uuid::Uuid;

/// Where cloud agent engines are provisioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeKind {
    Docker,
    K8s,
}

/// How incoming bearer tokens are validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Static shared secret (`HOUSTON_CLOUD_TOKEN`). Dev / self-hosted default.
    Local,
    /// HS256 JWT signed with `HOUSTON_CLOUD_JWT_SECRET`.
    Jwt,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    pub auth_mode: AuthMode,
    /// Required for [`AuthMode::Jwt`].
    pub jwt_secret: Option<String>,
    /// Required for [`AuthMode::Local`]. Generated at startup when unset.
    pub local_token: Option<String>,
    pub local_user_id: Uuid,
    pub local_email: Option<String>,
    pub engine_image: String,
    pub runtime: RuntimeKind,
    pub docker_socket: String,
    pub kubectl_bin: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let auth_mode = parse_auth_mode()?;
        let jwt_secret = env::var("HOUSTON_CLOUD_JWT_SECRET")
            .ok()
            .or_else(|| env::var("SUPABASE_JWT_SECRET").ok())
            .filter(|s| !s.is_empty());
        let local_token = env::var("HOUSTON_CLOUD_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());

        match auth_mode {
            AuthMode::Jwt if jwt_secret.is_none() => {
                anyhow::bail!(
                    "HOUSTON_CLOUD_AUTH=jwt requires HOUSTON_CLOUD_JWT_SECRET (or SUPABASE_JWT_SECRET)"
                );
            }
            _ => {}
        }

        let local_user_id = env::var("HOUSTON_CLOUD_LOCAL_USER_ID")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| Uuid::parse_str(&s))
            .transpose()
            .map_err(|e| anyhow::anyhow!("invalid HOUSTON_CLOUD_LOCAL_USER_ID: {e}"))?
            .unwrap_or_else(|| Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap());

        Ok(Self {
            bind: env::var("HOUSTON_CLOUD_BIND").unwrap_or_else(|_| "0.0.0.0:8788".into()),
            database_url: env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?,
            auth_mode,
            jwt_secret,
            local_token,
            local_user_id,
            local_email: env::var("HOUSTON_CLOUD_LOCAL_EMAIL")
                .ok()
                .filter(|s| !s.is_empty()),
            engine_image: env::var("HOUSTON_ENGINE_IMAGE")
                .unwrap_or_else(|_| "houston/engine:dev".into()),
            runtime: parse_runtime_kind()?,
            docker_socket: env::var("DOCKER_HOST")
                .unwrap_or_else(|_| "unix:///var/run/docker.sock".into()),
            kubectl_bin: env::var("KUBECTL_BIN").unwrap_or_else(|_| "kubectl".into()),
        })
    }
}

fn parse_runtime_kind() -> anyhow::Result<RuntimeKind> {
    match env::var("HOUSTON_CLOUD_RUNTIME")
        .unwrap_or_else(|_| "docker".into())
        .to_lowercase()
        .as_str()
    {
        "docker" => Ok(RuntimeKind::Docker),
        "k8s" | "kubernetes" | "k3s" => Ok(RuntimeKind::K8s),
        other => anyhow::bail!("HOUSTON_CLOUD_RUNTIME must be 'docker' or 'k8s', got '{other}'"),
    }
}

fn parse_auth_mode() -> anyhow::Result<AuthMode> {
    match env::var("HOUSTON_CLOUD_AUTH")
        .unwrap_or_else(|_| "local".into())
        .to_lowercase()
        .as_str()
    {
        "local" => Ok(AuthMode::Local),
        "jwt" => Ok(AuthMode::Jwt),
        other => anyhow::bail!("HOUSTON_CLOUD_AUTH must be 'local' or 'jwt', got '{other}'"),
    }
}

pub fn generate_local_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::thread_rng().gen();
    format!("hst_{}", bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
}
