use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::ltk_release::receipt::{
    ArtifactReceipt, ProvenanceStatus, RECEIPT_FILE, ReleaseInspectionReceipt,
};

struct TempFixture(PathBuf);

impl TempFixture {
    fn new(label: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("epoch")
            .as_nanos();
        let p = std::env::temp_dir().join(format!(
            "bullet_recovery_{label}_{}_{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&p).expect("create fixture dir");
        Self(p)
    }
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0); // ignore-ok: best-effort test fixture cleanup
    }
}

fn sample_receipt() -> ReleaseInspectionReceipt {
    let artifacts = INJECTOR_FILES
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let bytes = format!("NEW{i}").into_bytes();
            ArtifactReceipt {
                name: (*name).to_string(),
                size_bytes: bytes.len(),
                sha256: bullet_inject::dll_validator::compute_sha256(&bytes),
                provenance: ProvenanceStatus::UnsignedOfficial,
            }
        })
        .collect();
    ReleaseInspectionReceipt::new("1.30.0".into(), "a".repeat(40), artifacts)
}

fn seed_tools(p: &Path) {
    for (i, name) in INJECTOR_FILES.iter().enumerate() {
        fs::write(p.join(name), format!("OLD{i}")).expect("write old");
        fs::write(p.join(format!("{name}.partial")), format!("NEW{i}")).expect("write partial");
    }
}

fn activate_moves(p: &Path, moves: usize) {
    for k in 0..moves {
        let name = INJECTOR_FILES[k / 2];
        let (from, to) = if k % 2 == 0 {
            (p.join(name), p.join(format!("{name}.previous")))
        } else {
            (p.join(format!("{name}.partial")), p.join(name))
        };
        fs::rename(from, to).expect("rename activation step");
    }
}

#[test]
fn updater_crash_each_activation_checkpoint_and_idempotency() {
    for checkpoint in 0..=8 {
        let f = TempFixture::new("checkpoint");
        seed_tools(&f.0);
        InstallTransaction::new(TransactionStage::Staged, Some(sample_receipt()))
            .save(&f.0)
            .expect("save staged");

        if checkpoint > 0 {
            InstallTransaction::new(TransactionStage::Activating, Some(sample_receipt()))
                .save(&f.0)
                .expect("save activating");
            activate_moves(&f.0, (checkpoint - 1).min(4));
        }

        if checkpoint >= 6 {
            InstallTransaction::new(TransactionStage::Committed, Some(sample_receipt()))
                .save(&f.0)
                .expect("save committed");
            for name in INJECTOR_FILES.iter().take(checkpoint - 6) {
                fs::remove_file(f.0.join(format!("{name}.previous"))).expect("remove previous");
            }
        }

        let first = recover_pending_transaction(&f.0).expect("first recovery must succeed");
        let expected_prefix = if checkpoint >= 5 { "NEW" } else { "OLD" };
        if checkpoint >= 5 {
            assert_eq!(first, RecoveryOutcome::CommittedFinalized);
            assert!(f.0.join(RECEIPT_FILE).is_file());
        } else {
            assert_eq!(first, RecoveryOutcome::RolledBackToPrevious);
        }

        let pair: Vec<String> = INJECTOR_FILES
            .iter()
            .map(|n| fs::read_to_string(f.0.join(n)).expect("read binary"))
            .collect();
        assert_eq!(
            pair,
            vec![format!("{expected_prefix}0"), format!("{expected_prefix}1")]
        );

        let second =
            recover_pending_transaction(&f.0).expect("idempotent second recovery must succeed");
        assert_eq!(second, RecoveryOutcome::NoTransaction);
    }
}

