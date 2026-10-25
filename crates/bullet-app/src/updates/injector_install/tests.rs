use super::*;

fn fixture(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bullet_injector_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir); // ignore-ok: fixture cleanup before test
    std::fs::create_dir_all(dir.join("staging")).expect("staging");
    dir
}

#[test]
fn unverifiable_staged_files_are_refused_and_leave_nothing_in_the_tools_folder() {
    let root = fixture("unsigned");
    let staging = root.join("staging");
    std::fs::write(staging.join(INJECTOR_FILES[0]), b"host").expect("host");
    std::fs::write(staging.join(INJECTOR_FILES[1]), b"dll").expect("dll");
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools");
    std::fs::write(tools.join(INJECTOR_FILES[1]), b"old dll").expect("old");

    assert!(matches!(
        install(&staging, &tools),
        Err(InstallError::NotTrusted(name, _)) if name == INJECTOR_FILES[0]
    ));
    let left: Vec<String> = std::fs::read_dir(&tools)
        .expect("list")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        left,
        vec![INJECTOR_FILES[1].to_owned()],
        "the old file stays and no partial is left"
    );
    assert_eq!(
        std::fs::read(tools.join(INJECTOR_FILES[1])).expect("old"),
        b"old dll"
    );
    let _ = std::fs::remove_dir_all(&root); // ignore-ok: fixture cleanup
}

#[test]
fn unsigned_binaries_replace_the_tools_and_leave_no_partial_behind() {
    let root = fixture("unsigned_pe");
    let staging = root.join("staging");
    let pe = std::env::current_exe().expect("test binary");
    for name in INJECTOR_FILES {
        std::fs::copy(&pe, staging.join(name)).expect("copy");
    }
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools");
    std::fs::write(tools.join(INJECTOR_FILES[1]), b"old dll").expect("old");

    install(&staging, &tools).expect("install");
    let mut left: Vec<String> = std::fs::read_dir(&tools)
        .expect("list")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    let mut expected = INJECTOR_FILES.map(str::to_owned).to_vec();
    expected.sort();
    assert_eq!(left, expected);
    assert_ne!(
        std::fs::read(tools.join(INJECTOR_FILES[1])).expect("dll"),
        b"old dll"
    );
    let _ = std::fs::remove_dir_all(&root); // ignore-ok: fixture cleanup
}

#[test]
#[ignore = "needs the LTK injector in Bullet's tools folder"]
fn signed_files_replace_the_tools_and_leave_no_partial_behind() {
    let installed = Path::new(r"C:\Program Files\Bullet\tools");
    let root = fixture("signed");
    let staging = root.join("staging");
    for name in INJECTOR_FILES {
        std::fs::copy(installed.join(name), staging.join(name)).expect("copy");
    }
    let tools = root.join("tools");
    install(&staging, &tools).expect("install");
    assert_eq!(std::fs::read_dir(&tools).expect("list").count(), 2);
    let _ = std::fs::remove_dir_all(&root); // ignore-ok: fixture cleanup
}

#[test]
fn only_the_running_copys_own_tools_folder_is_accepted_as_a_target() {
    let exe = Path::new(r"C:\Program Files\Bullet\bullet.exe");
    let accepts = |tools: &str| is_bullet_tools_folder(Path::new(tools), Some(exe));
    assert!(accepts(r"C:\Program Files\Bullet\tools"));
    assert!(!accepts(r"D:\Portable\Bullet\tools"));
    assert!(!accepts(r"C:\Windows\System32"));
    assert!(!accepts(r"C:\Program Files\Bullet"));
    assert!(!is_bullet_tools_folder(
        Path::new(r"C:\Program Files\Bullet\tools"),
        None
    ));
}

