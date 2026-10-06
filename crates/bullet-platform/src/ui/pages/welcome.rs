use std::thread;

use tracing::warn;
use windows::core::w;

use super::dialog_host::{self, Dialog, DialogSpec, WM_HOST_TICK};

const WELCOME_HTML: &str = include_str!("welcome_ui.html");
const TIMER_AUTO_DISMISS: usize = 2001;
const AUTO_DISMISS_MS: u32 = 15_000;

const WELCOME_WIDTH: i32 = 530;
const WELCOME_HEIGHT: i32 = 440;
const WELCOME_MIN_WIDTH: i32 = 420;
const WELCOME_MIN_HEIGHT: i32 = 340;

fn list_item(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else {
        format!(
            "<li><span class=\"bullet\">&#9670;</span><span>{}</span></li>",
            escape_html(value)
        )
    }
}

pub(crate) fn welcome_html(text: &crate::i18n::Text) -> String {
    WELCOME_HTML
        .replace("{{lang}}", text.html_lang)
        .replace("{{version}}", crate::version::display_version())
        .replace("{{welcome_active}}", &escape_html(text.welcome_active))
        .replace(
            "{{welcome_background}}",
            &escape_html(text.welcome_background),
        )
        .replace("{{welcome_author}}", &escape_html(text.welcome_author))
        .replace(
            "{{welcome_tray_hint_item}}",
            &list_item(text.welcome_tray_hint),
        )
        .replace("{{welcome_dismiss}}", &escape_html(text.welcome_dismiss))
        .replace("{{welcome_quote_item}}", &list_item(text.welcome_quote))
}

pub(crate) fn about_html(text: &crate::i18n::Text) -> String {
    WELCOME_HTML
        .replace("{{lang}}", text.html_lang)
        .replace("{{version}}", crate::version::display_version())
        .replace("{{welcome_active}}", &escape_html(text.about_title))
        .replace(
            "{{welcome_background}}",
            &escape_html(text.about_educational),
        )
        .replace("{{welcome_author}}", &escape_html(text.welcome_author))
        .replace("{{welcome_tray_hint_item}}", "")
        .replace("{{welcome_dismiss}}", &escape_html(text.about_dismiss))
        .replace("{{welcome_quote_item}}", &list_item(text.about_quote))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn show_welcome_window() {
    thread::spawn(|| run_welcome_window(true, false));
}

pub fn show_about_window() {
    thread::spawn(|| run_welcome_window(false, true));
}

fn run_welcome_window(auto_dismiss: bool, is_about: bool) {
    let html = if is_about {
        about_html(crate::i18n::text())
    } else {
        welcome_html(crate::i18n::text())
    };
    let spec = DialogSpec {
        class: w!("BulletWelcomeWindowClass"),
        title: "Bullet \u{2014} League of Legends Skin Changer",
        size: (WELCOME_WIDTH, WELCOME_HEIGHT),
        min_size: Some((WELCOME_MIN_WIDTH, WELCOME_MIN_HEIGHT)),
    };
    let mut dialog = match Dialog::create(&spec) {
        Ok(dialog) => dialog,
        Err(e) => {
            warn!(error = %e, "Welcome window could not be opened");
            return;
        }
    };
    let hwnd = dialog.raw();
    let webview = match dialog.webview(html, move |request| {
        if request.body() == "dismiss" {
            dialog_host::close(hwnd);
        }
    }) {
        Ok(webview) => webview,
        Err(e) => {
            warn!(error = %e, "Welcome window could not be opened");
            return;
        }
    };
    dialog.show(false);
    if auto_dismiss {
        dialog.tick_every(TIMER_AUTO_DISMISS, AUTO_DISMISS_MS);
    }
    dialog.run(&webview, |message, id| {
        if message == WM_HOST_TICK && id == TIMER_AUTO_DISMISS {
            dialog.destroy();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Language;

    #[test]
    fn about_page_is_distinct_from_the_intro_and_states_it_is_educational() {
        for language in [Language::Portuguese, Language::Spanish, Language::English] {
            let text = language.text();
            let about = about_html(text);
            let welcome = welcome_html(text);
            assert!(
                !about.contains("{{"),
                "{language:?} about left a placeholder"
            );
            assert_ne!(
                about, welcome,
                "{language:?}: About must differ from the intro"
            );
            assert!(
                about.contains(&escape_html(text.about_title)),
                "{language:?}: About shows its own title"
            );
            assert!(
                about.contains("Miss Fortune"),
                "{language:?}: About carries the Miss Fortune line"
            );

            assert!(!about.contains(&escape_html(text.welcome_tray_hint)));
            assert!(
                !about.contains("<span></span>"),
                "{language:?}: About must not render an empty list row"
            );
        }
    }

    #[test]
    fn every_language_fills_every_placeholder() {
        for language in [Language::Portuguese, Language::Spanish, Language::English] {
            let html = welcome_html(language.text());
            assert!(!html.contains("{{"), "{language:?} left a placeholder");
            assert!(html.contains(language.text().welcome_dismiss));
        }
        assert!(
            !welcome_html(Language::English.text()).contains("Miss Fortune"),
            "no guessed translation of the quote"
        );
    }
}
