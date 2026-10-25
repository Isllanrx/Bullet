use std::fmt;
use std::path::{Path, PathBuf};

use crate::ltk_release::{
    INJECTOR_FILES, ReleaseInspectionReceipt, download_injector, download_injector_with_receipt,
};

pub mod transaction;

pub use transaction::{
    InstallTransaction, RecoveryOutcome, TransactionStage, recover_pending_transaction,
};

pub const INSTALL_FLAG: &str = "--install-injector";

#[derive(Debug, PartialEq, Eq)]
pub enum InstallError {
    Denied,
    NotTrusted(&'static str, String),
    Failed(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Denied => write!(f, "Windows denied writing to the tools folder"),
            Self::NotTrusted(name, why) => write!(f, "{name} is not trusted: {why}"),
            Self::Failed(why) => write!(f, "{why}"),
        }
    }
}

pub const EXIT_DENIED: i32 = 4;
pub const EXIT_NOT_TRUSTED: i32 = 5;
pub const EXIT_FAILED: i32 = 6;

impl InstallError {
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Denied => EXIT_DENIED,
            Self::NotTrusted(..) => EXIT_NOT_TRUSTED,
            Self::Failed(_) => EXIT_FAILED,
        }
    }
}

#[must_use]
pub fn exit_reason(code: i32) -> &'static str {
    match code {
        2 => "the elevated copy received the wrong arguments or no release version",
        3 => "the target is not Bullet's own tools folder",
        EXIT_DENIED => "Windows denied writing to the tools folder even when elevated",
        EXIT_NOT_TRUSTED => "a staged injector file carries a foreign or broken signature",
        EXIT_FAILED => "reading, writing or replacing the injector files failed",
        _ => "unknown",
    }
}

fn io_error(context: &str, e: &std::io::Error) -> InstallError {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        InstallError::Denied
    } else {
        InstallError::Failed(format!("{context}: {e}"))
    }
}

fn verify(name: &'static str, path: &Path) -> Result<(), InstallError> {
    bullet_inject::trust::verify_injector_file(path)
        .map(|_| ())
        .map_err(|e| InstallError::NotTrusted(name, e.to_string()))
}

