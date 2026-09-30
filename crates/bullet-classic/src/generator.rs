use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bullet_wad::hash::{prop_key_hash, wad_path_hash};
use bullet_wad::prop::{
    PropEntry, PropFile, field_value, parse_prop_file, serialize_prop_file, set_u32_field,
};
use bullet_wad::wad::WadFile;
use tracing::{debug, info, warn};

use crate::builder::CLASSIC_DEFAULT_SLOTS;
use crate::error::ClassicError;

pub const CLASSIC_MOD_PREFIX: &str = "classic_";

const SKIN_CLASSIFICATION_FIELD: &str = "skinClassification";
const BASE_SKIN_CLASSIFICATION: u32 = 1;

#[must_use]
pub fn is_safe_alias(alias: &str) -> bool {
    !alias.is_empty()
        && alias
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[must_use]
pub fn skin_number(skin_or_chroma_id: u32) -> u32 {
    crate::builder::ClassicIdMapper::normalize_skin_id(skin_or_chroma_id) % 1000
}

#[must_use]
pub fn main_character(alias: &str) -> String {
    format!("jade_{}", alias.to_ascii_lowercase())
}

fn character_bin(character: &str) -> String {
    format!("data/characters/{character}/{character}.bin")
}

fn skin_bin(character: &str, skin: u32) -> String {
    format!("data/characters/{character}/skins/skin{skin}.bin")
}

fn animation_bin(character: &str, skin: u32) -> String {
    format!("data/characters/{character}/animations/skin{skin}.bin")
}

pub fn retarget_skin_bin(
    source: &[u8],
    character: &str,
    source_skin: u32,
    target_skin: u32,
) -> Result<Vec<u8>, ClassicError> {
    let source_prefix = format!("Characters/{character}/Skins/Skin{source_skin}");
    let target_prefix = format!("Characters/{character}/Skins/Skin{target_skin}");
    let mut skin_source_hash = prop_key_hash(&source_prefix);
    let skin_target_hash = prop_key_hash(&target_prefix);

    let resources_source = format!("{source_prefix}/Resources");
    let resources_target = format!("{target_prefix}/Resources");
    let mut resources_source_hash = prop_key_hash(&resources_source);
    let resources_target_hash = prop_key_hash(&resources_target);

    let parsed = parse_prop_file(source).map_err(|e| ClassicError::Bin(e.to_string()))?;

    if !parsed
        .entries
        .iter()
        .any(|e| e.key_hash == skin_source_hash)
    {
        let alt_prefix = format!(
            "Characters/{}/Skins/Skin{source_skin}",
            character.to_ascii_lowercase()
        );
        let alt_hash = prop_key_hash(&alt_prefix);
        if parsed.entries.iter().any(|e| e.key_hash == alt_hash) {
            skin_source_hash = alt_hash;
            let alt_resources = format!("{alt_prefix}/Resources");
            resources_source_hash = prop_key_hash(&alt_resources);
        }
    }

    let mut selected: Vec<PropEntry> = parsed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let renamed = if entry.key_hash == skin_source_hash {
                skin_target_hash
            } else if entry.key_hash == resources_source_hash {
                resources_target_hash
            } else {
                return None;
            };
            Some(PropEntry {
                key_hash: renamed,
                ..entry
            })
        })
        .collect();

    if target_skin == 0 {
        for entry in selected
            .iter_mut()
            .filter(|e| e.key_hash == skin_target_hash)
        {
            if let Err(e) = set_u32_field(
                &mut entry.body,
                prop_key_hash(SKIN_CLASSIFICATION_FIELD),
                BASE_SKIN_CLASSIFICATION,
            ) {
                warn!(
                    character,
                    source_skin,
                    error = %e,
                    "Skin classification left as in the source bin; its fields could not be walked"
                );
            }
        }
    }

    if !selected.iter().any(|e| e.key_hash == skin_target_hash) {
        return Err(ClassicError::Bin(format!(
            "{source_prefix} object not found in the skin bin"
        )));
    }

    let mut links = vec![format!(
        "DATA/Characters/{character}/Skins/Skin{source_skin}.bin"
    )];
    for link in parsed.links {
        if !links.contains(&link) {
            links.push(link);
        }
    }

    serialize_prop_file(&PropFile {
        version: parsed.version,
        links,
        entries: selected,
    })
    .map_err(|e| ClassicError::Bin(e.to_string()))
}

