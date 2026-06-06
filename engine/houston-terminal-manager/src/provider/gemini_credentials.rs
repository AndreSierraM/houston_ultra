//! Gemini API-key reads for spawn-time injection and auth checks.

use crate::provider_env;
use std::path::Path;

const ENV_VAR: &str = "GEMINI_API_KEY";
const GOOGLE_ENV_VAR: &str = "GOOGLE_API_KEY";
const PROVIDER: &str = "gemini";

pub(crate) fn gemini_api_key_configured() -> bool {
    read_gemini_api_key_for_spawn().is_some()
}

pub(crate) fn read_gemini_api_key_for_spawn() -> Option<String> {
    for var in [ENV_VAR, GOOGLE_ENV_VAR] {
        if let Ok(value) = std::env::var(var) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    provider_env::read_stored_api_key(PROVIDER, ENV_VAR)
}

/// When settings.json selects API-key auth but no key is stored, surface
/// a visible error before spawn instead of letting gemini-cli fail silently.
pub(crate) fn gemini_api_key_mode_missing_error() -> Option<String> {
    let home = dirs::home_dir()?;
    let settings_path = home.join(".gemini").join("settings.json");
    let Ok(bytes) = std::fs::read(&settings_path) else {
        return None;
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return None;
    };
    let selected = value
        .pointer("/security/auth/selectedType")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if selected != "gemini-api-key" {
        return None;
    }
    if gemini_api_key_configured() {
        return None;
    }
    Some(
        "Gemini API key missing. Connect Gemini in settings or paste an API key."
            .to_string(),
    )
}

pub(crate) fn detect_selected_auth_type(gemini_dir: &Path) -> String {
    let path = gemini_dir.join("settings.json");
    let Ok(bytes) = std::fs::read(&path) else {
        return "oauth-personal".to_string();
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return "oauth-personal".to_string();
    };
    value
        .pointer("/security/auth/selectedType")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("oauth-personal")
        .to_string()
}
