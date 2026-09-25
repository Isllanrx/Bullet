use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::error::InjectError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayConfig {
    pub mods_dir: PathBuf,

    pub overlay_dir: PathBuf,

    pub game_dir: PathBuf,

    pub config_file: PathBuf,
}

pub struct OverlayManager;

impl OverlayManager {
    pub fn sanitize_mod_name(name: &str) -> Result<String, InjectError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(InjectError::Overlay("mod name cannot be empty".into()));
        }

        if trimmed.contains("..")
            || trimmed.contains('/')
            || trimmed.contains('\\')
            || trimmed.contains(':')
        {
            return Err(InjectError::Overlay(format!(
                "invalid mod name '{name}': contains path traversal or separators"
            )));
        }

        Ok(trimmed.to_string())
    }

    pub fn build_mkoverlay_args(
        config: &OverlayConfig,
        mods: &[String],
    ) -> Result<Vec<String>, InjectError> {
        if mods.is_empty() {
            return Err(InjectError::Overlay(
                "at least one mod must be specified".into(),
            ));
        }

        let mut sanitized_mods = Vec::with_capacity(mods.len());
        for m in mods {
            sanitized_mods.push(Self::sanitize_mod_name(m)?);
        }

        let mods_joined = sanitized_mods.join("/");

        let effective_game_dir = bullet_platform::paths::normalize_game_dir(&config.game_dir)
            .unwrap_or_else(|| config.game_dir.clone());

        let mut args = Vec::new();
        args.push("mkoverlay".to_string());
        args.push(config.mods_dir.to_string_lossy().to_string());
        args.push(config.overlay_dir.to_string_lossy().to_string());
        args.push(format!("--game:{}", effective_game_dir.to_string_lossy()));
        args.push(format!("--mods:{mods_joined}"));
        args.push("--noTFT".to_string());
        args.push("--ignoreConflict".to_string());

        info!(
            mods = %mods_joined,
            mods_dir = %config.mods_dir.display(),
            overlay_dir = %config.overlay_dir.display(),
            game_dir = %effective_game_dir.display(),
            "mkoverlay arguments assembled"
        );

        Ok(args)
    }

    pub fn build_runoverlay_args(config: &OverlayConfig) -> Vec<String> {
        let effective_game_dir = bullet_platform::paths::normalize_game_dir(&config.game_dir)
            .unwrap_or_else(|| config.game_dir.clone());

        debug!(
            overlay_dir = %config.overlay_dir.display(),
            config_file = %config.config_file.display(),
            game_dir = %effective_game_dir.display(),
            "runoverlay arguments assembled"
        );
        let mut args = Vec::new();
        args.push("runoverlay".to_string());
        args.push(config.overlay_dir.to_string_lossy().to_string());
        args.push(config.config_file.to_string_lossy().to_string());
        args.push(format!("--game:{}", effective_game_dir.to_string_lossy()));
        args.push("--opts:configless".to_string());
        args
    }

    pub fn prepare_overlay_dir(overlay_dir: &Path) -> Result<(), InjectError> {
        if !overlay_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(overlay_dir) {
                warn!(
                    overlay_dir = %overlay_dir.display(),
                    error = %e,
                    "Could not create the overlay directory"
                );
                return Err(InjectError::Io(e));
            }
            debug!(overlay_dir = %overlay_dir.display(), "Overlay directory created");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_mkoverlay_args_uses_forward_slash() {
        let config = OverlayConfig {
            mods_dir: PathBuf::from("C:\\Bullet\\mods"),
            overlay_dir: PathBuf::from("C:\\Bullet\\overlay"),
            game_dir: PathBuf::from("C:\\Riot Games\\League of Legends\\Game"),
            config_file: PathBuf::from("C:\\Bullet\\config.json"),
        };

        let mods = vec!["mod_alistar_base".to_string(), "mod_chroma_red".to_string()];
        let args = OverlayManager::build_mkoverlay_args(&config, &mods).expect("build args");

        assert_eq!(args[0], "mkoverlay");
        assert_eq!(args[1], "C:\\Bullet\\mods");
        assert_eq!(args[2], "C:\\Bullet\\overlay");
        assert_eq!(args[3], "--game:C:\\Riot Games\\League of Legends\\Game");

        assert_eq!(args[4], "--mods:mod_alistar_base/mod_chroma_red");
        assert!(args.contains(&"--noTFT".to_string()));
        assert!(args.contains(&"--ignoreConflict".to_string()));
    }

    #[test]
    fn test_build_runoverlay_args() {
        let config = OverlayConfig {
            mods_dir: PathBuf::from("C:\\Bullet\\mods"),
            overlay_dir: PathBuf::from("C:\\Bullet\\overlay"),
            game_dir: PathBuf::from("C:\\Riot Games\\League of Legends\\Game"),
            config_file: PathBuf::from("C:\\Bullet\\config.json"),
        };

        let args = OverlayManager::build_runoverlay_args(&config);
        assert_eq!(args[0], "runoverlay");
        assert_eq!(args[1], "C:\\Bullet\\overlay");
        assert_eq!(args[2], "C:\\Bullet\\config.json");
        assert_eq!(args[3], "--game:C:\\Riot Games\\League of Legends\\Game");
        assert_eq!(args[4], "--opts:configless");
    }

    #[test]
    fn test_sanitize_mod_name_rejects_traversal() {
        assert!(OverlayManager::sanitize_mod_name("../evil").is_err());
        assert!(OverlayManager::sanitize_mod_name("evil/mod").is_err());
        assert!(OverlayManager::sanitize_mod_name("evil\\mod").is_err());
        assert!(OverlayManager::sanitize_mod_name("").is_err());
        assert_eq!(
            OverlayManager::sanitize_mod_name(" valid_mod_123 ").unwrap(),
            "valid_mod_123"
        );
    }
}