const SKIN_DATA_CLASS: u32 = 0x9b67_e9f6;

#[derive(Debug, Default, PartialEq, Eq)]
struct SkinBinFacts {
    links: Vec<String>,
    classification: Option<u32>,
    animation_graph: Option<u32>,
    objects: usize,
}

fn skin_bin_facts(bytes: &[u8]) -> Option<SkinBinFacts> {
    let parsed = parse_prop_file(bytes).ok()?;
    let skin = parsed
        .entries
        .iter()
        .find(|e| e.class_hash == SKIN_DATA_CLASS);
    let read = |path: &[u32]| {
        skin.and_then(|e| field_value(&e.body, path).ok().flatten())
            .and_then(|v| v.as_u32())
    };
    Some(SkinBinFacts {
        classification: read(&[prop_key_hash(SKIN_CLASSIFICATION_FIELD)]),
        animation_graph: read(&[
            prop_key_hash("skinAnimationProperties"),
            prop_key_hash("animationGraphData"),
        ]),
        objects: parsed.entries.len(),
        links: parsed.links,
    })
}

fn object_changes(source: &[u8], generated: &[u8]) -> Vec<serde_json::Value> {
    let (Ok(before), Ok(after)) = (parse_prop_file(source), parse_prop_file(generated)) else {
        return Vec::new();
    };
    after
        .entries
        .iter()
        .map(|made| {
            let original = before
                .entries
                .iter()
                .find(|e| e.class_hash == made.class_hash);
            let changes = original.map(|o| bullet_wad::prop::diff_fields(&o.body, &made.body));
            serde_json::json!({
                "class": format!("{:08x}", made.class_hash),
                "key": format!("{:08x}", made.key_hash),
                "source_key": original.map(|o| format!("{:08x}", o.key_hash)),
                "bytes": made.body.len(),
                "field_changes": match changes {
                    Some(Ok(list)) => serde_json::to_value(list).unwrap_or_default(),
                    Some(Err(e)) => serde_json::json!({ "unreadable": e.to_string() }),
                    None => serde_json::json!({ "unreadable": "no object of this class in the source bin" }),
                },
            })
        })
        .collect()
}

fn generated_bin_record(
    alias: &str,
    character: &str,
    source_skin: u32,
    source: &[u8],
    generated: &[u8],
) -> serde_json::Value {
    let before = skin_bin_facts(source).unwrap_or_default();
    let after = skin_bin_facts(generated).unwrap_or_default();
    let objects = object_changes(source, generated);
    let changed_fields: usize = objects
        .iter()
        .filter_map(|o| o["field_changes"].as_array().map(Vec::len))
        .sum();
    let unreadable = objects
        .iter()
        .any(|o| o["field_changes"].get("unreadable").is_some());
    let source_checksum = format!("{:016x}", bullet_wad::hash::content_checksum(source));
    let generated_checksum = format!("{:016x}", bullet_wad::hash::content_checksum(generated));
    let graph = after.animation_graph.map(|h| format!("{h:08x}"));
    info!(
        alias,
        character,
        source_skin,
        source_bytes = source.len(),
        source_checksum = %source_checksum,
        generated_bytes = generated.len(),
        generated_checksum = %generated_checksum,
        source_objects = before.objects,
        kept_objects = after.objects,
        links = after.links.len(),
        classification_before = ?before.classification,
        classification_after = ?after.classification,
        animation_graph = ?graph,
        animation_graph_is_source = before.animation_graph == after.animation_graph,
        changed_fields,
        unreadable_objects = unreadable,
        "Skin bin generated for slot 0"
    );
    for object in &objects {
        if let Some(list) = object["field_changes"].as_array() {
            for change in list {
                debug!(
                    alias,
                    character,
                    class = %object["class"],
                    path = %change["path"],
                    before = %change["before"],
                    after = %change["after"],
                    "Generated field differs from the source bin"
                );
            }
        }
    }
    debug!(alias, character, source_skin, links = ?after.links, "Skin bin links");
    serde_json::json!({
        "character": character,
        "source_skin": source_skin,
        "file": format!("data/characters/{character}/skins/skin0.bin"),
        "source_file": skin_bin(character, source_skin),
        "source_bytes": source.len(),
        "source_checksum": source_checksum,
        "generated_bytes": generated.len(),
        "generated_checksum": generated_checksum,
        "links": after.links,
        "source_links": before.links,
        "classification_before": before.classification,
        "classification_after": after.classification,
        "animation_graph": graph,
        "objects": objects,
    })
}