#[test]
#[cfg(windows)]
fn updater_failed_rollback_must_not_report_success_and_recovers_once_unlocked() {
    use std::os::windows::fs::OpenOptionsExt;

    let f = TempFixture::new("rollback_lock");
    seed_tools(&f.0);
    activate_moves(&f.0, 4);
    InstallTransaction::new(TransactionStage::Activating, None)
        .save(&f.0)
        .expect("save activating");

    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(f.0.join(INJECTOR_FILES[0]))
        .expect("open exclusive lock");

    let blocked = recover_pending_transaction(&f.0);
    assert!(
        blocked.is_err(),
        "rollback failure must not be reported as success"
    );
    assert!(
        f.0.join(transaction::TRANSACTION_FILE).is_file(),
        "transaction journal must be preserved when rollback fails"
    );

    drop(lock);

    let retried =
        recover_pending_transaction(&f.0).expect("retry after releasing lock must succeed");
    assert_eq!(retried, RecoveryOutcome::RolledBackToPrevious);
    let pair: Vec<String> = INJECTOR_FILES
        .iter()
        .map(|n| fs::read_to_string(f.0.join(n)).expect("read restored binary"))
        .collect();
    assert_eq!(pair, vec!["OLD0".to_string(), "OLD1".to_string()]);
}

#[test]
fn updater_committed_missing_or_corrupt_artifact_preserves_recovery_backups() {
    let f = TempFixture::new("committed_invalid");
    seed_tools(&f.0);
    activate_moves(&f.0, 4);
    fs::write(f.0.join(INJECTOR_FILES[1]), b"CORRUPT").expect("corrupt dll");
    InstallTransaction::new(TransactionStage::Committed, Some(sample_receipt()))
        .save(&f.0)
        .expect("save committed");

    let result = recover_pending_transaction(&f.0);
    let backups_preserved = INJECTOR_FILES
        .iter()
        .all(|n| f.0.join(format!("{n}.previous")).is_file());
    let journal_preserved = f.0.join(transaction::TRANSACTION_FILE).is_file();

    assert!(result.is_err(), "corrupt committed state must return error");
    assert!(
        backups_preserved,
        "previous backups must not be erased when committed artifacts fail verification"
    );
    assert!(
        journal_preserved,
        "transaction journal must remain on disk when committed verification fails"
    );
}

#[test]
fn updater_receipt_write_failure_must_preserve_journal_and_backups() {
    let f = TempFixture::new("receipt_failure");
    seed_tools(&f.0);
    activate_moves(&f.0, 4);
    fs::create_dir(f.0.join(RECEIPT_FILE)).expect("create conflicting directory at receipt.json");
    InstallTransaction::new(TransactionStage::Activating, Some(sample_receipt()))
        .save(&f.0)
        .expect("save activating");

    let result = recover_pending_transaction(&f.0);
    assert!(
        result.is_err(),
        "failure to write receipt.json must fail recovery"
    );
    assert!(
        f.0.join(transaction::TRANSACTION_FILE).is_file(),
        "transaction journal must be preserved when receipt write fails"
    );
    assert!(
        INJECTOR_FILES
            .iter()
            .all(|n| f.0.join(format!("{n}.previous")).is_file()),
        "previous backups must be preserved when receipt write fails"
    );
}

#[test]
fn updater_corrupt_journal_and_partial_pair_are_rejected() {
    let f = TempFixture::new("corrupt_journal");
    fs::write(f.0.join(INJECTOR_FILES[0]), b"NEW0").expect("write host");
    fs::write(f.0.join(format!("{}.partial", INJECTOR_FILES[1])), b"NEW1")
        .expect("write partial dll");
    fs::write(f.0.join(transaction::TRANSACTION_FILE), b"{").expect("write corrupt journal");

    let result = recover_pending_transaction(&f.0);
    assert!(
        result.is_err(),
        "corrupt journal must be rejected explicitly"
    );

    fs::remove_file(f.0.join(transaction::TRANSACTION_FILE)).expect("remove journal");
    let orphan_partial_pair = recover_pending_transaction(&f.0);
    assert!(
        orphan_partial_pair.is_err(),
        "incomplete binary pair without backup must be rejected even without journal"
    );
}
