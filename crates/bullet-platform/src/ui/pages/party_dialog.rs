use std::sync::{Arc, Mutex};

use tracing::{debug, warn};
use windows::Win32::UI::WindowsAndMessaging::WM_APP;
use windows::core::w;

use super::dialog_host::{self, Dialog, DialogSpec};
use crate::error::PlatformError;

const DIALOG_HTML: &str = include_str!("party_dialog_ui.html");

const WM_DIALOG_PASTE: u32 = WM_APP + 1;

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn show_party_created_dialog(code: &str) -> Result<(), PlatformError> {
    let text = crate::i18n::text();
    run_dialog_modal(text.party_dialog_create_title, created_html(text, code)).map(|_| ())
}

pub(crate) fn created_html(text: &crate::i18n::Text, code: &str) -> String {
    let inline_btn = format!(
        "<button type=\"button\" class=\"btn-inline\" onclick=\"doCopy()\">{}</button>",
        escape_html(text.party_dialog_btn_copy)
    );
    let footer_btns = format!(
        "<button type=\"button\" class=\"btn btn-primary\" onclick=\"doSubmit()\">{}</button>",
        escape_html(text.party_dialog_btn_ok)
    );

    DIALOG_HTML
        .replace("{{lang}}", text.html_lang)
        .replace("{{title}}", &escape_html(text.party_dialog_create_title))
        .replace("{{desc}}", &escape_html(text.party_dialog_create_desc))
        .replace("{{label}}", &escape_html(text.party_dialog_label_code))
        .replace("{{initial_code}}", &escape_html(code))
        .replace("{{placeholder}}", "")
        .replace("{{readonly_attr}}", "readonly")
        .replace("{{action_inline_button}}", &inline_btn)
        .replace("{{footer_buttons}}", &footer_btns)
        .replace("{{copied_text}}", &escape_html(text.party_dialog_copied))
        .replace(
            "{{error_empty}}",
            &escape_html(text.party_dialog_error_empty),
        )
}

pub fn show_party_join_dialog(initial_code: Option<&str>) -> Result<Option<String>, PlatformError> {
    let text = crate::i18n::text();
    run_dialog_modal(text.party_dialog_join_title, join_html(text, initial_code))
}

pub(crate) fn join_html(text: &crate::i18n::Text, initial_code: Option<&str>) -> String {
    let inline_btn = format!(
        "<button type=\"button\" class=\"btn-inline\" onclick=\"doPaste()\">{}</button>",
        escape_html(text.party_dialog_btn_paste)
    );
    let footer_btns = format!(
        "<button type=\"button\" class=\"btn btn-secondary\" onclick=\"doCancel()\">{}</button>\
         <button type=\"button\" class=\"btn btn-primary\" onclick=\"doSubmit()\">{}</button>",
        escape_html(text.party_dialog_btn_cancel),
        escape_html(text.party_dialog_btn_join)
    );

    DIALOG_HTML
        .replace("{{lang}}", text.html_lang)
        .replace("{{title}}", &escape_html(text.party_dialog_join_title))
        .replace("{{desc}}", &escape_html(text.party_dialog_join_desc))
        .replace("{{label}}", &escape_html(text.party_dialog_label_code))
        .replace("{{initial_code}}", &escape_html(initial_code.unwrap_or("")))
        .replace(
            "{{placeholder}}",
            &escape_html(text.party_dialog_placeholder),
        )
        .replace("{{readonly_attr}}", "")
        .replace("{{action_inline_button}}", &inline_btn)
        .replace("{{footer_buttons}}", &footer_btns)
        .replace("{{copied_text}}", &escape_html(text.party_dialog_copied))
        .replace(
            "{{error_empty}}",
            &escape_html(text.party_dialog_error_empty),
        )
}

fn run_dialog_modal(title: &str, html: String) -> Result<Option<String>, PlatformError> {
    let spec = DialogSpec {
        class: w!("BulletPartyDialogClass"),
        title,
        size: (520, 280),
        min_size: None,
    };
    let mut dialog = Dialog::create(&spec).inspect_err(|e| {
        warn!(error = %e, "Party dialog could not be opened");
    })?;
    let hwnd = dialog.raw();

    let result: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let result_for_ipc = Arc::clone(&result);
    let pending_paste: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let paste_for_ipc = Arc::clone(&pending_paste);

    let webview = dialog
        .webview(html, move |request| {
            let body = request.body();
            if body == "cancel" {
                dialog_host::close(hwnd);
            } else if let Some(code) = body.strip_prefix("submit:") {
                if let Ok(mut slot) = result_for_ipc.lock() {
                    *slot = Some(code.trim().to_string());
                }
                dialog_host::close(hwnd);
            } else if let Some(code) = body.strip_prefix("copy:") {
                if let Err(e) = crate::clipboard::set_text(code) {
                    warn!(error = %e, "Party dialog: could not copy the room code to the clipboard");
                }
            } else if body == "request_paste" {
                paste_from_clipboard(&paste_for_ipc, hwnd);
            }
        })
        .inspect_err(|e| warn!(error = %e, "Party dialog could not be opened"))?;

    dialog.show(false);
    dialog.run(&webview, |message, _| {
        if message == WM_DIALOG_PASTE {
            apply_pending_paste(&webview, &pending_paste);
        }
    });

    Ok(result.lock().ok().and_then(|slot| slot.clone()))
}

fn paste_from_clipboard(pending: &Mutex<Option<String>>, hwnd: isize) {
    let text = match crate::clipboard::get_text() {
        Ok(Some(text)) => text,
        Ok(None) => {
            debug!("Party dialog: paste requested but the clipboard holds no text");
            return;
        }
        Err(e) => {
            warn!(error = %e, "Party dialog: could not read the clipboard for paste");
            return;
        }
    };
    match pending.lock() {
        Ok(mut slot) => *slot = Some(text),
        Err(e) => {
            warn!(error = %e, "Party dialog: paste hand-off is poisoned; paste dropped");
            return;
        }
    }

    dialog_host::post(hwnd, WM_DIALOG_PASTE);
}

fn apply_pending_paste(webview: &wry::WebView, pending: &Mutex<Option<String>>) {
    let text = match pending.lock() {
        Ok(mut slot) => slot.take(),
        Err(e) => {
            warn!(error = %e, "Party dialog: paste hand-off is poisoned; paste dropped");
            return;
        }
    };
    let Some(text) = text else { return };

    let literal = match serde_json::to_string(&text) {
        Ok(literal) => literal,
        Err(e) => {
            warn!(error = %e, "Party dialog: could not encode the pasted text");
            return;
        }
    };
    if let Err(e) = webview.evaluate_script(&format!("applyPasteFromRust({literal})")) {
        warn!(error = %e, "Party dialog: could not deliver the pasted text to the page");
    }
}