pub fn retarget_animation_bin(
    source: &[u8],
    character: &str,
    source_skin: u32,
    target_skin: u32,
) -> Result<Vec<u8>, ClassicError> {
    let source_prefix = format!("Characters/{character}/Animations/Skin{source_skin}");
    let target_prefix = format!("Characters/{character}/Animations/Skin{target_skin}");
    let mut anim_source_hash = prop_key_hash(&source_prefix);
    let anim_target_hash = prop_key_hash(&target_prefix);

    let parsed = parse_prop_file(source).map_err(|e| ClassicError::Bin(e.to_string()))?;

    if !parsed
        .entries
        .iter()
        .any(|e| e.key_hash == anim_source_hash)
    {
        let alt_prefix = format!(
            "Characters/{}/Animations/Skin{source_skin}",
            character.to_ascii_lowercase()
        );
        let alt_hash = prop_key_hash(&alt_prefix);
        if parsed.entries.iter().any(|e| e.key_hash == alt_hash) {
            anim_source_hash = alt_hash;
        }
    }

    let selected: Vec<PropEntry> = parsed
        .entries
        .into_iter()
        .map(|entry| {
            let key_hash = if entry.key_hash == anim_source_hash {
                anim_target_hash
            } else {
                entry.key_hash
            };
            PropEntry { key_hash, ..entry }
        })
        .collect();

    if !selected.iter().any(|e| e.key_hash == anim_target_hash) {
        return Err(ClassicError::Bin(format!(
            "{source_prefix} object not found in the animation bin"
        )));
    }

    let mut links = parsed.links;
    let source_link = format!("DATA/Characters/{character}/Animations/Skin{source_skin}.bin");
    if !links.contains(&source_link) {
        links.push(source_link);
    }

    serialize_prop_file(&PropFile {
        version: parsed.version,
        links,
        entries: selected,
    })
    .map_err(|e| ClassicError::Bin(e.to_string()))
}

#[must_use]
pub fn jade_characters(hashes_path: &Path, cache_path: &Path) -> BTreeSet<String> {
    let meta = match std::fs::metadata(hashes_path) {
        Ok(meta) => meta,
        Err(e) => {
            warn!(
                table = %hashes_path.display(),
                error = %e,
                "Game hash table unavailable; Rift Classic companion characters will be recovered from the champion bins"
            );
            return BTreeSet::new();
        }
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let fingerprint = format!("{}:{mtime}", meta.len());

    if let Ok(bytes) = std::fs::read(cache_path) {
        if let Ok(cached) = serde_json::from_slice::<CharacterCache>(&bytes) {
            if cached.source == fingerprint {
                debug!(
                    characters = cached.characters.len(),
                    "Rift Classic character index from cache"
                );
                return cached.characters;
            }
        }
    }

    let characters = match scan_jade_characters(hashes_path) {
        Ok(characters) => characters,
        Err(e) => {
            warn!(
                table = %hashes_path.display(),
                error = %e,
                "Game hash table could not be read; Rift Classic companion characters will be recovered from the champion bins"
            );
            return BTreeSet::new();
        }
    };

    let cache = CharacterCache {
        source: fingerprint,
        characters: characters.clone(),
    };
    match serde_json::to_vec(&cache) {
        Ok(bytes) => {
            if let Some(parent) = cache_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    debug!(error = %e, "Rift Classic character cache folder unavailable");
                }
            }
            if let Err(e) = std::fs::write(cache_path, bytes) {
                debug!(error = %e, "Rift Classic character index could not be cached");
            }
        }
        Err(e) => debug!(error = %e, "Rift Classic character index could not be serialized"),
    }
    info!(
        characters = characters.len(),
        "Rift Classic characters indexed from the game hash table"
    );
    characters
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CharacterCache {
    source: String,
    characters: BTreeSet<String>,
}

