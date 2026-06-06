//! Houston-managed `.env` probe for Gemini API-key auth.

use super::{read_file_with_timeout, ReadOutcome};
use crate::provider_env::read_stored_api_key;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DotenvProbe {
    Authenticated,
    Absent,
    Unreadable,
}

pub(super) async fn probe_dotenv(_gemini_dir: &Path) -> DotenvProbe {
    if read_stored_api_key("gemini", "GEMINI_API_KEY").is_some()
        || read_stored_api_key("gemini", "GOOGLE_API_KEY").is_some()
    {
        return DotenvProbe::Authenticated;
    }

    // Legacy-only: user may still have ~/.gemini/.env without canonical copy.
    let env_path = _gemini_dir.join(".env");
    let bytes = match read_file_with_timeout(&env_path).await {
        ReadOutcome::Ok(b) => b,
        ReadOutcome::NotFound => return DotenvProbe::Absent,
        ReadOutcome::Error => return DotenvProbe::Unreadable,
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => return DotenvProbe::Unreadable,
    };
    if extract_dotenv_value(text, "GEMINI_API_KEY").is_some()
        || extract_dotenv_value(text, "GOOGLE_API_KEY").is_some()
    {
        DotenvProbe::Authenticated
    } else {
        DotenvProbe::Absent
    }
}

fn extract_dotenv_value(text: &str, key: &str) -> Option<String> {
    crate::provider_env::extract_env_value(text, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_dotenv(dir: &Path, body: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(".env"), body).unwrap();
    }

    #[tokio::test]
    async fn dotenv_reads_canonical_houston_path() {
        let tmp = TempDir::new().unwrap();
        let prior_home = std::env::var_os("HOME");
        let prior_houston = std::env::var_os("HOUSTON_HOME");
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("HOUSTON_HOME", tmp.path().join("houston-data"));

        let path = crate::provider_env::canonical_env_path("gemini");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "GEMINI_API_KEY=AIzaTestKey1234567890\n").unwrap();

        let gemini_dir = tmp.path().join(".gemini");
        assert_eq!(probe_dotenv(&gemini_dir).await, DotenvProbe::Authenticated);

        match prior_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match prior_houston {
            Some(v) => std::env::set_var("HOUSTON_HOME", v),
            None => std::env::remove_var("HOUSTON_HOME"),
        }
    }

    #[tokio::test]
    async fn dotenv_with_non_empty_legacy_key_is_authenticated() {
        let tmp = TempDir::new().unwrap();
        let gemini_dir = tmp.path().join(".gemini");
        write_dotenv(&gemini_dir, "GEMINI_API_KEY=AIzaTestKey1234567890\n");
        assert_eq!(probe_dotenv(&gemini_dir).await, DotenvProbe::Authenticated);
    }
}
