//! Strict path allowlist for provider credential sync.

use crate::error::{CoreError, CoreResult};
use std::path::{Component, Path, PathBuf};

/// Provider ids supported by credential export/import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialProvider {
    OpenAi,
    Anthropic,
    OpenRouter,
    Composio,
}

impl CredentialProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
            Self::OpenRouter => "openrouter",
            Self::Composio => "composio",
        }
    }

    pub fn parse(name: &str) -> CoreResult<Self> {
        match name {
            "openai" => Ok(Self::OpenAi),
            "anthropic" | "claude" => Ok(Self::Anthropic),
            "openrouter" => Ok(Self::OpenRouter),
            "composio" => Ok(Self::Composio),
            other => Err(CoreError::BadRequest(format!(
                "credential sync is not supported for provider '{other}'"
            ))),
        }
    }

    /// Relative paths (from HOME) that may be exported/imported.
    pub fn allowed_rel_paths(self) -> &'static [&'static str] {
        match self {
            Self::OpenAi => &[".codex/auth.json", ".houston/providers/openai/.env"],
            Self::Anthropic => &[
                ".claude/.credentials.json",
                ".houston/providers/anthropic/.env",
            ],
            Self::OpenRouter => &[".houston/providers/openrouter/.env"],
            Self::Composio => &[".composio/user_data.json"],
        }
    }

    pub fn default_file_mode(self, rel_path: &str) -> u32 {
        if rel_path.ends_with(".env") || rel_path.contains("credentials") || rel_path.contains("auth")
        {
            0o600
        } else {
            0o600
        }
    }
}

/// Normalize and validate a relative path against the provider allowlist.
pub fn validate_rel_path(provider: CredentialProvider, rel_path: &str) -> CoreResult<String> {
    let normalized = normalize_rel_path(rel_path)?;
    let allowed = provider.allowed_rel_paths();
    if allowed.iter().any(|p| *p == normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(CoreError::BadRequest(format!(
            "path '{}' is not allowlisted for provider '{}'",
            normalized,
            provider.id()
        )))
    }
}

pub fn home_join(rel_path: &str) -> CoreResult<PathBuf> {
    let rel = normalize_rel_path(rel_path)?;
    // Houston-managed credential files (`.houston/providers/<p>/.env`, plus the
    // pre-`providers/` legacy `.houston/<p>/.env`) live under the Houston data
    // root, which is `~/.houston` in release, `~/.dev-houston` in debug, and
    // honors `HOUSTON_HOME`. Resolving these against the raw home dir broke dev
    // credential export (key written to `~/.dev-houston` but read from
    // `~/.houston`). External CLI creds (`.codex`, `.claude`, `.gemini`,
    // `.composio`) stay under the real home directory.
    if let Some(under_root) = rel.strip_prefix(".houston/") {
        return Ok(houston_terminal_manager::houston_data_root::houston_data_root().join(under_root));
    }
    let home = dirs::home_dir().ok_or_else(|| {
        CoreError::Internal("could not resolve home directory for credential sync".into())
    })?;
    Ok(home.join(rel))
}

pub fn normalize_rel_path(rel_path: &str) -> CoreResult<String> {
    let trimmed = rel_path.trim();
    if trimmed.is_empty() {
        return Err(CoreError::BadRequest("credential path cannot be empty".into()));
    }
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return Err(CoreError::BadRequest(
            "credential paths must be relative to HOME".into(),
        ));
    }
    let path = Path::new(trimmed);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {}
            _ => {
                return Err(CoreError::BadRequest(format!(
                    "credential path '{}' contains forbidden segments",
                    rel_path
                )));
            }
        }
    }
    Ok(path
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

/// Validate Composio `user_data.json` shape before export/import.
pub fn validate_composio_user_data(content: &str) -> CoreResult<()> {
    let value: serde_json::Value = serde_json::from_str(content).map_err(|e| {
        CoreError::BadRequest(format!("composio user_data.json is not valid JSON: {e}"))
    })?;
    let api_key = value
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if api_key.trim().is_empty() {
        return Err(CoreError::BadRequest(
            "composio user_data.json is missing a non-empty api_key".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        assert!(normalize_rel_path("../.codex/auth.json").is_err());
        assert!(normalize_rel_path(".codex/../auth.json").is_err());
    }

    #[test]
    fn allowlist_accepts_openai_paths() {
        let p = CredentialProvider::OpenAi;
        assert!(validate_rel_path(p, ".codex/auth.json").is_ok());
        assert!(validate_rel_path(p, ".houston/providers/openai/.env").is_ok());
        assert!(validate_rel_path(p, ".houston/openai/.env").is_err());
    }

    #[test]
    fn composio_validation_requires_api_key() {
        assert!(validate_composio_user_data(r#"{"api_key":"k"}"#).is_ok());
        assert!(validate_composio_user_data(r#"{"api_key":""}"#).is_err());
    }
}
