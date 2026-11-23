use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use tracing::warn;

use super::i18n::{ENGLISH, PORTUGUESE, SPANISH, Text};
use super::loader;

const LANGUAGE_FILE: &str = "language.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    English,
    Portuguese,
    Spanish,
    French,
    German,
    Italian,
    Polish,
    Russian,
    Turkish,
    Korean,
    Japanese,
    ChineseSimplified,
    ChineseTraditional,
    Vietnamese,
    Thai,
    Indonesian,
    Arabic,
    Dutch,
    Swedish,
    Czech,
    Hungarian,
    Romanian,
    Greek,
    Ukrainian,
    Hindi,
}

impl Language {
    pub const ALL: [Self; 25] = [
        Self::English,
        Self::Portuguese,
        Self::Spanish,
        Self::French,
        Self::German,
        Self::Italian,
        Self::Polish,
        Self::Russian,
        Self::Turkish,
        Self::Korean,
        Self::Japanese,
        Self::ChineseSimplified,
        Self::ChineseTraditional,
        Self::Vietnamese,
        Self::Thai,
        Self::Indonesian,
        Self::Arabic,
        Self::Dutch,
        Self::Swedish,
        Self::Czech,
        Self::Hungarian,
        Self::Romanian,
        Self::Greek,
        Self::Ukrainian,
        Self::Hindi,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "EN",
            Self::Portuguese => "PT",
            Self::Spanish => "ES",
            Self::French => "FR",
            Self::German => "DE",
            Self::Italian => "IT",
            Self::Polish => "PL",
            Self::Russian => "RU",
            Self::Turkish => "TR",
            Self::Korean => "KO",
            Self::Japanese => "JA",
            Self::ChineseSimplified => "ZH",
            Self::ChineseTraditional => "TW",
            Self::Vietnamese => "VI",
            Self::Thai => "TH",
            Self::Indonesian => "ID",
            Self::Arabic => "AR",
            Self::Dutch => "NL",
            Self::Swedish => "SV",
            Self::Czech => "CS",
            Self::Hungarian => "HU",
            Self::Romanian => "RO",
            Self::Greek => "EL",
            Self::Ukrainian => "UK",
            Self::Hindi => "HI",
        }
    }

    #[must_use]
    pub fn from_code(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().to_ascii_uppercase();
        match trimmed.as_str() {
            "EN" => Some(Self::English),
            "PT" | "BR" | "PT-BR" | "PT_BR" => Some(Self::Portuguese),
            "ES" => Some(Self::Spanish),
            "FR" => Some(Self::French),
            "DE" => Some(Self::German),
            "IT" => Some(Self::Italian),
            "PL" => Some(Self::Polish),
            "RU" => Some(Self::Russian),
            "TR" => Some(Self::Turkish),
            "KO" | "KR" => Some(Self::Korean),
            "JA" | "JP" => Some(Self::Japanese),
            "ZH" | "CN" | "ZH-CN" | "ZH_CN" => Some(Self::ChineseSimplified),
            "TW" | "ZH-TW" | "ZH_TW" | "HK" => Some(Self::ChineseTraditional),
            "VI" | "VN" => Some(Self::Vietnamese),
            "TH" => Some(Self::Thai),
            "ID" => Some(Self::Indonesian),
            "AR" => Some(Self::Arabic),
            "NL" => Some(Self::Dutch),
            "SV" | "SE" => Some(Self::Swedish),
            "CS" | "CZ" => Some(Self::Czech),
            "HU" => Some(Self::Hungarian),
            "RO" => Some(Self::Romanian),
            "EL" | "GR" => Some(Self::Greek),
            "UK" | "UA" => Some(Self::Ukrainian),
            "HI" => Some(Self::Hindi),
            _ => None,
        }
    }

    #[must_use]
    pub fn from_locale(locale: &str) -> Option<Self> {
        let mut parts = locale.trim().split(['_', '-']);
        let language = parts.next()?.to_ascii_lowercase();
        let region = parts.next().map(str::to_ascii_uppercase);
        match language.as_str() {
            "en" => Some(Self::English),
            "pt" => Some(Self::Portuguese),
            "es" => Some(Self::Spanish),
            "fr" => Some(Self::French),
            "de" => Some(Self::German),
            "it" => Some(Self::Italian),
            "pl" => Some(Self::Polish),
            "ru" => Some(Self::Russian),
            "tr" => Some(Self::Turkish),
            "ko" => Some(Self::Korean),
            "ja" => Some(Self::Japanese),
            "zh" => match region.as_deref() {
                Some("TW" | "HK" | "MO" | "HANT") => Some(Self::ChineseTraditional),
                _ => Some(Self::ChineseSimplified),
            },
            "vi" => Some(Self::Vietnamese),
            "th" => Some(Self::Thai),
            "id" | "in" => Some(Self::Indonesian),
            "ar" => Some(Self::Arabic),
            "nl" => Some(Self::Dutch),
            "sv" => Some(Self::Swedish),
            "cs" => Some(Self::Czech),
            "hu" => Some(Self::Hungarian),
            "ro" => Some(Self::Romanian),
            "el" => Some(Self::Greek),
            "uk" => Some(Self::Ukrainian),
            "hi" => Some(Self::Hindi),
            _ => None,
        }
    }

    #[must_use]
    pub fn of_windows() -> Self {
        static CACHED: OnceLock<Language> = OnceLock::new();
        *CACHED.get_or_init(Self::of_system)
    }

    #[cfg(windows)]
    fn of_system() -> Self {
        let lang_id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        let primary = lang_id & 0x3ff;
        let sub = (lang_id >> 10) & 0x3f;
        if primary == 0x04 && matches!(sub, 0x01 | 0x03 | 0x05) {
            return Self::ChineseTraditional;
        }
        Self::from_primary_lang_id(primary)
    }

    #[cfg(target_os = "macos")]
    fn of_system() -> Self {
        std::process::Command::new("/usr/bin/defaults")
            .args(["read", "-g", "AppleLocale"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| Self::from_locale(String::from_utf8_lossy(&output.stdout).trim()))
            .unwrap_or(Self::English)
    }

    #[cfg(windows)]
    pub(crate) fn from_primary_lang_id(primary: u16) -> Self {
        match primary {
            0x01 => Self::Arabic,
            0x04 => Self::ChineseSimplified,
            0x05 => Self::Czech,
            0x07 => Self::German,
            0x08 => Self::Greek,
            0x09 => Self::English,
            0x0a => Self::Spanish,
            0x0c => Self::French,
            0x0e => Self::Hungarian,
            0x10 => Self::Italian,
            0x11 => Self::Japanese,
            0x12 => Self::Korean,
            0x13 => Self::Dutch,
            0x15 => Self::Polish,
            0x16 => Self::Portuguese,
            0x18 => Self::Romanian,
            0x19 => Self::Russian,
            0x1d => Self::Swedish,
            0x1e => Self::Thai,
            0x1f => Self::Turkish,
            0x21 => Self::Indonesian,
            0x22 => Self::Ukrainian,
            0x2a => Self::Vietnamese,
            0x39 => Self::Hindi,
            _ => Self::English,
        }
    }

    #[must_use]
    pub fn for_locale(locale: Option<&str>) -> Self {
        locale
            .and_then(Self::from_locale)
            .unwrap_or_else(Self::of_windows)
    }

    #[must_use]
    pub fn text(self) -> &'static Text {
        match self {
            Self::English => &ENGLISH,
            Self::Portuguese => &PORTUGUESE,
            Self::Spanish => &SPANISH,
            other => loader::load(other),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SavedLanguage {
    code: String,
}

static ACTIVE_LANGUAGE: RwLock<Option<Language>> = RwLock::new(None);
static USER_OVERRIDE: RwLock<Option<Language>> = RwLock::new(None);
static STORE_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

pub fn init_language_store(state_dir: &Path) {
    *STORE_DIR.write().unwrap_or_else(|e| e.into_inner()) = Some(state_dir.to_path_buf());
    let path = state_dir.join(LANGUAGE_FILE);
    if let Ok(raw) = std::fs::read_to_string(&path)
        && let Ok(saved) = serde_json::from_str::<SavedLanguage>(&raw)
        && let Some(lang) = Language::from_code(&saved.code)
    {
        *USER_OVERRIDE.write().unwrap_or_else(|e| e.into_inner()) = Some(lang);
    }
}

pub fn set_user_language(lang: Language) {
    *USER_OVERRIDE.write().unwrap_or_else(|e| e.into_inner()) = Some(lang);
    let dir = STORE_DIR
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .or_else(|| crate::paths::state_dir().ok());
    if let Some(dir) = dir {
        let payload = SavedLanguage {
            code: lang.code().to_owned(),
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&payload)
            && let Err(e) = crate::fs::atomic_write(&dir.join(LANGUAGE_FILE), &bytes, true)
        {
            warn!(error = %e, "Could not persist selected language");
        }
    }
}

pub fn clear_user_language() {
    *USER_OVERRIDE.write().unwrap_or_else(|e| e.into_inner()) = None;
}

pub fn set_active_locale(locale: &str) {
    match Language::from_locale(locale) {
        Some(lang) => {
            *ACTIVE_LANGUAGE.write().unwrap_or_else(|e| e.into_inner()) = Some(lang);
        }
        None => tracing::debug!(locale, "Unsupported locale; the language is unchanged"),
    }
}

pub fn reset_active_language() {
    *ACTIVE_LANGUAGE.write().unwrap_or_else(|e| e.into_inner()) = None;
}

#[must_use]
pub fn active_language() -> Language {
    if let Some(lang) = *USER_OVERRIDE.read().unwrap_or_else(|e| e.into_inner()) {
        return lang;
    }
    match *ACTIVE_LANGUAGE.read().unwrap_or_else(|e| e.into_inner()) {
        Some(lang) => lang,
        None => Language::of_windows(),
    }
}

#[must_use]
pub fn effective_language(client_locale: Option<&str>) -> Language {
    if let Some(lang) = *USER_OVERRIDE.read().unwrap_or_else(|e| e.into_inner()) {
        return lang;
    }
    Language::for_locale(client_locale)
}

#[must_use]
pub fn text() -> &'static Text {
    active_language().text()
}