fn scan_jade_characters(hashes_path: &Path) -> std::io::Result<BTreeSet<String>> {
    use std::io::BufRead;

    const NEEDLE: &[u8] = b"data/characters/jade_";
    let file = std::fs::File::open(hashes_path)?;
    let mut reader = std::io::BufReader::with_capacity(1 << 20, file);
    let mut line = Vec::new();
    let mut found = BTreeSet::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        let Some(start) = line.windows(NEEDLE.len()).position(|w| w == NEEDLE) else {
            continue;
        };
        let name_start = start + b"data/characters/".len();
        let name: Vec<u8> = line[name_start..]
            .iter()
            .copied()
            .take_while(|b| *b != b'/')
            .collect();

        let terminated = line.get(name_start + name.len()) == Some(&b'/');
        if terminated
            && name
                .iter()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
        {
            if let Ok(name) = String::from_utf8(name) {
                found.insert(name);
            }
        }
    }
    Ok(found)
}

fn wad_stamp(path: &Path) -> String {
    std::fs::metadata(path)
        .map(|meta| {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            format!("{}:{mtime}", meta.len())
        })
        .unwrap_or_default()
}

fn character_names_in_bins(wad: &WadFile, alias: &str) -> BTreeSet<String> {
    const MAX_BIN_BYTES: usize = 8 * 1024 * 1024;
    const NEEDLE: &[u8] = b"characters/";

    let mut found = BTreeSet::new();
    let mut unreadable = 0usize;
    for (hash, size) in wad.entries() {
        if size > MAX_BIN_BYTES {
            continue;
        }
        match wad.read_prefix(hash, 4) {
            Ok(Some(head)) if head.starts_with(b"PROP") || head.starts_with(b"PTCH") => {}
            Ok(_) => continue,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        }
        let bytes = match wad.read(hash) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => continue,
            Err(_) => {
                unreadable += 1;
                continue;
            }
        };
        if !(bytes.starts_with(b"PROP") || bytes.starts_with(b"PTCH")) {
            continue;
        }
        let lower = bytes.to_ascii_lowercase();
        let mut from = 0;
        while let Some(pos) = lower[from..]
            .windows(NEEDLE.len())
            .position(|w| w == NEEDLE)
        {
            let name_start = from + pos + NEEDLE.len();
            let name: Vec<u8> = lower[name_start..]
                .iter()
                .copied()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .collect();
            if !name.is_empty() && lower.get(name_start + name.len()) == Some(&b'/') {
                if let Ok(name) = String::from_utf8(name) {
                    found.insert(name);
                }
            }
            from = name_start;
        }
    }
    if unreadable > 0 {
        warn!(
            alias,
            unreadable,
            "Some entries of the champion WAD could not be read while looking for character names"
        );
    }
    found
}

fn cached_names(
    cache_path: &Path,
    stamp: &str,
    alias: &str,
    scan: impl FnOnce() -> BTreeSet<String>,
) -> BTreeSet<String> {
    if !stamp.is_empty() {
        if let Ok(bytes) = std::fs::read(cache_path) {
            if let Ok(cached) = serde_json::from_slice::<CharacterCache>(&bytes) {
                if cached.source == stamp {
                    debug!(
                        alias,
                        characters = cached.characters.len(),
                        "Character names from the bin-scan cache"
                    );
                    return cached.characters;
                }
            }
        }
    }

    let started = std::time::Instant::now();
    let characters = scan();
    info!(
        alias,
        names = ?characters,
        elapsed_ms = started.elapsed().as_millis(),
        "Character names recovered from the champion's bins"
    );
    if !stamp.is_empty() {
        let cache = CharacterCache {
            source: stamp.to_owned(),
            characters: characters.clone(),
        };
        match serde_json::to_vec(&cache) {
            Ok(bytes) => {
                if let Some(parent) = cache_path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        debug!(error = %e, "Bin-scan cache folder unavailable");
                    }
                }
                if let Err(e) = std::fs::write(cache_path, bytes) {
                    debug!(error = %e, "Bin-scan cache not written; the next build scans again");
                }
            }
            Err(e) => debug!(error = %e, "Bin-scan cache not serialized"),
        }
    }
    characters
}

