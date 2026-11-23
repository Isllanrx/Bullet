use std::path::PathBuf;

use tracing::{info, warn};

const LTK_MANAGER_PATH_FILE: &str = "ltk_manager_path.txt";

pub fn spawn_pick_ltk_manager(
    repair_state: bullet_app::control_panel::RepairState,
    state_dir: PathBuf,
    notifier: bullet_platform::tray::TrayController,
) {
    drop(tokio::task::spawn_blocking(move || {
        let t = bullet_platform::i18n::text();
        let chosen = match bullet_platform::dialog::pick_file(
            0,
            t.panel_pick_ltk_title,
            "LTK Manager (*.exe)",
            "*.exe",
        ) {
            Ok(Some(path)) => path,
            Ok(None) => return,
            Err(e) => {
                warn!(error = %e, "Could not open file picker for LTK Manager");
                repair_state.set_feedback(false, e.to_string());
                bullet_platform::panel::refresh_open_panel();
                return;
            }
        };
        let valid_ext = chosen
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"));
        if !chosen.is_file() || (cfg!(windows) && !valid_ext) {
            repair_state.set_feedback(false, t.panel_pick_ltk_invalid.to_owned());
            bullet_platform::panel::refresh_open_panel();
            return;
        }
        let target = state_dir.join(LTK_MANAGER_PATH_FILE);
        let text_path = chosen.to_string_lossy().into_owned();
        if let Err(e) = bullet_platform::fs::atomic_write(&target, text_path.as_bytes(), true) {
            warn!(error = %e, path = %chosen.display(), "Could not save LTK Manager path");
            repair_state.set_feedback(false, e.to_string());
            bullet_platform::panel::refresh_open_panel();
            return;
        }
        info!(path = %chosen.display(), "Custom LTK Manager path saved");
        let msg = bullet_platform::i18n::fill(t.panel_pick_ltk_saved, "path", &text_path);
        repair_state.set_feedback(true, msg.clone());
        bullet_platform::panel::refresh_open_panel();
        notifier.notify(t.panel_pick_ltk, &msg);
    }));
}

pub fn handle_mark_problem(
    mark_state: &bullet_core::state::StateReceiver,
    mark_live: &bullet_app::live_game::LiveGame,
    repair: &bullet_app::control_panel::RepairState,
) {
    let phase = mark_state.borrow().phase;
    match mark_live.latest() {
        Some(live) => warn!(
            phase = ?phase,
            game_time = %mark_live
                .game_time_at(std::time::SystemTime::now())
                .map(bullet_app::live_game::format_game_time)
                .unwrap_or_default(),
            champion = %live.champion,
            skin_id = live.skin_id,
            skin_name = %live.skin_name,
            "User marked a problem"
        ),
        None => {
            warn!(phase = ?phase, "User marked a problem (no live game data at this moment)");
        }
    }
    let text = bullet_platform::i18n::text();
    repair.set_feedback(true, text.panel_problem_marked.to_owned());
    bullet_platform::panel::refresh_open_panel();
}

pub fn handle_export_diagnostics(
    logs_dir: Option<&std::path::Path>,
    repair: &bullet_app::control_panel::RepairState,
) {
    let text = bullet_platform::i18n::text();
    match logs_dir.map(|dir| {
        bullet_app::control_panel::export_diagnostics(dir, std::time::SystemTime::now(), &[])
    }) {
        Some(Ok(zip)) => {
            info!(file = %zip.display(), "Diagnostics exported");
            let name = zip
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| zip.display().to_string());
            repair.set_feedback(
                true,
                bullet_platform::i18n::fill(text.panel_diagnostics_exported, "file", &name),
            );
            bullet_platform::panel::refresh_open_panel();
            if let Some(dir) = zip.parent()
                && let Err(e) = bullet_platform::shell::open_folder(dir)
            {
                warn!(error = %e, "Could not open the diagnostics folder");
            }
        }
        Some(Err(e)) => {
            warn!(error = %e, "Diagnostics could not be exported");
            repair.set_feedback(
                false,
                bullet_platform::i18n::fill(text.panel_diagnostics_failed, "error", &e.to_string()),
            );
            bullet_platform::panel::refresh_open_panel();
        }
        None => {
            warn!("Diagnostics requested, but the logs folder could not be resolved at boot");
            repair.set_feedback(
                false,
                bullet_platform::i18n::fill(text.panel_diagnostics_failed, "error", "logs"),
            );
            bullet_platform::panel::refresh_open_panel();
        }
    }
}

pub fn handle_reset_defaults(repair: &bullet_app::control_panel::RepairState) {
    let text = bullet_platform::i18n::text();
    match bullet_platform::preferences::reset_defaults() {
        Ok(()) => {
            info!("User preferences restored to shipped defaults");
            repair.set_feedback(true, text.panel_reset_defaults_done.to_owned());
        }
        Err(e) => {
            warn!(error = %e, "Could not restore default preferences");
            repair.set_feedback(false, e.to_string());
        }
    }
    bullet_platform::panel::refresh_open_panel();
}

pub fn spawn_repair_skins(
    repair_state: bullet_app::control_panel::RepairState,
    custom_mods: bullet_app::mod_repair::CustomModsNotice,
    mod_roots: Vec<PathBuf>,
    state_dir: PathBuf,
    notifier: bullet_platform::tray::TrayController,
) {
    let text = bullet_platform::i18n::text();
    if !repair_state.begin_skins(text.panel_repair_skins_running) {
        return;
    }
    bullet_platform::panel::refresh_open_panel();
    let game = bullet_platform::paths::discover_game_dir().unwrap_or_default();
    drop(tokio::task::spawn_blocking(move || {
        let report =
            bullet_app::control_panel::repair_skins_and_mods(&mod_roots, &state_dir, &game);
        if let Some(totals) = report.totals {
            custom_mods.set(totals);
        }
        info!(
            cleared = report.cleared_generated,
            checked = report.checked,
            repaired = report.repaired,
            incompatible = report.incompatible,
            game_found = report.game_found,
            "Manual skin and mod repair completed"
        );
        let t = bullet_platform::i18n::text();
        let (ok, message) = bullet_app::control_panel::format_skins_feedback(&report, t);
        repair_state.finish_skins(ok, message.clone());
        bullet_platform::panel::refresh_open_panel();
        notifier.notify(t.panel_repair_skins, &message);
    }));
}

pub fn spawn_repair_client(
    repair_state: bullet_app::control_panel::RepairState,
    state_dir: PathBuf,
    notifier: bullet_platform::tray::TrayController,
) {
    let text = bullet_platform::i18n::text();
    if !repair_state.begin_client(text.panel_repair_client_running) {
        return;
    }
    bullet_platform::panel::refresh_open_panel();
    drop(tokio::task::spawn_blocking(move || {
        let report = bullet_app::control_panel::repair_client(&state_dir);
        info!(
            cleared_caches = report.cleared_caches,
            stopped_processes = report.stopped_processes,
            reset_bridge = report.reset_bridge,
            warnings = report.warnings.len(),
            "Manual client and Bullet cache repair completed"
        );
        let t = bullet_platform::i18n::text();
        let (ok, message) = bullet_app::control_panel::format_client_feedback(&report, t);
        repair_state.finish_client(ok, message.clone());
        bullet_platform::panel::refresh_open_panel();
        notifier.notify(t.panel_repair_client, &message);
    }));
}