#[test]
fn every_install_error_has_its_own_exit_code_and_reason() {
    let codes = [
        InstallError::Denied.exit_code(),
        InstallError::NotTrusted("x", String::new()).exit_code(),
        InstallError::Failed(String::new()).exit_code(),
    ];
    assert_eq!(codes, [EXIT_DENIED, EXIT_NOT_TRUSTED, EXIT_FAILED]);
    assert!(codes.iter().all(|c| exit_reason(*c) != "unknown"));
}

#[test]
fn the_elevated_command_line_carries_the_release_version_not_a_folder_to_copy_from() {
    assert_eq!(
        elevated_parameters("1.29.3", Path::new(r"C:\Program Files\Bullet\tools")),
        r#"--install-injector "1.29.3" "C:\Program Files\Bullet\tools""#
    );
}

#[test]
fn staging_with_corrupt_receipt_hash_fails_and_leaves_tools_intact() {
    use crate::ltk_release::receipt::{
        ArtifactReceipt, ProvenanceStatus, ReleaseInspectionReceipt,
    };

    let root = fixture("corrupt_receipt");
    let staging = root.join("staging");
    let pe = std::env::current_exe().expect("test binary");
    for name in INJECTOR_FILES {
        std::fs::copy(&pe, staging.join(name)).expect("copy");
    }

    let wrong_sha = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let valid_commit = "0123456789abcdef0123456789abcdef01234567";
    let host_bytes = std::fs::read(staging.join(INJECTOR_FILES[0])).expect("host bytes");
    let host_sha = bullet_inject::dll_validator::compute_sha256(&host_bytes);

    let artifacts = vec![
        ArtifactReceipt {
            name: INJECTOR_FILES[0].to_string(),
            size_bytes: host_bytes.len(),
            sha256: host_sha,
            provenance: ProvenanceStatus::UnsignedOfficial,
        },
        ArtifactReceipt {
            name: INJECTOR_FILES[1].to_string(),
            size_bytes: 100,
            sha256: wrong_sha.to_string(),
            provenance: ProvenanceStatus::UnsignedOfficial,
        },
    ];

    let receipt =
        ReleaseInspectionReceipt::new("1.30.0".to_string(), valid_commit.to_string(), artifacts);
    receipt
        .save_to_file(&staging.join(crate::ltk_release::receipt::RECEIPT_FILE))
        .expect("save receipt");

    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools");
    std::fs::write(tools.join(INJECTOR_FILES[1]), b"sentinel tool bytes").expect("sentinel");

    let result = install(&staging, &tools);
    assert!(
        result.is_err(),
        "Install must fail due to corrupted receipt artifact digest"
    );

    assert_eq!(
        std::fs::read(tools.join(INJECTOR_FILES[1])).expect("read tool"),
        b"sentinel tool bytes"
    );
    let _ = std::fs::remove_dir_all(&root); // ignore-ok: cleanup test fixture
}

#[test]
fn staging_with_valid_receipt_installs_receipt_and_binaries() {
    use crate::ltk_release::receipt::{
        ArtifactReceipt, ProvenanceStatus, ReleaseInspectionReceipt,
    };

    let root = fixture("valid_receipt_install");
    let staging = root.join("staging");
    let pe = std::env::current_exe().expect("test binary");
    for name in INJECTOR_FILES {
        std::fs::copy(&pe, staging.join(name)).expect("copy");
    }

    let host_bytes = std::fs::read(staging.join(INJECTOR_FILES[0])).expect("host bytes");
    let host_sha = bullet_inject::dll_validator::compute_sha256(&host_bytes);
    let dll_bytes = std::fs::read(staging.join(INJECTOR_FILES[1])).expect("dll bytes");
    let dll_sha = bullet_inject::dll_validator::compute_sha256(&dll_bytes);
    let valid_commit = "0123456789abcdef0123456789abcdef01234567";

    let artifacts = vec![
        ArtifactReceipt {
            name: INJECTOR_FILES[0].to_string(),
            size_bytes: host_bytes.len(),
            sha256: host_sha,
            provenance: ProvenanceStatus::UnsignedOfficial,
        },
        ArtifactReceipt {
            name: INJECTOR_FILES[1].to_string(),
            size_bytes: dll_bytes.len(),
            sha256: dll_sha,
            provenance: ProvenanceStatus::UnsignedOfficial,
        },
    ];

    let receipt =
        ReleaseInspectionReceipt::new("1.30.0".to_string(), valid_commit.to_string(), artifacts);
    receipt
        .save_to_file(&staging.join(crate::ltk_release::receipt::RECEIPT_FILE))
        .expect("save receipt");

    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools");

    install(&staging, &tools).expect("install should succeed");

    let installed_receipt = ReleaseInspectionReceipt::load_from_file(
        &tools.join(crate::ltk_release::receipt::RECEIPT_FILE),
    )
    .expect("load tools receipt");
    assert_eq!(installed_receipt.version, "1.30.0");
    assert_eq!(installed_receipt.commit_sha, valid_commit);

    let _ = std::fs::remove_dir_all(&root); // ignore-ok: cleanup test fixture
}

