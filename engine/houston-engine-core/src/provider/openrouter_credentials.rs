//! Persist an OpenRouter API key to `~/.houston/providers/openrouter/.env`.
//!
//! Legacy read: `~/.houston/openrouter/.env`. Houston injects
//! `OPENROUTER_API_KEY` into Codex subprocesses at spawn time.

use super::provider_env_store::{read_stored_api_key, set_api_key, strip_api_key_from_storage};
use crate::error::{CoreError, CoreResult};
use houston_terminal_manager::provider_env::canonical_env_path;

pub const ENV_VAR: &str = "OPENROUTER_API_KEY";
pub const ENV_REL_PATH: &str = ".houston/providers/openrouter/.env";
const PROVIDER: &str = "openrouter";

pub async fn set_openrouter_api_key(api_key: &str) -> CoreResult<()> {
    set_api_key(PROVIDER, ENV_VAR, api_key, validate_key).await
}

pub async fn read_openrouter_api_key() -> CoreResult<Option<String>> {
    read_stored_api_key(PROVIDER, ENV_VAR).await
}

pub fn resolve_env_path() -> CoreResult<std::path::PathBuf> {
    dirs::home_dir().ok_or_else(|| {
        CoreError::Internal(
            "could not resolve home directory for ~/.houston/providers/openrouter/.env".into(),
        )
    })?;
    Ok(canonical_env_path(PROVIDER))
}

fn validate_key(api_key: &str) -> CoreResult<&str> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err(CoreError::BadRequest("API key cannot be empty".into()));
    }
    if trimmed.len() < 20 || trimmed.len() > 300 {
        return Err(CoreError::BadRequest(
            "API key length looks wrong. OpenRouter keys are usually longer than 20 characters."
                .into(),
        ));
    }
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return Err(CoreError::BadRequest(
            "API key cannot contain whitespace. Paste only the key value.".into(),
        ));
    }
    if trimmed.contains('"') || trimmed.contains('\'') {
        return Err(CoreError::BadRequest(
            "API key cannot contain quote characters. Paste the raw key value.".into(),
        ));
    }
    Ok(trimmed)
}

pub(super) fn is_openrouter_api_key_line(line: &str) -> bool {
    houston_terminal_manager::provider_env::is_env_var_line(line, ENV_VAR)
}

pub async fn strip_openrouter_api_key_storage() -> CoreResult<()> {
    strip_api_key_from_storage(PROVIDER, ENV_VAR).await
}

pub(super) use super::provider_env_store::write_atomic;

#[cfg(test)]
mod tests {
    use super::*;
    use houston_terminal_manager::provider_env::merge_env_contents;

    #[test]
    fn validate_rejects_empty() {
        assert!(matches!(
            validate_key(""),
            Err(CoreError::BadRequest(_))
        ));
    }

    #[test]
    fn merge_appends_to_empty_file() {
        let out = merge_env_contents("", ENV_VAR, "sk-or-v1-testkey1234567890");
        assert_eq!(out, "OPENROUTER_API_KEY=sk-or-v1-testkey1234567890\n");
    }

    #[tokio::test]
    async fn set_openrouter_api_key_rejects_empty_input() {
        let err = set_openrouter_api_key("").await.unwrap_err();
        assert!(matches!(err, CoreError::BadRequest(_)));
    }
}
