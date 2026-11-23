use super::*;

#[test]
fn client_locales_map_by_language_family() {
    assert_eq!(Language::from_locale("pt_BR"), Some(Language::Portuguese));
    assert_eq!(Language::from_locale("es_MX"), Some(Language::Spanish));
    assert_eq!(Language::from_locale("en-GB"), Some(Language::English));
    assert_eq!(Language::from_locale("ko_KR"), Some(Language::Korean));
    assert_eq!(
        Language::from_locale("zh_CN"),
        Some(Language::ChineseSimplified)
    );
    assert_eq!(
        Language::from_locale("zh_TW"),
        Some(Language::ChineseTraditional)
    );
    assert_eq!(Language::from_locale("xx_XX"), None);
    assert_eq!(Language::from_locale(""), None);
}

#[test]
fn all_twenty_five_language_codes_round_trip_and_accept_aliases() {
    assert_eq!(Language::ALL.len(), 25);
    let mut codes = Vec::new();
    for lang in Language::ALL {
        let code = lang.code();
        assert_eq!(code.len(), 2, "{lang:?} code {code}");
        assert_eq!(Language::from_code(code), Some(lang));
        assert_eq!(Language::from_code(&code.to_ascii_lowercase()), Some(lang));
        codes.push(code);
    }
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), 25);
    assert_eq!(Language::from_code("BR"), Some(Language::Portuguese));
    assert_eq!(Language::from_code("PT-BR"), Some(Language::Portuguese));
    assert_eq!(Language::from_code("JP"), Some(Language::Japanese));
    assert_eq!(Language::from_code("KR"), Some(Language::Korean));
    assert_eq!(Language::from_code("ZZ"), None);
}

#[cfg(windows)]
#[test]
fn windows_primary_language_ids_map_to_dictionaries() {
    assert_eq!(Language::from_primary_lang_id(0x16), Language::Portuguese);
    assert_eq!(Language::from_primary_lang_id(0x0a), Language::Spanish);
    assert_eq!(Language::from_primary_lang_id(0x09), Language::English);
    assert_eq!(Language::from_primary_lang_id(0x11), Language::Japanese);
    assert_eq!(Language::from_primary_lang_id(0x12), Language::Korean);
    assert_eq!(Language::from_primary_lang_id(0x7f), Language::English);
}

#[test]
fn templates_are_filled_and_every_dictionary_keeps_its_placeholders() {
    for lang in Language::ALL {
        let text = lang.text();
        assert!(fill(text.party_in_room, "n", "3").contains('3'));
        for (template, key) in [
            (text.party_in_room, "n"),
            (text.star_gate_missing_star, "user"),
            (text.star_gate_unreachable, "error"),
            (text.party_copy_failed_body, "code"),
            (text.party_unavailable_body, "reason"),
            (text.import_refused, "reason"),
            (text.import_not_a_mod, "error"),
            (text.import_io_error, "error"),
            (text.party_invalid_code, "error"),
            (text.party_join_clipboard_error, "error"),
            (text.update_available_title, "version"),
            (text.update_offer_title, "version"),
            (text.update_offer_body, "version"),
            (text.panel_update_line, "version"),
            (text.panel_update_line, "current"),
            (text.overlay_no_results_sub, "term"),
            (text.overlay_no_library_big, "champion"),
            (text.overlay_skin_many, "count"),
            (text.overlay_no_mods_match, "term"),
            (text.detail_custom_mods, "repaired"),
            (text.detail_custom_mods, "refused"),
            (text.custom_mods_repaired_body, "n"),
            (text.custom_mods_refused_body, "n"),
            (text.custom_mods_restored_body, "n"),
            (text.custom_mods_restore_failed, "error"),
            (text.manager_outdated_body, "found"),
            (text.manager_outdated_body, "bullet"),
            (text.manager_outdated_body, "required"),
            (text.detail_manager_outdated, "found"),
            (text.detail_manager_outdated, "required"),
            (text.ltk_update_title, "latest"),
            (text.ltk_update_body, "latest"),
            (text.ltk_update_body, "installed"),
            (text.panel_ltk_update_line, "latest"),
            (text.panel_ltk_update_line, "installed"),
            (text.detail_manager_update, "installed"),
            (text.detail_manager_update, "latest"),
            (text.panel_pick_ltk_saved, "path"),
            (text.panel_diagnostics_exported, "file"),
            (text.panel_diagnostics_failed, "error"),
        ] {
            assert!(
                template.contains(&format!("{{{key}}}")),
                "{lang:?}: {template:?} must keep {{{key}}}"
            );
        }
    }
}

fn check_clean_text(origin: &str, text: &str) {
    assert!(
        !text
            .chars()
            .any(|c| matches!(u32::from(c), 0x2190..=0x2BFF | 0xFE0F | 0x1F000..=0x1FFFF)),
        "{origin}: {text:?} draws a symbol; icons are vector paths in the interface"
    );
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    assert!(
        !(letters.len() > 4 && letters.iter().all(|c| c.is_uppercase())),
        "{origin}: {text:?} is written in capitals; the interface uses sentence case"
    );
}

fn literals(source: &str) -> Vec<String> {
    source
        .replace("\\\"", "'")
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

#[test]
fn no_text_carries_emoji_symbols_or_shouting_capitals() {
    for (name, source) in [
        ("pt.rs", include_str!("pt.rs")),
        ("en.rs", include_str!("en.rs")),
        ("es.rs", include_str!("es.rs")),
    ] {
        for text in &literals(source) {
            check_clean_text(name, text);
        }
    }
    for lang in Language::ALL {
        let t = lang.text();
        for sample in [
            t.status_waiting_league,
            t.panel_tab_home,
            t.panel_mode_title,
            t.panel_repair_skins,
            t.panel_repair_client,
            t.panel_pick_ltk,
            t.panel_language_label,
            t.notice_btn_ok,
            t.ltk_update_title,
            t.star_gate_title,
            t.star_gate_copied,
        ] {
            assert!(!sample.is_empty(), "{lang:?} has empty core label");
            check_clean_text(lang.code(), sample);
        }
    }
}
