use super::*;

#[test]
fn client_locales_map_by_language_family() {
    assert_eq!(Language::from_locale("pt_BR"), Some(Language::Portuguese));
    assert_eq!(Language::from_locale("es_MX"), Some(Language::Spanish));
    assert_eq!(Language::from_locale("en-GB"), Some(Language::English));
    assert_eq!(Language::from_locale("ko_KR"), None);
    assert_eq!(Language::from_locale(""), None);
}

#[test]
fn windows_primary_language_ids_map_to_dictionaries() {
    assert_eq!(Language::from_primary_lang_id(0x16), Language::Portuguese);
    assert_eq!(Language::from_primary_lang_id(0x0a), Language::Spanish);
    assert_eq!(Language::from_primary_lang_id(0x09), Language::English);
    assert_eq!(Language::from_primary_lang_id(0x11), Language::English);
}

#[test]
fn templates_are_filled_and_every_dictionary_keeps_its_placeholders() {
    for text in [&PORTUGUESE, &SPANISH, &ENGLISH] {
        assert!(fill(text.party_in_room, "n", "3").contains('3'));
        for (template, key) in [
            (text.party_in_room, "n"),
            (text.party_copy_failed_body, "code"),
            (text.party_unavailable_body, "reason"),
            (text.import_refused, "reason"),
            (text.import_not_a_mod, "error"),
            (text.import_io_error, "error"),
            (text.party_invalid_code, "error"),
            (text.party_join_clipboard_error, "error"),
            (text.update_available_title, "version"),
            (text.panel_update_line, "version"),
            (text.panel_update_line, "current"),
            (text.detail_dll_days_left, "n"),
            (text.detail_ltk_audited, "version"),
            (text.detail_ltk_new, "latest"),
            (text.panel_ltk_missing_line, "version"),
            (text.panel_ltk_new_line, "latest"),
            (text.panel_ltk_download, "version"),
            (text.ltk_new_title, "version"),
            (text.missing_tools_body, "version"),
            (text.injector_auto_body, "version"),
            (text.injector_auto_failed, "error"),
            (text.broken_tools_body, "version"),
        ] {
            assert!(
                template.contains(&format!("{{{key}}}")),
                "{template:?} must keep {{{key}}}"
            );
        }
    }
}
