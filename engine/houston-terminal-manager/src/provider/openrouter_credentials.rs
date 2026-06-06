//! Read Houston-managed provider credentials from `~/.houston/providers/<provider>/.env`.
//! Legacy paths are read for idempotent migration.

use crate::provider_env::read_stored_api_key;

const ENV_VAR: &str = "OPENROUTER_API_KEY";
const OPENROUTER_PROVIDER: &str = "openrouter";

pub(crate) fn openrouter_api_key_configured() -> bool {
    read_openrouter_api_key().is_ok()
}

pub(crate) fn read_openrouter_api_key() -> Result<String, String> {
    if let Ok(value) = std::env::var(ENV_VAR) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    read_stored_api_key(OPENROUTER_PROVIDER, ENV_VAR).ok_or_else(|| {
        "OpenRouter API key missing. Connect OpenRouter in settings.".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn read_openrouter_api_key_reads_canonical_path() {
        let tmp = TempDir::new().unwrap();
        let prior_home = std::env::var_os("HOME");
        let prior_houston = std::env::var_os("HOUSTON_HOME");
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("HOUSTON_HOME", tmp.path().join("houston-data"));

        let path = crate::provider_env::canonical_env_path(OPENROUTER_PROVIDER);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "OPENROUTER_API_KEY=sk-or-v1-testkey1234567890\n").unwrap();

        assert_eq!(
            read_openrouter_api_key().as_deref(),
            Ok("sk-or-v1-testkey1234567890")
        );

        match prior_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match prior_houston {
            Some(v) => std::env::set_var("HOUSTON_HOME", v),
            None => std::env::remove_var("HOUSTON_HOME"),
        }
    }
}
