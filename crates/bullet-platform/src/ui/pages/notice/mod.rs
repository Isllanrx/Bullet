use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc;

use slint::ComponentHandle;
use tracing::{debug, warn};

use super::runtime;
use super::views::NoticeDialog;
use crate::error::PlatformError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Warning,
    Question,
}

impl NoticeKind {
    fn code(self) -> i32 {
        match self {
            Self::Info => 0,
            Self::Warning => 1,
            Self::Question => 2,
        }
    }

    fn badge(self, text: &crate::i18n::Text) -> &'static str {
        match self {
            Self::Info => text.notice_badge_info,
            Self::Warning => text.notice_badge_warning,
            Self::Question => text.notice_badge_question,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NoticeContent {
    pub kind: NoticeKind,
    pub badge: String,
    pub title: String,
    pub body: String,
    pub confirm: String,
    pub cancel: String,
}

thread_local! {
    static OPEN_QUESTIONS: RefCell<Vec<(String, slint::Weak<NoticeDialog>)>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone)]
pub(crate) struct Reply(Rc<Cell<Option<mpsc::Sender<bool>>>>);

impl Reply {
    pub(crate) fn new(sender: mpsc::Sender<bool>) -> Self {
        Self(Rc::new(Cell::new(Some(sender))))
    }

    fn send(&self, value: bool) {
        if let Some(sender) = self.0.take() {
            if sender.send(value).is_err() {
                debug!("Nobody is waiting for the notice dialog any more");
            }
        }
    }
}

pub fn show_info(title: &str, body: &str) -> Result<(), PlatformError> {
    let text = crate::i18n::text();
    let content = NoticeContent {
        kind: NoticeKind::Info,
        badge: NoticeKind::Info.badge(text).to_owned(),
        title: title.to_owned(),
        body: body.to_owned(),
        confirm: text.notice_btn_ok.to_owned(),
        cancel: String::new(),
    };
    run_modal(content).map(|_| ())
}

pub fn show_warning(title: &str, body: &str) -> Result<(), PlatformError> {
    let text = crate::i18n::text();
    let content = NoticeContent {
        kind: NoticeKind::Warning,
        badge: NoticeKind::Warning.badge(text).to_owned(),
        title: title.to_owned(),
        body: body.to_owned(),
        confirm: text.notice_btn_ok.to_owned(),
        cancel: String::new(),
    };
    run_modal(content).map(|_| ())
}

pub fn ask_question(title: &str, body: &str) -> Result<bool, PlatformError> {
    let text = crate::i18n::text();
    ask_action(title, body, text.notice_btn_yes, text.notice_btn_no)
}

pub fn ask_action(
    title: &str,
    body: &str,
    confirm: &str,
    cancel: &str,
) -> Result<bool, PlatformError> {
    let text = crate::i18n::text();
    let content = NoticeContent {
        kind: NoticeKind::Question,
        badge: NoticeKind::Question.badge(text).to_owned(),
        title: title.to_owned(),
        body: body.to_owned(),
        confirm: confirm.to_owned(),
        cancel: cancel.to_owned(),
    };
    run_modal(content)
}

pub fn dismiss_question(title: &str) -> bool {
    let needle = title.to_owned();
    let (tx, rx) = mpsc::channel();
    if runtime::run_on_ui(move || {
        let mut dismissed = false;
        OPEN_QUESTIONS.with_borrow_mut(|open| {
            open.retain(|(entry_title, weak)| {
                let Some(dialog) = weak.upgrade() else {
                    return false;
                };
                if entry_title == &needle {
                    dialog.invoke_cancel();
                    dismissed = true;
                    return false;
                }
                true
            });
        });
        let _ = tx.send(dismissed); // ignore-ok: caller may have stopped waiting
    })
    .is_err()
    {
        return false;
    }
    rx.recv().unwrap_or(false)
}

fn run_modal(content: NoticeContent) -> Result<bool, PlatformError> {
    let (tx, rx) = mpsc::channel();
    runtime::run_on_ui(move || {
        let reply = Reply::new(tx);
        if let Err(e) = open(&content, reply.clone()) {
            warn!(error = %e, title = %content.title, "The styled notice dialog could not be opened");
            reply.send(false);
        }
    })?;
    rx.recv()
        .map_err(|_| PlatformError::Window("the notice dialog closed without a reply".into()))
}

pub(crate) fn wire(dialog: &NoticeDialog, content: &NoticeContent, reply: Reply) {
    dialog.set_version(crate::version::display_version().into());
    dialog.set_kind(content.kind.code());
    dialog.set_badge(content.badge.as_str().into());
    dialog.set_heading(content.title.as_str().into());
    dialog.set_body(content.body.as_str().into());
    dialog.set_confirm_label(content.confirm.as_str().into());
    dialog.set_cancel_label(content.cancel.as_str().into());

    let weak = dialog.as_weak();
    let on_confirm = reply.clone();
    dialog.on_confirm(move || {
        on_confirm.send(true);
        if let Some(dialog) = weak.upgrade() {
            close(&dialog);
        }
    });

    let weak = dialog.as_weak();
    let on_cancel = reply.clone();
    dialog.on_cancel(move || {
        on_cancel.send(false);
        if let Some(dialog) = weak.upgrade() {
            close(&dialog);
        }
    });

    dialog.window().on_close_requested(move || {
        reply.send(false);
        slint::CloseRequestResponse::HideWindow
    });
}

fn open(content: &NoticeContent, reply: Reply) -> Result<(), slint::PlatformError> {
    let dialog = NoticeDialog::new()?;
    dialog
        .global::<super::views::Theme>()
        .set_reduced_motion(crate::preferences::REDUCE_MOTION.is_enabled());
    runtime::repaint_on_expose(&dialog, |d| d.set_expose_flip(!d.get_expose_flip()));
    wire(&dialog, content, reply);
    dialog.show()?;
    if content.kind == NoticeKind::Question {
        let title = content.title.clone();
        let weak = dialog.as_weak();
        OPEN_QUESTIONS.with_borrow_mut(|open| {
            open.retain(|(_, existing)| existing.upgrade().is_some());
            open.push((title, weak));
        });
    }
    runtime::when_created(&dialog, |_, hwnd| {
        crate::client_window::take_foreground(hwnd);
    });
    Ok(())
}

fn close(dialog: &NoticeDialog) {
    if let Err(e) = dialog.hide() {
        debug!(error = %e, "The notice dialog was already closed");
    }
}

#[cfg(test)]
mod tests;
