use super::*;

fn skin_bin_fixture(character: &str, skin: u32) -> Vec<u8> {
    let prefix = format!("Characters/{character}/Skins/Skin{skin}");
    serialize_prop_file(&PropFile {
        version: 3,
        links: vec!["DATA/Characters/Annie/Annie.bin".into()],
        entries: vec![
            PropEntry {
                class_hash: 1,
                key_hash: prop_key_hash(&prefix),
                body: vec![0xAA; 8],
            },
            PropEntry {
                class_hash: 2,
                key_hash: prop_key_hash(&format!("{prefix}/Resources")),
                body: vec![0xBB; 4],
            },
            PropEntry {
                class_hash: 3,
                key_hash: prop_key_hash("Characters/Annie/Skins/Skin5/Particles/Fire"),
                body: vec![0xCC; 16],
            },
        ],
    })
    .expect("fixture")
}

#[test]
fn test_retarget_keeps_only_the_skin_objects_rekeyed_and_links_the_original() {
    let source = skin_bin_fixture("Jade_Annie", 5);
    let out = retarget_skin_bin(&source, "Jade_Annie", 5, 301).expect("retarget");
    let parsed = parse_prop_file(&out).expect("parse output");

    assert_eq!(
        parsed.links,
        vec![
            "DATA/Characters/Jade_Annie/Skins/Skin5.bin".to_string(),
            "DATA/Characters/Annie/Annie.bin".to_string(),
        ],
        "the source bin comes first, then its own dependencies"
    );
    assert_eq!(
        parsed.entries.len(),
        2,
        "only the skin object and its Resources survive"
    );
    assert_eq!(
        parsed.entries[0].key_hash,
        prop_key_hash("Characters/Jade_Annie/Skins/Skin301")
    );
    assert_eq!(
        parsed.entries[1].key_hash,
        prop_key_hash("Characters/Jade_Annie/Skins/Skin301/Resources")
    );
    assert_eq!(
        parsed.entries[0].body,
        vec![0xAA; 8],
        "bodies are carried untouched"
    );
    assert_eq!(parsed.entries[0].class_hash, 1);
}

fn classified_skin_bin(character: &str, skin: u32, classification: u32) -> Vec<u8> {
    let mut body = 1u16.to_le_bytes().to_vec();
    body.extend_from_slice(&prop_key_hash("skinClassification").to_le_bytes());
    body.push(7);
    body.extend_from_slice(&classification.to_le_bytes());
    serialize_prop_file(&PropFile {
        version: 3,
        links: Vec::new(),
        entries: vec![PropEntry {
            class_hash: 0x9b67_e9f6,
            key_hash: prop_key_hash(&format!("Characters/{character}/Skins/Skin{skin}")),
            body,
        }],
    })
    .expect("fixture")
}

fn classification_of(bin: &[u8]) -> u32 {
    let body = &parse_prop_file(bin).expect("parse").entries[0].body;
    u32::from_le_bytes([body[7], body[8], body[9], body[10]])
}

#[test]
fn test_a_chroma_retargeted_to_slot_0_is_classified_like_a_base_skin() {
    let chroma = classified_skin_bin("Zed", 70, 2);
    let out = retarget_skin_bin(&chroma, "Zed", 70, 0).expect("retarget");
    assert_eq!(
        classification_of(&out),
        1,
        "slot 0 is never a chroma in the game"
    );
    let base = classified_skin_bin("Zed", 69, 1);
    assert_eq!(
        classification_of(&retarget_skin_bin(&base, "Zed", 69, 0).expect("retarget")),
        1
    );
    let classic = retarget_skin_bin(&chroma, "Zed", 70, 301).expect("classic slot");
    assert_eq!(
        classification_of(&classic),
        2,
        "only the slot 0 retarget changes it"
    );
}

#[test]
fn test_retarget_refuses_a_bin_without_the_skin_object() {
    let source = skin_bin_fixture("Jade_Annie", 5);
    assert!(retarget_skin_bin(&source, "Jade_Annie", 6, 0).is_err());
}

