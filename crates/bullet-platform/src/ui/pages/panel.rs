use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

use serde::Serialize;
use tracing::{debug, warn};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    IsIconic, SW_RESTORE, SetForegroundWindow, ShowWindow, WM_APP,
};
use windows::core::w;

use super::dialog_host::{self, Dialog, DialogSpec, WM_HOST_TICK};
use crate::tray::TrayEvent;

const PANEL_HTML: &str = include_str!("panel_ui.html");
const WM_PANEL_REFRESH: u32 = WM_APP + 12;
const TIMER_REFRESH: usize = 3001;
const REFRESH_MS: u32 = 700;

const PANEL_WIDTH: i32 = 460;
const PANEL_HEIGHT: i32 = 640;
const PANEL_MIN_WIDTH: i32 = 380;
const PANEL_MIN_HEIGHT: i32 = 420;

static OPEN_PANEL: AtomicIsize = AtomicIsize::new(0);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelCheck {
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelSnapshot {
    pub status: String,
    pub party_line: String,
    pub in_room: bool,
    pub auto_accept: bool,
    pub random_skin: bool,
    pub light_loading: bool,
    pub autostart: bool,
    pub update_line: Option<String>,
    pub ltk_line: Option<String>,
    pub ltk_download: Option<String>,
    pub checks: Vec<PanelCheck>,
}

pub type SnapshotSource = Arc<dyn Fn() -> PanelSnapshot + Send + Sync>;

#[derive(Clone)]
pub struct PanelLinks {
    pub events: Sender<TrayEvent>,
    pub snapshot: SnapshotSource,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelLabels {
    lang: &'static str,
    version: &'static str,
    section_options: &'static str,
    section_party: &'static str,
    section_folders: &'static str,
    section_diagnostics: &'static str,
    auto_accept: &'static str,
    random_skin: &'static str,
    random_skin_hint: &'static str,
    light_loading: &'static str,
    light_loading_hint: &'static str,
    autostart: &'static str,
    party_create: &'static str,
    party_join: &'static str,
    party_leave: &'static str,
    open_mods: &'static str,
    open_tools: &'static str,
    open_logs: &'static str,
    about: &'static str,
    quit: &'static str,
    update_download: &'static str,
    mark_problem: &'static str,
    mark_problem_hint: &'static str,
    export_diagnostics: &'static str,
}

impl PanelLabels {
    fn from_text(text: &crate::i18n::Text) -> Self {
        Self {
            lang: text.html_lang,
            version: crate::version::display_version(),
            section_options: text.panel_section_options,
            section_party: text.menu_group_party,
            section_folders: text.menu_group_folders,
            section_diagnostics: text.panel_section_diagnostics,
            auto_accept: text.menu_auto_accept,
            random_skin: text.menu_random_skin,
            random_skin_hint: text.panel_random_skin_hint,
            light_loading: text.menu_light_loading,
            light_loading_hint: text.panel_light_loading_hint,
            autostart: text.menu_autostart,
            party_create: text.menu_party_create,
            party_join: text.menu_party_join,
            party_leave: text.menu_party_leave,
            open_mods: text.menu_open_mods,
            open_tools: text.menu_open_tools,
            open_logs: text.menu_open_logs,
            about: text.menu_about,
            quit: text.menu_quit,
            update_download: text.panel_update_download,
            mark_problem: text.panel_mark_problem,
            mark_problem_hint: text.panel_mark_problem_hint,
            export_diagnostics: text.panel_export_diagnostics,
        }
    }
}

#[must_use]
pub fn event_for(message: &str) -> Option<TrayEvent> {
    Some(match message {
        "toggle:autoAccept" => TrayEvent::ToggleAutoAccept,
        "toggle:randomSkin" => TrayEvent::ToggleRandomSkin,
        "toggle:lightLoading" => TrayEvent::ToggleLightLoading,
        "toggle:autostart" => TrayEvent::ToggleAutostart,
        "party:create" => TrayEvent::PartyCreate,
        "party:join" => TrayEvent::PartyJoin,
        "party:leave" => TrayEvent::PartyLeave,
        "open:mods" => TrayEvent::OpenMods,
        "open:tools" => TrayEvent::OpenTools,
        "open:logs" => TrayEvent::OpenLogs,
        "about" => TrayEvent::About,
        "quit" => TrayEvent::Quit,
        "open:release" => TrayEvent::OpenRelease,
        "open:ltk" => TrayEvent::OpenLtkRelease,
        "diag:mark" => TrayEvent::MarkProblem,
        "diag:export" => TrayEvent::ExportDiagnostics,
        _ => return None,
    })
}

fn panel_html(labels: &PanelLabels) -> Result<String, serde_json::Error> {
    let json = serde_json::to_string(labels)?.replace("</", "<\\/");
    Ok(PANEL_HTML
        .replace("{{lang}}", labels.lang)
        .replace("{{labels}}", &json))
}

pub fn show_panel(links: PanelLinks) {
    let existing = OPEN_PANEL.load(Ordering::SeqCst);
    if existing != 0 {
        let hwnd = HWND(existing as *mut _);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                // ignore-ok: ShowWindow returns the previous visibility, not a status
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            // ignore-ok: Windows may refuse the foreground; the window is still open and visible
            let _ = SetForegroundWindow(hwnd);
        }
        return;
    }
    thread::spawn(move || run_panel(&links));
}

fn run_panel(links: &PanelLinks) {
    let html = match panel_html(&PanelLabels::from_text(crate::i18n::text())) {
        Ok(html) => html,
        Err(e) => {
            warn!(error = %e, "Control panel: labels could not be serialized");
            return;
        }
    };
    let spec = DialogSpec {
        class: w!("BulletPanelWindowClass"),
        title: "Bullet",
        size: (PANEL_WIDTH, PANEL_HEIGHT),
        min_size: Some((PANEL_MIN_WIDTH, PANEL_MIN_HEIGHT)),
    };
    let mut dialog = match Dialog::create(&spec) {
        Ok(dialog) => dialog,
        Err(e) => {
            warn!(error = %e, "Control panel could not be opened");
            return;
        }
    };
    let hwnd = dialog.raw();
    if OPEN_PANEL
        .compare_exchange(0, hwnd, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        dialog.destroy();
        return;
    }

    let events = links.events.clone();
    let webview = dialog.webview(html, move |request| {
        let body = request.body().as_str();
        match event_for(body) {
            Some(event) => {
                if events.send(event).is_err() {
                    warn!(
                        message = body,
                        "Control panel: Bullet is shutting down; the action was not taken"
                    );
                }
            }
            None if body == "ready" => {}
            None => debug!(message = body, "Control panel: unknown message ignored"),
        }
        dialog_host::post(hwnd, WM_PANEL_REFRESH);
    });
    let webview = match webview {
        Ok(webview) => webview,
        Err(e) => {
            warn!(error = %e, "Control panel could not be opened");
            OPEN_PANEL.store(0, Ordering::SeqCst);
            return;
        }
    };

    dialog.show(true);
    dialog.tick_every(TIMER_REFRESH, REFRESH_MS);

    let mut last_sent = String::new();
    dialog.run(&webview, |message, id| {
        let refresh =
            message == WM_PANEL_REFRESH || (message == WM_HOST_TICK && id == TIMER_REFRESH);
        if !refresh {
            return;
        }
        match serde_json::to_string(&(links.snapshot)()) {
            Ok(json) if json != last_sent => {
                let script = format!("window.bulletPanel && window.bulletPanel.render({json});");
                match webview.evaluate_script(&script) {
                    Ok(()) => last_sent = json,
                    Err(e) => debug!(error = %e, "Control panel: the page could not be refreshed"),
                }
            }
            Ok(_) => {}
            Err(e) => warn!(error = %e, "Control panel: the state could not be serialized"),
        }
    });
    OPEN_PANEL.store(0, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn test_every_page_message_maps_to_one_tray_event() {
        let page = PANEL_HTML;
        for message in [
            "toggle:autoAccept",
            "toggle:randomSkin",
            "toggle:lightLoading",
            "toggle:autostart",
            "party:create",
            "party:join",
            "party:leave",
            "open:mods",
            "open:tools",
            "open:logs",
            "about",
            "quit",
            "open:release",
            "diag:mark",
            "diag:export",
        ] {
            assert!(event_for(message).is_some(), "{message}");
            assert!(page.contains(message), "the page sends {message}");
        }
        assert_eq!(event_for("rm -rf"), None);
    }

    #[test]
    #[ignore = "writes the rendered control panel for the visual tests into BULLET_UI_DUMP"]
    fn dump_rendered_panel() {
        let Ok(dir) = std::env::var("BULLET_UI_DUMP") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("dump dir");
        for (tag, language) in [
            ("pt", Language::Portuguese),
            ("es", Language::Spanish),
            ("en", Language::English),
        ] {
            let html = panel_html(&PanelLabels::from_text(language.text())).expect("labels");
            std::fs::write(dir.join(format!("panel-{tag}.html")), html).expect("panel");
        }
    }

    #[test]
    fn test_every_language_fills_the_page() {
        for language in [Language::Portuguese, Language::Spanish, Language::English] {
            let html = panel_html(&PanelLabels::from_text(language.text())).expect("labels");
            assert!(!html.contains("{{"), "{language:?} left a placeholder");
        }
    }
}
