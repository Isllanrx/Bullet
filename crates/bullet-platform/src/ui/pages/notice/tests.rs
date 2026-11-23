use super::*;
use crate::i18n::Language;
use i_slint_backend_testing::ElementHandle;

#[test]
fn notice_dialog_renders_and_confirms_or_cancels_cleanly() {
    i_slint_backend_testing::init_no_event_loop();
    let text = Language::Portuguese.text();

    let (tx, rx) = mpsc::channel();
    let dialog = NoticeDialog::new().expect("notice dialog");
    let content = NoticeContent {
        kind: NoticeKind::Question,
        badge: NoticeKind::Question.badge(text).to_owned(),
        title: "Bullet 1.17".into(),
        body: "Uma nova versão está disponível.".into(),
        confirm: text.notice_btn_yes.to_owned(),
        cancel: text.notice_btn_no.to_owned(),
    };
    wire(&dialog, &content, Reply::new(tx));
    assert_eq!(dialog.get_heading(), "Bullet 1.17");
    assert_eq!(dialog.get_kind(), 2);

    ElementHandle::find_by_accessible_label(&dialog, text.notice_btn_yes)
        .next()
        .expect("confirm button")
        .invoke_accessible_default_action();
    assert!(rx.recv().expect("reply"));

    let (tx2, rx2) = mpsc::channel();
    let dialog2 = NoticeDialog::new().expect("notice dialog 2");
    wire(&dialog2, &content, Reply::new(tx2));
    ElementHandle::find_by_accessible_label(&dialog2, text.notice_btn_no)
        .next()
        .expect("cancel button")
        .invoke_accessible_default_action();
    assert!(!rx2.recv().expect("reply"));
}

#[test]
fn warning_notice_hides_cancel_button_and_every_language_fills_badges() {
    i_slint_backend_testing::init_no_event_loop();
    for lang in [Language::Portuguese, Language::Spanish, Language::English] {
        let t = lang.text();
        assert!(!t.notice_badge_info.is_empty());
        assert!(!t.notice_badge_warning.is_empty());
        assert!(!t.notice_badge_question.is_empty());
        assert!(!t.notice_btn_ok.is_empty());
        assert!(!t.notice_btn_yes.is_empty());
        assert!(!t.notice_btn_no.is_empty());
    }

    let text = Language::English.text();
    let (tx, rx) = mpsc::channel();
    let dialog = NoticeDialog::new().expect("warning dialog");
    let content = NoticeContent {
        kind: NoticeKind::Warning,
        badge: NoticeKind::Warning.badge(text).to_owned(),
        title: "Mod Manager missing".into(),
        body: "Install LTK Manager.".into(),
        confirm: text.notice_btn_ok.to_owned(),
        cancel: String::new(),
    };
    wire(&dialog, &content, Reply::new(tx));
    assert!(
        ElementHandle::find_by_accessible_label(&dialog, text.notice_btn_no)
            .next()
            .is_none()
    );
    ElementHandle::find_by_accessible_label(&dialog, text.notice_btn_ok)
        .next()
        .expect("ok button")
        .invoke_accessible_default_action();
    assert!(rx.recv().expect("ok reply"));
}
