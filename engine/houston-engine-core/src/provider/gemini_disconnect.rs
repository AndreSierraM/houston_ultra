//! Gemini disconnect helper — clears OAuth files and Houston-managed API keys.

use super::gemini_credentials::{strip_gemini_api_key_storage, ENV_VAR as GEMINI_API_KEY_ENV};
use super::provider_env_store::blocking_env_var_with;
use crate::error::{CoreError, CoreResult};
use std::path::{Path, PathBuf};

const GOOGLE_API_KEY_ENV: &str = "GOOGLE_API_KEY";

pub async fn disconnect_gemini() -> CoreResult<()> {
    if let Some(var) = blocking_env_var() {
        return Err(CoreError::Conflict(format!(
            "`{var}` is set in your shell. Unset it there, then try disconnecting again."
        )));
    }
    let gemini_dir = resolve_gemini_dir()?;
    disconnect_gemini_at(&gemini_dir).await
}

fn resolve_gemini_dir() -> CoreResult<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        CoreError::Internal("could not resolve home directory for gemini disconnect".into())
    })?;
    Ok(home.join(".gemini"))
}

fn blocking_env_var() -> Option<&'static str> {
    blocking_env_var_with(GEMINI_API_KEY_ENV, |name| std::env::var(name).ok())
        .or_else(|| blocking_env_var_with(GOOGLE_API_KEY_ENV, |name| std::env::var(name).ok()))
}

async fn disconnect_gemini_at(gemini_dir: &Path) -> CoreResult<()> {
    strip_gemini_api_key_storage().await?;
    remove_file_if_present(&gemini_dir.join("oauth_creds.json"), "oauth_creds.json").await?;
    remove_file_if_present(
        &gemini_dir.join("google_accounts.json"),
        "google_accounts.json",
    )
    .await?;
    tracing::info!(
        "[gemini-creds] disconnect: credential files cleared at {}",
        gemini_dir.display()
    );
    Ok(())
}

async fn remove_file_if_present(path: &Path, log_name: &str) -> CoreResult<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => {
            tracing::info!("[gemini-creds] disconnect: removed {log_name}");
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(CoreError::Internal(format!(
            "failed to remove {}: {e}",
            path.display()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use tokio::fs;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn reader(map: HashMap<String, String>) -> impl Fn(&str) -> Option<String> {
        move |name: &str| map.get(name).cloned()
    }

    #[test]
    fn blocking_env_var_detects_gemini_api_key() {
        let r = reader(env(&[("GEMINI_API_KEY", "AIzaSampleKey")]));
        assert_eq!(
            blocking_env_var_with("GEMINI_API_KEY", r),
            Some("GEMINI_API_KEY")
        );
    }

    #[tokio::test]
    async fn disconnect_clears_oauth_and_houston_storage() {
        let tmp = TempDir::new().unwrap();
        let prior_home = std::env::var_os("HOME");
        let prior_houston = std::env::var_os("HOUSTON_HOME");
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("HOUSTON_HOME", tmp.path().join("houston-data"));

        let gemini_dir = tmp.path().join(".gemini");
        fs::create_dir_all(&gemini_dir).await.unwrap();
        fs::write(gemini_dir.join("oauth_creds.json"), r#"{"token":"x"}"#)
            .await
            .unwrap();
        fs::write(gemini_dir.join("google_accounts.json"), r#"{"active":"a@b.c"}"#)
            .await
            .unwrap();

        let canonical = houston_terminal_manager::provider_env::canonical_env_path("gemini");
        fs::create_dir_all(canonical.parent().unwrap()).await.unwrap();
        fs::write(&canonical, "GEMINI_API_KEY=secret\n")
            .await
            .unwrap();

        disconnect_gemini_at(&gemini_dir).await.unwrap();

        assert!(!gemini_dir.join("oauth_creds.json").exists());
        assert!(!gemini_dir.join("google_accounts.json").exists());
        assert!(!canonical.exists());

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
