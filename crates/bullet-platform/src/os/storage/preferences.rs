use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::PlatformError;

pub struct Preference {
    file: &'static str,
    label: &'static str,
    default: bool,
    value: AtomicBool,
}

pub static AUTO_ACCEPT: Preference =
    Preference::new("auto_accept.json", "automatic match accept", false);

pub static RANDOM_SKIN: Preference =
    Preference::new("random_skin.json", "random skin when none is chosen", true);

pub static INJECTION: Preference = Preference::new("injection.json", "skin injection", true);

pub static LIGHT_LOADING: Preference =
    Preference::new("light_loading.json", "light game loading", true);

pub static IN_CLIENT_UI: Preference = Preference::new(
    "in_client_ui.json",
    "pick skins inside the League client",
    false,
);

pub static HOOK_LOADING_GAME: Preference = Preference::new(
    "hook_loading_game.json",
    "hook a game that is already loading",
    false,
);

pub static REDUCE_MOTION: Preference =
    Preference::new("reduce_motion.json", "reduce UI animations", false);

pub static NOTIFICATIONS: Preference =
    Preference::new("notifications.json", "system notifications", true);

#[cfg(windows)]
#[must_use]
pub fn system_prefers_reduced_motion() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    let mut enabled: i32 = 1;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut enabled as *mut i32).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    ok.is_ok() && enabled == 0
}

#[cfg(not(windows))]
#[must_use]
pub fn system_prefers_reduced_motion() -> bool {
    false
}

pub fn load_reduce_motion() -> bool {
    let custom = crate::paths::state_dir()
        .ok()
        .map(|d| d.join(REDUCE_MOTION.file))
        .is_some_and(|p| p.exists());
    if custom {
        return REDUCE_MOTION.load();
    }
    let preferred = system_prefers_reduced_motion();
    REDUCE_MOTION.value.store(preferred, Ordering::Relaxed);
    preferred
}

pub fn reset_defaults() -> Result<(), PlatformError> {
    INJECTION.set_enabled(INJECTION.default)?;
    RANDOM_SKIN.set_enabled(RANDOM_SKIN.default)?;
    LIGHT_LOADING.set_enabled(LIGHT_LOADING.default)?;
    IN_CLIENT_UI.set_enabled(IN_CLIENT_UI.default)?;
    AUTO_ACCEPT.set_enabled(AUTO_ACCEPT.default)?;
    REDUCE_MOTION.set_enabled(system_prefers_reduced_motion())?;
    NOTIFICATIONS.set_enabled(NOTIFICATIONS.default)?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    enabled: bool,
}

impl Preference {
    const fn new(file: &'static str, label: &'static str, default: bool) -> Self {
        Self {
            file,
            label,
            default,
            value: AtomicBool::new(default),
        }
    }

    pub fn load(&self) -> bool {
        match crate::paths::state_dir() {
            Ok(dir) => self.load_in(&dir),
            Err(e) => {
                warn!(
                    error = %e,
                    preference = self.label,
                    default = self.default,
                    "State folder unavailable; the preference keeps its default"
                );
                self.value.store(self.default, Ordering::Relaxed);
                self.default
            }
        }
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.value.load(Ordering::Relaxed)
    }