#[test]
fn test_transactional_recovery_rolls_back_interrupted_activating_state() {
    let root = fixture("recovery_rollback");
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools");

    let old_host = b"ORIGINAL_HOST_V1";
    let old_dll = b"ORIGINAL_DLL_V1";
    std::fs::write(tools.join(INJECTOR_FILES[0]), old_host).expect("write old host");
    std::fs::write(tools.join(INJECTOR_FILES[1]), old_dll).expect("write old dll");

    std::fs::rename(
        tools.join(INJECTOR_FILES[0]),
        tools.join(format!("{}.previous", INJECTOR_FILES[0])),
    )
    .expect("rename host to previous");
    std::fs::write(tools.join(INJECTOR_FILES[0]), b"NEW_INCOMPLETE_HOST").expect("write new host");

    let tx = InstallTransaction::new(TransactionStage::Activating, None);
    tx.save(&tools).expect("save activating transaction");

    let outcome = recover_pending_transaction(&tools).expect("recovery should succeed");
    assert_eq!(outcome, RecoveryOutcome::RolledBackToPrevious);

    assert_eq!(
        std::fs::read(tools.join(INJECTOR_FILES[0])).expect("read host"),
        old_host,
        "Host must be rolled back to original version"
    );
    assert_eq!(
        std::fs::read(tools.join(INJECTOR_FILES[1])).expect("read dll"),
        old_dll,
        "DLL must remain at original version"
    );
    assert!(
        !tools.join(transaction::TRANSACTION_FILE).exists(),
        "Transaction file must be cleared after rollback"
    );

    let _ = std::fs::remove_dir_all(&root); // ignore-ok: cleanup test fixture
}

#[test]
fn test_transactional_recovery_cleans_orphans_without_transaction_log() {
    let root = fixture("orphans_cleanup");
    let tools = root.join("tools");
    std::fs::create_dir_all(&tools).expect("tools");

    let good_host = b"ORIGINAL_HOST_OK";
    let good_dll = b"ORIGINAL_DLL_OK";
    std::fs::write(
        tools.join(format!("{}.previous", INJECTOR_FILES[0])),
        good_host,
    )
    .expect("write previous host");
    std::fs::write(tools.join(INJECTOR_FILES[1]), good_dll).expect("write existing dll");
    std::fs::write(
        tools.join(format!("{}.partial", INJECTOR_FILES[1])),
        b"PARTIAL_LEFTOVER",
    )
    .expect("write partial dll");

    let outcome = recover_pending_transaction(&tools).expect("recovery should succeed");
    assert_eq!(outcome, RecoveryOutcome::CleanedOrphans);

    assert_eq!(
        std::fs::read(tools.join(INJECTOR_FILES[0])).expect("read restored host"),
        good_host
    );
    assert!(
        !tools
            .join(format!("{}.partial", INJECTOR_FILES[1]))
            .exists()
    );

    let _ = std::fs::remove_dir_all(&root); // ignore-ok: cleanup test fixture
}
