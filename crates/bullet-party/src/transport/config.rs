use std::path::Path;

use crate::error::PartyError;

pub const RELAY_ENV: &str = bullet_core::env::RELAY_URL;

pub const RELAY_FILE: &str = "party.json";

pub const DEFAULT_RELAY_URL: &str = "wss://bullet-party-relay.isllan-toso79.workers.dev";

pub const LEGACY_DEFAULT_RELAY_URL: &str = "wss://bullet-party-relay.longhaired-angora.workers.dev";

pub const FALLBACK_RELAY_URLS: &[&str] = &[LEGACY_DEFAULT_RELAY_URL];

#[derive(serde::Deserialize)]
struct PartyFile {
    relay_url: String,
    #[serde(default)]
    fallback_relay_urls: Vec<String>,
}

fn default_party_json() -> String {
    let fallbacks = FALLBACK_RELAY_URLS
        .iter()
        .map(|url| format!("    \"{url}\""))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "{{\n  \"relay_url\": \"{DEFAULT_RELAY_URL}\",\n  \"fallback_relay_urls\": [\n{fallbacks}\n  ]\n}}\n"
    )
}

fn env_relay_list() -> Option<Vec<String>> {
    match std::env::var(RELAY_ENV) {
        Ok(raw) => {
            let list: Vec<String> = raw
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            (!list.is_empty()).then_some(list)
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            tracing::warn!(
                variable = RELAY_ENV,
                "The relay variable is not valid Unicode; using party.json"
            );
            None
        }
        Err(std::env::VarError::NotPresent) => None,
    }
}

fn read_party_file(state_dir: &Path) -> Result<PartyFile, PartyError> {
    let path = state_dir.join(RELAY_FILE);
    if !path.exists() {
        let written = std::fs::create_dir_all(state_dir)
            .and_then(|()| std::fs::write(&path, default_party_json()));
        if let Err(e) = written {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "Could not write the default party.json; using the built-in relay"
            );
        }
        return Ok(PartyFile {
            relay_url: DEFAULT_RELAY_URL.to_owned(),
            fallback_relay_urls: FALLBACK_RELAY_URLS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        });
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| PartyError::NoRelay(format!("failed to read {}: {e}", path.display())))?;
    let mut file: PartyFile = serde_json::from_slice(&bytes)
        .map_err(|e| PartyError::NoRelay(format!("{} is not valid: {e}", path.display())))?;
    if file.relay_url.trim().trim_end_matches('/') == LEGACY_DEFAULT_RELAY_URL
        && file.fallback_relay_urls.is_empty()
    {
        file.relay_url = DEFAULT_RELAY_URL.to_owned();
        file.fallback_relay_urls = FALLBACK_RELAY_URLS
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let _ = std::fs::write(&path, default_party_json()); // ignore-ok: best-effort migration of default party.json
    }
    Ok(file)
}

pub fn relay_url(state_dir: &Path) -> Result<String, PartyError> {
    if let Some(list) = env_relay_list() {
        return validate_relay_url(&list[0]);
    }
    let file = read_party_file(state_dir)?;
    validate_relay_url(file.relay_url.trim())
}

#[must_use]
pub fn fallback_relay_urls(state_dir: &Path, primary: &str) -> Vec<String> {
    let primary_norm = primary.trim().trim_end_matches('/');
    let candidates: Vec<String> = if let Some(list) = env_relay_list() {
        list.into_iter().skip(1).collect()
    } else if let Ok(file) = read_party_file(state_dir) {
        if file.fallback_relay_urls.is_empty() && primary_norm == DEFAULT_RELAY_URL {
            FALLBACK_RELAY_URLS
                .iter()
                .map(|s| (*s).to_owned())
                .collect()
        } else {
            file.fallback_relay_urls
        }
    } else if primary_norm == DEFAULT_RELAY_URL {
        FALLBACK_RELAY_URLS
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    } else {
        Vec::new()
    };

    let mut out = Vec::new();
    for raw in candidates {
        if let Ok(valid) = validate_relay_url(raw.trim())
            && valid != primary_norm
            && !out.contains(&valid)
        {
            out.push(valid);
        }
    }
    out
}

pub fn validate_relay_url(url: &str) -> Result<String, PartyError> {
    let url = url.trim_end_matches('/');
    let host_part = if let Some(rest) = url.strip_prefix("wss://") {
        rest
    } else if let Some(rest) = url.strip_prefix("ws://") {
        let host = rest.split(['/', ':']).next().unwrap_or_default();
        if host != "127.0.0.1" && host != "localhost" {
            return Err(PartyError::NoRelay(
                "ws:// is only accepted for 127.0.0.1/localhost; use wss://".into(),
            ));
        }
        rest
    } else {
        return Err(PartyError::NoRelay(
            "the relay URL must start with wss://".into(),
        ));
    };
    if host_part.is_empty() || url.contains(['?', '#', ' ']) {
        return Err(PartyError::NoRelay("the relay URL is malformed".into()));
    }
    Ok(url.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_only_secure_or_loopback_urls_are_accepted() {
        assert_eq!(
            validate_relay_url("wss://relay.example.workers.dev/").expect("ok"),
            "wss://relay.example.workers.dev"
        );
        assert!(validate_relay_url("ws://127.0.0.1:8787").is_ok());
        assert!(validate_relay_url("ws://relay.example.com").is_err());
        assert!(validate_relay_url("https://relay.example.com").is_err());
        assert!(validate_relay_url("wss://").is_err());
        assert!(validate_relay_url("wss://x.dev/room?key=1").is_err());
    }

    #[test]
    fn test_auto_creates_party_json_with_fallbacks_if_missing() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = std::env::temp_dir().join(format!("bullet_party_test_{unique}"));
        let url = relay_url(&tmp).expect("auto create party.json");
        assert_eq!(url, DEFAULT_RELAY_URL);
        assert!(tmp.join(RELAY_FILE).exists());
        let fallbacks = fallback_relay_urls(&tmp, &url);
        assert_eq!(fallbacks, vec![LEGACY_DEFAULT_RELAY_URL.to_owned()]);
        let _ = std::fs::remove_dir_all(&tmp); // ignore-ok: test cleanup
    }

    #[test]
    fn test_migrates_legacy_default_party_json_to_10_slot_worker() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp = std::env::temp_dir().join(format!("bullet_party_legacy_{unique}"));
        std::fs::create_dir_all(&tmp).expect("dir");
        std::fs::write(
            tmp.join(RELAY_FILE),
            format!("{{\"relay_url\":\"{LEGACY_DEFAULT_RELAY_URL}\"}}"),
        )
        .expect("write legacy");
        let url = relay_url(&tmp).expect("migrated url");
        assert_eq!(url, DEFAULT_RELAY_URL);
        let fallbacks = fallback_relay_urls(&tmp, &url);
        assert_eq!(fallbacks, vec![LEGACY_DEFAULT_RELAY_URL.to_owned()]);
        let _ = std::fs::remove_dir_all(&tmp); // ignore-ok: test cleanup
    }
}
