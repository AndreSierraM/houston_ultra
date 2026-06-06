//! Persist a Gemini API key to `~/.houston/providers/gemini/.env`.
//!
//! Legacy read: `~/.gemini/.env`. The gemini runtime HOME symlinks the
//! canonical file so spawned CLI sessions still load the key.

use super::provider_env_store::{read_stored_api_key, set_api_key, strip_api_key_from_storage};
use crate::error::{CoreError, CoreResult};

pub const ENV_VAR: &str = "GEMINI_API_KEY";
const PROVIDER: &str = "gemini";

pub async fn set_gemini_api_key(api_key: &str) -> CoreResult<()> {
    set_api_key(PROVIDER, ENV_VAR, api_key, validate_key).await
}

pub async fn read_gemini_api_key() -> CoreResult<Option<String>> {
    read_stored_api_key(PROVIDER, ENV_VAR).await
}

fn validate_key(api_key: &str) -> CoreResult<&str> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        return Err(CoreError::BadRequest("API key cannot be empty".into()));
    }
    if trimmed.len() < 10 || trimmed.len() > 200 {
        return Err(CoreError::BadRequest(
            "API key length looks wrong. Gemini keys are roughly 39 characters.".into(),
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

pub(super) fn is_gemini_api_key_line(line: &str) -> bool {
    houston_terminal_manager::provider_env::is_env_var_line(line, ENV_VAR)
}

pub async fn strip_gemini_api_key_storage() -> CoreResult<()> {
    strip_api_key_from_storage(PROVIDER, ENV_VAR).await
}

pub(super) use super::provider_env_store::write_atomic;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_empty() {
        assert!(matches!(
            validate_key(""),
            Err(CoreError::BadRequest(_))
        ));
    }

    #[test]
    fn validate_accepts_well_formed_key() {
        let key = "  AIzaSyAbcDefGhiJklMnoPqrStuVwxYz0123456789  ";
        assert_eq!(
            validate_key(key).unwrap(),
            "AIzaSyAbcDefGhiJklMnoPqrStuVwxYz0123456789"
        );
    }

    #[tokio::test]
    async fn set_gemini_api_key_rejects_empty_input() {
        let err = set_gemini_api_key("").await.unwrap_err();
        assert!(matches!(err, CoreError::BadRequest(_)));
    }
}