pub struct ClassicChampion {
    alias: String,
    wad: WadFile,

    wad_stamp: String,
    main: String,
    main_display: String,
}

impl ClassicChampion {
    pub fn open(game_dir: &Path, alias: &str) -> Result<Self, ClassicError> {
        if !is_safe_alias(alias) {
            return Err(ClassicError::InvalidAlias(alias.to_owned()));
        }
        let path = game_dir
            .join("DATA")
            .join("FINAL")
            .join("Champions")
            .join(format!("{alias}.wad.client"));
        if !path.is_file() {
            return Err(ClassicError::ChampionNotFound {
                alias: format!("{alias} (no {})", path.display()),
            });
        }
        Ok(Self {
            alias: alias.to_owned(),
            wad: WadFile::open(&path)?,
            wad_stamp: wad_stamp(&path),
            main: main_character(alias),
            main_display: format!("Jade_{alias}"),
        })
    }

    #[must_use]
    pub fn with_client_character(mut self, classic_alias: Option<&str>) -> Self {
        let Some(display) = classic_alias.filter(|name| is_safe_alias(name)) else {
            return self;
        };
        let name = display.to_ascii_lowercase();
        if name == self.main {
            return self;
        }
        if self.wad.contains(wad_path_hash(&character_bin(&name))) {
            info!(
                alias = %self.alias,
                derived = %self.main,
                client = %name,
                "Rift Classic character named by the client"
            );
            self.main = name;
            self.main_display = display.to_owned();
        } else {
            warn!(
                alias = %self.alias,
                client = %name,
                derived = %self.main,
                "The client's Rift Classic character is not in the champion archive; keeping the derived name"
            );
        }
        self
    }

    #[must_use]
    pub fn main_character(&self) -> &str {
        &self.main
    }

    #[must_use]
    pub fn jade_names_in_bins(&self) -> BTreeSet<String> {
        character_names_in_bins(&self.wad, &self.alias)
            .into_iter()
            .filter(|name| name.starts_with("jade_"))
            .collect()
    }

    #[must_use]
    pub fn jade_names_from_bins_cached(&self, cache_dir: &Path) -> BTreeSet<String> {
        let cache_path = cache_dir.join(format!(
            "classic_bin_names_{}.json",
            self.alias.to_ascii_lowercase()
        ));
        cached_names(&cache_path, &self.wad_stamp, &self.alias, || {
            self.jade_names_in_bins()
        })
    }

    #[must_use]
    pub fn present_characters(&self, known: &BTreeSet<String>) -> Vec<String> {
        let mut candidates: BTreeSet<String> = known.clone();
        candidates.insert(self.main.clone());
        candidates
            .into_iter()
            .filter(|c| self.wad.contains(wad_path_hash(&character_bin(c))))
            .collect()
    }

    #[must_use]
    pub fn has_skin(&self, character: &str, skin: u32) -> bool {
        self.wad.contains(wad_path_hash(&skin_bin(character, skin)))
    }

    #[must_use]
    pub fn has_animation(&self, character: &str, skin: u32) -> bool {
        self.wad
            .contains(wad_path_hash(&animation_bin(character, skin)))
    }

    #[must_use]
    pub fn skin_numbers(&self, character: &str, limit: u32) -> Vec<u32> {
        (0..limit)
            .filter(|n| self.has_skin(character, *n))
            .collect()
    }

