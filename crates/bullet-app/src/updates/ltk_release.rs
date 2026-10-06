use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bullet_core::state::StateReceiver;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::update_check::{Version, is_busy};

pub const LTK_REPOSITORY: &str = "https://github.com/LeagueToolkit/ltk-manager";

const LTK_API: &str = "https://api.github.com/repos/LeagueToolkit/ltk-manager";
const RESOURCES_PATH: &str = "src-tauri/resources";
const USER_AGENT: &str = concat!("Bullet/", env!("CARGO_PKG_VERSION"), " (injector check)");
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(60);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const BUSY_RETRY: Duration = Duration::from_secs(60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BINARY_BYTES: usize = 16 * 1024 * 1024;
const RELEASES_PER_PAGE: u32 = 30;
const MAX_INSPECTIONS_PER_CHECK: usize = 12;
const VERDICTS_FILE: &str = "ltk_releases.txt";
const NOTIFIED_FILE: &str = "ltk_notified.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditedInjector {
    pub host_sha256: &'static str,
    pub dll_sha256: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Injector {
    Audited,
    New,
}

impl Injector {
    fn as_str(self) -> &'static str {
        match self {
            Self::Audited => "audited",
            Self::New => "new",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "audited" => Some(Self::Audited),
            "new" => Some(Self::New),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LtkStatus {
    pub latest: String,
    pub latest_injector: Injector,
    pub compatible: Option<String>,
}

#[must_use]
pub fn release_page(version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{LTK_REPOSITORY}/releases/tag/v{version}"),
        None => format!("{LTK_REPOSITORY}/releases"),
    }
}

#[derive(Debug, Deserialize)]
struct PublishedRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub fn published_versions(body: &str) -> Result<Vec<String>, String> {
    let releases: Vec<PublishedRelease> =
        serde_json::from_str(body).map_err(|e| format!("unreadable release list: {e}"))?;
    let mut versions: Vec<(Version, String)> = releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| {
            let tag = r.tag_name.trim();
            let raw = tag
                .strip_prefix('v')
                .or_else(|| tag.strip_prefix('V'))
                .unwrap_or(tag);
            Version::parse(raw).map(|v| (v, raw.to_owned()))
        })
        .collect();
    versions.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    versions.dedup_by(|a, b| a.0 == b.0);
    Ok(versions.into_iter().map(|(_, raw)| raw).collect())
}

#[must_use]
pub fn classify(audited: AuditedInjector, host_sha256: &str, dll_sha256: &str) -> Injector {
    if host_sha256.eq_ignore_ascii_case(audited.host_sha256)
        && dll_sha256.eq_ignore_ascii_case(audited.dll_sha256)
    {
        Injector::Audited
    } else {
        Injector::New
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verdicts {
    fingerprint: String,
    by_version: BTreeMap<String, Injector>,
}

impl Verdicts {
    #[must_use]
    pub fn new(audited: AuditedInjector) -> Self {
        Self {
            fingerprint: fingerprint(audited),
            by_version: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn get(&self, version: &str) -> Option<Injector> {
        self.by_version.get(version).copied()
    }

    pub fn insert(&mut self, version: &str, injector: Injector) {
        self.by_version.insert(version.to_owned(), injector);
    }

    #[must_use]
    pub fn status(&self, newest_first: &[String]) -> Option<LtkStatus> {
        let latest = newest_first.first()?;
        let latest_injector = self.get(latest)?;
        let compatible = newest_first
            .iter()
            .find(|v| self.get(v) == Some(Injector::Audited))
            .cloned();
        Some(LtkStatus {
            latest: latest.clone(),
            latest_injector,
            compatible,
        })
    }

    #[must_use]
    pub fn status_from_cache(&self) -> Option<LtkStatus> {
        let mut known: Vec<(Version, &String)> = self
            .by_version
            .keys()
            .filter_map(|raw| Version::parse(raw).map(|v| (v, raw)))
            .collect();
        known.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        let ordered: Vec<String> = known.into_iter().map(|(_, raw)| raw.clone()).collect();
        self.status(&ordered)
    }

    fn render(&self) -> String {
        let mut out = format!("{}\n", self.fingerprint);
        for (version, injector) in &self.by_version {
            out.push_str(&format!("{version} {}\n", injector.as_str()));
        }
        out
    }

    fn parse(raw: &str, audited: AuditedInjector) -> Self {
        let mut lines = raw.lines();
        let mut verdicts = Self::new(audited);
        if lines.next().map(str::trim) != Some(verdicts.fingerprint.as_str()) {
            return verdicts;
        }
        for line in lines {
            let mut parts = line.split_whitespace();
            if let (Some(version), Some(injector), None) =
                (parts.next(), parts.next(), parts.next())
            {
                if let (Some(_), Some(injector)) =
                    (Version::parse(version), Injector::parse(injector))
                {
                    verdicts.insert(version, injector);
                }
            }
        }
        verdicts
    }
}

fn fingerprint(audited: AuditedInjector) -> String {
    format!(
        "{}:{}",
        audited.host_sha256.to_ascii_lowercase(),
        audited.dll_sha256.to_ascii_lowercase()
    )
}

#[must_use]
pub fn load_verdicts(state_dir: &Path, audited: AuditedInjector) -> Verdicts {
    std::fs::read_to_string(state_dir.join(VERDICTS_FILE))
        .map(|raw| Verdicts::parse(&raw, audited))
        .unwrap_or_else(|_| Verdicts::new(audited))
}

pub fn save_verdicts(state_dir: &Path, verdicts: &Verdicts) {
    if let Err(e) = bullet_platform::fs::atomic_write(
        &state_dir.join(VERDICTS_FILE),
        verdicts.render().as_bytes(),
        false,
    ) {
        warn!(error = %e, "Could not record the LTK Manager release check; it will run again next launch");
    }
}

#[must_use]
pub fn already_notified(state_dir: &Path, version: &str) -> bool {
    std::fs::read_to_string(state_dir.join(NOTIFIED_FILE))
        .is_ok_and(|saved| saved.trim() == version)
}

pub fn remember_notified(state_dir: &Path, version: &str) {
    if let Err(e) =
        bullet_platform::fs::atomic_write(&state_dir.join(NOTIFIED_FILE), version.as_bytes(), false)
    {
        warn!(error = %e, version = %version, "Could not record the injector notice; it may be shown again next launch");
    }
}

#[derive(Debug, Clone, Default)]
pub struct LtkNotice(Arc<Mutex<Option<LtkStatus>>>);

impl LtkNotice {
    #[must_use]
    pub fn status(&self) -> Option<LtkStatus> {
        self.0.lock().map(|v| v.clone()).unwrap_or_default()
    }

    fn set(&self, status: LtkStatus) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(status);
        }
    }
}

pub struct LtkCheck {
    pub audited: AuditedInjector,
    pub state_dir: PathBuf,
    pub notice: LtkNotice,
    pub notify: Box<dyn Fn(&LtkStatus) + Send>,
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())
}

async fn get(client: &reqwest::Client, url: &str, accept: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::ACCEPT, accept)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("GitHub answered HTTP {}", response.status()));
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_BINARY_BYTES as u64)
    {
        return Err(format!("{url} is larger than {MAX_BINARY_BYTES} bytes"));
    }
    let body = response.bytes().await.map_err(|e| e.to_string())?;
    if body.len() > MAX_BINARY_BYTES {
        return Err(format!("{url} is larger than {MAX_BINARY_BYTES} bytes"));
    }
    Ok(body.to_vec())
}

