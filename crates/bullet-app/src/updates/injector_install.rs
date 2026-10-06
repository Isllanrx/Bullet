use std::fmt;
use std::path::{Path, PathBuf};

use crate::ltk_release::{AuditedInjector, INJECTOR_FILES, download_injector};

pub const INSTALL_FLAG: &str = "--install-injector";

const STAGING_DIR: &str = "injector_staging";

#[derive(Debug, PartialEq, Eq)]
pub enum InstallError {
    Denied,
    NotAudited(&'static str),
    Failed(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied => write!(f, "Windows denied writing to the tools folder"),
            Self::NotAudited(name) => write!(f, "{name} is not the audited build"),
            Self::Failed(why) => write!(f, "{why}"),
        }
    }
}

fn io_error(context: &str, e: &std::io::Error) -> InstallError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        InstallError::Denied
    } else {
        InstallError::Failed(format!("{context}: {e}"))
    }
}

fn expected_hash(audited: AuditedInjector, name: &str) -> &'static str {
    if name == INJECTOR_FILES[0] {
        audited.host_sha256
    } else {
        audited.dll_sha256
    }
}

fn verify(audited: AuditedInjector, name: &'static str, bytes: &[u8]) -> Result<(), InstallError> {
    let actual = bullet_inject::dll_validator::compute_sha256(bytes);
    if actual.eq_ignore_ascii_case(expected_hash(audited, name)) {
        Ok(())
    } else {
        Err(InstallError::NotAudited(name))
    }
}

pub async fn stage(
    audited: AuditedInjector,
    version: &str,
    state_dir: &Path,
) -> Result<PathBuf, InstallError> {
    let files = download_injector(version)
        .await
        .map_err(InstallError::Failed)?;
    let dir = state_dir.join(STAGING_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| io_error("creating the staging folder", &e))?;
    for (name, bytes) in files {
        verify(audited, name, &bytes)?;
        std::fs::write(dir.join(name), bytes).map_err(|e| io_error("staging a download", &e))?;
    }
    Ok(dir)
}

pub fn install(audited: AuditedInjector, staging: &Path, tools: &Path) -> Result<(), InstallError> {
    let mut files = Vec::with_capacity(INJECTOR_FILES.len());
    for name in INJECTOR_FILES {
        let bytes =
            std::fs::read(staging.join(name)).map_err(|e| io_error("reading a staged file", &e))?;
        verify(audited, name, &bytes)?;
        files.push((name, bytes));
    }
    std::fs::create_dir_all(tools).map_err(|e| io_error("creating the tools folder", &e))?;
    for (name, bytes) in files {
        let partial = tools.join(format!("{name}.partial"));
        std::fs::write(&partial, &bytes).map_err(|e| io_error("writing the tools folder", &e))?;
        std::fs::rename(&partial, tools.join(name)).map_err(|e| {
            let _ = std::fs::remove_file(&partial); // ignore-ok: the rename error is what gets reported
            io_error("replacing a tool", &e)
        })?;
    }
    Ok(())
}

#[must_use]
pub fn elevated_parameters(staging: &Path, tools: &Path) -> String {
    format!(
        "{INSTALL_FLAG} \"{}\" \"{}\"",
        staging.display(),
        tools.display()
    )
}

#[must_use]
pub fn is_bullet_tools_folder(
    tools: &Path,
    install_dir: Option<&Path>,
    exe: Option<&Path>,
) -> bool {
    [install_dir, exe.and_then(Path::parent)]
        .into_iter()
        .flatten()
        .any(|dir| dir.join("tools") == tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOST: &[u8] = b"host bytes";
    const DLL: &[u8] = b"dll bytes";

    fn audited() -> AuditedInjector {
        let leak = |bytes: &[u8]| -> &'static str {
            Box::leak(bullet_inject::dll_validator::compute_sha256(bytes).into_boxed_str())
        };
        AuditedInjector {
            host_sha256: leak(HOST),
            dll_sha256: leak(DLL),
        }
    }

    fn fixture(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("bullet_injector_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture may not exist yet
        std::fs::create_dir_all(dir.join("staging")).expect("staging");
        dir
    }

    #[test]
    fn audited_files_replace_the_tools_and_leave_no_partial_behind() {
        let root = fixture("ok");
        let staging = root.join("staging");
        std::fs::write(staging.join(INJECTOR_FILES[0]), HOST).expect("host");
        std::fs::write(staging.join(INJECTOR_FILES[1]), DLL).expect("dll");
        let tools = root.join("tools");
        std::fs::create_dir_all(&tools).expect("tools");
        std::fs::write(tools.join(INJECTOR_FILES[1]), b"old dll").expect("old");

        install(audited(), &staging, &tools).expect("install");

        assert_eq!(
            std::fs::read(tools.join(INJECTOR_FILES[0])).expect("host"),
            HOST
        );
        assert_eq!(
            std::fs::read(tools.join(INJECTOR_FILES[1])).expect("dll"),
            DLL
        );
        let leftovers = std::fs::read_dir(&tools).expect("list").count();
        assert_eq!(leftovers, 2);
        let _ = std::fs::remove_dir_all(&root); // ignore-ok: fixture cleanup
    }

    #[test]
    fn a_swapped_staged_file_is_refused_before_anything_is_written() {
        let root = fixture("swapped");
        let staging = root.join("staging");
        std::fs::write(staging.join(INJECTOR_FILES[0]), HOST).expect("host");
        std::fs::write(staging.join(INJECTOR_FILES[1]), b"tampered").expect("dll");
        let tools = root.join("tools");

        assert_eq!(
            install(audited(), &staging, &tools),
            Err(InstallError::NotAudited(INJECTOR_FILES[1]))
        );
        assert!(
            !tools.exists(),
            "nothing is written when one file is refused"
        );
        let _ = std::fs::remove_dir_all(&root); // ignore-ok: fixture cleanup
    }

    #[test]
    fn only_bullets_own_tools_folders_are_accepted_as_a_target() {
        let install_dir = Path::new(r"C:\Program Files\Bullet");
        let exe = Path::new(r"D:\Portable\Bullet\bullet.exe");
        let accepts =
            |tools: &str| is_bullet_tools_folder(Path::new(tools), Some(install_dir), Some(exe));
        assert!(accepts(r"C:\Program Files\Bullet\tools"));
        assert!(accepts(r"D:\Portable\Bullet\tools"));
        assert!(!accepts(r"C:\Windows\System32"));
        assert!(!accepts(r"C:\Program Files\Bullet"));
    }

    #[test]
    fn the_elevated_command_line_quotes_both_paths() {
        assert_eq!(
            elevated_parameters(
                Path::new(r"C:\Users\A B\staging"),
                Path::new(r"C:\Program Files\Bullet\tools")
            ),
            r#"--install-injector "C:\Users\A B\staging" "C:\Program Files\Bullet\tools""#
        );
    }
}