    pub fn build_mod(
        &self,
        skin: u32,
        slots: &[u32],
        known_characters: &BTreeSet<String>,
        mods_dir: &Path,
    ) -> Result<String, ClassicError> {
        let main = self.main.clone();
        let present = self.present_characters(known_characters);
        if !present.contains(&main) {
            return Err(ClassicError::ChampionNotFound {
                alias: format!("{} has no Rift Classic version in this patch", self.alias),
            });
        }
        let targets: Vec<&String> = present.iter().filter(|c| self.has_skin(c, skin)).collect();
        if targets.is_empty() {
            return Err(ClassicError::SkinNotFound {
                champion_id: 0,
                skin_id: skin,
            });
        }

        let folder = format!(
            "{CLASSIC_MOD_PREFIX}{}_{skin}",
            self.alias.to_ascii_lowercase()
        );
        let final_dir = mods_dir.join(&folder);
        let partial = mods_dir.join(format!("{folder}.partial"));
        remove_if_present(&partial)?;

        let mut written = 0usize;
        for character in &targets {
            let display = if **character == main {
                self.main_display.clone()
            } else {
                (*character).clone()
            };
            let source = self
                .wad
                .read(wad_path_hash(&skin_bin(character, skin)))?
                .ok_or_else(|| ClassicError::Bin(format!("{character} skin{skin}.bin vanished")))?;

            let bins_dir = partial
                .join("WAD")
                .join(format!("{}.wad.client", self.alias))
                .join("data")
                .join("characters")
                .join(character.as_str())
                .join("skins");
            std::fs::create_dir_all(&bins_dir)?;
            for slot in slots.iter().copied().filter(|slot| *slot != skin) {
                let bin = retarget_skin_bin(&source, &display, skin, slot)?;
                std::fs::write(bins_dir.join(format!("skin{slot}.bin")), bin)?;
                written += 1;
            }

            let anim_target = animation_bin(character, skin);
            if self.wad.contains(wad_path_hash(&anim_target)) {
                if let Ok(Some(anim_source)) = self.wad.read(wad_path_hash(&anim_target)) {
                    let anim_dir = partial
                        .join("WAD")
                        .join(format!("{}.wad.client", self.alias))
                        .join("data")
                        .join("characters")
                        .join(character.as_str())
                        .join("animations");
                    let _ = std::fs::create_dir_all(&anim_dir); // ignore-ok: classic anim dir
                    for slot in slots.iter().copied().filter(|slot| *slot != skin) {
                        if let Ok(retargeted_anim) =
                            retarget_animation_bin(&anim_source, &display, skin, slot)
                        {
                            // ignore-ok: classic anim slot write
                            let _ = std::fs::write(
                                anim_dir.join(format!("skin{slot}.bin")),
                                retargeted_anim,
                            );
                        }
                    }
                }
            }
        }

        let meta = partial.join("META");
        std::fs::create_dir_all(&meta)?;
        let info = serde_json::json!({
            "Author": "Bullet",
            "Name": format!("{} skin {skin} (Rift Classic)", self.alias),
            "Version": "1.0",
            "Description": "Generated from installed game data",
        });
        std::fs::write(meta.join("info.json"), info.to_string())?;

        remove_if_present(&final_dir)?;
        std::fs::rename(&partial, &final_dir)?;

        info!(
            alias = %self.alias,
            skin,
            characters = ?targets,
            slots = ?slots,
            bins = written,
            folder = %folder,
            "Rift Classic mod generated from the installed game"
        );
        Ok(folder)
    }
}

fn remove_if_present(dir: &Path) -> Result<(), ClassicError> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[must_use]
pub fn slots_for(client_skin_id: Option<u32>) -> Vec<u32> {
    let mut slots = CLASSIC_DEFAULT_SLOTS.to_vec();
    if let Some(current) = client_skin_id.map(skin_number) {
        if !slots.contains(&current) {
            slots.push(current);
        }
    }
    slots
}

pub const STANDARD_MOD_PREFIX: &str = "std_";

#[must_use]
pub fn is_generated_folder(name: &str) -> bool {
    name.starts_with(CLASSIC_MOD_PREFIX) || name.starts_with(STANDARD_MOD_PREFIX)
}

#[derive(Debug)]
pub struct StandardChampion {
    pub alias: String,
    wad: WadFile,
    wad_stamp: String,
    cache_dir: Option<PathBuf>,
    scanned: std::sync::OnceLock<BTreeSet<String>>,
}