async fn release_versions(client: &reqwest::Client) -> Result<Vec<String>, String> {
    let body = get(
        client,
        &format!("{LTK_API}/releases?per_page={RELEASES_PER_PAGE}"),
        "application/vnd.github+json",
    )
    .await?;
    published_versions(&String::from_utf8_lossy(&body))
}

async fn resource_sha256(
    client: &reqwest::Client,
    version: &str,
    file: &str,
) -> Result<String, String> {
    let url = format!("{LTK_API}/contents/{RESOURCES_PATH}/{file}?ref=v{version}");
    let bytes = get(client, &url, "application/vnd.github.raw").await?;
    Ok(bullet_inject::dll_validator::compute_sha256(&bytes))
}

async fn inspect(
    client: &reqwest::Client,
    audited: AuditedInjector,
    version: &str,
) -> Result<Injector, String> {
    let host = resource_sha256(client, version, "ltk_patcher_host.exe").await?;
    let dll = resource_sha256(client, version, "ltk_patcher_dll.dll").await?;
    Ok(classify(audited, &host, &dll))
}

async fn refresh(
    client: &reqwest::Client,
    audited: AuditedInjector,
    verdicts: &mut Verdicts,
) -> Result<Option<LtkStatus>, String> {
    let versions = release_versions(client).await?;
    let mut inspected = 0;
    for version in &versions {
        let injector = match verdicts.get(version) {
            Some(known) => known,
            None if inspected < MAX_INSPECTIONS_PER_CHECK => {
                inspected += 1;
                let injector = inspect(client, audited, version).await?;
                verdicts.insert(version, injector);
                injector
            }
            None => break,
        };
        if injector == Injector::Audited {
            break;
        }
    }
    Ok(verdicts.status(&versions))
}

