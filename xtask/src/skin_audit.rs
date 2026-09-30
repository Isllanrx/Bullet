use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use bullet_classic::generator::StandardChampion;
use bullet_wad::hash::wad_path_hash;
use bullet_wad::wad::WadFile;

const MAX_SKIN_NUMBER: u32 = 200;

#[derive(Debug, Default)]
pub struct SkinFinding {
    pub build_error: Option<String>,
    pub companions_written: Vec<String>,
    pub source_links_dropped: bool,
    pub shared_with_map: Vec<String>,
}

impl SkinFinding {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.build_error.is_none() && !self.source_links_dropped
    }
}

#[derive(Debug, Default)]
pub struct ChampionReport {
    pub alias: String,
    pub open_error: Option<String>,
    pub companions: BTreeSet<String>,
    pub companion_skins: BTreeMap<String, BTreeSet<u32>>,
    pub skins: BTreeMap<u32, SkinFinding>,
    pub scan_ms: u128,
}

pub fn champion_aliases(game: &Path, filters: &[String]) -> Vec<String> {
    let dir = game.join("DATA").join("FINAL").join("Champions");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut aliases: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let alias = name.strip_suffix(".wad.client")?;
            (!alias.contains('.')).then(|| alias.to_owned())
        })
        .filter(|alias| filters.is_empty() || filters.iter().any(|f| f.eq_ignore_ascii_case(alias)))
        .collect();
    aliases.sort();
    aliases
}

pub type MapIndex = Vec<(String, HashSet<u64>)>;

pub fn map_hashes(game: &Path) -> MapIndex {
    let dir = game
        .join("DATA")
        .join("FINAL")
        .join("Maps")
        .join("Shipping");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut maps: MapIndex = entries
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| {
            let name = path
                .file_name()?
                .to_str()?
                .strip_suffix(".wad.client")?
                .to_owned();
            let wad = WadFile::open_toc_only(&path).ok()?;
            Some((name, wad.toc().map(|e| e.path_hash).collect()))
        })
        .collect();
    maps.sort_by(|a, b| a.0.cmp(&b.0));
    maps
}

fn maps_holding(maps: &MapIndex, hash: u64) -> Vec<&str> {
    maps.iter()
        .filter(|(_, hashes)| hashes.contains(&hash))
        .map(|(name, _)| name.as_str())
        .collect()
}

fn skin_bin(character: &str, skin: u32) -> String {
    format!("data/characters/{character}/skins/skin{skin}.bin")
}

fn keeps_source_links(
    champion: &StandardChampion,
    main: &str,
    skin: u32,
    character_dir: &Path,
) -> bool {
    let links = |bytes: Vec<u8>| {
        bullet_wad::prop::parse_prop_file(&bytes)
            .ok()
            .map(|bin| bin.links)
    };
    let source = champion
        .read_skin_bin(main, skin)
        .ok()
        .flatten()
        .and_then(links);
    let generated = std::fs::read(character_dir.join("skins").join("skin0.bin"))
        .ok()
        .and_then(links);
    match (source, generated) {
        (Some(source), Some(generated)) => source.iter().all(|link| generated.contains(link)),
        _ => false,
    }
}

fn generated_paths(wad_root: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    let mut stack = vec![wad_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(relative) = path.strip_prefix(wad_root) {
                paths.push(
                    relative
                        .to_string_lossy()
                        .replace('\\', "/")
                        .to_ascii_lowercase(),
                );
            }
        }
    }
    paths.sort();
    paths
}

pub fn audit_champion(
    game: &Path,
    alias: &str,
    maps: &MapIndex,
    cache_dir: &Path,
    staging: &Path,
    keep: bool,
) -> ChampionReport {
    let mut report = ChampionReport {
        alias: alias.to_owned(),
        ..ChampionReport::default()
    };
    let champion = match StandardChampion::open(game, alias) {
        Ok(champion) => champion.with_cache_dir(cache_dir),
        Err(e) => {
            report.open_error = Some(e.to_string());
            return report;
        }
    };
    let main = alias.to_ascii_lowercase();

    let started = std::time::Instant::now();
    report.companions = champion.companions();
    report.scan_ms = started.elapsed().as_millis();
    for companion in &report.companions {
        let numbers: BTreeSet<u32> = (0..MAX_SKIN_NUMBER)
            .filter(|n| champion.companion_source_skin(companion, *n, None) == Some(*n))
            .collect();
        if !numbers.is_empty() {
            report.companion_skins.insert(companion.clone(), numbers);
        }
    }

    for skin in champion
        .skin_numbers(MAX_SKIN_NUMBER)
        .into_iter()
        .filter(|n| *n != 0)
    {
        let mut finding = SkinFinding::default();
        match champion.build_mod(skin, None, staging) {
            Ok(folder) => {
                let wad_root = staging
                    .join(&folder)
                    .join("WAD")
                    .join(format!("{alias}.wad.client"));
                let characters = wad_root.join("data").join("characters");
                for (companion, numbers) in &report.companion_skins {
                    if numbers.contains(&skin)
                        && characters
                            .join(companion)
                            .join("skins")
                            .join("skin0.bin")
                            .is_file()
                    {
                        finding.companions_written.push(companion.clone());
                    }
                }
                finding.source_links_dropped =
                    !keeps_source_links(&champion, &main, skin, &characters.join(&main));
                finding.shared_with_map = generated_paths(&wad_root)
                    .into_iter()
                    .filter_map(|path| {
                        let holders = maps_holding(maps, wad_path_hash(&path));
                        (!holders.is_empty()).then(|| format!("{path} ({})", holders.join("/")))
                    })
                    .collect();
                if !keep {
                    let _ = std::fs::remove_dir_all(staging.join(&folder)); // ignore-ok: probe scratch folder
                }
            }
            Err(e) => finding.build_error = Some(e.to_string()),
        }
        report.skins.insert(skin, finding);
    }
    report
}