impl StandardChampion {
    pub fn open(game_dir: &Path, alias: &str) -> Result<Self, ClassicError> {
        if !is_safe_alias(alias) {
            return Err(ClassicError::InvalidAlias(alias.to_owned()));
        }
        let wad_path = game_dir
            .join("DATA")
            .join("FINAL")
            .join("Champions")
            .join(format!("{alias}.wad.client"));
        let wad = WadFile::open(&wad_path).map_err(ClassicError::Wad)?;
        Ok(Self {
            alias: alias.to_owned(),
            wad,
            wad_stamp: wad_stamp(&wad_path),
            cache_dir: None,
            scanned: std::sync::OnceLock::new(),
        })
    }

    #[must_use]
    pub fn with_cache_dir(mut self, cache_dir: &Path) -> Self {
        self.cache_dir = Some(cache_dir.to_path_buf());
        self
    }

    #[must_use]
    pub fn has_skin(&self, skin: u32) -> bool {
        let main = self.alias.to_ascii_lowercase();
        self.wad.contains(wad_path_hash(&skin_bin(&main, skin)))
    }

    #[must_use]
    pub fn skin_numbers(&self, limit: u32) -> Vec<u32> {
        (0..limit).filter(|n| self.has_skin(*n)).collect()
    }

    #[must_use]
    pub fn companions(&self) -> BTreeSet<String> {
        let main = self.alias.to_ascii_lowercase();
        let scanned = self.scanned.get_or_init(|| match &self.cache_dir {
            Some(dir) => cached_names(
                &dir.join(format!("companion_names_{main}.json")),
                &self.wad_stamp,
                &self.alias,
                || character_names_in_bins(&self.wad, &self.alias),
            ),
            None => character_names_in_bins(&self.wad, &self.alias),
        });
        let mut names: BTreeSet<String> = scanned.clone();
        names.remove(&main);
        names.retain(|name| is_safe_alias(name) && !name.starts_with("jade_"));
        names
    }

    #[must_use]
    pub fn companion_source_skin(
        &self,
        companion: &str,
        skin: u32,
        base_skin: Option<u32>,
    ) -> Option<u32> {
        std::iter::once(skin)
            .chain(base_skin.filter(|base| *base != skin && *base != 0))
            .find(|n| self.wad.contains(wad_path_hash(&skin_bin(companion, *n))))
    }

    pub fn read_skin_bin(
        &self,
        character: &str,
        skin: u32,
    ) -> Result<Option<Vec<u8>>, ClassicError> {
        Ok(self.wad.read(wad_path_hash(&skin_bin(character, skin)))?)
    }

    pub fn build_mod(
        &self,
        skin: u32,
        base_skin: Option<u32>,
        mods_dir: &Path,
    ) -> Result<String, ClassicError> {
        let main = self.alias.to_ascii_lowercase();
        let target_bin = skin_bin(&main, skin);
        if !self.wad.contains(wad_path_hash(&target_bin)) {
            return Err(ClassicError::SkinNotFound {
                champion_id: 0,
                skin_id: skin,
            });
        }

        let folder = format!("{STANDARD_MOD_PREFIX}{main}_{skin}");
        let final_dir = mods_dir.join(&folder);
        let partial = mods_dir.join(format!("{folder}.partial"));
        remove_if_present(&partial)?;
        let characters_dir = partial
            .join("WAD")
            .join(format!("{}.wad.client", self.alias))
            .join("data")
            .join("characters");

        let source = self
            .wad
            .read(wad_path_hash(&target_bin))?
            .ok_or_else(|| ClassicError::Bin(format!("{main} skin{skin}.bin not found in WAD")))?;
        let bins_dir = characters_dir.join(&main).join("skins");
        std::fs::create_dir_all(&bins_dir)?;
        let retargeted = retarget_skin_bin(&source, &self.alias, skin, 0)?;
        let mut records = vec![generated_bin_record(
            &self.alias,
            &main,
            skin,
            &source,
            &retargeted,
        )];
        std::fs::write(bins_dir.join("skin0.bin"), retargeted)?;

        let mut retargeted_companions = Vec::new();
        for companion in self.companions() {
            let Some(source_skin) = self.companion_source_skin(&companion, skin, base_skin) else {
                continue;
            };
            let written = self
                .read_skin_bin(&companion, source_skin)
                .and_then(|source| {
                    source.ok_or_else(|| {
                        ClassicError::Bin(format!("{companion} skin{source_skin}.bin vanished"))
                    })
                })
                .and_then(|source| {
                    let retargeted = retarget_skin_bin(&source, &companion, source_skin, 0)?;
                    records.push(generated_bin_record(
                        &self.alias,
                        &companion,
                        source_skin,
                        &source,
                        &retargeted,
                    ));
                    Ok(retargeted)
                })
                .and_then(|retargeted| {
                    let comp_dir = characters_dir.join(&companion).join("skins");
                    std::fs::create_dir_all(&comp_dir)?;
                    std::fs::write(comp_dir.join("skin0.bin"), retargeted)?;
                    Ok(())
                });
            if let Err(e) = written {
                warn!(
                    alias = %self.alias,
                    companion = %companion,
                    skin = source_skin,
                    error = %e,
                    "Companion skin not generated; it keeps its base look in this match"
                );
                continue;
            }
            retargeted_companions.push(companion);
        }

        let meta = partial.join("META");
        std::fs::create_dir_all(&meta)?;
        let info = serde_json::json!({
            "Author": "Bullet",
            "Name": format!("{} skin {skin}", self.alias),
            "Version": "1.0",
            "Description": "Generated dynamically from installed game WAD",
        });
        std::fs::write(meta.join("info.json"), info.to_string())?;
        let manifest = serde_json::json!({
            "alias": self.alias,
            "skin": skin,
            "base_skin": base_skin,
            "game_wad": self.wad_stamp,
            "generated": records,
        });
        std::fs::write(
            meta.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).map_err(|e| ClassicError::Bin(e.to_string()))?,
        )?;