pub async fn compatible_version(audited: AuditedInjector, state_dir: &Path) -> Option<String> {
    let mut verdicts = load_verdicts(state_dir, audited);
    let refreshed = match http_client() {
        Ok(client) => refresh(&client, audited, &mut verdicts).await,
        Err(e) => Err(e),
    };
    match refreshed {
        Ok(status) => {
            save_verdicts(state_dir, &verdicts);
            status.and_then(|s| s.compatible)
        }
        Err(e) => {
            save_verdicts(state_dir, &verdicts);
            debug!(error = %e, "LTK Manager releases could not be checked; using the last known result");
            verdicts.status_from_cache().and_then(|s| s.compatible)
        }
    }
}

async fn pause(token: &CancellationToken, duration: Duration) -> bool {
    tokio::select! {
        _ = token.cancelled() => false,
        () = tokio::time::sleep(duration) => true,
    }
}

pub async fn run(check: LtkCheck, state_rx: StateReceiver, token: CancellationToken) {
    let client = match http_client() {
        Ok(client) => client,
        Err(e) => {
            warn!(error = %e, "Injector check off: the HTTP client could not be created");
            return;
        }
    };
    let mut verdicts = load_verdicts(&check.state_dir, check.audited);
    if let Some(status) = verdicts.status_from_cache() {
        check.notice.set(status);
    }
    if !pause(&token, FIRST_CHECK_DELAY).await {
        return;
    }
    loop {
        match refresh(&client, check.audited, &mut verdicts).await {
            Ok(Some(status)) => {
                save_verdicts(&check.state_dir, &verdicts);
                if check.notice.status().as_ref() != Some(&status) {
                    info!(
                        latest = %status.latest,
                        latest_injector = status.latest_injector.as_str(),
                        compatible = status.compatible.as_deref().unwrap_or("none"),
                        "LTK Manager releases checked against the audited injector"
                    );
                    check.notice.set(status.clone());
                }
                if status.latest_injector == Injector::New
                    && !already_notified(&check.state_dir, &status.latest)
                {
                    while is_busy(state_rx.borrow().phase) {
                        if !pause(&token, BUSY_RETRY).await {
                            return;
                        }
                    }
                    (check.notify)(&status);
                    remember_notified(&check.state_dir, &status.latest);
                }
            }
            Ok(None) => {
                save_verdicts(&check.state_dir, &verdicts);
                debug!("No published LTK Manager release could be classified");
            }
            Err(e) => {
                save_verdicts(&check.state_dir, &verdicts);
                debug!(error = %e, "Injector check failed; trying again later");
            }
        }
        if !pause(&token, CHECK_INTERVAL).await {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUDITED: AuditedInjector = AuditedInjector {
        host_sha256: "a7c4047ce7548c7ae820bc440735f15b9d1a495acf061dbb5a5a2893a0ed8d7c",
        dll_sha256: "07a43bf36a389eb00f6276e333bd7f2b95218f25a58e1e128ff4d2e4ab2dc99b",
    };

    fn list(versions: &[&str]) -> Vec<String> {
        versions.iter().map(|v| (*v).to_owned()).collect()
    }

    #[test]
    fn published_versions_come_newest_first_without_drafts_or_prereleases() {
        let body = r#"[
            {"tag_name":"v1.24.0"},
            {"tag_name":"v1.26.1","draft":false,"prerelease":false},
            {"tag_name":"v1.27.0","prerelease":true},
            {"tag_name":"v1.28.0","draft":true},
            {"tag_name":"nightly"},
            {"tag_name":"v1.26.0"}
        ]"#;
        assert_eq!(
            published_versions(body),
            Ok(list(&["1.26.1", "1.26.0", "1.24.0"]))
        );
        assert!(published_versions("not json").is_err());
    }

    #[test]
    fn only_both_audited_hashes_count_as_the_audited_injector() {
        let (host, dll) = (AUDITED.host_sha256, AUDITED.dll_sha256);
        assert_eq!(classify(AUDITED, host, dll), Injector::Audited);
        assert_eq!(
            classify(AUDITED, &host.to_ascii_uppercase(), dll),
            Injector::Audited
        );
        assert_eq!(classify(AUDITED, host, &"0".repeat(64)), Injector::New);
        assert_eq!(classify(AUDITED, &"0".repeat(64), dll), Injector::New);
    }

    #[test]
    fn the_compatible_version_is_the_newest_release_with_the_audited_files() {
        let mut verdicts = Verdicts::new(AUDITED);
        verdicts.insert("1.27.0", Injector::New);
        verdicts.insert("1.26.1", Injector::Audited);
        verdicts.insert("1.26.0", Injector::Audited);
        let status = verdicts
            .status(&list(&["1.27.0", "1.26.1", "1.26.0"]))
            .expect("status");
        assert_eq!(status.latest, "1.27.0");
        assert_eq!(status.latest_injector, Injector::New);
        assert_eq!(status.compatible.as_deref(), Some("1.26.1"));

        let current = verdicts
            .status(&list(&["1.26.1", "1.26.0"]))
            .expect("status");
        assert_eq!(current.latest_injector, Injector::Audited);
        assert_eq!(current.compatible.as_deref(), Some("1.26.1"));

        assert_eq!(verdicts.status(&list(&["1.28.0"])), None);
        assert_eq!(verdicts.status(&[]), None);
    }

    #[test]
    fn the_cache_orders_versions_numerically() {
        let mut verdicts = Verdicts::new(AUDITED);
        verdicts.insert("1.9.0", Injector::Audited);
        verdicts.insert("1.10.0", Injector::New);
        let status = verdicts.status_from_cache().expect("status");
        assert_eq!(status.latest, "1.10.0");
        assert_eq!(status.compatible.as_deref(), Some("1.9.0"));
    }

    #[test]
    fn verdicts_round_trip_and_reset_when_the_audited_build_changes() {
        let dir = std::env::temp_dir().join(format!("bullet_ltk_check_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture may not exist yet
        std::fs::create_dir_all(&dir).expect("fixture dir");
        assert_eq!(load_verdicts(&dir, AUDITED), Verdicts::new(AUDITED));

        let mut verdicts = Verdicts::new(AUDITED);
        verdicts.insert("1.26.1", Injector::Audited);
        verdicts.insert("1.27.0", Injector::New);
        save_verdicts(&dir, &verdicts);
        assert_eq!(load_verdicts(&dir, AUDITED), verdicts);

        let other = AuditedInjector {
            host_sha256: AUDITED.host_sha256,
            dll_sha256: "1111111111111111111111111111111111111111111111111111111111111111",
        };
        assert_eq!(load_verdicts(&dir, other), Verdicts::new(other));

        let garbage = format!(
            "{}\n1.26.1 maybe\nnot-a-version audited\n1.27.0 new extra\n",
            fingerprint(AUDITED)
        );
        std::fs::write(dir.join(VERDICTS_FILE), garbage).expect("write");
        assert_eq!(load_verdicts(&dir, AUDITED), Verdicts::new(AUDITED));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture cleanup
    }

    #[test]
    fn a_new_injector_is_announced_once() {
        let dir = std::env::temp_dir().join(format!("bullet_ltk_notice_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture may not exist yet
        std::fs::create_dir_all(&dir).expect("fixture dir");
        assert!(!already_notified(&dir, "1.27.0"));
        remember_notified(&dir, "1.27.0");
        assert!(already_notified(&dir, "1.27.0"));
        assert!(!already_notified(&dir, "1.28.0"));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture cleanup
    }

    #[test]
    fn pages_point_at_the_ltk_manager_releases() {
        assert_eq!(
            release_page(Some("1.26.1")),
            format!("{LTK_REPOSITORY}/releases/tag/v1.26.1")
        );
        assert_eq!(release_page(None), format!("{LTK_REPOSITORY}/releases"));
    }

    #[test]
    fn the_notice_holds_the_latest_status() {
        let notice = LtkNotice::default();
        assert_eq!(notice.status(), None);
        let status = LtkStatus {
            latest: "1.26.1".into(),
            latest_injector: Injector::Audited,
            compatible: Some("1.26.1".into()),
        };
        notice.set(status.clone());
        assert_eq!(notice.clone().status(), Some(status));
    }
}