#[must_use]
pub fn missed_companions(
    reports: &[ChampionReport],
    game: &Path,
) -> BTreeMap<String, BTreeSet<String>> {
    let every_name: BTreeSet<String> = reports
        .iter()
        .flat_map(|r| r.companions.iter().cloned())
        .chain(reports.iter().map(|r| r.alias.to_ascii_lowercase()))
        .collect();
    let mut missed = BTreeMap::new();
    for report in reports.iter().filter(|r| r.open_error.is_none()) {
        let path = game
            .join("DATA")
            .join("FINAL")
            .join("Champions")
            .join(format!("{}.wad.client", report.alias));
        let Ok(wad) = WadFile::open_toc_only(&path) else {
            continue;
        };
        let main = report.alias.to_ascii_lowercase();
        let found: BTreeSet<String> = every_name
            .iter()
            .filter(|name| **name != main && !report.companions.contains(*name))
            .filter(|name| {
                (1..MAX_SKIN_NUMBER).any(|n| wad.contains(wad_path_hash(&skin_bin(name, n))))
            })
            .cloned()
            .collect();
        if !found.is_empty() {
            missed.insert(report.alias.clone(), found);
        }
    }
    missed
}

#[must_use]
pub fn render(reports: &[ChampionReport], missed: &BTreeMap<String, BTreeSet<String>>) -> String {
    let skins: usize = reports.iter().map(|r| r.skins.len()).sum();
    let clean: usize = reports
        .iter()
        .flat_map(|r| r.skins.values())
        .filter(|f| f.is_clean())
        .count();
    let with_companions = reports
        .iter()
        .filter(|r| !r.companion_skins.is_empty())
        .count();

    let mut lines = vec![
        "# Skin audit".to_owned(),
        String::new(),
        format!(
            "- champions: {} | skins and chromas generated: {skins} | clean: {clean} | with findings: {}",
            reports.len(),
            skins - clean
        ),
        format!("- champions with companion characters: {with_companions}"),
        String::new(),
        "## Companion characters per champion".to_owned(),
        String::new(),
        "| Champion | Companion | Skin numbers with their own bin |".to_owned(),
        "| --- | --- | --- |".to_owned(),
    ];
    for report in reports {
        for (companion, numbers) in &report.companion_skins {
            lines.push(format!("| {} | {companion} | {numbers:?} |", report.alias));
        }
    }
    lines.extend([
        String::new(),
        "## Findings per skin".to_owned(),
        String::new(),
        "| Champion | Skin | Build error | Source links dropped | Paths also written into a map WAD |"
            .to_owned(),
        "| --- | --- | --- | --- | --- |".to_owned(),
    ]);
    for report in reports {
        if let Some(error) = &report.open_error {
            lines.push(format!(
                "| {} | - | WAD not opened: {error} | | |",
                report.alias
            ));
            continue;
        }
        for (skin, finding) in report
            .skins
            .iter()
            .filter(|(_, f)| !f.is_clean() || !f.shared_with_map.is_empty())
        {
            lines.push(format!(
                "| {} | {skin} | {} | {} | {} |",
                report.alias,
                finding.build_error.as_deref().unwrap_or(""),
                if finding.source_links_dropped {
                    "yes"
                } else {
                    ""
                },
                finding.shared_with_map.join(", ")
            ));
        }
    }
    lines.extend([
        String::new(),
        "## Characters with skin bins that the scan did not attach".to_owned(),
        String::new(),
    ]);
    if missed.is_empty() {
        lines.push("None.".to_owned());
    }
    for (alias, names) in missed {
        lines.push(format!("- {alias}: {names:?}"));
    }
    lines.push(String::new());
    lines.join("\n")
}

#[must_use]
pub fn default_report_path() -> PathBuf {
    std::env::temp_dir().join("bullet_skin_audit.md")
}