#[test]
fn test_skin_numbers_and_slots_follow_rose() {
    assert_eq!(skin_number(60_012_301), 301);
    assert_eq!(skin_number(12_005), 5, "a chroma has its own skin number");
    assert_eq!(slots_for(None), vec![0, 301, 302]);
    assert_eq!(slots_for(Some(60_012_007)), vec![0, 301, 302, 7]);
    assert_eq!(
        slots_for(Some(12_301)),
        vec![0, 301, 302],
        "no duplicate slot"
    );
}

#[test]
fn test_aliases_are_restricted_to_safe_names() {
    assert!(is_safe_alias("MonkeyKing"));
    assert!(is_safe_alias("Jade_X1"));
    assert!(!is_safe_alias(""));
    assert!(!is_safe_alias("../Annie"));
    assert!(!is_safe_alias("Annie.wad"));
}

#[test]
fn test_jade_characters_are_read_from_the_hash_table_and_cached() {
    let dir = std::env::temp_dir().join(format!("bullet_jade_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture may not exist yet
    std::fs::create_dir_all(&dir).expect("dir");
    let table = dir.join("hashes.game.txt");
    std::fs::write(
        &table,
        "0123 data/characters/jade_annie/jade_annie.bin\n\
             4567 data/characters/jade_annietibbers/skins/skin0.bin\n\
             89ab data/characters/annie/annie.bin\n\
             cdef data/characters/jade_bad name/x.bin\n",
    )
    .expect("table");
    let cache = dir.join("cache.json");

    let found = jade_characters(&table, &cache);
    let expected: BTreeSet<String> = ["jade_annie", "jade_annietibbers"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(found, expected);
    assert!(cache.is_file());
    assert_eq!(
        jade_characters(&table, &cache),
        expected,
        "served from the cache"
    );
    assert!(
        jade_characters(&dir.join("missing.txt"), &cache).is_empty(),
        "no table degrades to no companions"
    );
    let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture cleanup
}

fn raw_wad(entries: &[(u64, Vec<u8>)]) -> Vec<u8> {
    let mut wad = vec![0u8; 272 + 32 * entries.len()];
    wad[0..4].copy_from_slice(b"RW\x03\x04");
    wad[268..272].copy_from_slice(&(entries.len() as u32).to_le_bytes());
    for (i, (hash, payload)) in entries.iter().enumerate() {
        let offset = wad.len() as u32;
        let toc = 272 + 32 * i;
        wad[toc..toc + 8].copy_from_slice(&hash.to_le_bytes());
        wad[toc + 8..toc + 12].copy_from_slice(&offset.to_le_bytes());
        wad[toc + 12..toc + 16].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        wad[toc + 16..toc + 20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        wad.extend_from_slice(payload);
    }
    wad
}

#[test]
fn test_companions_are_recovered_from_bins_without_a_hash_table() {
    let game = std::env::temp_dir().join(format!("bullet_jade_bins_{}", std::process::id()));
    let champions = game.join("DATA").join("FINAL").join("Champions");
    std::fs::create_dir_all(&champions).expect("fixture dir");

    let mut bin = b"PROP".to_vec();
    bin.extend_from_slice(b"...Characters/Jade_Annie/Skins/Skin0...");
    bin.extend_from_slice(b"...Characters/Jade_AnnieTibbers/Skins/Skin0...");

    bin.extend_from_slice(b"...Characters/Jade_Soraka/Skins/Skin0...");

    let texture = b"DDS characters/jade_fake/x".to_vec();
    let wad = raw_wad(&[
        (1, bin),
        (2, texture),
        (
            wad_path_hash(&character_bin("jade_annie")),
            b"PROP".to_vec(),
        ),
        (
            wad_path_hash(&character_bin("jade_annietibbers")),
            b"PROP".to_vec(),
        ),
    ]);
    std::fs::write(champions.join("Annie.wad.client"), wad).expect("write wad");

    let champion = ClassicChampion::open(&game, "Annie").expect("open");
    let names = champion.jade_names_in_bins();
    assert!(names.contains("jade_annietibbers"));
    assert!(!names.contains("jade_fake"), "{names:?}");
    assert_eq!(
        champion.present_characters(&names),
        vec!["jade_annie".to_owned(), "jade_annietibbers".to_owned()]
    );

    let cached = champion.jade_names_from_bins_cached(&game);
    assert_eq!(cached, names);
    assert!(game.join("classic_bin_names_annie.json").is_file());
    assert_eq!(champion.jade_names_from_bins_cached(&game), names);

    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

#[test]
fn test_standard_champion_generates_slot_0_redirection() {
    let game = std::env::temp_dir().join(format!("bullet_std_wad_{}", std::process::id()));
    let champions = game.join("DATA").join("FINAL").join("Champions");
    std::fs::create_dir_all(&champions).expect("fixture dir");

    let skin_bin_content = skin_bin_fixture("Zed", 1);
    let wad = raw_wad(&[(wad_path_hash(&skin_bin("zed", 1)), skin_bin_content)]);
    std::fs::write(champions.join("Zed.wad.client"), wad).expect("write wad");

    let mods_dir = game.join("mods");
    std::fs::create_dir_all(&mods_dir).expect("mods dir");

    let champion = StandardChampion::open(&game, "Zed").expect("open");
    assert!(champion.has_skin(1));
    assert!(!champion.has_skin(2));

    let folder = champion.build_mod(1, None, &mods_dir).expect("build mod");
    assert_eq!(folder, "std_zed_1");
    assert!(
        mods_dir
            .join(&folder)
            .join("WAD")
            .join("Zed.wad.client")
            .join("data")
            .join("characters")
            .join("zed")
            .join("skins")
            .join("skin0.bin")
            .is_file()
    );
    assert!(
        mods_dir
            .join(&folder)
            .join("META")
            .join("info.json")
            .is_file()
    );

    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

fn tibbers_bin(skin: u32) -> Vec<u8> {
    let mut prop = parse_prop_file(&skin_bin_fixture("annietibbers", skin)).expect("fixture");
    prop.links
        .push("DATA/Characters/AnnieTibbers/AnnieTibbers.bin".into());
    serialize_prop_file(&prop).expect("fixture")
}

#[test]
fn test_standard_champion_retargets_the_companion_too() {
    let game = std::env::temp_dir().join(format!("bullet_std_pet_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture may not exist yet
    let champions = game.join("DATA").join("FINAL").join("Champions");
    std::fs::create_dir_all(&champions).expect("fixture dir");
    let wad = raw_wad(&[
        (
            wad_path_hash(&skin_bin("annie", 5)),
            skin_bin_fixture("Annie", 5),
        ),
        (wad_path_hash(&skin_bin("annietibbers", 5)), tibbers_bin(5)),
    ]);
    std::fs::write(champions.join("Annie.wad.client"), wad).expect("write wad");
    let mods_dir = game.join("mods");

    let folder = StandardChampion::open(&game, "Annie")
        .expect("open")
        .build_mod(5, None, &mods_dir)
        .expect("build");
    let chars = mods_dir
        .join(&folder)
        .join("WAD")
        .join("Annie.wad.client")
        .join("data")
        .join("characters");
    for (dir, character) in [("annie", "Annie"), ("annietibbers", "annietibbers")] {
        let bytes = std::fs::read(chars.join(dir).join("skins").join("skin0.bin"))
            .unwrap_or_else(|e| panic!("{dir} skin0.bin: {e}"));
        let prop = parse_prop_file(&bytes).expect("valid PROP");
        let keys: Vec<u32> = prop.entries.iter().map(|e| e.key_hash).collect();
        assert!(
            keys.contains(&prop_key_hash(&format!(
                "Characters/{character}/Skins/Skin0"
            ))),
            "{dir}: the skin object is re-keyed to slot 0"
        );
        assert!(
            !keys.contains(&prop_key_hash(&format!(
                "Characters/{character}/Skins/Skin5"
            ))),
            "{dir}: nothing left under the source slot"
        );
        assert_eq!(
            prop.links.first(),
            Some(&format!("DATA/Characters/{character}/Skins/Skin5.bin")),
            "{dir}: links the original skin bin for everything else"
        );
        assert!(
            prop.links
                .contains(&"DATA/Characters/Annie/Annie.bin".to_string()),
            "{dir}: keeps the source bin's own dependencies"
        );
    }
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

#[test]
fn test_a_broken_companion_bin_still_yields_the_champion_skin() {
    let game = std::env::temp_dir().join(format!("bullet_std_badpet_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture may not exist yet
    let champions = game.join("DATA").join("FINAL").join("Champions");
    std::fs::create_dir_all(&champions).expect("fixture dir");
    let wad = raw_wad(&[
        (
            wad_path_hash(&skin_bin("annie", 5)),
            skin_bin_fixture("Annie", 5),
        ),
        (
            wad_path_hash(&skin_bin("annietibbers", 5)),
            b"not a prop file".to_vec(),
        ),
    ]);
    std::fs::write(champions.join("Annie.wad.client"), wad).expect("write wad");
    let mods_dir = game.join("mods");
    let folder = StandardChampion::open(&game, "Annie")
        .expect("open")
        .build_mod(5, None, &mods_dir)
        .expect("the champion skin is still built");
    let chars = mods_dir
        .join(&folder)
        .join("WAD")
        .join("Annie.wad.client")
        .join("data")
        .join("characters");
    assert!(
        chars
            .join("annie")
            .join("skins")
            .join("skin0.bin")
            .is_file()
    );
    assert!(
        !chars.join("annietibbers").exists(),
        "no half-written companion"
    );
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

#[test]
fn test_standard_champion_live_wad_if_installed() {
    let game = Path::new(r"D:\Riot Games\League of Legends\Game");
    if !game
        .join("DATA")
        .join("FINAL")
        .join("Champions")
        .join("Zed.wad.client")
        .is_file()
    {
        return;
    }
    let champion = StandardChampion::open(game, "Zed").expect("open live zed wad");
    let zed_skins = champion.skin_numbers(50);
    assert!(zed_skins.contains(&1), "Zed must have skin 1");
    assert!(zed_skins.contains(&4), "Zed chroma 4 must have skin4.bin");
    let mods_dir = std::env::temp_dir().join(format!("bullet_live_mod_{}", std::process::id()));
    let folder = champion
        .build_mod(1, None, &mods_dir)
        .expect("build live mod");
    assert_eq!(folder, "std_zed_1");
    assert!(
        mods_dir
            .join(&folder)
            .join("WAD")
            .join("Zed.wad.client")
            .join("data")
            .join("characters")
            .join("zed")
            .join("skins")
            .join("skin0.bin")
            .is_file()
    );
    let _ = std::fs::remove_dir_all(&mods_dir); // ignore-ok: fixture cleanup

    let annie = StandardChampion::open(game, "Annie").expect("open live annie wad");
    assert!(annie.has_skin(1), "Annie must have skin 1 in live patch");
    let folder = annie
        .build_mod(1, None, &mods_dir)
        .expect("build live annie mod");
    assert_eq!(folder, "std_annie_1");
    let _ = std::fs::remove_dir_all(&mods_dir); // ignore-ok: fixture cleanup

    if let Ok(garen) = StandardChampion::open(game, "Garen") {
        if garen.has_skin(44) {
            let folder = garen
                .build_mod(44, None, &mods_dir)
                .expect("build live garen 44 mod");
            assert_eq!(folder, "std_garen_44");

            let skin_file = mods_dir
                .join(&folder)
                .join("WAD")
                .join("Garen.wad.client")
                .join("data")
                .join("characters")
                .join("garen")
                .join("skins")
                .join("skin0.bin");
            assert!(skin_file.is_file(), "skin0.bin must be generated");

            let skin_bytes = std::fs::read(&skin_file).expect("read generated skin0.bin");
            let skin_prop = parse_prop_file(&skin_bytes).expect("parse generated skin0.bin");
            assert_eq!(
                skin_prop.entries[0].key_hash,
                prop_key_hash("Characters/Garen/Skins/Skin0")
            );
            assert_eq!(
                skin_prop.links.first().map(String::as_str),
                Some("DATA/Characters/Garen/Skins/Skin44.bin"),
                "skin must link to source skin 44 bin first"
            );
            let source = garen
                .wad
                .read(wad_path_hash(&skin_bin("garen", 44)))
                .expect("read")
                .expect("skin44.bin");
            let source_links = parse_prop_file(&source).expect("parse skin44.bin").links;
            for link in &source_links {
                assert!(
                    skin_prop.links.contains(link),
                    "skin0.bin must keep skin44.bin's dependency {link}"
                );
            }

            assert!(
                skin_prop
                    .links
                    .contains(&"DATA/Characters/Garen/Animations/Skin44.bin".to_string()),
                "the skin's own animation graph is reached through its links"
            );
            assert!(
                !mods_dir
                    .join(&folder)
                    .join("WAD")
                    .join("Garen.wad.client")
                    .join("data")
                    .join("characters")
                    .join("garen")
                    .join("animations")
                    .join("skin0.bin")
                    .exists(),
                "the base animation graph is never replaced"
            );

            let _ = std::fs::remove_dir_all(&mods_dir); // ignore-ok: fixture cleanup
        }
    }

    if zed_skins.contains(&15) {
        let folder = champion
            .build_mod(15, None, &mods_dir)
            .expect("build live legendary mod");
        assert_eq!(folder, "std_zed_15");
        let characters = mods_dir
            .join(&folder)
            .join("WAD")
            .join("Zed.wad.client")
            .join("data")
            .join("characters")
            .join("zed");
        assert!(
            !characters.join("animations").join("skin0.bin").exists(),
            "the base animation graph is never replaced"
        );
        let skin = std::fs::read(characters.join("skins").join("skin0.bin")).expect("skin0");
        assert!(
            parse_prop_file(&skin)
                .expect("parse")
                .links
                .contains(&"DATA/Characters/Zed/Animations/Skin15.bin".to_string()),
            "the legendary graph is reached through the skin's links"
        );
        let _ = std::fs::remove_dir_all(&mods_dir); // ignore-ok: fixture cleanup
    }
}

#[test]
fn test_ultimate_skins_compatibility_if_installed() {
    let game = Path::new(r"D:\Riot Games\League of Legends\Game");
    if !game.join("DATA").join("FINAL").join("Champions").is_dir() {
        return;
    }

    let ultimates = [
        ("Lux", 7, "Elementalist Lux"),
        ("Sona", 6, "DJ Sona"),
        ("Udyr", 3, "Spirit Guard Udyr"),
        ("Ezreal", 5, "Pulsefire Ezreal"),
        ("MissFortune", 16, "Gun Goddess Miss Fortune"),
        ("Samira", 10, "Soul Fighter Samira"),
    ];

    let mods_dir = std::env::temp_dir().join(format!("bullet_ultimate_{}", std::process::id()));

    for (champ, skin, name) in ultimates {
        if let Ok(c) = StandardChampion::open(game, champ) {
            if c.has_skin(skin) {
                eprintln!("[ULTIMATE] {name} ({champ}, skin {skin})");

                let folder = c
                    .build_mod(skin, None, &mods_dir)
                    .expect("build ultimate mod");
                let skin_file = mods_dir
                    .join(&folder)
                    .join("WAD")
                    .join(format!("{champ}.wad.client"))
                    .join("data")
                    .join("characters")
                    .join(champ.to_ascii_lowercase())
                    .join("skins")
                    .join("skin0.bin");
                assert!(
                    skin_file.is_file(),
                    "ultimate skin0.bin must be generated for {name}"
                );

                let links = parse_prop_file(&std::fs::read(&skin_file).expect("skin0"))
                    .expect("parse skin0")
                    .links;
                let source = c
                    .read_skin_bin(&champ.to_ascii_lowercase(), skin)
                    .expect("read")
                    .expect("source bin");
                for link in parse_prop_file(&source).expect("parse source").links {
                    assert!(
                        links.contains(&link),
                        "{name} keeps the source dependency {link}, where its animation graph lives"
                    );
                }
                eprintln!("  -> Successfully built and verified {}!", folder);
                let _ = std::fs::remove_dir_all(&mods_dir); // ignore-ok: fixture cleanup
            } else {
                eprintln!(
                    "[ULTIMATE] {} ({}, skin {}) - not in wad",
                    name, champ, skin
                );
            }
        }
    }
}

#[test]
fn test_retarget_animation_bin_rekeys_and_links() {
    let source_prop = serialize_prop_file(&bullet_wad::prop::PropFile {
        version: 3,
        links: vec![],
        entries: vec![bullet_wad::prop::PropEntry {
            class_hash: 0x1234_5678,
            key_hash: prop_key_hash("Characters/Zed/Animations/Skin15"),
            body: b"anim_graph_data".to_vec(),
        }],
    })
    .expect("serialize");

    let retargeted = retarget_animation_bin(&source_prop, "Zed", 15, 0).expect("retarget");
    let parsed = parse_prop_file(&retargeted).expect("parse");
    assert_eq!(
        parsed.entries[0].key_hash,
        prop_key_hash("Characters/Zed/Animations/Skin0")
    );
    assert_eq!(
        parsed.links,
        vec!["DATA/Characters/Zed/Animations/Skin15.bin"]
    );
}

#[test]
fn test_classic_champion_retargets_animation_bin() {
    let game = std::env::temp_dir().join(format!("bullet_classic_anim_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture may not exist yet
    let champions = game.join("DATA").join("FINAL").join("Champions");
    std::fs::create_dir_all(&champions).expect("fixture dir");

    let anim_fixture = serialize_prop_file(&bullet_wad::prop::PropFile {
        version: 3,
        links: vec![],
        entries: vec![bullet_wad::prop::PropEntry {
            class_hash: 0x1234_5678,
            key_hash: prop_key_hash("Characters/Jade_Annie/Animations/Skin15"),
            body: b"anim_data".to_vec(),
        }],
    })
    .expect("serialize anim fixture");

    let wad = raw_wad(&[
        (
            wad_path_hash(&character_bin("jade_annie")),
            b"PROP".to_vec(),
        ),
        (
            wad_path_hash(&skin_bin("jade_annie", 15)),
            skin_bin_fixture("Jade_Annie", 15),
        ),
        (
            wad_path_hash(&animation_bin("jade_annie", 15)),
            anim_fixture,
        ),
    ]);
    std::fs::write(champions.join("Annie.wad.client"), wad).expect("write wad");
    let mods_dir = game.join("mods");

    let champion = ClassicChampion::open(&game, "Annie").expect("open");
    assert!(champion.has_skin("jade_annie", 15));
    assert!(champion.has_animation("jade_annie", 15));

    let mut known = BTreeSet::new();
    known.insert("jade_annie".to_string());
    let folder = champion
        .build_mod(15, &[0], &known, &mods_dir)
        .expect("build classic mod");

    let anim_file = mods_dir
        .join(&folder)
        .join("WAD")
        .join("Annie.wad.client")
        .join("data")
        .join("characters")
        .join("jade_annie")
        .join("animations")
        .join("skin0.bin");
    assert!(
        anim_file.is_file(),
        "classic animation skin0.bin must be generated"
    );

    let bytes = std::fs::read(&anim_file).expect("read generated anim");
    let prop = parse_prop_file(&bytes).expect("parse generated anim prop");
    assert_eq!(
        prop.entries[0].key_hash,
        prop_key_hash("Characters/Jade_Annie/Animations/Skin0")
    );

    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

fn companion_bin(character: &str, skin: u32) -> Vec<u8> {
    let prefix = format!("Characters/{character}/Skins/Skin{skin}");
    serialize_prop_file(&PropFile {
        version: 3,
        links: vec![format!("DATA/Characters/{character}/{character}.bin")],
        entries: vec![PropEntry {
            class_hash: 1,
            key_hash: prop_key_hash(&prefix),
            body: vec![0xAA; 8],
        }],
    })
    .expect("fixture")
}

fn standard_game(name: &str, entries: &[(u64, Vec<u8>)]) -> PathBuf {
    let game = std::env::temp_dir().join(format!("bullet_std_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture may not exist yet
    let champions = game.join("DATA").join("FINAL").join("Champions");
    std::fs::create_dir_all(&champions).expect("fixture dir");
    std::fs::write(champions.join("Zed.wad.client"), raw_wad(entries)).expect("write wad");
    game
}

fn generated_skin0(mods_dir: &Path, folder: &str, character: &str) -> Option<PropFile> {
    let path = mods_dir
        .join(folder)
        .join("WAD")
        .join("Zed.wad.client")
        .join("data")
        .join("characters")
        .join(character)
        .join("skins")
        .join("skin0.bin");
    std::fs::read(path)
        .ok()
        .map(|bytes| parse_prop_file(&bytes).expect("valid PROP"))
}

#[test]
fn test_a_companion_outside_the_registry_is_found_in_the_bins_and_retargeted() {
    let game = standard_game(
        "shadow",
        &[
            (
                wad_path_hash(&skin_bin("zed", 10)),
                companion_bin("Zed", 10),
            ),
            (
                wad_path_hash(&skin_bin("zedshadow", 10)),
                companion_bin("ZedShadow", 10),
            ),
            (
                wad_path_hash(&skin_bin("jade_zed", 10)),
                companion_bin("Jade_Zed", 10),
            ),
        ],
    );
    let mods_dir = game.join("mods");
    let champion = StandardChampion::open(&game, "Zed")
        .expect("open")
        .with_cache_dir(&game);
    assert!(champion.companions().contains("zedshadow"));
    assert!(
        !champion.companions().iter().any(|c| c.starts_with("jade_")),
        "Rift Classic characters belong to the Classic generator"
    );

    let folder = champion.build_mod(10, None, &mods_dir).expect("build");
    let shadow = generated_skin0(&mods_dir, &folder, "zedshadow").expect("shadow skin0.bin");
    assert_eq!(
        shadow.entries[0].key_hash,
        prop_key_hash("Characters/zedshadow/Skins/Skin0")
    );
    assert!(generated_skin0(&mods_dir, &folder, "jade_zed").is_none());
    assert!(game.join("companion_names_zed.json").is_file());
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

#[test]
fn test_a_chroma_without_its_own_companion_bin_uses_the_base_skin_one() {
    let game = standard_game(
        "chroma",
        &[
            (
                wad_path_hash(&skin_bin("zed", 12)),
                companion_bin("Zed", 12),
            ),
            (
                wad_path_hash(&skin_bin("zedshadow", 10)),
                companion_bin("ZedShadow", 10),
            ),
        ],
    );
    let mods_dir = game.join("mods");
    let champion = StandardChampion::open(&game, "Zed").expect("open");

    let without_base = champion.build_mod(12, None, &mods_dir).expect("build");
    assert!(generated_skin0(&mods_dir, &without_base, "zedshadow").is_none());

    let with_base = champion.build_mod(12, Some(10), &mods_dir).expect("build");
    let shadow = generated_skin0(&mods_dir, &with_base, "zedshadow").expect("shadow skin0.bin");
    assert_eq!(
        shadow.links.first().map(String::as_str),
        Some("DATA/Characters/zedshadow/Skins/Skin10.bin")
    );
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

#[test]
fn test_the_client_names_the_classic_character_when_the_alias_does_not() {
    let game = std::env::temp_dir().join(format!("bullet_classic_wukong_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture may not exist yet
    let champions = game.join("DATA").join("FINAL").join("Champions");
    std::fs::create_dir_all(&champions).expect("fixture dir");
    let wad = raw_wad(&[
        (
            wad_path_hash(&character_bin("jade_wukong")),
            b"PROP".to_vec(),
        ),
        (
            wad_path_hash(&skin_bin("jade_wukong", 3)),
            skin_bin_fixture("Jade_Wukong", 3),
        ),
    ]);
    std::fs::write(champions.join("MonkeyKing.wad.client"), wad).expect("write wad");
    let mods_dir = game.join("mods");
    let known = BTreeSet::new();

    let derived = ClassicChampion::open(&game, "MonkeyKing").expect("open");
    assert_eq!(derived.main_character(), "jade_monkeyking");
    assert!(
        derived
            .skin_numbers(derived.main_character(), 50)
            .is_empty()
    );
    assert!(
        derived.build_mod(3, &[0], &known, &mods_dir).is_err(),
        "the name derived from the archive does not exist in the game"
    );

    let named = ClassicChampion::open(&game, "MonkeyKing")
        .expect("open")
        .with_client_character(Some("Jade_Wukong"));
    assert_eq!(named.main_character(), "jade_wukong");
    assert_eq!(named.skin_numbers(named.main_character(), 50), vec![3]);
    let folder = named
        .build_mod(3, &[0], &known, &mods_dir)
        .expect("builds under the client's name");
    assert!(
        mods_dir
            .join(&folder)
            .join("WAD")
            .join("MonkeyKing.wad.client")
            .join("data")
            .join("characters")
            .join("jade_wukong")
            .join("skins")
            .join("skin0.bin")
            .is_file()
    );

    let unknown = ClassicChampion::open(&game, "MonkeyKing")
        .expect("open")
        .with_client_character(Some("Jade_Nobody"));
    assert_eq!(
        unknown.main_character(),
        "jade_monkeyking",
        "a client name absent from the archive is not trusted"
    );
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

#[test]
fn test_the_companion_falls_back_only_to_a_real_other_base() {
    let game = standard_game(
        "fallback",
        &[
            (
                wad_path_hash(&skin_bin("zed", 12)),
                companion_bin("Zed", 12),
            ),
            (
                wad_path_hash(&skin_bin("zedshadow", 0)),
                companion_bin("ZedShadow", 0),
            ),
            (
                wad_path_hash(&skin_bin("zedshadow", 10)),
                companion_bin("ZedShadow", 10),
            ),
        ],
    );
    let champion = StandardChampion::open(&game, "Zed").expect("open");
    assert_eq!(
        champion.companion_source_skin("zedshadow", 12, Some(10)),
        Some(10)
    );
    assert_eq!(
        champion.companion_source_skin("zedshadow", 12, Some(0)),
        None,
        "the default skin is never a fallback"
    );
    assert_eq!(
        champion.companion_source_skin("zedshadow", 12, Some(12)),
        None
    );
    assert_eq!(
        champion.companion_source_skin("zedshadow", 10, None),
        Some(10)
    );
    assert_eq!(champion.companion_source_skin("zedshadow", 12, None), None);
    let _ = std::fs::remove_dir_all(&game); // ignore-ok: fixture cleanup
}

#[test]
fn test_generated_bin_facts_report_what_changed() {
    let chroma = classified_skin_bin("Zed", 70, 2);
    let out = retarget_skin_bin(&chroma, "Zed", 70, 0).expect("retarget");
    let before = skin_bin_facts(&chroma).expect("source facts");
    let after = skin_bin_facts(&out).expect("generated facts");
    assert_eq!(before.classification, Some(2));
    assert_eq!(after.classification, Some(1));
    assert_eq!(after.links[0], "DATA/Characters/Zed/Skins/Skin70.bin");
    assert_eq!(after.objects, 1);
    assert_eq!(skin_bin_facts(b"not a bin"), None);
}