        remove_if_present(&final_dir)?;
        std::fs::rename(&partial, &final_dir)?;

        info!(
            alias = %self.alias,
            skin,
            companions = ?retargeted_companions,
            folder = %folder,
            "Standard skin mod generated directly from installed game WAD"
        );

        Ok(folder)
    }
}

#[must_use]
pub fn resolve_alias_with_id(
    game_dir: &Path,
    client_alias: Option<&str>,
    champion_id: Option<u32>,
    library_champion_dir: &Path,
) -> Option<String> {
    let champions = game_dir.join("DATA").join("FINAL").join("Champions");
    if let Some(alias) = client_alias.filter(|a| is_safe_alias(a)) {
        if champions.join(format!("{alias}.wad.client")).is_file() {
            return Some(alias.to_owned());
        }
    }
    if let Some(installed) =
        champion_id.and_then(|id| crate::client_data::champion_alias(game_dir, id))
    {
        if is_safe_alias(&installed) && champions.join(format!("{installed}.wad.client")).is_file()
        {
            return Some(installed);
        }
    }
    resolve_alias(game_dir, client_alias, library_champion_dir)
}

#[must_use]
pub fn resolve_alias(
    game_dir: &Path,
    client_alias: Option<&str>,
    library_champion_dir: &Path,
) -> Option<String> {
    let champions = game_dir.join("DATA").join("FINAL").join("Champions");
    if let Some(alias) = client_alias.filter(|a| is_safe_alias(a)) {
        if champions.join(format!("{alias}.wad.client")).is_file() {
            return Some(alias.to_owned());
        }
        warn!(
            alias,
            "Client alias has no champion WAD; trying the skin library"
        );
    }

    let mut archives: Vec<PathBuf> = Vec::new();
    let mut stack = vec![library_champion_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("fantome") || e.eq_ignore_ascii_case("zip"))
            {
                archives.push(path);
            }
        }
    }
    archives.sort();

    for archive in archives {
        let Ok(file) = std::fs::File::open(&archive) else {
            continue;
        };
        match bullet_wad::fantome::wad_names_in_archive(std::io::BufReader::new(file)) {
            Ok(names) if names.len() == 1 => {
                if let Some(alias) = names.into_iter().next().filter(|a| is_safe_alias(a)) {
                    return Some(alias);
                }
            }
            Ok(names) => debug!(
                archive = %archive.display(),
                wads = names.len(),
                "Archive does not target exactly one champion WAD"
            ),
            Err(e) => debug!(archive = %archive.display(), error = %e, "Archive unreadable"),
        }
    }
    None
}

#[cfg(test)]
#[path = "generator_tests.rs"]
mod tests;
