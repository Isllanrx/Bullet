use std::path::PathBuf;

use bullet_core::party::PartyStatus;
use bullet_platform::i18n::{Text, fill};
use bullet_platform::panel::{PanelCheck, PanelSnapshot};

mod export;
mod redact;

pub use export::export_diagnostics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagerStatus {
    Ready { version: Option<String> },
    Outdated { found: String, required: String },
    Missing,
}

#[derive(Debug, Clone)]
pub struct Facts {
    pub status: String,
    pub party_line: String,
    pub in_room: bool,
    pub auto_accept: bool,
    pub injection: bool,
    pub random_skin: bool,
    pub light_loading: bool,
    pub in_client_ui: bool,
    pub autostart: bool,
    pub tools_present: bool,
    pub manager: ManagerStatus,
    pub game_found: bool,
    pub lcu_connected: bool,
    pub elevated: bool,
    pub update: Option<String>,
    pub custom_mods: Option<crate::mod_repair::ScanSummary>,
    pub stats: bullet_platform::panel::PanelStats,
}

#[must_use]
pub fn party_line(status: &PartyStatus, hosting: bool, text: &Text) -> (String, bool) {
    match status {
        PartyStatus::Off => (text.party_off.to_owned(), false),
        PartyStatus::Unavailable { .. } => (text.party_unavailable.to_owned(), false),
        PartyStatus::Connecting if hosting => (text.party_created_connecting.to_owned(), true),
        PartyStatus::Connecting => (text.party_connecting.to_owned(), true),
        PartyStatus::Connected { members } => {
            let line = if hosting {
                text.party_created_in_room
            } else {
                text.party_in_room
            };
            (fill(line, "n", &members.to_string()), true)
        }
        PartyStatus::Error { .. } => (text.party_reconnecting.to_owned(), true),
    }
}

#[must_use]
pub fn snapshot(facts: &Facts, text: &Text) -> PanelSnapshot {
    let check = |label: &str, ok: bool, detail: String| PanelCheck {
        label: label.to_owned(),
        ok,
        detail,
    };
    let custom_mods = facts.custom_mods.map(|summary| {
        check(
            text.check_custom_mods,
            summary.refused == 0,
            fill(
                &fill(
                    text.detail_custom_mods,
                    "repaired",
                    &summary.repaired.to_string(),
                ),
                "refused",
                &summary.refused.to_string(),
            ),
        )
    });
    PanelSnapshot {
        status: facts.status.clone(),
        party_line: facts.party_line.clone(),
        in_room: facts.in_room,
        auto_accept: facts.auto_accept,
        injection: facts.injection,
        random_skin: facts.random_skin,
        light_loading: facts.light_loading,
        in_client_ui: facts.in_client_ui,
        autostart: facts.autostart,
        update_line: facts.update.as_deref().map(|latest| {
            fill(
                &fill(text.panel_update_line, "version", latest),
                "current",
                bullet_platform::version::display_version(),
            )
        }),
        stats: facts.stats.clone(),
        checks: vec![
            check(
                text.check_injector,
                facts.tools_present,
                if facts.tools_present {
                    text.detail_ok
                } else {
                    text.detail_injector_missing
                }
                .to_owned(),
            ),
            {
                let (ok, detail) = match &facts.manager {
                    ManagerStatus::Ready { version: Some(v) } => {
                        (true, format!("{} (v{v})", text.detail_ok))
                    }
                    ManagerStatus::Ready { version: None } => (true, text.detail_ok.to_owned()),
                    ManagerStatus::Outdated { found, required } => (
                        false,
                        fill(
                            &fill(text.detail_manager_outdated, "found", found),
                            "required",
                            required,
                        ),
                    ),
                    ManagerStatus::Missing => (false, text.detail_manager_missing.to_owned()),
                };
                check(text.check_manager, ok, detail)
            },
            check(
                text.check_game,
                facts.game_found,
                if facts.game_found {
                    text.detail_ok
                } else {
                    text.detail_game_missing
                }
                .to_owned(),
            ),
            check(
                text.check_client,
                facts.lcu_connected,
                if facts.lcu_connected {
                    text.detail_client_connected
                } else {
                    text.detail_client_waiting
                }
                .to_owned(),
            ),
            check(
                text.check_privileges,
                true,
                if facts.elevated {
                    text.detail_elevated
                } else {
                    text.detail_not_elevated
                }
                .to_owned(),
            ),
        ]
        .into_iter()
        .chain(custom_mods)
        .collect(),
    }
}

#[must_use]
pub fn game_found(configured: &std::path::Path) -> bool {
    bullet_platform::paths::is_valid_game_dir(configured)
        || bullet_platform::paths::discover_game_dir().is_some()
}

#[must_use]
pub fn tools_present(files: &[PathBuf]) -> bool {
    files.iter().all(|file| file.is_file())
}

pub fn repair_skins_and_mods(
    mods_dir: &std::path::Path,
    state_dir: &std::path::Path,
    game_dir: &std::path::Path,
) -> crate::mod_repair::RepairReport {
    let _ = std::fs::remove_file(state_dir.join("mod_repair.json")); // ignore-ok: force fresh repair scan
    if let Ok(data_dir) = bullet_platform::paths::data_dir() {
        let generated_dir = data_dir.join("mods");
        if let Ok(entries) = std::fs::read_dir(&generated_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if let Some(s) = name.to_str() {
                    if s.starts_with("std_") || s == "bullet_loading_name" {
                        let _ = std::fs::remove_dir_all(entry.path()); // ignore-ok: clear cached generated skin mod
                    }
                }
            }
        }
    }
    let resolved_game = bullet_platform::paths::normalize_game_dir(game_dir)
        .or_else(bullet_platform::paths::discover_game_dir)
        .unwrap_or_else(|| game_dir.to_path_buf());
    crate::mod_repair::repair_custom_mods(
        &[mods_dir.to_path_buf()],
        &resolved_game,
        state_dir,
        &|| false,
    )
}

pub fn repair_client(state_dir: &std::path::Path) {
    if let Ok(data_dir) = bullet_platform::paths::data_dir() {
        for sub in ["staging", "overlay"] {
            let _ = std::fs::remove_dir_all(data_dir.join(sub)); // ignore-ok: clear transient overlay/staging cache
        }
    }
    let _ = std::fs::remove_file(state_dir.join("ltf_journal.json")); // ignore-ok: clear stale journal
    #[cfg(windows)]
    if let Ok(Some(pid)) = bullet_platform::process::ProcessFinder::find_any_process(&[
        "ltk-manager.exe",
        "LTK Manager.exe",
    ]) {
        let _ = bullet_platform::process::terminate_process(pid); // ignore-ok: restart stalled mod manager
    }
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        let bridge_dir = std::path::PathBuf::from(local_app).join("BulletBridge");
        let _ = std::fs::remove_file(bridge_dir.join("port.txt")); // ignore-ok: reset bridge discovery
        let _ = std::fs::remove_file(bridge_dir.join("token.txt")); // ignore-ok: reset bridge token
    }
}

#[cfg(test)]
mod tests;
