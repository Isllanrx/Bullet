use std::path::Path;
use tracing::{info, warn};

use crate::ltk_release::{INJECTOR_FILES, ReleaseInspectionReceipt};
use crate::updates::injector_install::{InstallError, io_error};

pub const TRANSACTION_FILE: &str = "install_transaction.json";

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum TransactionStage {
    Staged,
    Activating,
    Committed,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct InstallTransaction {
    pub stage: TransactionStage,
    pub receipt: Option<ReleaseInspectionReceipt>,
    pub timestamp_epoch_secs: u64,
}

impl InstallTransaction {
    #[must_use]
    pub fn new(stage: TransactionStage, receipt: Option<ReleaseInspectionReceipt>) -> Self {
        let timestamp_epoch_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            stage,
            receipt,
            timestamp_epoch_secs,
        }
    }

    pub fn save(&self, tools: &Path) -> Result<(), InstallError> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| InstallError::Failed(format!("serializing transaction: {e}")))?;
        let path = tools.join(TRANSACTION_FILE);
        let tmp_path = tools.join(format!("{TRANSACTION_FILE}.tmp"));
        std::fs::write(&tmp_path, json).map_err(|e| io_error("writing transaction file", &e))?;
        std::fs::rename(&tmp_path, &path)
            .map_err(|e| io_error("activating transaction file", &e))?;
        Ok(())
    }

    pub fn load(tools: &Path) -> Result<Option<Self>, InstallError> {
        let path = tools.join(TRANSACTION_FILE);
        let tmp_path = tools.join(format!("{TRANSACTION_FILE}.tmp"));
        let candidate = if path.exists() {
            Some(path)
        } else if tmp_path.exists() {
            Some(tmp_path)
        } else {
            None
        };

        let Some(journal_path) = candidate else {
            return Ok(None);
        };

        if !journal_path.is_file() {
            return Err(InstallError::Failed(format!(
                "transaction journal '{}' is not a regular file",
                journal_path.display()
            )));
        }

        let bytes = std::fs::read(&journal_path)
            .map_err(|e| io_error("reading transaction journal", &e))?;
        let tx: Self = serde_json::from_slice(&bytes)
            .map_err(|e| InstallError::Failed(format!("corrupt transaction journal: {e}")))?;

        if let Some(ref r) = tx.receipt {
            r.validate_structural().map_err(InstallError::Failed)?;
        }

        Ok(Some(tx))
    }

    pub fn clear(tools: &Path) -> Result<(), InstallError> {
        for file_name in [
            TRANSACTION_FILE.to_string(),
            format!("{TRANSACTION_FILE}.tmp"),
        ] {
            let p = tools.join(file_name);
            match std::fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(io_error("clearing transaction journal", &e)),
            }
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryOutcome {
    NoTransaction,
    CommittedFinalized,
    RolledBackToPrevious,
    CleanedOrphans,
}

fn verify_active_files_against_receipt(
    tools: &Path,
    receipt: &ReleaseInspectionReceipt,
) -> Result<(), InstallError> {
    receipt
        .validate_structural()
        .map_err(InstallError::Failed)?;
    for name in INJECTOR_FILES {
        let path = tools.join(name);
        let bytes =
            std::fs::read(&path).map_err(|e| io_error("reading active injector binary", &e))?;
        receipt
            .verify_bytes(name, &bytes)
            .map_err(InstallError::Failed)?;
    }
    Ok(())
}

fn load_disk_receipt(tools: &Path) -> Result<Option<ReleaseInspectionReceipt>, InstallError> {
    let receipt_path = tools.join(crate::ltk_release::receipt::RECEIPT_FILE);
    if !receipt_path.exists() {
        return Ok(None);
    }
    if !receipt_path.is_file() {
        return Err(InstallError::Failed(format!(
            "receipt path '{}' is not a regular file",
            receipt_path.display()
        )));
    }
    let receipt =
        ReleaseInspectionReceipt::load_from_file(&receipt_path).map_err(InstallError::Failed)?;
    receipt
        .validate_structural()
        .map_err(InstallError::Failed)?;
    Ok(Some(receipt))
}

fn verify_installed_pair_state(tools: &Path) -> Result<(), InstallError> {
    let mut present_count = 0usize;
    for name in INJECTOR_FILES {
        let p = tools.join(name);
        if p.exists() {
            if !p.is_file() {
                return Err(InstallError::Failed(format!(
                    "installed injector path '{}' is not a regular file",
                    p.display()
                )));
            }
            present_count += 1;
        }
    }

    if present_count != 0 && present_count != INJECTOR_FILES.len() {
        return Err(InstallError::Failed(
            "incomplete injector binary pair in tools directory".into(),
        ));
    }

    if let Some(disk_receipt) = load_disk_receipt(tools)? {
        if present_count == 0 {
            return Err(InstallError::Failed(
                "receipt.json exists without installed injector binaries".into(),
            ));
        }
        verify_active_files_against_receipt(tools, &disk_receipt)?;
    }

    Ok(())
}

fn commit_verified_transaction(
    tools: &Path,
    receipt: Option<&ReleaseInspectionReceipt>,
) -> Result<RecoveryOutcome, InstallError> {
    if let Some(r) = receipt {
        verify_active_files_against_receipt(tools, r)?;
        let receipt_dest = tools.join(crate::ltk_release::receipt::RECEIPT_FILE);
        r.save_to_file(&receipt_dest)
            .map_err(InstallError::Failed)?;
    } else {
        let all_present = INJECTOR_FILES.iter().all(|n| tools.join(n).is_file());
        if !all_present {
            return Err(InstallError::Failed(
                "cannot commit transaction with missing active injector binaries".into(),
            ));
        }
        verify_installed_pair_state(tools)?;
    }

    cleanup_previous_backups(tools)?;
    cleanup_partials(tools)?;
    InstallTransaction::clear(tools)?;
    Ok(RecoveryOutcome::CommittedFinalized)
}

pub fn recover_pending_transaction(tools: &Path) -> Result<RecoveryOutcome, InstallError> {
    if !tools.is_dir() {
        return Ok(RecoveryOutcome::NoTransaction);
    }

    if let Some(tx) = InstallTransaction::load(tools)? {
        info!(stage = ?tx.stage, "Found pending injector installation transaction; recovering");
        match tx.stage {
            TransactionStage::Committed => {
                return commit_verified_transaction(tools, tx.receipt.as_ref());
            }
            TransactionStage::Staged | TransactionStage::Activating => {
                let can_commit = tx
                    .receipt
                    .as_ref()
                    .is_some_and(|r| verify_active_files_against_receipt(tools, r).is_ok());

                if can_commit {
                    info!("All active injector files match receipt; committing transaction");
                    return commit_verified_transaction(tools, tx.receipt.as_ref());
                }

                warn!(
                    "Pending transaction is inconsistent or partial; rolling back to previous state"
                );
                rollback_transaction(tools, tx.receipt.as_ref())?;
                verify_installed_pair_state(tools)?;
                cleanup_partials(tools)?;
                InstallTransaction::clear(tools)?;
                return Ok(RecoveryOutcome::RolledBackToPrevious);
            }
        }
    }

    let has_previous = INJECTOR_FILES
        .iter()
        .any(|name| tools.join(format!("{name}.previous")).exists());
    let has_partials = INJECTOR_FILES
        .iter()
        .any(|name| tools.join(format!("{name}.partial")).exists());

    if has_previous || has_partials {
        warn!(
            "Found orphaned .previous or .partial files without active transaction log; restoring previous state"
        );
        rollback_to_previous(tools)?;
        verify_installed_pair_state(tools)?;
        cleanup_partials(tools)?;
        return Ok(RecoveryOutcome::CleanedOrphans);
    }

    verify_installed_pair_state(tools)?;
    Ok(RecoveryOutcome::NoTransaction)
}

pub fn cleanup_previous_backups(tools: &Path) -> Result<(), InstallError> {
    for name in INJECTOR_FILES {
        let prev = tools.join(format!("{name}.previous"));
        match std::fs::remove_file(&prev) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error("removing previous backup", &e)),
        }
    }
    Ok(())
}