    pub fn toggle(&self) -> Result<bool, PlatformError> {
        self.toggle_in(&crate::paths::state_dir()?)
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<(), PlatformError> {
        self.set_enabled_in(&crate::paths::state_dir()?, enabled)
    }

    fn load_in(&self, state_dir: &Path) -> bool {
        let enabled = self.read_from(&state_dir.join(self.file));
        self.value.store(enabled, Ordering::Relaxed);
        enabled
    }

    fn set_enabled_in(&self, state_dir: &Path, enabled: bool) -> Result<(), PlatformError> {
        write_to(&state_dir.join(self.file), enabled, self.label)?;
        self.value.store(enabled, Ordering::Relaxed);
        Ok(())
    }

    fn toggle_in(&self, state_dir: &Path) -> Result<bool, PlatformError> {
        let next = !self.is_enabled();
        write_to(&state_dir.join(self.file), next, self.label)?;
        self.value.store(next, Ordering::Relaxed);
        Ok(next)
    }

    fn read_from(&self, path: &Path) -> bool {
        match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Stored>(&bytes) {
                Ok(stored) => stored.enabled,
                Err(e) => {
                    warn!(
                        path = %path.display(),
                        error = %e,
                        preference = self.label,
                        default = self.default,
                        "Preference file unreadable; the default applies"
                    );
                    self.default
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => self.default,
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    preference = self.label,
                    default = self.default,
                    "Preference file could not be read; the default applies"
                );
                self.default
            }
        }
    }
}

fn write_to(path: &Path, enabled: bool, label: &str) -> Result<(), PlatformError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| PlatformError::Io {
            context: format!("failed to create {}", parent.display()),
            source: e,
        })?;
    }
    let json = serde_json::to_vec(&Stored { enabled })
        .map_err(|e| PlatformError::Path(format!("{label}: {e}")))?;
    crate::fs::atomic_write(path, &json, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!("bullet_preference_{name}_{}", std::process::id()))
            .join("preference.json")
    }

    fn cleanup(path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir); // ignore-ok: fixture cleanup
        }
    }

    #[test]
    fn test_a_missing_file_means_the_default() {
        let off = Preference::new("x.json", "off by default", false);
        let on = Preference::new("x.json", "on by default", true);
        assert!(!off.read_from(&temp_file("missing_off")));
        assert!(on.read_from(&temp_file("missing_on")));
    }

    #[test]
    fn test_the_setting_round_trips() {
        let preference = Preference::new("x.json", "round trip", true);
        let path = temp_file("roundtrip");
        write_to(&path, false, "round trip").expect("write off");
        assert!(!preference.read_from(&path));
        write_to(&path, true, "round trip").expect("write on");
        assert!(preference.read_from(&path));
        cleanup(&path);
    }

    #[test]
    fn test_garbage_means_the_default() {
        let path = temp_file("garbage");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("fixture dir");
        }
        std::fs::write(&path, b"not json").expect("write garbage");
        assert!(!Preference::new("x.json", "off", false).read_from(&path));
        assert!(Preference::new("x.json", "on", true).read_from(&path));
        cleanup(&path);
    }

    #[test]
    fn test_a_toggle_persists_and_survives_a_restart() {
        let dir = std::env::temp_dir().join(format!("bullet_pref_toggle_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture may not exist yet
        let running = Preference::new("pref.json", "toggle", true);
        assert!(running.load_in(&dir), "nothing stored yet: the default");
        assert!(!running.toggle_in(&dir).expect("toggle off"));
        assert!(!running.is_enabled());

        let restarted = Preference::new("pref.json", "toggle", true);
        assert!(
            !restarted.load_in(&dir),
            "the stored choice wins over the default"
        );
        assert!(restarted.toggle_in(&dir).expect("toggle on"));
        assert!(Preference::new("pref.json", "toggle", false).load_in(&dir));
        let pref_set = Preference::new("pref_set.json", "set", false);
        pref_set.set_enabled_in(&dir, true).expect("set enabled");
        assert!(pref_set.load_in(&dir));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture cleanup
    }

    #[test]
    fn test_defaults_of_the_shipped_preferences() {
        assert!(
            !AUTO_ACCEPT.default,
            "accepting a match for the user is opt-in"
        );
        assert!(
            RANDOM_SKIN.default,
            "no match starts without a skin by accident"
        );
        assert!(
            !HOOK_LOADING_GAME.default,
            "a late pick must not hook a game that is already loading by default"
        );
        assert!(
            !IN_CLIENT_UI.default,
            "the native Bullet window is the default until the user opts into the in-client carousel"
        );
    }
}
