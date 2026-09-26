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
        vec!["DATA/Characters/Jade_Annie/Skins/Skin5.bin".to_string()]
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

    let folder = champion.build_mod(1, &mods_dir).expect("build mod");
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
        (
            wad_path_hash(&skin_bin("annietibbers", 5)),
            skin_bin_fixture("annietibbers", 5),
        ),
    ]);
    std::fs::write(champions.join("Annie.wad.client"), wad).expect("write wad");
    let mods_dir = game.join("mods");

    let folder = StandardChampion::open(&game, "Annie")
        .expect("open")
        .build_mod(5, &mods_dir)
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
            prop.links,
            [format!("DATA/Characters/{character}/Skins/Skin5.bin")],
            "{dir}: links the original skin bin for everything else"
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
        .build_mod(5, &mods_dir)
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
    let folder = champion.build_mod(1, &mods_dir).expect("build live mod");
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
    let folder = annie.build_mod(1, &mods_dir).expect("build live annie mod");
    assert_eq!(folder, "std_annie_1");
    let _ = std::fs::remove_dir_all(&mods_dir); // ignore-ok: fixture cleanup
}