pub fn cleanup_partials(tools: &Path) -> Result<(), InstallError> {
    for name in INJECTOR_FILES {
        let partial = tools.join(format!("{name}.partial"));
        match std::fs::remove_file(&partial) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error("removing partial file", &e)),
        }
    }
    Ok(())
}

fn rollback_transaction(
    tools: &Path,
    tx_receipt: Option<&ReleaseInspectionReceipt>,
) -> Result<(), InstallError> {
    let any_previous = INJECTOR_FILES
        .iter()
        .any(|name| tools.join(format!("{name}.previous")).exists());
    let disk_receipt_exists = tools
        .join(crate::ltk_release::receipt::RECEIPT_FILE)
        .exists();

    if !any_previous
        && !disk_receipt_exists
        && let Some(r) = tx_receipt
    {
        let present: Vec<&str> = INJECTOR_FILES
            .iter()
            .copied()
            .filter(|name| tools.join(name).is_file())
            .collect();
        let all_placed_by_tx = !present.is_empty()
            && present.len() < INJECTOR_FILES.len()
            && present.iter().all(|name| {
                std::fs::read(tools.join(name))
                    .ok()
                    .is_some_and(|b| r.verify_bytes(name, &b).is_ok())
            });
        if all_placed_by_tx {
            for name in present {
                std::fs::remove_file(tools.join(name)).map_err(|e| {
                    io_error(
                        "removing partial initial install binary during rollback",
                        &e,
                    )
                })?;
            }
            return Ok(());
        }
    }

    rollback_to_previous(tools)
}

pub fn rollback_to_previous(tools: &Path) -> Result<(), InstallError> {
    for name in INJECTOR_FILES {
        let target = tools.join(name);
        let previous = tools.join(format!("{name}.previous"));
        if previous.exists() {
            if !previous.is_file() {
                return Err(InstallError::Failed(format!(
                    "backup '{}' is not a regular file",
                    previous.display()
                )));
            }
            if target.exists() {
                std::fs::remove_file(&target)
                    .map_err(|e| io_error("removing target before rollback restore", &e))?;
            }
            std::fs::rename(&previous, &target)
                .map_err(|e| io_error("restoring previous backup", &e))?;
        }
    }
    Ok(())
}
