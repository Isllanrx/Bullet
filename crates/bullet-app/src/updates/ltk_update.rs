use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "macos"))]
use std::time::Duration;

use futures_util::future::BoxFuture;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tracing::warn;
#[cfg(not(target_os = "macos"))]
use tracing::{debug, info};

use bullet_core::state::StateReceiver;

#[cfg(not(target_os = "macos"))]
use crate::update_check;
use crate::update_check::Version;

pub const LTK_RELEASES_API: &str =
    "https://api.github.com/repos/LeagueToolkit/ltk-manager/releases/latest";
pub const LTK_RELEASES_PAGE: &str = "https://github.com/LeagueToolkit/ltk-manager/releases/latest";

#[cfg(not(target_os = "macos"))]
const USER_AGENT: &str = concat!("Bullet/", env!("CARGO_PKG_VERSION"), " (ltk update check)");
#[cfg(not(target_os = "macos"))]
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(18);
#[cfg(not(target_os = "macos"))]
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
#[cfg(not(target_os = "macos"))]
const BUSY_RETRY: Duration = Duration::from_secs(60);
#[cfg(not(target_os = "macos"))]
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const NOTIFIED_FILE: &str = "ltk_update_notified.txt";

#[derive(Debug, Clone, Default)]
pub struct LtkUpdateNotice(Arc<Mutex<Option<(String, String)>>>);

impl LtkUpdateNotice {
    #[must_use]
    pub fn available(&self) -> Option<(String, String)> {
        self.0.lock().ok().and_then(|slot| slot.clone())
    }

    pub fn set(&self, installed: String, latest: String) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some((installed, latest));
        }
    }
}

#[derive(Debug, Deserialize)]
struct LatestLtkRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub fn newer_ltk_release(
    installed: (u16, u16, u16),
    body: &str,
) -> Result<Option<Version>, String> {
    let release: LatestLtkRelease =
        serde_json::from_str(body).map_err(|e| format!("unreadable LTK release data: {e}"))?;
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let latest = Version::parse(&release.tag_name)
        .ok_or_else(|| format!("LTK release tag '{}' is not a version", release.tag_name))?;
    let current = Version {
        major: u64::from(installed.0),
        minor: u64::from(installed.1),
        patch: u64::from(installed.2),
    };
    Ok((latest > current).then_some(latest))
}

#[must_use]
pub fn already_notified(state_dir: &Path, latest: &str) -> bool {
    match std::fs::read_to_string(state_dir.join(NOTIFIED_FILE)) {
        Ok(saved) => saved.trim() == latest,
        Err(_) => false,
    }
}

pub fn remember_notified(state_dir: &Path, latest: &str) {
    if let Err(e) =
        bullet_platform::fs::atomic_write(&state_dir.join(NOTIFIED_FILE), latest.as_bytes(), false)
    {
        warn!(error = %e, latest, "Could not record the LTK update notice");
    }
}

pub struct LtkUpdateCheck {
    pub state_dir: PathBuf,
    pub notice: LtkUpdateNotice,
    pub notify: Box<dyn Fn(String, String) -> BoxFuture<'static, ()> + Send>,
}

#[cfg(not(target_os = "macos"))]
async fn pause(token: &CancellationToken, duration: Duration) -> bool {
    tokio::select! {
        _ = token.cancelled() => false,
        () = tokio::time::sleep(duration) => true,
    }
}

#[cfg(not(target_os = "macos"))]
async fn fetch_latest_ltk(
    client: &reqwest::Client,
    installed: (u16, u16, u16),
) -> Result<Option<Version>, String> {
    let response = client
        .get(LTK_RELEASES_API)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub answered HTTP {} for LTK",
            response.status()
        ));
    }
    let body = response.text().await.map_err(|e| e.to_string())?;
    newer_ltk_release(installed, &body)
}

#[cfg_attr(target_os = "macos", allow(unused_variables))]
pub async fn run(check: LtkUpdateCheck, state_rx: StateReceiver, token: CancellationToken) {
    #[cfg(not(target_os = "macos"))]
    {
        let client = match reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build() {
            Ok(client) => client,
            Err(e) => {
                warn!(error = %e, "LTK update check off: HTTP client unavailable");
                return;
            }
        };
        if !pause(&token, FIRST_CHECK_DELAY).await {
            return;
        }
        loop {
            let installed = match bullet_inject::ltf::check_manager_install() {
                bullet_inject::ltf::ManagerInstallState::Ready { version, .. } => version,
                bullet_inject::ltf::ManagerInstallState::Outdated { found, .. } => Some(found),
                bullet_inject::ltf::ManagerInstallState::Missing => None,
            };
            if let Some(installed_ver) = installed {
                match fetch_latest_ltk(&client, installed_ver).await {
                    Ok(Some(latest)) => {
                        let installed_str = bullet_platform::version::format_version(installed_ver);
                        let latest_str = latest.to_string();
                        if check.notice.available().as_ref().map(|(_, l)| l.as_str())
                            != Some(latest_str.as_str())
                        {
                            info!(
                                installed = %installed_str,
                                latest = %latest_str,
                                "A newer LTK Manager release is available"
                            );
                            check.notice.set(installed_str.clone(), latest_str.clone());
                        }
                        if !already_notified(&check.state_dir, &latest_str) {
                            while update_check::is_busy(state_rx.borrow().phase) {
                                if !pause(&token, BUSY_RETRY).await {
                                    return;
                                }
                            }
                            (check.notify)(installed_str, latest_str).await;
                            if token.is_cancelled() {
                                return;
                            }
                        }
                    }
                    Ok(None) => debug!("LTK Manager is up to date"),
                    Err(e) => debug!(error = %e, "LTK Manager update check failed; retrying later"),
                }
            }
            if !pause(&token, CHECK_INTERVAL).await {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_only_newer_stable_ltk_releases() {
        let newer = r#"{"tag_name":"v1.34.0","draft":false,"prerelease":false}"#;
        assert_eq!(
            newer_ltk_release((1, 33, 0), newer),
            Ok(Some(Version {
                major: 1,
                minor: 34,
                patch: 0
            }))
        );
        let same = r#"{"tag_name":"v1.33.0","draft":false,"prerelease":false}"#;
        assert_eq!(newer_ltk_release((1, 33, 0), same), Ok(None));
        let pre = r#"{"tag_name":"v1.35.0","draft":false,"prerelease":true}"#;
        assert_eq!(newer_ltk_release((1, 33, 0), pre), Ok(None));
        let draft = r#"{"tag_name":"v1.35.0","draft":true,"prerelease":false}"#;
        assert_eq!(newer_ltk_release((1, 33, 0), draft), Ok(None));
        assert!(newer_ltk_release((1, 33, 0), "bad json").is_err());
    }

    #[test]
    fn remembers_notified_ltk_version_once_and_stores_notice() {
        let dir = std::env::temp_dir().join(format!("bullet_ltk_upd_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture cleanup
        std::fs::create_dir_all(&dir).expect("dir");
        assert!(!already_notified(&dir, "1.34"));
        remember_notified(&dir, "1.34");
        assert!(already_notified(&dir, "1.34"));
        assert!(!already_notified(&dir, "1.35"));
        let notice = LtkUpdateNotice::default();
        assert_eq!(notice.available(), None);
        notice.set("1.33.0".into(), "1.34".into());
        assert_eq!(notice.available(), Some(("1.33.0".into(), "1.34".into())));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture cleanup
    }
}