pub fn create_staging_dir(state_dir: &Path) -> Result<PathBuf, InstallError> {
    let unique_name = format!(
        "injector_staging_{}_{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    let dir = state_dir.join(unique_name);
    std::fs::create_dir_all(&dir).map_err(|e| io_error("creating the staging folder", &e))?;
    Ok(dir)
}

pub async fn stage(version: &str, state_dir: &Path) -> Result<PathBuf, InstallError> {
    let client = crate::ltk_release::http_client().map_err(InstallError::Failed)?;
    let dir = create_staging_dir(state_dir)?;
    let receipt = crate::ltk_release::inspect_release_receipt(&client, version, &dir)
        .await
        .map_err(InstallError::Failed)?;
    receipt
        .validate_structural()
        .map_err(InstallError::Failed)?;

    let receipt_path = dir.join(crate::ltk_release::receipt::RECEIPT_FILE);
    receipt
        .save_to_file(&receipt_path)
        .map_err(InstallError::Failed)?;

    for name in INJECTOR_FILES {
        let path = dir.join(name);
        let bytes = std::fs::read(&path).map_err(|e| io_error("reading staged file", &e))?;
        receipt
            .verify_bytes(name, &bytes)
            .map_err(InstallError::Failed)?;
        verify(name, &path)?;
    }
    Ok(dir)
}

pub async fn stage_with_receipt(
    receipt: &ReleaseInspectionReceipt,
    state_dir: &Path,
) -> Result<PathBuf, InstallError> {
    receipt
        .validate_structural()
        .map_err(InstallError::Failed)?;
    let files = download_injector_with_receipt(receipt)
        .await
        .map_err(InstallError::Failed)?;
    let dir = create_staging_dir(state_dir)?;

    let receipt_path = dir.join(crate::ltk_release::receipt::RECEIPT_FILE);
    receipt
        .save_to_file(&receipt_path)
        .map_err(InstallError::Failed)?;

    for (name, bytes) in &files {
        let path = dir.join(name);
        std::fs::write(&path, bytes).map_err(|e| io_error("staging a download", &e))?;
        receipt
            .verify_bytes(name, bytes)
            .map_err(InstallError::Failed)?;
        verify(name, &path)?;
    }
    Ok(dir)
}

pub fn install(staging: &Path, tools: &Path) -> Result<(), InstallError> {
    let receipt_path = staging.join(crate::ltk_release::receipt::RECEIPT_FILE);
    let receipt = if receipt_path.is_file() {
        let r = ReleaseInspectionReceipt::load_from_file(&receipt_path)
            .map_err(InstallError::Failed)?;
        r.validate_structural().map_err(InstallError::Failed)?;
        Some(r)
    } else {
        None
    };

    let mut files = Vec::with_capacity(INJECTOR_FILES.len());
    for name in INJECTOR_FILES {
        let bytes =
            std::fs::read(staging.join(name)).map_err(|e| io_error("reading a staged file", &e))?;
        if let Some(ref r) = receipt {
            r.verify_bytes(name, &bytes).map_err(InstallError::Failed)?;
        }
        files.push((name, bytes));
    }
    install_files_internal(files, tools, receipt.as_ref())
}

pub async fn install_release(version: &str, tools: &Path) -> Result<(), InstallError> {
    let files = download_injector(version)
        .await
        .map_err(InstallError::Failed)?;
    install_files(files, tools)
}

pub async fn install_release_with_receipt(
    receipt: &ReleaseInspectionReceipt,
    tools: &Path,
) -> Result<(), InstallError> {
    receipt
        .validate_structural()
        .map_err(InstallError::Failed)?;
    let files = download_injector_with_receipt(receipt)
        .await
        .map_err(InstallError::Failed)?;
    install_files_internal(files, tools, Some(receipt))
}

fn install_files(files: Vec<(&'static str, Vec<u8>)>, tools: &Path) -> Result<(), InstallError> {
    install_files_internal(files, tools, None)
}

fn install_files_internal(
    files: Vec<(&'static str, Vec<u8>)>,
    tools: &Path,
    receipt: Option<&ReleaseInspectionReceipt>,
) -> Result<(), InstallError> {
    std::fs::create_dir_all(tools).map_err(|e| io_error("creating the tools folder", &e))?;

    let tx = InstallTransaction::new(TransactionStage::Staged, receipt.cloned());
    tx.save(tools)?;

    let mut partials = Vec::with_capacity(files.len());
    let staged = files.iter().try_for_each(|(name, bytes)| {
        if let Some(r) = receipt {
            r.verify_bytes(name, bytes).map_err(InstallError::Failed)?;
        }
        let partial = tools.join(format!("{name}.partial"));
        std::fs::write(&partial, bytes).map_err(|e| io_error("writing the tools folder", &e))?;
        partials.push((*name, partial.clone()));
        verify(name, &partial)
    });

    if let Err(e) = staged {
        let _ = transaction::cleanup_partials(tools); // ignore-ok: preserve primary staging error after best-effort cleanup
        let _ = InstallTransaction::clear(tools); // ignore-ok: preserve primary staging error after best-effort cleanup
        return Err(e);
    }

    let activating_tx = InstallTransaction::new(TransactionStage::Activating, receipt.cloned());
    activating_tx.save(tools)?;

    let placed = replace_all(tools, &partials);
    if let Err(e) = placed {
        if transaction::rollback_to_previous(tools).is_ok()
            && transaction::cleanup_partials(tools).is_ok()
        {
            let _ = InstallTransaction::clear(tools); // ignore-ok: clear journal only when rollback and partial cleanup succeeded
        }
        return Err(e);
    }

    if let Some(r) = receipt {
        for (name, _) in &files {
            let placed_path = tools.join(name);
            let placed_bytes = std::fs::read(&placed_path)
                .map_err(|e| io_error("verifying installed tool", &e))?;
            if let Err(v_err) = r.verify_bytes(name, &placed_bytes) {
                if transaction::rollback_to_previous(tools).is_ok()
                    && transaction::cleanup_partials(tools).is_ok()
                {
                    let _ = InstallTransaction::clear(tools); // ignore-ok: clear journal only when rollback and partial cleanup succeeded
                }
                return Err(InstallError::Failed(v_err));
            }
        }
        let receipt_dest = tools.join(crate::ltk_release::receipt::RECEIPT_FILE);
        r.save_to_file(&receipt_dest)
            .map_err(InstallError::Failed)?;
    }

    let committed_tx = InstallTransaction::new(TransactionStage::Committed, receipt.cloned());
    committed_tx.save(tools)?;

    transaction::cleanup_previous_backups(tools)?;
    InstallTransaction::clear(tools)?;

    Ok(())
}

fn replace_all(tools: &Path, partials: &[(&str, PathBuf)]) -> Result<(), InstallError> {
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::with_capacity(partials.len());
    for (name, partial) in partials {
        let target = tools.join(name);
        let previous = target
            .is_file()
            .then(|| tools.join(format!("{name}.previous")));
        let step = match &previous {
            Some(previous) => std::fs::rename(&target, previous),
            None => Ok(()),
        }
        .and_then(|()| std::fs::rename(partial, &target));
        match step {
            Ok(()) => done.push((target, previous)),
            Err(e) => {
                done.push((target, previous));
                let failed = io_error("replacing a tool", &e);
                return Err(match roll_back(&done) {
                    Some(why) => InstallError::Failed(format!(
                        "{failed}; the previous files could not be restored: {why}"
                    )),
                    None => failed,
                });
            }
        }
    }
    Ok(())
}

fn roll_back(done: &[(PathBuf, Option<PathBuf>)]) -> Option<String> {
    let mut first_error = None;
    for (target, previous) in done.iter().rev() {
        let restored = match previous {
            Some(previous) if previous.is_file() => {
                let _ = std::fs::remove_dir_all(target); // ignore-ok: target may be dir or file
                std::fs::rename(previous, target)
            }
            Some(_) => Ok(()),
            None => std::fs::remove_file(target).or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            }),
        };
        if let Err(e) = restored {
            first_error.get_or_insert_with(|| format!("{}: {e}", target.display()));
        }
    }
    first_error
}

#[must_use]
pub fn elevated_parameters(version: &str, tools: &Path) -> String {
    format!("{INSTALL_FLAG} \"{version}\" \"{}\"", tools.display())
}

#[must_use]
pub fn is_bullet_tools_folder(tools: &Path, exe: Option<&Path>) -> bool {
    exe.and_then(Path::parent)
        .is_some_and(|dir| dir.join("tools") == tools)
}

#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod tests;
