use std::path::{Path, PathBuf};

use bullet_core::overlay::{Catalog, OverlayCommand};
use bullet_core::state::StateSender;

use crate::error::InjectError;
use crate::overlay_process::OverlayProcess;

pub const MIN_MANAGER_VERSION: (u16, u16, u16) = (1, 33, 0);

#[derive(Debug, PartialEq, Eq)]
pub enum ManagerInstallState {
    Ready {
        path: PathBuf,
        version: Option<(u16, u16, u16)>,
    },
    Outdated {
        path: PathBuf,
        found: (u16, u16, u16),
    },
    Missing,
}

#[must_use]
pub fn check_manager_install() -> ManagerInstallState {
    #[cfg(target_os = "macos")]
    {
        return ManagerInstallState::Ready {
            path: std::env::current_exe().unwrap_or_else(|_| {
                PathBuf::from("/Applications/Bullet.app/Contents/MacOS/bullet")
            }),
            version: Some(MIN_MANAGER_VERSION),
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        ManagerInstallState::Missing
    }
}

#[must_use]
pub fn find_manager_manager() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return Some(
            std::env::current_exe().unwrap_or_else(|_| {
                PathBuf::from("/Applications/Bullet.app/Contents/MacOS/bullet")
            }),
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub async fn warmup_bridge(_tools_dirs: &[PathBuf]) {}

pub async fn shutdown_bridge() {}

pub async fn deactivate_in_client_ui(_tools_dirs: &[PathBuf]) {}

pub async fn sync_in_client_ui(
    _enabled: bool,
    _tools_dirs: &[PathBuf],
    _catalog: Option<&Catalog>,
    _selected_id: Option<u32>,
) -> (Vec<OverlayCommand>, bool) {
    (Vec::new(), false)
}

#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub async fn spawn_ltf_patcher(
    _tools_dirs: &[PathBuf],
    mods_dir: &Path,
    mods: &[String],
    state_tx: Option<StateSender>,
) -> Result<OverlayProcess, InjectError> {
    #[cfg(target_os = "macos")]
    {
        return crate::macos_patcher::spawn_macos_patcher(mods_dir, mods, state_tx).await;
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(InjectError::Process(
            "Private LTK bridge module is not linked in this build".into(),
        ))
    }
}
