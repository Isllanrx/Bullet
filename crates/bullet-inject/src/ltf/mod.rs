use std::path::{Path, PathBuf};

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
    ManagerInstallState::Missing
}

#[must_use]
pub fn find_manager_manager() -> Option<PathBuf> {
    None
}

pub async fn warmup_bridge(_tools_dirs: &[PathBuf]) {}

pub async fn spawn_ltf_patcher(
    _tools_dirs: &[PathBuf],
    _mods_dir: &Path,
    _mods: &[String],
    _state_tx: Option<StateSender>,
) -> Result<OverlayProcess, InjectError> {
    Err(InjectError::Process(
        "Private LTK bridge module is not linked in this build".into(),
    ))
}
