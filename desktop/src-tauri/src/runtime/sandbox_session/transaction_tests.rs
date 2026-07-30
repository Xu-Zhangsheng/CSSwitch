use super::{
    advance_runtime_transaction, cleanup_tombstone_path, clear_runtime_transaction,
    finalize_registered_authority_cleanup, gateway_model_catalog_timeout_ms,
    interrupted_science_environment_runtime_id, parse_pending_cleanup_manifest,
    prevalidate_one_click_system_ssh, retry_pending_authority_cleanup,
    runtime_transaction_requires_snapshot_preservation, science_health_control_error,
    test_arm_authority_cleanup_parent_sync_failure, test_arm_authority_snapshot_capture_failure,
    test_arm_authority_snapshot_cleanup_fault, test_arm_authority_snapshot_clone_errno,
    test_arm_authority_snapshot_completion_sync_failure,
    test_arm_authority_snapshot_directory_barrier,
    test_arm_authority_snapshot_fallback_create_failure,
    test_arm_authority_snapshot_parent_barrier, validate_interrupted_science_transaction_entry,
    verify_gateway_model_catalog, AuthorityCopyBudget, AuthoritySnapshotCategory,
    AuthoritySnapshotScope, AuthorityTreeSnapshot, OneClickAuthoritySnapshot, PendingCleanupEntry,
    RegisteredAuthorityCleanup, AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX,
    MAX_AUTHORITY_FULL_COPY_FILE_BYTES, MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES,
    MAX_AUTHORITY_SNAPSHOT_ENTRIES, MAX_AUTHORITY_SNAPSHOT_FILE_BYTES,
    MAX_AUTHORITY_SNAPSHOT_TOTAL_BYTES, PENDING_CLEANUP_MARKER_FILE,
    SCIENCE_ENVIRONMENT_PENDING_STAGE_PREFIX, SCIENCE_OWNED_OPAQUE_ROOTS,
};
use crate::config::{self, Config, RuntimeBindingCommit};
use crate::provider_contracts::ModelPolicy;
use crate::runtime::proxy::ProxyAction;
use crate::{AppState, SharedAppState};
use csswitch_skill_install_core::AttachError;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

struct ScopedEnv {
    saved: Vec<(String, Option<std::ffi::OsString>)>,
}

impl ScopedEnv {
    fn new() -> Self {
        Self { saved: Vec::new() }
    }

    fn set(&mut self, key: &str, value: impl AsRef<std::ffi::OsStr>) {
        self.saved.push((key.to_string(), std::env::var_os(key)));
        std::env::set_var(key, value);
    }

    fn remove(&mut self, key: &str) {
        self.saved.push((key.to_string(), std::env::var_os(key)));
        std::env::remove_var(key);
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        for (key, value) in self.saved.iter().rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

#[test]
fn health_bootstrap_retries_only_explicit_transport_failures() {
    let transient = science_health_control_error(AttachError {
        code: "SCIENCE_HEALTH_UNREACHABLE".into(),
        message: "safe transport failure".into(),
        retryable: true,
        uncertain: false,
    });
    assert_eq!(transient, "science_api_health_unreachable");

    for code in [
        "SCIENCE_RUNTIME_CHANGED",
        "SCIENCE_CONTROL_FAILED",
        "SCIENCE_NOT_READY",
    ] {
        let hard_failure = science_health_control_error(AttachError {
            code: code.into(),
            message: "safe hard failure".into(),
            retryable: true,
            uncertain: false,
        });
        assert!(
            hard_failure.starts_with("science_api_health_control_failed"),
            "{code} must never enter the transient bootstrap retry lane"
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TreeEntry {
    kind: &'static str,
    mode: u32,
    bytes: Vec<u8>,
}

fn isolated_tmpdir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "csswitch-sandbox-transaction-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

fn tree(root: &Path) -> BTreeMap<PathBuf, TreeEntry> {
    fn walk(root: &Path, current: &Path, entries: &mut BTreeMap<PathBuf, TreeEntry>) {
        let metadata = match fs::symlink_metadata(current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("cannot inspect {}: {error}", current.display()),
        };
        let relative = current.strip_prefix(root).unwrap().to_path_buf();
        if metadata.file_type().is_symlink() {
            entries.insert(
                relative,
                TreeEntry {
                    kind: "symlink",
                    mode: metadata.permissions().mode() & 0o777,
                    bytes: fs::read_link(current)
                        .unwrap()
                        .as_os_str()
                        .as_encoded_bytes()
                        .to_vec(),
                },
            );
        } else if metadata.is_file() {
            entries.insert(
                relative,
                TreeEntry {
                    kind: "file",
                    mode: metadata.permissions().mode() & 0o777,
                    bytes: fs::read(current).unwrap(),
                },
            );
        } else {
            assert!(metadata.is_dir(), "fixture contains a special file");
            entries.insert(
                relative,
                TreeEntry {
                    kind: "dir",
                    mode: metadata.permissions().mode() & 0o777,
                    bytes: Vec::new(),
                },
            );
            let mut children = fs::read_dir(current)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            children.sort();
            for child in children {
                walk(root, &child, entries);
            }
        }
    }

    let mut entries = BTreeMap::new();
    walk(root, root, &mut entries);
    entries
}

#[test]
fn authority_snapshot_uses_independent_inodes_and_restores_in_place_mutation() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut science_0125_budget = AuthorityCopyBudget::default();
    AuthorityTreeSnapshot::charge_entry(
        &mut science_0125_budget,
        187_050_734,
        AuthoritySnapshotScope::ScienceData,
        AuthoritySnapshotCategory::CondaCache,
    )
    .expect("observed Science 0.1.25 files below 512 MiB must remain snapshotable");
    let mut oversized_budget = AuthorityCopyBudget::default();
    let oversized = AuthorityTreeSnapshot::charge_entry(
        &mut oversized_budget,
        MAX_AUTHORITY_SNAPSHOT_FILE_BYTES + 1,
        AuthoritySnapshotScope::ScienceData,
        AuthoritySnapshotCategory::ScienceRuntime,
    )
    .expect_err("authority files above 512 MiB must remain fail-closed");
    assert!(oversized.contains("code=authority_snapshot_file_limit"));
    assert!(oversized.contains("scope=science_data"));
    assert!(oversized.contains("category=science_runtime"));
    assert!(!oversized.contains('/'));

    let mut total_budget = AuthorityCopyBudget::default();
    for _ in 0..16 {
        AuthorityTreeSnapshot::charge_entry(
            &mut total_budget,
            MAX_AUTHORITY_SNAPSHOT_FILE_BYTES,
            AuthoritySnapshotScope::ScienceData,
            AuthoritySnapshotCategory::ScienceRuntime,
        )
        .expect("exact 8 GiB logical authority boundary must pass");
    }
    assert_eq!(total_budget.bytes, MAX_AUTHORITY_SNAPSHOT_TOTAL_BYTES);
    let total_error = AuthorityTreeSnapshot::charge_entry(
        &mut total_budget,
        1,
        AuthoritySnapshotScope::ScienceData,
        AuthoritySnapshotCategory::Other,
    )
    .expect_err("logical authority above 8 GiB must fail closed");
    assert!(total_error.contains("code=authority_snapshot_total_limit"));

    let mut entry_budget = AuthorityCopyBudget::default();
    for _ in 0..MAX_AUTHORITY_SNAPSHOT_ENTRIES {
        AuthorityTreeSnapshot::charge_entry(
            &mut entry_budget,
            0,
            AuthoritySnapshotScope::Test,
            AuthoritySnapshotCategory::Other,
        )
        .expect("exact entry boundary must pass");
    }
    let entry_error = AuthorityTreeSnapshot::charge_entry(
        &mut entry_budget,
        0,
        AuthoritySnapshotScope::Test,
        AuthoritySnapshotCategory::Other,
    )
    .expect_err("entry boundary plus one must fail closed");
    assert!(entry_error.contains("code=authority_snapshot_entry_limit"));

    let mut overflow_budget = AuthorityCopyBudget {
        bytes: u64::MAX,
        ..AuthorityCopyBudget::default()
    };
    let overflow_error = AuthorityTreeSnapshot::charge_entry(
        &mut overflow_budget,
        1,
        AuthoritySnapshotScope::Test,
        AuthoritySnapshotCategory::Other,
    )
    .expect_err("logical byte addition overflow must fail closed");
    assert!(overflow_error.contains("code=authority_snapshot_total_overflow"));

    let mut fallback_budget = AuthorityCopyBudget::default();
    for _ in 0..4 {
        AuthorityTreeSnapshot::charge_full_copy(
            &mut fallback_budget,
            MAX_AUTHORITY_FULL_COPY_FILE_BYTES,
            AuthoritySnapshotScope::Test,
            AuthoritySnapshotCategory::Other,
        )
        .expect("exact 512 MiB full-copy boundary must pass");
    }
    assert_eq!(
        fallback_budget.full_copy_bytes,
        MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES
    );
    let fallback_total_error = AuthorityTreeSnapshot::charge_full_copy(
        &mut fallback_budget,
        1,
        AuthoritySnapshotScope::Test,
        AuthoritySnapshotCategory::Other,
    )
    .expect_err("full-copy boundary plus one must require clone support");
    assert!(fallback_total_error.contains("code=authority_snapshot_clone_required"));
    let fallback_file_error = AuthorityTreeSnapshot::charge_full_copy(
        &mut AuthorityCopyBudget::default(),
        MAX_AUTHORITY_FULL_COPY_FILE_BYTES + 1,
        AuthoritySnapshotScope::Test,
        AuthoritySnapshotCategory::Other,
    )
    .expect_err("large individual full copy must require clone support");
    assert!(fallback_file_error.contains("code=authority_snapshot_clone_required"));

    let tmp = isolated_tmpdir("independent-inodes");
    let source = tmp.join("authority");
    let backup = tmp.join("rollback/authority");
    fs::create_dir_all(source.join("nested/empty")).unwrap();
    fs::create_dir(tmp.join("rollback")).unwrap();
    fs::write(source.join("database.db"), b"prior-database-bytes\n").unwrap();
    fs::write(source.join("nested/state.json"), br#"{"prior":true}"#).unwrap();
    symlink("state.json", source.join("nested/state-link")).unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(
        source.join("database.db"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::set_permissions(
        source.join("nested/state.json"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    let before = tree(&source);

    let mut snapshot = AuthorityTreeSnapshot::capture(source.clone(), backup.clone()).unwrap();
    for relative in [Path::new("database.db"), Path::new("nested/state.json")] {
        let live = fs::metadata(source.join(relative)).unwrap();
        let saved = fs::metadata(backup.join(relative)).unwrap();
        assert_eq!(
            live.dev(),
            saved.dev(),
            "transaction snapshot must stay on the same isolated filesystem"
        );
        assert_ne!(
            live.ino(),
            saved.ino(),
            "transaction snapshot must never share a mutable inode with live authority"
        );
    }
    assert_eq!(tree(&backup), before);

    let mut database = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(source.join("database.db"))
        .unwrap();
    database.write_all(b"mutated-in-place\n").unwrap();
    database.sync_all().unwrap();
    fs::set_permissions(
        source.join("database.db"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    fs::remove_file(source.join("nested/state.json")).unwrap();
    fs::remove_file(source.join("nested/state-link")).unwrap();
    symlink("new-authority", source.join("nested/state-link")).unwrap();
    fs::write(source.join("nested/new-authority"), b"must disappear\n").unwrap();
    fs::create_dir(source.join("new-directory")).unwrap();

    snapshot.restore().unwrap();
    assert_eq!(
        tree(&source),
        before,
        "restore must recover exact bytes, modes, empty directories, and object set"
    );
    let linked_target = tmp.join("linked-authority-target");
    let linked_source = tmp.join("linked-authority-root");
    fs::create_dir(&linked_target).unwrap();
    symlink(&linked_target, &linked_source).unwrap();
    let linked_error =
        AuthorityTreeSnapshot::capture(linked_source, tmp.join("rollback/linked-authority-root"))
            .err()
            .expect("authority root symlinks must remain fail-closed");
    assert!(linked_error.contains("code=authority_snapshot_root_symlink"));

    let restore_source = tmp.join("restore-authority");
    let restore_backup = tmp.join("rollback/restore-authority");
    fs::create_dir(&restore_source).unwrap();
    fs::write(restore_source.join("state.json"), b"prior\n").unwrap();
    let mut restore_snapshot =
        AuthorityTreeSnapshot::capture(restore_source, restore_backup.clone()).unwrap();
    fs::remove_dir_all(&restore_backup).unwrap();
    symlink(&linked_target, &restore_backup).unwrap();
    let restore_error = restore_snapshot
        .restore()
        .expect_err("restore backup roots that become symlinks must remain fail-closed");
    assert!(
        restore_error.contains("code=authority_restore_backup_identity_changed")
            || restore_error.contains("code=authority_restore_backup_validate_failed")
    );

    for (label, errno) in [("enotsup", libc::ENOTSUP), ("exdev", libc::EXDEV)] {
        let fallback_source = tmp.join(format!("fallback-{label}"));
        let fallback_backup = tmp.join(format!("rollback/fallback-{label}"));
        fs::create_dir(&fallback_source).unwrap();
        fs::write(fallback_source.join("state"), b"fallback-bytes\n").unwrap();
        {
            let _clone_seam = test_arm_authority_snapshot_clone_errno(errno);
            AuthorityTreeSnapshot::capture(fallback_source.clone(), fallback_backup.clone())
                .expect("ENOTSUP and EXDEV must use the bounded independent-copy fallback");
        }
        assert_eq!(
            fs::read(fallback_backup.join("state")).unwrap(),
            b"fallback-bytes\n"
        );
        let live = fs::metadata(fallback_source.join("state")).unwrap();
        let saved = fs::metadata(fallback_backup.join("state")).unwrap();
        assert!(
            live.dev() != saved.dev() || live.ino() != saved.ino(),
            "fallback must create an independent regular file"
        );
    }

    let unexpected_source = tmp.join("unexpected-clone-error");
    let unexpected_backup = tmp.join("rollback/unexpected-clone-error");
    fs::create_dir(&unexpected_source).unwrap();
    fs::write(unexpected_source.join("state"), b"unchanged\n").unwrap();
    {
        let _clone_seam = test_arm_authority_snapshot_clone_errno(libc::EIO);
        let error = AuthorityTreeSnapshot::capture(unexpected_source, unexpected_backup.clone())
            .err()
            .expect("unexpected clone errors must fail closed");
        assert!(error.contains("code=authority_snapshot_clone_failed"));
        assert!(error.contains("os_error=5"));
    }
    assert!(
        !unexpected_backup.join("state").exists(),
        "unexpected clone failure must not leave a destination file"
    );

    let injected_source = tmp.join("injected-fallback-failure");
    let injected_backup = tmp.join("rollback/injected-fallback-failure");
    fs::create_dir(&injected_source).unwrap();
    fs::write(injected_source.join("state"), b"unchanged\n").unwrap();
    {
        let _clone_seam = test_arm_authority_snapshot_clone_errno(libc::ENOTSUP);
        let _copy_seam = test_arm_authority_snapshot_fallback_create_failure();
        let error = AuthorityTreeSnapshot::capture(injected_source, injected_backup.clone())
            .err()
            .expect("fallback failure after create must fail closed");
        assert!(error.contains("code=authority_snapshot_copy_injected_failure"));
    }
    assert!(
        !injected_backup.join("state").exists(),
        "failed fallback must unlink the pinned destination entry"
    );

    let per_file_source = tmp.join("fallback-per-file-limit");
    let per_file_backup = tmp.join("rollback/fallback-per-file-limit");
    fs::create_dir(&per_file_source).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(per_file_source.join("large"))
        .unwrap()
        .set_len(MAX_AUTHORITY_FULL_COPY_FILE_BYTES + 1)
        .unwrap();
    {
        let _clone_seam = test_arm_authority_snapshot_clone_errno(libc::ENOTSUP);
        let error = AuthorityTreeSnapshot::capture(per_file_source, per_file_backup.clone())
            .err()
            .expect("fallback above the per-file copy budget must fail closed");
        assert!(error.contains("code=authority_snapshot_clone_required"));
    }
    assert!(!per_file_backup.join("large").exists());

    let aggregate_source = tmp.join("fallback-aggregate-limit");
    let aggregate_backup = tmp.join("rollback/fallback-aggregate-limit");
    fs::write(&aggregate_source, b"x").unwrap();
    let mut exhausted_budget = AuthorityCopyBudget {
        full_copy_bytes: MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES,
        ..AuthorityCopyBudget::default()
    };
    {
        let _clone_seam = test_arm_authority_snapshot_clone_errno(libc::EXDEV);
        let error = AuthorityTreeSnapshot::copy_tree(
            &aggregate_source,
            &aggregate_backup,
            &mut exhausted_budget,
            false,
            AuthoritySnapshotScope::Test,
            &aggregate_source,
        )
        .expect_err("fallback aggregate budget plus one must fail closed");
        assert!(error.contains("code=authority_snapshot_clone_required"));
    }
    assert!(!aggregate_backup.exists());

    let fallback_restore_source = tmp.join("fallback-restore");
    let fallback_restore_backup = tmp.join("rollback/fallback-restore");
    fs::create_dir(&fallback_restore_source).unwrap();
    fs::write(fallback_restore_source.join("state"), b"prior\n").unwrap();
    let mut fallback_restore_snapshot =
        AuthorityTreeSnapshot::capture(fallback_restore_source.clone(), fallback_restore_backup)
            .unwrap();
    fs::write(fallback_restore_source.join("state"), b"mutated\n").unwrap();
    {
        let _clone_seam = test_arm_authority_snapshot_clone_errno(libc::ENOTSUP);
        fallback_restore_snapshot
            .restore()
            .expect("restore must use the bounded independent-copy fallback");
    }
    assert_eq!(
        fs::read(fallback_restore_source.join("state")).unwrap(),
        b"prior\n"
    );

    let rebound_parent = tmp.join("restore-rebound-parent");
    let displaced_parent = tmp.join("restore-displaced-parent");
    let rebound_foreign = tmp.join("restore-foreign");
    let rebound_source = rebound_parent.join("authority");
    let rebound_backup = tmp.join("rollback/restore-rebound");
    let rebound_barrier = tmp.join("restore-rebound-barrier");
    fs::create_dir_all(&rebound_source).unwrap();
    fs::create_dir(&rebound_foreign).unwrap();
    fs::write(rebound_source.join("state"), b"prior-pinned\n").unwrap();
    let mut rebound_snapshot =
        AuthorityTreeSnapshot::capture(rebound_source.clone(), rebound_backup.clone()).unwrap();
    fs::write(rebound_source.join("state"), b"mutated\n").unwrap();
    let rebound_seam =
        test_arm_authority_snapshot_directory_barrier(rebound_backup, rebound_barrier.clone());
    let restore_worker = thread::spawn(move || rebound_snapshot.restore());
    for _ in 0..200 {
        if rebound_barrier.join("ready").is_file() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(rebound_barrier.join("ready").is_file());
    fs::rename(&rebound_parent, &displaced_parent).unwrap();
    symlink(&rebound_foreign, &rebound_parent).unwrap();
    fs::write(rebound_barrier.join("release"), b"release\n").unwrap();
    let restore_rebound_error = restore_worker
        .join()
        .unwrap()
        .expect_err("restore parent rebind must fail closed");
    drop(rebound_seam);
    assert!(
        restore_rebound_error.contains("code=authority_restore_parent_rebound")
            || restore_rebound_error.contains("code=authority_restore_parent_revalidate_failed")
    );
    assert!(
        !rebound_foreign.join("authority").exists(),
        "restore must not write through a rebound parent symlink"
    );
    assert_eq!(
        fs::read(displaced_parent.join("authority/state")).unwrap(),
        b"prior-pinned\n"
    );

    let backup_parent_rebind_source = tmp.join("backup-parent-rebind-live");
    let backup_parent_rebind_parent = tmp.join("backup-parent-rebind-root");
    let backup_parent_displaced = tmp.join("backup-parent-rebind-displaced");
    let backup_parent_rebind_backup = backup_parent_rebind_parent.join("authority");
    fs::create_dir(&backup_parent_rebind_source).unwrap();
    fs::create_dir(&backup_parent_rebind_parent).unwrap();
    fs::write(
        backup_parent_rebind_source.join("state"),
        b"trusted-prior\n",
    )
    .unwrap();
    let mut backup_parent_rebind_snapshot = AuthorityTreeSnapshot::capture(
        backup_parent_rebind_source.clone(),
        backup_parent_rebind_backup.clone(),
    )
    .unwrap();
    fs::write(backup_parent_rebind_source.join("state"), b"live-mutated\n").unwrap();
    fs::rename(&backup_parent_rebind_parent, &backup_parent_displaced).unwrap();
    fs::create_dir_all(&backup_parent_rebind_backup).unwrap();
    fs::write(
        backup_parent_rebind_backup.join("state"),
        b"foreign-replacement\n",
    )
    .unwrap();
    let backup_parent_rebind_error = backup_parent_rebind_snapshot
        .restore()
        .expect_err("backup parent rebind must fail closed");
    assert!(
        backup_parent_rebind_error.contains("code=authority_restore_backup_parent_rebound")
            || backup_parent_rebind_error
                .contains("code=authority_restore_backup_parent_revalidate_failed")
    );
    assert_eq!(
        fs::read(backup_parent_rebind_source.join("state")).unwrap(),
        b"live-mutated\n",
        "restore must not mutate live authority after backup parent rebind"
    );
    assert_eq!(
        fs::read(backup_parent_rebind_backup.join("state")).unwrap(),
        b"foreign-replacement\n",
        "restore must never read or mutate the replacement backup tree"
    );
    assert_eq!(
        fs::read(backup_parent_displaced.join("authority/state")).unwrap(),
        b"trusted-prior\n",
        "the pinned original backup must remain intact for diagnosis"
    );
    let _ = fs::remove_dir_all(tmp);
}

#[test]
fn authority_snapshot_accepts_observed_science_0125_tree_via_independent_clones() {
    // Installed 0.1.25 normal-HOME metadata-only observation after first-run
    // R/Conda setup: 75,588 entries, 4,386,369,604 logical bytes total, and
    // a 189,776,400-byte largest regular file. Keep the filesystem fixture sparse:
    // the regression is about bounded logical authority and independent
    // snapshot objects, not allocating GiBs in the test.
    const OBSERVED_ENTRIES: usize = 75_588;
    const OBSERVED_TOTAL_BYTES: u64 = 4_386_369_604;
    const OBSERVED_MAX_FILE_BYTES: u64 = 189_776_400;

    let mut observed_budget = AuthorityCopyBudget::default();
    let mut observed_remaining = OBSERVED_TOTAL_BYTES;
    for _ in 0..OBSERVED_ENTRIES {
        let bytes = if observed_remaining == 0 {
            0
        } else {
            OBSERVED_MAX_FILE_BYTES.min(observed_remaining)
        };
        observed_remaining -= bytes;
        AuthorityTreeSnapshot::charge_entry(
            &mut observed_budget,
            bytes,
            AuthoritySnapshotScope::ScienceData,
            AuthoritySnapshotCategory::CondaCache,
        )
        .expect("observed normal Science 0.1.25 authority budget must pass");
    }
    assert_eq!(observed_remaining, 0);
    assert_eq!(observed_budget.entries, OBSERVED_ENTRIES);
    assert_eq!(observed_budget.bytes, OBSERVED_TOTAL_BYTES);

    let tmp = isolated_tmpdir("science-0125-observed-authority");
    let source = tmp.join("authority");
    let backup = tmp.join("rollback/authority");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir(tmp.join("rollback")).unwrap();

    let mut remaining = OBSERVED_TOTAL_BYTES;
    let mut index = 0usize;
    while remaining > 0 {
        let size = remaining.min(OBSERVED_MAX_FILE_BYTES);
        let path = source.join(format!("payload-{index}"));
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.set_len(size).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        remaining -= size;
        index += 1;
    }

    let mut snapshot = AuthorityTreeSnapshot::capture(source.clone(), backup.clone())
        .expect("observed normal Science 0.1.25 authority must be snapshotable");
    for entry in fs::read_dir(&source).unwrap() {
        let entry = entry.unwrap();
        let live = fs::metadata(entry.path()).unwrap();
        let saved = fs::metadata(backup.join(entry.file_name())).unwrap();
        assert_eq!(live.len(), saved.len());
        assert_eq!(
            live.permissions().mode() & 0o777,
            saved.permissions().mode() & 0o777
        );
        assert_eq!(live.dev(), saved.dev());
        assert_ne!(live.ino(), saved.ino());
    }
    snapshot.restore().unwrap();
    let restored_total = fs::read_dir(&source)
        .unwrap()
        .map(|entry| fs::metadata(entry.unwrap().path()).unwrap().len())
        .sum::<u64>();
    assert_eq!(restored_total, OBSERVED_TOTAL_BYTES);
    let _ = fs::remove_dir_all(tmp);
}

#[test]
fn authority_snapshot_limit_diagnostic_is_path_and_credential_free() {
    let tmp = isolated_tmpdir("authority-limit-redaction");
    let source = tmp.join("science-data");
    let backup = tmp.join("rollback/science-data");
    fs::create_dir_all(source.join("conda")).unwrap();
    fs::create_dir(tmp.join("rollback")).unwrap();
    let canary = "sk-private-canary-path-secret";
    let path = source.join("conda").join(canary);
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.set_len(MAX_AUTHORITY_SNAPSHOT_FILE_BYTES + 1).unwrap();

    let error =
        AuthorityTreeSnapshot::capture_scoped(AuthoritySnapshotScope::ScienceData, source, backup)
            .err()
            .expect("oversized Science authority must fail closed");
    assert!(error.contains("code=authority_snapshot_file_limit"));
    assert!(error.contains("scope=science_data"));
    assert!(error.contains("category=conda_cache"));
    assert!(!error.contains(canary));
    assert!(!error.contains(tmp.to_string_lossy().as_ref()));

    let special_source = tmp.join("special-science-data");
    let special_backup = tmp.join("rollback/special-science-data");
    fs::create_dir(&special_source).unwrap();
    let special_canary = "sk-private-special-file-canary";
    let socket_path = special_source.join(special_canary);
    let socket_path_raw =
        std::ffi::CString::new(socket_path.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(socket_path_raw.as_ptr(), 0o600) }, 0);
    let special_error = AuthorityTreeSnapshot::capture_scoped(
        AuthoritySnapshotScope::ScienceData,
        special_source,
        special_backup,
    )
    .err()
    .expect("special authority files must fail closed");
    assert!(special_error.contains("code=authority_snapshot_special_file"));
    assert!(!special_error.contains(special_canary));
    assert!(!special_error.contains(tmp.to_string_lossy().as_ref()));
    let _ = fs::remove_dir_all(tmp);
}

#[test]
fn authority_snapshot_fails_closed_when_directory_membership_changes_mid_capture() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmp = isolated_tmpdir("directory-membership-race");
    let source = tmp.join("authority");
    let backup = tmp.join("rollback/authority");
    let barrier = tmp.join("barrier");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir(tmp.join("rollback")).unwrap();
    fs::write(source.join("database.db"), b"prior-database\n").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(
        source.join("database.db"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let _seam = test_arm_authority_snapshot_directory_barrier(source.clone(), barrier.clone());

    let capture_source = source.clone();
    let capture_backup = backup.clone();
    let worker =
        thread::spawn(move || AuthorityTreeSnapshot::capture(capture_source, capture_backup));
    for _ in 0..200 {
        if barrier.join("ready").is_file() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        barrier.join("ready").is_file(),
        "test-only barrier must observe the directory enumeration boundary"
    );
    fs::write(source.join("database.db-wal"), b"concurrent-wal\n").unwrap();
    fs::set_permissions(
        source.join("database.db-wal"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::write(barrier.join("release"), b"release\n").unwrap();
    let capture = worker.join().unwrap();
    let accepted_torn_tree = capture.is_ok()
        && backup.join("database.db").is_file()
        && !backup.join("database.db-wal").exists();
    assert!(
            capture.is_err(),
            "authority snapshot must fail closed when a DB/WAL directory entry appears after enumeration; accepted_torn_tree={accepted_torn_tree}"
        );
    drop(_seam);

    let rebound_source = tmp.join("rebound-authority");
    let rebound_backup = tmp.join("rollback/rebound-authority");
    let displaced_backup = tmp.join("rollback/displaced-authority");
    let foreign = tmp.join("foreign-must-remain-empty");
    let rebound_barrier = tmp.join("rebound-barrier");
    fs::create_dir(&rebound_source).unwrap();
    fs::create_dir(&foreign).unwrap();
    fs::write(rebound_source.join("state"), b"must-stay-pinned\n").unwrap();
    let rebound_seam = test_arm_authority_snapshot_directory_barrier(
        rebound_source.clone(),
        rebound_barrier.clone(),
    );
    let rebound_worker = {
        let source = rebound_source.clone();
        let backup = rebound_backup.clone();
        thread::spawn(move || AuthorityTreeSnapshot::capture(source, backup))
    };
    for _ in 0..200 {
        if rebound_barrier.join("ready").is_file() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(rebound_barrier.join("ready").is_file());
    fs::rename(&rebound_backup, &displaced_backup).unwrap();
    symlink(&foreign, &rebound_backup).unwrap();
    fs::write(rebound_barrier.join("release"), b"release\n").unwrap();
    let rebound_error = rebound_worker
        .join()
        .unwrap()
        .err()
        .expect("destination entry rebind must fail closed");
    drop(rebound_seam);
    assert!(rebound_error.contains("code=authority_snapshot_destination_rebound"));
    assert!(
        !foreign.join("state").exists(),
        "dirfd-anchored copy must never write through a rebound destination symlink"
    );
    assert_eq!(
        fs::read(displaced_backup.join("state")).unwrap(),
        b"must-stay-pinned\n"
    );
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn fresh_authority_snapshot_parent_is_private_and_cleanup_safe() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    let tmp = isolated_tmpdir("fresh-authority-parent");
    let home = tmp.join("home");
    env.set("HOME", &home);
    let config_dir = config::default_dir();
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let config = Config::default();
    config::save_to(&config_dir, &config).unwrap();
    assert!(
        !sandbox_home.parent().unwrap().exists(),
        "fixture must begin before the managed sandbox directory exists"
    );
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let mut snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .expect("fresh config must create a safe private snapshot parent");
    let parent = sandbox_home.parent().unwrap();
    let parent_metadata = fs::symlink_metadata(parent).unwrap();
    assert!(
        parent_metadata.is_dir()
            && !parent_metadata.file_type().is_symlink()
            && parent_metadata.uid() == unsafe { libc::geteuid() }
            && parent_metadata.permissions().mode() & 0o777 == 0o700,
        "fresh snapshot parent must be an owned private directory"
    );
    let backup_root = snapshot.backup_root.clone();
    let registered = config::read_pending_authority_cleanup_manifest(&config_dir)
        .unwrap()
        .unwrap();
    let registered: serde_json::Value = serde_json::from_slice(&registered).unwrap();
    assert_eq!(
        registered["entries"].as_array().map(Vec::len),
        Some(1),
        "a complete authority snapshot must be durably registered before protected writes"
    );
    let runtime_id = "b".repeat(64);
    advance_runtime_transaction(
        &config_dir,
        "snapshot-crash-fixture",
        None,
        &format!("{AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX}{runtime_id}"),
    )
    .unwrap();
    let retry_error = retry_pending_authority_cleanup(&state)
        .expect_err("an active authority transaction must preserve its recovery snapshot");
    assert!(
        retry_error.contains("code=authority_snapshot_recovery_required") && backup_root.is_dir(),
        "active crash recovery must preserve the exact registered root: {retry_error}"
    );
    clear_runtime_transaction(&config_dir).unwrap();
    snapshot
        .restore(&config_dir, &state, ProxyAction::Reused)
        .expect("fresh missing authority parents must already satisfy prior absence");
    let manifest = config::read_pending_authority_cleanup_manifest(&config_dir)
        .unwrap()
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    assert!(
            parent.is_dir()
                && !sandbox_home.exists()
                && !backup_root.exists()
                && manifest["entries"].as_array().is_some_and(Vec::is_empty),
            "restore must retain the private sandbox parent, preserve prior HOME absence, and durably clear rollback state"
        );
    let panic_snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .unwrap();
    let panic_recovery_root = panic_snapshot.backup_root.clone();
    fs::create_dir_all(&auth_dir).unwrap();
    fs::set_permissions(&auth_dir, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        auth_dir.join("active-org.json"),
        b"partial-protected-write\n",
    )
    .unwrap();
    advance_runtime_transaction(
        &config_dir,
        "snapshot-panic-fixture",
        None,
        &format!("{AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX}{runtime_id}"),
    )
    .unwrap();
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _held_snapshot = panic_snapshot;
        panic!("test-only panic after protected mutation");
    }));
    assert!(unwind.is_err());
    let panic_retry = retry_pending_authority_cleanup(&state)
        .expect_err("panic unwind must not convert ActiveRecovery to cleanup-only");
    assert!(
        panic_retry.contains("cleanup_code=authority_snapshot_recovery_required")
            && panic_retry.contains(&panic_recovery_root.to_string_lossy().to_string())
            && panic_recovery_root.is_dir(),
        "panic unwind must preserve the exact registered recovery root: {panic_retry}"
    );
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn one_click_snapshot_does_not_descend_or_restore_science_owned_environment() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmp = isolated_tmpdir("science-owned-environment-boundary");
    let config_dir = tmp.join("config");
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let org_db = auth_dir.join("orgs/org-test/history.db");
    let conda_sentinel = auth_dir.join("conda/candidate-state");
    let unknown_sentinel = auth_dir.join("future-science-state/candidate-state");
    let config = Config::default();
    config::save_to(&config_dir, &config).unwrap();
    fs::create_dir_all(org_db.parent().unwrap()).unwrap();
    fs::create_dir_all(conda_sentinel.parent().unwrap()).unwrap();
    fs::create_dir_all(unknown_sentinel.parent().unwrap()).unwrap();
    fs::write(
        auth_dir.join("active-org.json"),
        b"{\"org_uuid\":\"org-test\"}\n",
    )
    .unwrap();
    fs::write(&org_db, b"prior-history\n").unwrap();
    fs::write(&conda_sentinel, b"prior-environment\n").unwrap();
    fs::write(&unknown_sentinel, b"prior-unknown\n").unwrap();
    for root in SCIENCE_OWNED_OPAQUE_ROOTS {
        let sparse_path = auth_dir.join(root).join("large-environment");
        fs::create_dir_all(sparse_path.parent().unwrap()).unwrap();
        let sparse = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&sparse_path)
            .unwrap();
        sparse.set_len(4 * 1024 * 1024 * 1024).unwrap();
    }
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let _clone_fallback = test_arm_authority_snapshot_clone_errno(libc::ENOTSUP);

    let mut snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .expect("opaque Science environment must not enter snapshot copy budgets");
    fs::write(
        auth_dir.join("active-org.json"),
        b"{\"org_uuid\":\"candidate\"}\n",
    )
    .unwrap();
    fs::write(&org_db, b"candidate-history\n").unwrap();
    fs::write(&conda_sentinel, b"candidate-environment\n").unwrap();
    fs::write(&unknown_sentinel, b"candidate-unknown\n").unwrap();

    snapshot
        .restore(&config_dir, &state, ProxyAction::Reused)
        .expect("protected authority projection must restore independently");
    assert_eq!(
        fs::read(auth_dir.join("active-org.json")).unwrap(),
        b"{\"org_uuid\":\"org-test\"}\n"
    );
    assert_eq!(fs::read(&org_db).unwrap(), b"prior-history\n");
    assert_eq!(
        fs::read(&conda_sentinel).unwrap(),
        b"candidate-environment\n",
        "CSSwitch rollback must preserve Science-owned environment in place"
    );
    assert_eq!(fs::read(&unknown_sentinel).unwrap(), b"candidate-unknown\n");
    for root in SCIENCE_OWNED_OPAQUE_ROOTS {
        assert_eq!(
            fs::metadata(auth_dir.join(root).join("large-environment"))
                .unwrap()
                .len(),
            4 * 1024 * 1024 * 1024,
            "CSSwitch must neither copy nor delete opaque environment objects"
        );
    }
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn one_click_snapshot_rejects_opaque_root_symlink_without_touching_target() {
    let tmp = isolated_tmpdir("science-owned-environment-symlink");
    let config_dir = tmp.join("config");
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let foreign = tmp.join("foreign");
    let config = Config::default();
    config::save_to(&config_dir, &config).unwrap();
    fs::create_dir_all(&auth_dir).unwrap();
    fs::create_dir(&foreign).unwrap();
    fs::write(foreign.join("canary"), b"must-not-change\n").unwrap();
    symlink(&foreign, auth_dir.join("conda")).unwrap();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));

    let error =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .err()
            .expect("opaque root symlink must fail closed");
    assert!(error.contains("code=science_environment_root_identity_failed"));
    assert_eq!(
        fs::read(foreign.join("canary")).unwrap(),
        b"must-not-change\n"
    );
    assert!(fs::symlink_metadata(auth_dir.join("conda"))
        .unwrap()
        .file_type()
        .is_symlink());
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn one_click_snapshot_blocks_protected_restore_after_opaque_root_rebind() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmp = isolated_tmpdir("science-owned-environment-restore-rebind");
    let config_dir = tmp.join("config");
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let foreign = tmp.join("foreign");
    let config = Config::default();
    config::save_to(&config_dir, &config).unwrap();
    fs::create_dir_all(auth_dir.join("conda")).unwrap();
    fs::create_dir(&foreign).unwrap();
    fs::write(auth_dir.join("active-org.json"), b"prior\n").unwrap();
    fs::write(foreign.join("canary"), b"must-not-change\n").unwrap();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));

    let mut snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .expect("safe top-level opaque directory must allow capture");
    fs::rename(auth_dir.join("conda"), auth_dir.join("conda-displaced")).unwrap();
    symlink(&foreign, auth_dir.join("conda")).unwrap();
    fs::write(auth_dir.join("active-org.json"), b"candidate\n").unwrap();

    let error = snapshot
        .restore(&config_dir, &state, ProxyAction::Reused)
        .expect_err("opaque-root rebind must block protected Science restore");
    assert!(error.contains("code=science_environment_root_identity_failed"));
    assert_eq!(
        fs::read(auth_dir.join("active-org.json")).unwrap(),
        b"candidate\n",
        "protected authority must not be restored after the root contract changes"
    );
    assert_eq!(
        fs::read(foreign.join("canary")).unwrap(),
        b"must-not-change\n"
    );
    drop(snapshot);
    let _ = fs::remove_dir_all(&tmp);

    let race_tmp = isolated_tmpdir("science-owned-environment-capture-rebind");
    let race_config_dir = race_tmp.join("config");
    let race_sandbox_home = race_config_dir.join("sandbox/home");
    let race_auth_dir = race_sandbox_home.join(".claude-science");
    let race_orgs = race_auth_dir.join("orgs");
    let displaced_auth = race_tmp.join("displaced-auth");
    let replacement_auth = race_tmp.join("replacement-auth");
    let barrier = race_tmp.join("barrier");
    let race_config = Config::default();
    config::save_to(&race_config_dir, &race_config).unwrap();
    fs::create_dir_all(race_orgs.join("org-test")).unwrap();
    fs::write(race_orgs.join("org-test/history.db"), b"captured-prior\n").unwrap();
    let race_state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let _barrier =
        test_arm_authority_snapshot_directory_barrier(race_orgs.clone(), barrier.clone());
    let worker_config_dir = race_config_dir.clone();
    let worker_sandbox_home = race_sandbox_home.clone();
    let worker_auth_dir = race_auth_dir.clone();
    let worker_state = race_state.clone();
    let worker = thread::spawn(move || {
        OneClickAuthoritySnapshot::capture(
            &worker_config_dir,
            &worker_sandbox_home,
            &worker_auth_dir,
            &race_config,
            &worker_state,
        )
    });
    for _ in 0..200 {
        if barrier.join("ready").is_file() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(barrier.join("ready").is_file());
    fs::rename(&race_auth_dir, &displaced_auth).unwrap();
    fs::create_dir_all(replacement_auth.join("orgs/org-test")).unwrap();
    fs::write(
        replacement_auth.join("orgs/org-test/replacement-canary"),
        b"replacement-must-not-be-read-or-changed\n",
    )
    .unwrap();
    fs::rename(&replacement_auth, &race_auth_dir).unwrap();
    fs::write(barrier.join("release"), b"release\n").unwrap();
    let capture_error = worker
        .join()
        .unwrap()
        .err()
        .expect("root rebind during capture must fail closed");
    assert!(
        capture_error.contains("code=science_authority_root_rebound")
            || capture_error.contains("code=authority_snapshot_source_parent_rebound"),
        "unexpected capture rebind refusal: {capture_error}"
    );
    assert_eq!(
        fs::read(race_auth_dir.join("orgs/org-test/replacement-canary")).unwrap(),
        b"replacement-must-not-be-read-or-changed\n"
    );
    let _ = fs::remove_dir_all(&race_tmp);
}

#[test]
fn fresh_authority_snapshot_parent_refuses_symlink_without_touching_target() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmp = isolated_tmpdir("fresh-authority-parent-symlink");
    let config_dir = tmp.join("config");
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let foreign = tmp.join("foreign");
    let config = Config::default();
    config::save_to(&config_dir, &config).unwrap();
    fs::create_dir(&foreign).unwrap();
    fs::write(foreign.join("canary"), b"must-not-change\n").unwrap();
    symlink(&foreign, config_dir.join("sandbox")).unwrap();
    let foreign_before = tree(&foreign);
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let error =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .err()
            .expect("a symlinked snapshot parent must fail closed");
    assert!(
        error.contains("code=authority_snapshot_root_parent_open_failed"),
        "unexpected symlink refusal: {error}"
    );
    assert_eq!(
        tree(&foreign),
        foreign_before,
        "snapshot parent setup must not follow or mutate the symlink target"
    );
    assert!(
        fs::symlink_metadata(config_dir.join("sandbox"))
            .unwrap()
            .file_type()
            .is_symlink(),
        "the rejected symlink must remain untouched"
    );
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn fresh_authority_snapshot_parent_refuses_non_sandbox_contract() {
    let tmp = isolated_tmpdir("fresh-authority-parent-contract");
    let config_dir = tmp.join("config");
    let sandbox_home = config_dir.join("foreign-child/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let config = Config::default();
    config::save_to(&config_dir, &config).unwrap();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let error =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .err()
            .expect("only the exact config/sandbox/home layout may create a snapshot parent");
    assert_eq!(
        error, "code=authority_snapshot_root_parent_contract_failed",
        "unexpected non-sandbox contract refusal"
    );
    assert!(
        !config_dir.join("foreign-child").exists(),
        "contract refusal must not create or mutate an arbitrary config child"
    );
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn fresh_authority_snapshot_parent_rebind_fails_before_mutating_replacement() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmp = isolated_tmpdir("fresh-authority-parent-rebind");
    let config_dir = tmp.join("config");
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let sandbox_parent = sandbox_home.parent().unwrap().to_path_buf();
    let displaced = tmp.join("displaced-sandbox");
    let replacement = tmp.join("replacement");
    let barrier = tmp.join("barrier");
    let config = Config::default();
    config::save_to(&config_dir, &config).unwrap();
    fs::create_dir(&sandbox_parent).unwrap();
    fs::set_permissions(&sandbox_parent, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(&replacement).unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o750)).unwrap();
    fs::write(replacement.join("canary"), b"must-not-change\n").unwrap();
    let replacement_before = tree(&replacement);
    let seam = test_arm_authority_snapshot_parent_barrier(sandbox_parent.clone(), barrier.clone());
    let worker = {
        let config_dir = config_dir.clone();
        let sandbox_home = sandbox_home.clone();
        let auth_dir = auth_dir.clone();
        let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
        thread::spawn(move || {
            OneClickAuthoritySnapshot::capture(
                &config_dir,
                &sandbox_home,
                &auth_dir,
                &config,
                &state,
            )
        })
    };
    for _ in 0..200 {
        if barrier.join("ready").is_file() {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(barrier.join("ready").is_file());
    fs::rename(&sandbox_parent, &displaced).unwrap();
    fs::rename(&replacement, &sandbox_parent).unwrap();
    fs::write(barrier.join("release"), b"release\n").unwrap();
    let error = worker
        .join()
        .unwrap()
        .err()
        .expect("snapshot parent replacement must fail closed");
    drop(seam);
    assert_eq!(
        error, "code=authority_snapshot_root_parent_identity_failed",
        "unexpected snapshot parent rebind failure"
    );
    assert_eq!(
        tree(&sandbox_parent),
        replacement_before,
        "identity refusal must occur before chmod or writes reach the replacement directory"
    );
    assert!(
        displaced.is_dir(),
        "the originally pinned sandbox directory must remain recoverable"
    );
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn one_shot_commit_cleanup_fault_is_retried_before_success() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmp = isolated_tmpdir("commit-cleanup-once");
    let config_dir = tmp.join("config");
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let cleanup_log = tmp.join("cleanup.log");
    fs::create_dir_all(&auth_dir).unwrap();
    fs::write(auth_dir.join("active-org.json"), b"private-authority\n").unwrap();
    fs::set_permissions(
        auth_dir.join("active-org.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let config = Config::default();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    {
        let _sync_seam = test_arm_authority_snapshot_completion_sync_failure();
        let error = OneClickAuthoritySnapshot::capture(
            &config_dir,
            &sandbox_home,
            &auth_dir,
            &config,
            &state,
        )
        .err()
        .expect("completion fsync failure must fail closed");
        assert!(
            error.contains("code=authority_snapshot_completion_sync_failed"),
            "unexpected completion-sync failure: {error}"
        );
    }
    let rollback_residue = fs::read_dir(sandbox_home.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".one-click-rollback-")
        });
    assert!(
        !rollback_residue,
        "completion fsync failure must register and finish rollback cleanup"
    );
    let mut cleanup_sync_snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .unwrap();
    let cleanup_sync_root = cleanup_sync_snapshot.backup_root.clone();
    let cleanup_sync_tombstone = cleanup_tombstone_path(&PendingCleanupEntry {
        managed_id: cleanup_sync_root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        path: cleanup_sync_root.clone(),
        device: 0,
        inode: 0,
        marker: String::new(),
    });
    {
        let _cleanup_sync_seam = test_arm_authority_cleanup_parent_sync_failure();
        let error = cleanup_sync_snapshot
            .cleanup_when_expendable()
            .expect_err("cleanup parent fsync failure must remain pending");
        assert!(error.contains("recovery_status=cleanup_required"));
        let manifest = config::read_pending_authority_cleanup_manifest(&config_dir)
            .unwrap()
            .unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(manifest["entries"].as_array().map(Vec::len), Some(1));
        assert!(cleanup_sync_tombstone.is_dir());
    }
    fs::remove_file(cleanup_sync_tombstone.join(PENDING_CLEANUP_MARKER_FILE)).unwrap();
    let pending_raw = config::read_pending_authority_cleanup_manifest(&config_dir)
        .unwrap()
        .unwrap();
    let pending = parse_pending_cleanup_manifest(&pending_raw).unwrap();
    let retry_ticket = RegisteredAuthorityCleanup {
        manifest_raw: pending_raw,
        entry: pending.entries.into_iter().next().unwrap(),
    };
    finalize_registered_authority_cleanup(&cleanup_sync_snapshot.cleanup_context, &retry_ticket)
        .unwrap();
    let cleared = config::read_pending_authority_cleanup_manifest(&config_dir)
        .unwrap()
        .unwrap();
    let cleared: serde_json::Value = serde_json::from_slice(&cleared).unwrap();
    assert_eq!(cleared["entries"].as_array().map(Vec::len), Some(0));
    cleanup_sync_snapshot.cleanup_prepared = true;
    cleanup_sync_snapshot.preserve_recovery = false;

    let mut rebound_cleanup_snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .unwrap();
    let rebound_root = rebound_cleanup_snapshot.backup_root.clone();
    let displaced_root = tmp.join("cleanup-register-displaced-root");
    fs::rename(&rebound_root, &displaced_root).unwrap();
    fs::create_dir(&rebound_root).unwrap();
    fs::set_permissions(&rebound_root, fs::Permissions::from_mode(0o700)).unwrap();
    let rebound_error = rebound_cleanup_snapshot
        .cleanup_when_expendable()
        .expect_err("registered cleanup ticket must reject a replacement root");
    assert!(
        rebound_error.contains("cleanup_manifest_identity_mismatch"),
        "unexpected registered-ticket identity refusal: {rebound_error}"
    );
    assert!(
        rebound_root.is_dir(),
        "replacement root must not be deleted"
    );
    assert!(
        displaced_root.is_dir(),
        "original rollback root must remain recoverable"
    );
    assert!(
        !rebound_root.join(PENDING_CLEANUP_MARKER_FILE).exists(),
        "replacement root must not receive a cleanup marker"
    );
    let rebound_manifest = config::read_pending_authority_cleanup_manifest(&config_dir)
        .unwrap()
        .unwrap();
    let rebound_manifest: serde_json::Value = serde_json::from_slice(&rebound_manifest).unwrap();
    assert_eq!(
        rebound_manifest["entries"].as_array().map(Vec::len),
        Some(1),
        "the exact pre-mutation cleanup ticket must remain registered"
    );
    fs::remove_dir(&rebound_root).unwrap();
    fs::rename(&displaced_root, &rebound_root).unwrap();
    rebound_cleanup_snapshot
        .cleanup_when_expendable()
        .expect("restored exact root identity must consume the registered ticket");

    let _seam = test_arm_authority_snapshot_cleanup_fault(tmp.clone(), "once", cleanup_log.clone());
    let mut snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .unwrap();
    let backup_root = snapshot.backup_root.clone();
    snapshot.commit();
    let cleanup_attempts = fs::read_to_string(&cleanup_log)
        .unwrap_or_default()
        .lines()
        .count();
    let root_removed_before_success = !backup_root.exists();
    if backup_root.exists() {
        fs::remove_dir_all(&backup_root).unwrap();
    }
    let _ = fs::remove_dir_all(&tmp);

    assert!(
            root_removed_before_success && cleanup_attempts >= 2,
            "one-shot cleanup fault must be retried before commit reports success: attempts={cleanup_attempts}, root_removed={root_removed_before_success}"
        );
}

#[test]
fn pending_cleanup_observer_mapping_only_does_not_claim_durable_cleanup() {
    let managed_id = ".one-click-rollback-0123456789abcdef0123456789abcdef";
    let identity = config::PendingCleanupIdentity {
        managed_id: managed_id.to_string(),
        path: PathBuf::from("/synthetic/mapping-only").join(managed_id),
        device: 41,
        inode: 73,
        marker: managed_id.to_string(),
    };
    let different = config::PendingCleanupIdentity {
        inode: 74,
        ..identity.clone()
    };
    let _lifecycle = config::test_arm_pending_cleanup_lifecycle(None);
    config::test_observe_pending_cleanup_manifest_validated(identity.clone());

    config::test_observe_pending_cleanup_initial_ticket(
        config::PendingCleanupInitialTicket::Present(identity.clone()),
    );
    config::test_observe_pending_cleanup_completion(
        config::PendingCleanupRemovalOutcome::Removed,
        config::PendingCleanupFinalState::NotFound,
    );
    config::test_observe_pending_cleanup_initial_ticket(
        config::PendingCleanupInitialTicket::Missing(identity.clone()),
    );
    config::test_observe_pending_cleanup_completion(
        config::PendingCleanupRemovalOutcome::AlreadyAbsent,
        config::PendingCleanupFinalState::NotFound,
    );

    for (ticket, outcome, final_state) in [
        (
            config::PendingCleanupInitialTicket::Present(identity.clone()),
            config::PendingCleanupRemovalOutcome::AlreadyAbsent,
            config::PendingCleanupFinalState::NotFound,
        ),
        (
            config::PendingCleanupInitialTicket::Missing(identity.clone()),
            config::PendingCleanupRemovalOutcome::Removed,
            config::PendingCleanupFinalState::NotFound,
        ),
        (
            config::PendingCleanupInitialTicket::Present(identity.clone()),
            config::PendingCleanupRemovalOutcome::Error,
            config::PendingCleanupFinalState::Error,
        ),
        (
            config::PendingCleanupInitialTicket::Present(identity.clone()),
            config::PendingCleanupRemovalOutcome::Removed,
            config::PendingCleanupFinalState::Present(different.clone()),
        ),
    ] {
        config::test_observe_pending_cleanup_initial_ticket(ticket);
        config::test_observe_pending_cleanup_completion(outcome, final_state);
    }

    config::test_observe_pending_cleanup_initial_ticket(
        config::PendingCleanupInitialTicket::Present(identity.clone()),
    );
    config::test_observe_pending_cleanup_completion(
        config::PendingCleanupRemovalOutcome::Removed,
        config::PendingCleanupFinalState::NotFound,
    );
    let observation = config::test_pending_cleanup_lifecycle_observation();
    assert_eq!(
            observation.events,
            vec![
                config::PendingCleanupLifecycleEvent::Register(identity.clone()),
                config::PendingCleanupLifecycleEvent::Remove {
                    identity: identity.clone(),
                    not_found: false,
                },
                config::PendingCleanupLifecycleEvent::Remove {
                    identity: identity.clone(),
                    not_found: true,
                },
                config::PendingCleanupLifecycleEvent::Remove {
                    identity,
                    not_found: false,
                },
            ],
            "mapping-only seam self-test must preserve exact Present/Removed=false and Missing/AlreadyAbsent=true outcomes without deduplication"
        );
    assert_eq!(
            observation.causal_mismatch_count, 4,
            "Present+AlreadyAbsent, Missing+Removed, error, and final Present are causal mismatches and must emit zero Remove"
        );
    assert_eq!(observation.completion_count, 7);
}

#[test]
fn partial_capture_cleanup_failure_returns_tracked_degraded_recovery() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let tmp = isolated_tmpdir("partial-capture-cleanup");
    let config_dir = tmp.join("config");
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let cleanup_log = tmp.join("cleanup.log");
    fs::create_dir_all(&auth_dir).unwrap();
    fs::write(auth_dir.join("active-org.json"), b"private-authority\n").unwrap();
    fs::set_permissions(
        auth_dir.join("active-org.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let config = Config::default();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let _capture_seam =
        test_arm_authority_snapshot_capture_failure(sandbox_home.parent().unwrap().join("state"));
    let _cleanup_seam =
        test_arm_authority_snapshot_cleanup_fault(tmp.clone(), "persistent", cleanup_log.clone());
    let failure =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .err()
            .expect("partial capture fault must fail");
    let cleanup_line = fs::read_to_string(&cleanup_log)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_string();
    let backup_root = PathBuf::from(
        cleanup_line
            .split('\t')
            .nth(2)
            .expect("cleanup observation must track the exact root"),
    );
    let degraded_and_tracked = (failure.contains("cleanup_required")
        || failure.contains("degraded"))
        && failure.contains(&backup_root.to_string_lossy().to_string())
        && backup_root.exists();
    if backup_root.exists() {
        fs::remove_dir_all(&backup_root).unwrap();
    }
    let _ = fs::remove_dir_all(&tmp);

    assert!(
            degraded_and_tracked,
            "partial-capture cleanup failure must return explicit degraded cleanup_required state with the exact residual path: failure={failure:?}, root={}",
            backup_root.display()
        );
}

#[test]
fn rollback_refusal_restores_independent_authorities_and_preserves_recovery_snapshot() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    let tmp = isolated_tmpdir("rollback-refusal");
    let home = tmp.join("home");
    env.set("HOME", &home);
    let config_dir = config::default_dir();
    let sandbox_home = config_dir.join("sandbox/home");
    let auth_dir = sandbox_home.join(".claude-science");
    let private_state = sandbox_home.parent().unwrap().join("state");
    let runtime_dir = config_dir.join("runtime");
    let receipt = config_dir.join("science-managed-launch.v1.json");
    fs::create_dir_all(&auth_dir).unwrap();
    fs::create_dir_all(&private_state).unwrap();
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::set_permissions(&auth_dir, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(auth_dir.join("active-org.json"), b"prior-auth\n").unwrap();
    fs::write(private_state.join("private.json"), b"prior-private\n").unwrap();
    fs::write(runtime_dir.join("bridge.key"), b"prior-runtime\n").unwrap();
    fs::write(&receipt, b"prior-receipt\n").unwrap();
    for path in [
        auth_dir.join("active-org.json"),
        private_state.join("private.json"),
        runtime_dir.join("bridge.key"),
        receipt.clone(),
    ] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let config = Config::default();
    config::save_to(&config_dir, &config).unwrap();
    let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
    let mut snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .unwrap();
    let backup_root = snapshot.backup_root.clone();
    let auth_before = tree(&auth_dir);
    let private_before = tree(&private_state);
    let runtime_before = tree(&runtime_dir);
    let receipt_before = tree(&receipt);
    let config_before = config::load_from(&config_dir).unwrap();

    fs::remove_dir_all(&auth_dir).unwrap();
    let foreign = tmp.join("foreign-target");
    fs::write(&foreign, b"foreign-must-not-change\n").unwrap();
    fs::set_permissions(&foreign, fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&foreign, &auth_dir).unwrap();
    fs::write(private_state.join("private.json"), b"mutated-private\n").unwrap();
    fs::write(runtime_dir.join("bridge.key"), b"mutated-runtime\n").unwrap();
    fs::write(&receipt, b"mutated-receipt\n").unwrap();
    config::update(&config_dir, |current| {
        current.proxy_port = 54321;
        current.secret = "candidate-config-secret".into();
        current.reuse_system_ssh = true;
    })
    .unwrap();
    {
        let mut app = state.lock().unwrap();
        app.proxy_port = 54321;
        app.secret = "candidate-app-secret".into();
        app.provider = "candidate-provider".into();
        app.gateway_kind = "candidate-gateway".into();
        app.shim_mode = "candidate-shim".into();
        app.launch_id = "candidate-launch".into();
        app.key_fp = 54321;
        app.sandbox_port = 54322;
        app.sandbox_url = Some("http://127.0.0.1:54322/candidate".into());
    }

    let error = snapshot
        .restore(&config_dir, &state, ProxyAction::Reused)
        .unwrap_err();
    let refused_without_following = (error
        .contains("code=science_authority_restore_root_revalidate_failed")
        || error.contains("code=science_authority_restore_root_rebound")
        || error.contains("code=science_authority_root_open_failed"))
        && fs::read(&foreign).unwrap() == b"foreign-must-not-change\n"
        && fs::symlink_metadata(&auth_dir)
            .unwrap()
            .file_type()
            .is_symlink();
    let independent_authorities_restored = tree(&private_state) == private_before
        && tree(&runtime_dir) == runtime_before
        && tree(&receipt) == receipt_before;
    let config_restored = config::load_from(&config_dir).unwrap() == config_before;
    let app_restored = {
        let app = state.lock().unwrap();
        app.proxy.is_none()
            && app.proxy_port == 0
            && app.secret.is_empty()
            && app.provider.is_empty()
            && app.gateway_kind.is_empty()
            && app.shim_mode.is_empty()
            && app.launch_id.is_empty()
            && app.key_fp == 0
            && app.sandbox.is_none()
            && app.sandbox_port == 0
            && app.sandbox_url.is_none()
    };
    drop(snapshot);
    let recovery_metadata = fs::symlink_metadata(&backup_root).ok();
    let recovery_root_preserved = recovery_metadata.as_ref().is_some_and(|metadata| {
        metadata.is_dir() && metadata.permissions().mode() & 0o777 == 0o700
    });
    let immutable_recovery_complete = tree(&backup_root.join("0")) == auth_before
        && tree(&backup_root.join("1")) == private_before
        && tree(&backup_root.join("2")) == runtime_before
        && tree(&backup_root.join("3")) == receipt_before;
    let recovery_has_no_symlink = tree(&backup_root)
        .values()
        .all(|entry| entry.kind != "symlink");
    let retry_error = retry_pending_authority_cleanup(&state)
        .expect_err("an incomplete compensation snapshot must never become cleanup-only");
    let recovery_survives_retry = retry_error
        .contains("cleanup_code=authority_snapshot_recovery_required")
        && backup_root.is_dir();
    assert!(
            refused_without_following
                && independent_authorities_restored
                && config_restored
                && app_restored
                && recovery_root_preserved
                && immutable_recovery_complete
                && recovery_has_no_symlink
                && recovery_survives_retry,
            "rollback refusal must aggregate safely: error={error}; independent={independent_authorities_restored}; config={config_restored}; app={app_restored}; recovery_root={recovery_root_preserved}; recovery_complete={immutable_recovery_complete}; no_symlink={recovery_has_no_symlink}; retry={retry_error}"
        );
    let _ = fs::remove_dir_all(tmp);
}

#[test]
#[ignore = "explicit Acceptance-boundary optional SSH feature-off smoke; temp HOME only"]
fn reuse_system_ssh_false_does_not_require_packaged_wrapper_or_system_home() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    let tmp = isolated_tmpdir("ssh-feature-off");
    let missing_wrapper = tmp.join("missing-wrapper");
    env.remove("HOME");
    env.set("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &missing_wrapper);
    let cfg = Config {
        reuse_system_ssh: false,
        ..Default::default()
    };
    let sandbox_home = tmp.join("sandbox/home");
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let result = prevalidate_one_click_system_ssh(&app.handle().clone(), &cfg, &sandbox_home);
    assert!(
            result.is_ok(),
            "disabled SSH must not require HOME, system config, host parsing, or packaged wrapper: {result:?}"
        );
    let _ = fs::remove_dir_all(tmp);
}

#[test]
#[ignore = "explicit Acceptance-boundary SSH write-authority prevalidation; temp HOME only"]
fn enabled_ssh_prevalidation_rejects_unwritable_science_authority() {
    let _env_lock = TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut env = ScopedEnv::new();
    let tmp = isolated_tmpdir("ssh-unwritable-authority");
    let home = tmp.join("home");
    let sandbox_home = tmp.join("sandbox/home");
    let science_data = sandbox_home.join(".claude-science");
    fs::create_dir_all(home.join(".ssh")).unwrap();
    fs::write(home.join(".ssh/config"), b"Host isolated-test-host\n").unwrap();
    fs::set_permissions(home.join(".ssh/config"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::create_dir_all(&science_data).unwrap();
    fs::write(science_data.join("config.toml"), b"quiet_logs = true\n").unwrap();
    fs::set_permissions(
        science_data.join("config.toml"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::set_permissions(&science_data, fs::Permissions::from_mode(0o500)).unwrap();
    let wrapper = tmp.join("ssh-wrapper");
    fs::write(&wrapper, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    env.set("HOME", &home);
    env.set("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &wrapper);
    let cfg = Config {
        reuse_system_ssh: true,
        ..Default::default()
    };
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let system_before = tree(&home.join(".ssh"));
    let science_before = tree(&science_data);
    let sandbox_stub_before = tree(&sandbox_home.join(".ssh"));
    let result = prevalidate_one_click_system_ssh(&app.handle().clone(), &cfg, &sandbox_home);
    let system_after = tree(&home.join(".ssh"));
    let science_after = tree(&science_data);
    let sandbox_stub_after = tree(&sandbox_home.join(".ssh"));
    let probe_residue = fs::read_dir(&science_data)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| {
            let name = name.to_string_lossy();
            name.starts_with('.') && name.contains("tmp")
        })
        .collect::<Vec<_>>();
    fs::set_permissions(&science_data, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
            result
                .as_ref()
                .is_err_and(|error| error == "隔离 Science SSH authority 不可写"),
            "enabled SSH must reject a statically unwritable bridge authority before OAuth or journal mutation"
        );
    assert_eq!(system_after, system_before);
    assert_eq!(science_after, science_before);
    assert_eq!(sandbox_stub_after, sandbox_stub_before);
    assert!(
        probe_residue.is_empty(),
        "read-only prevalidation must not create a write probe or temp residue"
    );
    let _ = fs::remove_dir_all(tmp);
}

fn profile_with_policy(policy: ModelPolicy) -> config::Profile {
    config::Profile {
        model_policy: policy,
        ..Default::default()
    }
}

fn serve_models_after(
    delay: Duration,
    body: impl Into<String>,
) -> (u16, Arc<AtomicUsize>, thread::JoinHandle<()>) {
    let body = body.into();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    assert_ne!(port, 8765);
    let requests = Arc::new(AtomicUsize::new(0));
    let server_requests = requests.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 1024];
        let read = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with("GET /test-secret/v1/models HTTP/1.0\r\n"));
        server_requests.fetch_add(1, Ordering::SeqCst);
        thread::sleep(delay);
        write!(
                stream,
                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
    });
    (port, requests, server)
}

#[test]
fn gateway_catalog_timeout_matches_model_policy_contract() {
    assert_eq!(
        gateway_model_catalog_timeout_ms(&profile_with_policy(ModelPolicy::DynamicCatalog)),
        crate::runtime::operation::CODEX_MODELS_PROBE_TIMEOUT_MS
    );
    assert_eq!(
        gateway_model_catalog_timeout_ms(&profile_with_policy(ModelPolicy::SavedCatalog)),
        crate::runtime::operation::LOCAL_HEALTH_TIMEOUT_MS
    );
}

#[test]
fn dynamic_catalog_cold_response_uses_one_long_local_request() {
    let body = r#"{"data":[
            {"id":"claude-csswitch-codex-gpt-5"},
            {"id":"claude-opus-5"},
            {"id":"claude-sonnet-5"},
            {"id":"claude-opus-4-8"},
            {"id":"claude-sonnet-4-6"},
            {"id":"claude-haiku-4-5-20251001"}
        ]}"#;
    let (port, requests, server) = serve_models_after(Duration::from_millis(600), body);
    let profile = profile_with_policy(ModelPolicy::DynamicCatalog);

    verify_gateway_model_catalog(port, "test-secret", &profile).unwrap();
    server.join().unwrap();
    assert_eq!(requests.load(Ordering::SeqCst), 1);
}

#[test]
fn dynamic_catalog_still_rejects_empty_or_non_codex_aliases() {
    for body in [
        r#"{"data":[]}"#,
        r#"{"data":[{"id":"gpt-5"}]}"#,
        r#"{"data":[{"id":"claude-sonnet-5"}]}"#,
        r#"{"data":[{"id":"claude-csswitch-codex-gpt-5"},{"id":"unknown-alias"}]}"#,
        r#"{"data":[
                {"id":"claude-csswitch-codex-gpt-5"},
                {"id":"claude-opus-5"},
                {"id":"claude-sonnet-5"},
                {"id":"claude-opus-4-8"},
                {"id":"claude-sonnet-4-6"}
            ]}"#,
    ] {
        let (port, _requests, server) = serve_models_after(Duration::ZERO, body);
        let profile = profile_with_policy(ModelPolicy::DynamicCatalog);
        let error = verify_gateway_model_catalog(port, "test-secret", &profile).unwrap_err();
        assert!(error.contains("Codex published model snapshot"));
        server.join().unwrap();
    }
}

#[test]
fn gateway_catalog_rejects_malformed_or_duplicate_rows_without_filtering_them() {
    for body in [
        r#"{"data":[{"id":"claude-csswitch-codex-gpt-5"},{}]}"#,
        r#"{"data":[{"id":"claude-csswitch-codex-gpt-5"},{"id":7}]}"#,
        r#"{"data":[{"id":"claude-csswitch-codex-gpt-5"},"ignored-before-flow3"]}"#,
        r#"{"data":[{"id":"claude-csswitch-codex-gpt-5"},{"id":"claude-csswitch-codex-gpt-5"}]}"#,
    ] {
        let (port, _requests, server) = serve_models_after(Duration::ZERO, body);
        let profile = profile_with_policy(ModelPolicy::DynamicCatalog);
        let error = verify_gateway_model_catalog(port, "test-secret", &profile).unwrap_err();
        assert!(
            error.contains("malformed") || error.contains("duplicate"),
            "strict catalog parser must reject the exact malformed row: {error}"
        );
        server.join().unwrap();
    }
}

#[test]
fn saved_catalog_requires_exact_selectors_plus_science_canonical_roles() {
    let selector = "claude-csswitch-relay-mock-model-0123456789ab";
    let profile = config::Profile {
        model_policy: ModelPolicy::SavedCatalog,
        model_catalog: vec![crate::model_catalog::ModelRoute {
            selector_id: selector.into(),
            display_name: "Mock model".into(),
            upstream_model: "mock-model".into(),
            supports_tools: Some(true),
            ..Default::default()
        }],
        default_model_route_id: selector.into(),
        ..Default::default()
    };
    let body = r#"{"data":[
            {"id":"claude-csswitch-relay-mock-model-0123456789ab"},
            {"id":"claude-opus-5"},
            {"id":"claude-sonnet-5"},
            {"id":"claude-opus-4-8"},
            {"id":"claude-sonnet-4-6"},
            {"id":"claude-haiku-4-5-20251001"}
        ]}"#;
    let (port, _requests, server) = serve_models_after(Duration::ZERO, body);
    verify_gateway_model_catalog(port, "test-secret", &profile).unwrap();
    server.join().unwrap();

    let unknown = r#"{"data":[
            {"id":"claude-csswitch-relay-mock-model-0123456789ab"},
            {"id":"claude-opus-5"},
            {"id":"claude-sonnet-5"},
            {"id":"claude-opus-4-8"},
            {"id":"claude-sonnet-4-6"},
            {"id":"claude-haiku-4-5-20251001"},
            {"id":"claude-csswitch-stale-provider"}
        ]}"#;
    let (port, _requests, server) = serve_models_after(Duration::ZERO, unknown);
    let error = verify_gateway_model_catalog(port, "test-secret", &profile).unwrap_err();
    assert!(error.contains("白名单/default selector"));
    server.join().unwrap();
}

#[test]
fn runtime_journal_advances_in_place_and_retargets_without_secrets() {
    let dir = std::env::temp_dir().join(format!(
        "csswitch-runtime-journal-{}-{}",
        std::process::id(),
        config::new_id()
    ));
    let previous = RuntimeBindingCommit {
        profile_id: "old".into(),
        route_fp: "route-fp".into(),
        catalog_fp: "catalog-fp".into(),
        binding_fp: "binding-fp".into(),
    };
    config::save_to(
        &dir,
        &Config {
            runtime_binding: Some(previous.clone()),
            ..Default::default()
        },
    )
    .unwrap();

    advance_runtime_transaction(&dir, "new", Some(previous.clone()), "start_gateway").unwrap();
    let first = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    assert_eq!(first.target_profile_id, "new");
    assert_eq!(first.stage, "start_gateway");
    assert_eq!(first.previous_binding, Some(previous.clone()));

    let runtime_id = "a".repeat(64);
    let environment_stage = format!("{SCIENCE_ENVIRONMENT_PENDING_STAGE_PREFIX}{runtime_id}");
    advance_runtime_transaction(&dir, "new", Some(previous.clone()), &environment_stage).unwrap();
    let second = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    assert_eq!(second.transaction_id, first.transaction_id);
    assert_eq!(second.stage, environment_stage);
    assert_eq!(
        interrupted_science_environment_runtime_id(&second.stage),
        Some(runtime_id.as_str())
    );
    assert!(interrupted_science_environment_runtime_id(
        "start_science_environment_pending:not-a-fingerprint"
    )
    .is_none());
    assert!(
        runtime_transaction_requires_snapshot_preservation("start_science")
            && runtime_transaction_requires_snapshot_preservation(
                "start_science_environment_pending"
            )
            && runtime_transaction_requires_snapshot_preservation(&environment_stage)
            && !runtime_transaction_requires_snapshot_preservation("recover_interrupted_gateway"),
        "legacy and fingerprinted environment-exposure stages must fail closed"
    );
    for listener_state in [
        "stopped-no-gateway",
        "running-no-gateway",
        "stopped-managed-gateway",
        "running-managed-gateway",
    ] {
        let legacy = validate_interrupted_science_transaction_entry(Some("start_science"), None)
            .expect_err("legacy 0.8.3 start_science must never authorize an automatic spawn");
        assert!(
            legacy.contains("environment_uncertain")
                && legacy.contains("newer_runtime_required")
                && legacy.contains("manual_recovery_required"),
            "legacy oracle {listener_state} must fail closed: {legacy}"
        );
    }
    let authority_stage = format!("{AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX}{runtime_id}");
    assert_eq!(
        interrupted_science_environment_runtime_id(&authority_stage),
        Some(runtime_id.as_str())
    );

    advance_runtime_transaction(&dir, "newer", Some(previous), "start_gateway").unwrap();
    let retargeted = config::load_from(&dir)
        .unwrap()
        .runtime_transaction
        .unwrap();
    assert_ne!(retargeted.transaction_id, second.transaction_id);
    assert_eq!(retargeted.target_profile_id, "newer");
    let encoded = serde_json::to_string(&retargeted).unwrap();
    assert!(!encoded.contains("api_key"));
    assert!(!encoded.contains("base_url"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn one_click_snapshot_has_one_commit_and_one_failure_compensation_funnel() {
    use syn::visit::{self, Visit};
    use syn::{Expr, ExprCall, ExprMethodCall, Item, ItemFn, Pat, Stmt};

    fn top_level<'a>(file: &'a syn::File, name: &str) -> Option<&'a ItemFn> {
        file.items.iter().find_map(|item| match item {
            Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
    }

    fn local_name(local: &syn::Local) -> Option<&syn::Ident> {
        match &local.pat {
            Pat::Ident(ident) => Some(&ident.ident),
            Pat::Type(typed) => match &*typed.pat {
                Pat::Ident(ident) => Some(&ident.ident),
                _ => None,
            },
            _ => None,
        }
    }

    fn result_arm_name(pattern: &Pat) -> Option<&syn::Ident> {
        match pattern {
            Pat::TupleStruct(tuple) => tuple.path.segments.last().map(|segment| &segment.ident),
            Pat::Struct(structure) => structure.path.segments.last().map(|segment| &segment.ident),
            Pat::Path(path) => path.path.segments.last().map(|segment| &segment.ident),
            Pat::Ident(ident) => ident
                .subpat
                .as_ref()
                .and_then(|(_, pattern)| result_arm_name(pattern)),
            _ => None,
        }
    }

    fn peel_expr(mut expression: &Expr) -> &Expr {
        loop {
            expression = match expression {
                Expr::Group(group) => &group.expr,
                Expr::Paren(paren) => &paren.expr,
                _ => return expression,
            };
        }
    }

    fn direct_call_name(expression: &Expr) -> Option<&syn::Ident> {
        let Expr::Call(call) = peel_expr(expression) else {
            return None;
        };
        let Expr::Path(path) = peel_expr(&call.func) else {
            return None;
        };
        path.path.segments.last().map(|segment| &segment.ident)
    }

    fn success_tail_is_infallible(expression: &Expr) -> bool {
        match peel_expr(expression) {
            Expr::Path(_) => true,
            Expr::Call(call)
                if direct_call_name(expression).is_some_and(|name| name == "Ok")
                    && call.args.len() == 1 =>
            {
                matches!(peel_expr(call.args.first().unwrap()), Expr::Path(_))
            }
            _ => false,
        }
    }

    #[derive(Default)]
    struct FlowFacts {
        calls: Vec<String>,
        methods: Vec<String>,
        tries: usize,
        closures: usize,
    }

    impl<'ast> Visit<'ast> for FlowFacts {
        fn visit_expr_call(&mut self, expression: &'ast ExprCall) {
            if let Expr::Path(path) = &*expression.func {
                if let Some(segment) = path.path.segments.last() {
                    self.calls.push(segment.ident.to_string());
                }
            }
            visit::visit_expr_call(self, expression);
        }

        fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
            self.methods.push(expression.method.to_string());
            visit::visit_expr_method_call(self, expression);
        }

        fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
            self.tries += 1;
            visit::visit_expr_try(self, expression);
        }

        fn visit_expr_closure(&mut self, expression: &'ast syn::ExprClosure) {
            self.closures += 1;
            visit::visit_expr_closure(self, expression);
        }
    }

    #[derive(Debug, Default)]
    struct OuterFacts {
        calls: Vec<String>,
        methods: Vec<String>,
        tries: usize,
        returns: usize,
        assignments: usize,
        macros: usize,
        closures: usize,
    }

    impl<'ast> Visit<'ast> for OuterFacts {
        fn visit_expr_call(&mut self, expression: &'ast ExprCall) {
            if let Expr::Path(path) = &*expression.func {
                if let Some(segment) = path.path.segments.last() {
                    self.calls.push(segment.ident.to_string());
                }
            }
            visit::visit_expr_call(self, expression);
        }

        fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
            self.methods.push(expression.method.to_string());
            visit::visit_expr_method_call(self, expression);
        }

        fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
            self.tries += 1;
            visit::visit_expr_try(self, expression);
        }

        fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) {
            self.returns += 1;
            visit::visit_expr_return(self, expression);
        }

        fn visit_expr_assign(&mut self, expression: &'ast syn::ExprAssign) {
            self.assignments += 1;
            visit::visit_expr_assign(self, expression);
        }

        fn visit_macro(&mut self, expression: &'ast syn::Macro) {
            self.macros += 1;
            visit::visit_macro(self, expression);
        }

        fn visit_expr_closure(&mut self, _expression: &'ast syn::ExprClosure) {
            self.closures += 1;
        }
        fn visit_item_fn(&mut self, _function: &'ast ItemFn) {}
    }

    #[derive(Default)]
    struct TransactionLocalCount(usize);

    impl<'ast> Visit<'ast> for TransactionLocalCount {
        fn visit_local(&mut self, local: &'ast syn::Local) {
            if local_name(local).is_some_and(|name| name == "transaction_result") {
                self.0 += 1;
            }
            visit::visit_local(self, local);
        }
    }

    let source = include_str!("mod.rs");
    let product_source = &source[..source
        .find("#[cfg(test)]\nmod transaction_tests")
        .expect("product source must precede transaction tests")];
    let file = syn::parse_file(product_source).expect("product Rust source must parse");
    let one_click = top_level(&file, "one_click_login_with_options")
        .expect("one-click product function must remain module-level");
    let recovery_restart = top_level(&file, "restart_managed_science_with_budget")
        .expect("DB recovery restart must remain a module-level bounded helper");
    assert!(
        top_level(&file, "compensate_one_click_failure").is_some(),
        "one-click must expose one release-visible failure compensation helper"
    );
    let snapshot_index = one_click
        .block
        .stmts
        .iter()
        .position(|statement| {
            matches!(
                statement,
                Stmt::Local(local)
                    if local_name(local).is_some_and(|name| name == "authority_snapshot")
            )
        })
        .expect("one-click must capture authority_snapshot before mutation");
    assert_eq!(
        one_click.block.stmts.len(),
        snapshot_index + 3,
        "authority_snapshot must be followed by exactly transaction_result and its final match"
    );
    let transaction_index = snapshot_index + 1;
    let transaction_statement = &one_click.block.stmts[transaction_index];
    let transaction_local = match transaction_statement {
        Stmt::Local(local)
            if local_name(local).is_some_and(|name| name == "transaction_result") =>
        {
            local
        }
        _ => {
            panic!("authority_snapshot must be followed immediately by let transaction_result")
        }
    };
    let mut transaction_locals = TransactionLocalCount::default();
    transaction_locals.visit_item_fn(one_click);
    assert_eq!(
        transaction_locals.0, 1,
        "one-click must contain exactly one transaction_result local"
    );
    let initializer = transaction_local
        .init
        .as_ref()
        .expect("transaction_result must have an immediate closure initializer");
    let transaction_call = match peel_expr(&initializer.expr) {
        Expr::Call(call) => call,
        _ => panic!("transaction_result initializer must directly invoke a closure"),
    };
    assert!(
        transaction_call.args.is_empty(),
        "transaction_result closure invocation must have zero arguments"
    );
    let transaction_closure = match peel_expr(&transaction_call.func) {
        Expr::Closure(closure) => closure,
        _ => panic!("transaction_result initializer must be a directly invoked closure"),
    };
    assert!(
        transaction_closure.inputs.is_empty(),
        "transaction_result closure must accept zero arguments"
    );
    let mut transaction = FlowFacts::default();
    transaction.visit_stmt(transaction_statement);
    assert_eq!(
        transaction.closures, 1,
        "transaction_result must be produced by one bounded mutation closure"
    );
    for required in [
        "ensure_virtual_login",
        "prepare_science_ssh_bridge",
        "revoke_science_ssh_bridge",
        "ensure_proxy",
        "record_managed_science_launch",
    ] {
        assert!(
            transaction.calls.iter().any(|call| call == required),
            "the single mutation closure must own the {required} error edge"
        );
    }
    assert!(
            transaction.methods.iter().any(|method| method == "spawn")
                && transaction.methods.iter().any(|method| method == "wait")
                && !transaction.methods.iter().any(|method| method == "status"),
            "the single mutation closure must own explicit shell spawn/wait and distinguish spawn from wait failure"
        );
    assert!(
            transaction
                .methods
                .iter()
                .filter(|method| *method == "validate_science_restore_root")
                .count()
                >= 3,
            "one-click must revalidate exact Science/opaque-root bindings before protected writes and immediately before spawn"
        );
    let mut recovery_restart_flow = FlowFacts::default();
    recovery_restart_flow.visit_item_fn(recovery_restart);
    assert!(
            recovery_restart_flow
                .methods
                .iter()
                .any(|method| method == "spawn")
                && recovery_restart_flow
                    .methods
                    .iter()
                    .any(|method| method == "try_wait")
                && recovery_restart_flow
                    .methods
                    .iter()
                    .any(|method| method == "saturating_duration_since")
                && recovery_restart_flow
                    .calls
                    .iter()
                    .any(|call| call == "http_health")
                && !recovery_restart_flow
                    .methods
                    .iter()
                    .any(|method| method == "status"),
            "DB recovery restart must enforce one absolute deadline across explicit shell try_wait and remaining-time-capped health"
        );
    assert!(
        !transaction.methods.iter().any(|method| method == "commit")
            && !transaction
                .calls
                .iter()
                .any(|call| call == "compensate_one_click_failure"),
        "transaction_result closure must neither commit nor compensate its own snapshot"
    );

    let final_statement = one_click
        .block
        .stmts
        .last()
        .expect("one-click must end in the transaction result match");
    let final_match = match final_statement {
        Stmt::Expr(Expr::Match(expression), _) => expression,
        _ => panic!("one-click must end with exactly one success/failure transaction match"),
    };
    assert!(
        matches!(
            &*final_match.expr,
            Expr::Path(path) if path.path.is_ident("transaction_result")
        ),
        "the final transaction match must consume transaction_result directly"
    );
    assert_eq!(
        final_match.arms.len(),
        2,
        "the final transaction match must contain only one success and one failure arm"
    );
    assert!(
        final_match.arms.iter().all(|arm| arm.guard.is_none()),
        "the final transaction match must not use guarded arms"
    );
    let success = final_match
        .arms
        .iter()
        .find(|arm| result_arm_name(&arm.pat).is_some_and(|name| name == "Ok"))
        .expect("the final transaction match must contain one Ok arm");
    let failure = final_match
        .arms
        .iter()
        .find(|arm| result_arm_name(&arm.pat).is_some_and(|name| name == "Err"))
        .expect("the final transaction match must contain one Err arm");
    let success_block = match peel_expr(&success.body) {
        Expr::Block(block) => &block.block,
        _ => panic!("Ok arm must be a block containing commit and an infallible tail"),
    };
    assert_eq!(
        success_block.stmts.len(),
        2,
        "Ok arm must contain only snapshot commit and an infallible success tail"
    );
    let direct_commit = match &success_block.stmts[0] {
        Stmt::Expr(Expr::MethodCall(call), Some(_)) => {
            call.method == "commit"
                && call.args.is_empty()
                && matches!(
                    peel_expr(&call.receiver),
                    Expr::Path(path) if path.path.is_ident("authority_snapshot")
                )
        }
        _ => false,
    };
    assert!(
        direct_commit,
        "Ok arm must begin with the sole direct authority_snapshot.commit()"
    );
    assert!(
        matches!(
            &success_block.stmts[1],
            Stmt::Expr(tail, None) if success_tail_is_infallible(tail)
        ),
        "Ok arm must end with only an infallible path or Ok(path) tail"
    );

    let failure_expression = match peel_expr(&failure.body) {
        Expr::Block(block) if matches!(block.block.stmts.as_slice(), [Stmt::Expr(_, None)]) => {
            match &block.block.stmts[0] {
                Stmt::Expr(expression, None) => expression,
                _ => unreachable!(),
            }
        }
        expression => expression,
    };
    assert!(
        direct_call_name(failure_expression)
            .is_some_and(|name| name == "compensate_one_click_failure"),
        "Err arm must be exactly one direct compensate_one_click_failure call"
    );
    let Expr::Call(failure_call) = peel_expr(failure_expression) else {
        unreachable!()
    };
    let mut failure_arguments = OuterFacts::default();
    for argument in &failure_call.args {
        failure_arguments.visit_expr(argument);
    }
    assert!(
        failure_arguments.calls.is_empty()
            && failure_arguments.methods.is_empty()
            && failure_arguments.tries == 0
            && failure_arguments.returns == 0
            && failure_arguments.assignments == 0
            && failure_arguments.macros == 0
            && failure_arguments.closures == 0,
        "Err compensation arguments must be operation-free: {failure_arguments:?}"
    );

    let mut post_snapshot = FlowFacts::default();
    for statement in one_click.block.stmts.iter().skip(snapshot_index + 1) {
        post_snapshot.visit_stmt(statement);
    }
    assert_eq!(
        post_snapshot
            .methods
            .iter()
            .filter(|method| *method == "commit")
            .count(),
        1,
        "all post-snapshot AST must contain exactly one commit, solely in Ok"
    );
    assert_eq!(
        post_snapshot
            .calls
            .iter()
            .filter(|call| *call == "compensate_one_click_failure")
            .count(),
        1,
        "all post-snapshot AST must contain exactly one compensation call, solely in Err"
    );
}

#[test]
fn ssh_wrapper_prevalidation_uses_the_running_runtime_validator_before_oauth() {
    use syn::visit::{self, Visit};
    use syn::{
        Attribute, Expr, ExprCall, ExprLit, GenericArgument, Item, ItemFn, Lit, Pat, PathArguments,
        Stmt, Type,
    };

    fn top_level<'a>(file: &'a syn::File, name: &str) -> &'a ItemFn {
        file.items
            .iter()
            .find_map(|item| match item {
                Item::Fn(function) if function.sig.ident == name => Some(function),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing module-level product function {name}"))
    }

    fn is_cfg(attribute: &Attribute) -> bool {
        attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
    }

    fn cfg_tokens(attribute: &Attribute) -> String {
        attribute
            .meta
            .require_list()
            .map(|list| list.tokens.to_string())
            .unwrap_or_default()
    }

    fn reject_cfg(attributes: &[Attribute], label: &str) {
        assert!(
            !attributes.iter().any(is_cfg),
            "{label} must be present in every release build"
        );
    }

    #[derive(Default)]
    struct Facts {
        calls: Vec<String>,
        strings: Vec<String>,
        has_cfg: bool,
    }

    impl<'ast> Visit<'ast> for Facts {
        fn visit_attribute(&mut self, attribute: &'ast Attribute) {
            self.has_cfg |= is_cfg(attribute);
            visit::visit_attribute(self, attribute);
        }

        fn visit_expr_call(&mut self, call: &'ast ExprCall) {
            if let Expr::Path(path) = &*call.func {
                if let Some(segment) = path.path.segments.last() {
                    self.calls.push(segment.ident.to_string());
                }
            }
            visit::visit_expr_call(self, call);
        }

        fn visit_expr_lit(&mut self, literal: &'ast ExprLit) {
            if let Lit::Str(value) = &literal.lit {
                self.strings.push(value.value());
            }
            visit::visit_expr_lit(self, literal);
        }

        fn visit_item_fn(&mut self, _function: &'ast ItemFn) {}
        fn visit_expr_closure(&mut self, _closure: &'ast syn::ExprClosure) {}
        fn visit_expr_async(&mut self, _expression: &'ast syn::ExprAsync) {}
    }

    fn statement_facts(statement: &Stmt) -> Facts {
        let mut facts = Facts::default();
        facts.visit_stmt(statement);
        facts
    }

    fn function_facts(function: &ItemFn) -> Facts {
        let mut facts = Facts::default();
        facts.visit_block(&function.block);
        facts
    }

    fn local_name(local: &syn::Local) -> Option<&syn::Ident> {
        match &local.pat {
            Pat::Ident(ident) => Some(&ident.ident),
            Pat::Type(typed) => match &*typed.pat {
                Pat::Ident(ident) => Some(&ident.ident),
                _ => None,
            },
            _ => None,
        }
    }

    fn peel_expression(mut expression: &Expr) -> &Expr {
        loop {
            expression = match expression {
                Expr::Group(group) => &group.expr,
                Expr::Paren(paren) => &paren.expr,
                _ => return expression,
            };
        }
    }

    fn direct_zero_arg_closure_body(local: &syn::Local) -> Option<&syn::Block> {
        let initializer = local.init.as_ref()?;
        let Expr::Call(call) = peel_expression(&initializer.expr) else {
            return None;
        };
        if !call.args.is_empty() {
            return None;
        }
        let Expr::Closure(closure) = peel_expression(&call.func) else {
            return None;
        };
        closure
            .inputs
            .is_empty()
            .then_some(&closure.body)
            .and_then(|body| match peel_expression(body) {
                Expr::Block(block) => Some(&block.block),
                _ => None,
            })
    }

    fn direct_call(expression: &Expr) -> Option<&ExprCall> {
        match expression {
            Expr::Call(call) => Some(call),
            Expr::Await(awaited) => direct_call(&awaited.base),
            Expr::Group(group) => direct_call(&group.expr),
            Expr::Paren(paren) => direct_call(&paren.expr),
            Expr::Try(tried) => direct_call(&tried.expr),
            // Typed failure projection wraps Result edges as `.map_err(...)?`.
            Expr::MethodCall(method)
                if method.method == "map_err" || method.method == "map_err_kind" =>
            {
                direct_call(&method.receiver)
            }
            _ => None,
        }
    }

    fn call_path(call: &ExprCall) -> Option<String> {
        let Expr::Path(path) = &*call.func else {
            return None;
        };
        Some(
            path.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        )
    }

    fn expression_path(expression: &Expr) -> Option<String> {
        let Expr::Path(path) = expression else {
            return None;
        };
        Some(
            path.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        )
    }

    fn statement_directly_calls(statement: &Stmt, expected: &str) -> bool {
        let expression = match statement {
            Stmt::Local(local) => local.init.as_ref().map(|init| &*init.expr),
            Stmt::Expr(expression, _) => Some(expression),
            _ => None,
        };
        expression
            .and_then(direct_call)
            .and_then(call_path)
            .as_deref()
            == Some(expected)
    }

    fn simple_argument_name(expression: &Expr) -> Option<String> {
        let expression = match expression {
            Expr::Reference(reference) => &*reference.expr,
            Expr::Group(group) => &*group.expr,
            Expr::Paren(paren) => &*paren.expr,
            expression => expression,
        };
        let Expr::Path(path) = expression else {
            return None;
        };
        (path.qself.is_none() && path.path.segments.len() == 1)
            .then(|| path.path.segments[0].ident.to_string())
    }

    fn statement_direct_call_arguments(statement: &Stmt) -> Option<Vec<String>> {
        let expression = match statement {
            Stmt::Local(local) => local.init.as_ref().map(|init| &*init.expr),
            Stmt::Expr(expression, _) => Some(expression),
            _ => None,
        }?;
        let call = direct_call(expression)?;
        call.args.iter().map(simple_argument_name).collect()
    }

    fn statement_propagates_direct_call(statement: &Stmt, expected: &str) -> bool {
        let expression = match statement {
            Stmt::Local(local) => local.init.as_ref().map(|init| &*init.expr),
            Stmt::Expr(expression, _) => Some(expression),
            _ => None,
        };
        let Some(Expr::Try(tried)) = expression else {
            return false;
        };
        // Accept both `call?` and `call.map_err(...)?` (typed failure projection).
        let call = match &*tried.expr {
            Expr::Call(call) => call,
            Expr::MethodCall(method)
                if (method.method == "map_err" || method.method == "map_err_kind") =>
            {
                match &*method.receiver {
                    Expr::Call(call) => call,
                    _ => return false,
                }
            }
            _ => return false,
        };
        call_path(call).as_deref() == Some(expected)
    }

    fn returns_result_pathbuf_string(function: &ItemFn) -> bool {
        let syn::ReturnType::Type(_, returned) = &function.sig.output else {
            return false;
        };
        let Type::Path(path) = &**returned else {
            return false;
        };
        let Some(result) = path.path.segments.last() else {
            return false;
        };
        if result.ident != "Result" {
            return false;
        }
        let PathArguments::AngleBracketed(arguments) = &result.arguments else {
            return false;
        };
        let types = arguments
            .args
            .iter()
            .filter_map(|argument| match argument {
                GenericArgument::Type(Type::Path(path)) => path
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        types == ["PathBuf", "String"]
    }

    #[derive(Default)]
    struct EarlyExitFacts {
        count: usize,
    }

    impl<'ast> Visit<'ast> for EarlyExitFacts {
        fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) {
            self.count += 1;
            visit::visit_expr_return(self, expression);
        }

        fn visit_expr_break(&mut self, expression: &'ast syn::ExprBreak) {
            self.count += 1;
            visit::visit_expr_break(self, expression);
        }

        fn visit_expr_continue(&mut self, expression: &'ast syn::ExprContinue) {
            self.count += 1;
            visit::visit_expr_continue(self, expression);
        }

        fn visit_expr_loop(&mut self, expression: &'ast syn::ExprLoop) {
            self.count += 1;
            visit::visit_expr_loop(self, expression);
        }

        fn visit_expr_while(&mut self, expression: &'ast syn::ExprWhile) {
            self.count += 1;
            visit::visit_expr_while(self, expression);
        }

        fn visit_expr_for_loop(&mut self, expression: &'ast syn::ExprForLoop) {
            self.count += 1;
            visit::visit_expr_for_loop(self, expression);
        }

        fn visit_expr_call(&mut self, call: &'ast ExprCall) {
            if call_path(call).is_some_and(|path| {
                matches!(
                    path.rsplit("::").next(),
                    Some("exit" | "abort" | "abort_internal")
                )
            }) {
                self.count += 1;
            }
            visit::visit_expr_call(self, call);
        }

        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            if invocation.path.segments.last().is_some_and(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "panic" | "todo" | "unreachable"
                )
            }) || token_stream_contains_any(
                invocation.tokens.clone(),
                &["return", "break", "continue"],
            ) {
                self.count += 1;
            }
            visit::visit_macro(self, invocation);
        }
    }

    #[derive(Default)]
    struct ProductLiterals(Vec<String>);

    impl<'ast> Visit<'ast> for ProductLiterals {
        fn visit_expr_lit(&mut self, literal: &'ast ExprLit) {
            if let Lit::Str(value) = &literal.lit {
                self.0.push(value.value());
            }
            visit::visit_expr_lit(self, literal);
        }
    }

    fn use_tree_contains_ident(tree: &syn::UseTree, expected: &str) -> bool {
        match tree {
            syn::UseTree::Path(path) => {
                path.ident == expected || use_tree_contains_ident(&path.tree, expected)
            }
            syn::UseTree::Name(name) => name.ident == expected,
            syn::UseTree::Rename(rename) => rename.ident == expected,
            syn::UseTree::Group(group) => group
                .items
                .iter()
                .any(|tree| use_tree_contains_ident(tree, expected)),
            syn::UseTree::Glob(_) => false,
        }
    }

    fn token_stream_contains_any(tokens: proc_macro2::TokenStream, expected: &[&str]) -> bool {
        tokens.into_iter().any(|token| match token {
            proc_macro2::TokenTree::Ident(ident) => expected.iter().any(|value| ident == *value),
            proc_macro2::TokenTree::Group(group) => {
                token_stream_contains_any(group.stream(), expected)
            }
            _ => false,
        })
    }

    #[derive(Default)]
    struct ForbiddenCfgMacros(usize);

    impl<'ast> Visit<'ast> for ForbiddenCfgMacros {
        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            if invocation
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "cfg")
                || token_stream_contains_any(invocation.tokens.clone(), &["cfg"])
            {
                self.0 += 1;
            }
            visit::visit_macro(self, invocation);
        }

        fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
            if use_tree_contains_ident(&item.tree, "cfg") {
                self.0 += 1;
            }
            visit::visit_item_use(self, item);
        }
    }

    #[derive(Default)]
    struct ValidatorFacts {
        cfg_attributes: Vec<String>,
        environment_reads: Vec<String>,
        environment_paths: Vec<String>,
        environment_imports: usize,
    }

    #[derive(Default)]
    struct ProductEnvironmentFacts {
        environment_paths: Vec<String>,
        environment_imports: usize,
    }

    impl<'ast> Visit<'ast> for ProductEnvironmentFacts {
        fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
            let path = expression
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            let last = path.rsplit("::").next().unwrap_or_default();
            if path.split("::").any(|segment| segment == "env")
                || matches!(last, "var" | "var_os" | "vars" | "vars_os")
            {
                self.environment_paths.push(path);
            }
            visit::visit_expr_path(self, expression);
        }

        fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
            if use_tree_contains_ident(&item.tree, "env") {
                self.environment_imports += 1;
            }
            visit::visit_item_use(self, item);
        }

        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            if token_stream_contains_any(
                invocation.tokens.clone(),
                &["env", "var", "var_os", "vars", "vars_os"],
            ) {
                self.environment_imports += 1;
            }
            visit::visit_macro(self, invocation);
        }
    }

    impl<'ast> Visit<'ast> for ValidatorFacts {
        fn visit_attribute(&mut self, attribute: &'ast Attribute) {
            if is_cfg(attribute) {
                self.cfg_attributes.push(cfg_tokens(attribute));
            }
            visit::visit_attribute(self, attribute);
        }

        fn visit_expr_call(&mut self, call: &'ast ExprCall) {
            if let Some(path) = call_path(call) {
                let last = path.rsplit("::").next().unwrap_or_default();
                if path.split("::").any(|segment| segment == "env")
                    || matches!(last, "var" | "var_os" | "vars" | "vars_os")
                {
                    self.environment_reads.push(path);
                }
            }
            visit::visit_expr_call(self, call);
        }

        fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
            let path = expression
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            let last = path.rsplit("::").next().unwrap_or_default();
            if path.split("::").any(|segment| segment == "env")
                || matches!(last, "var" | "var_os" | "vars" | "vars_os")
            {
                self.environment_paths.push(path);
            }
            visit::visit_expr_path(self, expression);
        }

        fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
            if use_tree_contains_ident(&item.tree, "env") {
                self.environment_imports += 1;
            }
            visit::visit_item_use(self, item);
        }
    }

    #[derive(Default)]
    struct WrapperLocalCount(usize);

    impl<'ast> Visit<'ast> for WrapperLocalCount {
        fn visit_local(&mut self, local: &'ast syn::Local) {
            if local_name(local).is_some_and(|name| name == "wrapper_override") {
                self.0 += 1;
            }
            visit::visit_local(self, local);
        }
    }

    #[derive(Default)]
    struct CfgAttributes(Vec<String>);

    impl<'ast> Visit<'ast> for CfgAttributes {
        fn visit_attribute(&mut self, attribute: &'ast Attribute) {
            if is_cfg(attribute) {
                self.0.push(cfg_tokens(attribute));
            }
            visit::visit_attribute(self, attribute);
        }
    }

    fn exact_test_override(local: &syn::Local) -> bool {
        if local.attrs.len() != 1
            || !local.attrs[0].path().is_ident("cfg")
            || cfg_tokens(&local.attrs[0]) != "test"
            || !matches!(&local.pat, Pat::Ident(ident) if ident.ident == "wrapper_override")
        {
            return false;
        }
        let Some(initializer) = &local.init else {
            return false;
        };
        let Expr::MethodCall(mapped) = &*initializer.expr else {
            return false;
        };
        if mapped.method != "map" || mapped.args.len() != 1 {
            return false;
        }
        let Expr::Call(read) = &*mapped.receiver else {
            return false;
        };
        if call_path(read).as_deref() != Some("std::env::var_os") || read.args.len() != 1 {
            return false;
        }
        if !matches!(
            read.args.first(),
            Some(Expr::Lit(ExprLit {
                lit: Lit::Str(value),
                ..
            })) if value.value() == "CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE"
        ) {
            return false;
        }
        expression_path(mapped.args.first().unwrap()).as_deref() == Some("PathBuf::from")
    }

    fn option_pathbuf_type(pattern: &Pat) -> bool {
        let Pat::Type(typed) = pattern else {
            return false;
        };
        if !matches!(&*typed.pat, Pat::Ident(ident) if ident.ident == "wrapper_override") {
            return false;
        }
        let Type::Path(path) = &*typed.ty else {
            return false;
        };
        let Some(option) = path.path.segments.last() else {
            return false;
        };
        if option.ident != "Option" {
            return false;
        }
        let PathArguments::AngleBracketed(arguments) = &option.arguments else {
            return false;
        };
        matches!(
            arguments.args.first(),
            Some(GenericArgument::Type(Type::Path(inner)))
                if inner.path.segments.last().is_some_and(|segment| segment.ident == "PathBuf")
        )
    }

    fn exact_release_override(local: &syn::Local) -> bool {
        if local.attrs.len() != 1
            || !local.attrs[0].path().is_ident("cfg")
            || cfg_tokens(&local.attrs[0]).replace(' ', "") != "not(test)"
            || !option_pathbuf_type(&local.pat)
        {
            return false;
        }
        let Some(initializer) = &local.init else {
            return false;
        };
        expression_path(&initializer.expr).as_deref() == Some("None")
    }

    // SSH validators live in ssh_preflight; orchestrator stays in façade mod.rs.
    let facade_source = include_str!("mod.rs");
    let facade_product = &facade_source[..facade_source
        .find("#[cfg(test)]\nmod transaction_tests")
        .expect("product source must precede transaction tests")];
    let facade = syn::parse_file(facade_product).expect("facade product Rust source must parse");
    let ssh = syn::parse_file(include_str!("ssh_preflight.rs"))
        .expect("ssh_preflight product Rust source must parse");
    let mut forbidden_cfg_macros = ForbiddenCfgMacros::default();
    forbidden_cfg_macros.visit_file(&facade);
    forbidden_cfg_macros.visit_file(&ssh);
    assert_eq!(
        forbidden_cfg_macros.0, 0,
        "product SSH transaction source must not branch on cfg!(test)"
    );
    let validator = top_level(&ssh, "validate_system_ssh_wrapper_path");
    let running = top_level(&ssh, "validate_running_system_ssh_bridge");
    let prevalidation = top_level(&ssh, "prevalidate_one_click_system_ssh");
    let one_click = top_level(&facade, "one_click_login_with_options");
    assert!(
        returns_result_pathbuf_string(validator),
        "shared wrapper validator must return Result<PathBuf, String>"
    );
    let mut product_environment = ProductEnvironmentFacts::default();
    product_environment.visit_file(&facade);
    product_environment.visit_file(&ssh);
    product_environment.environment_paths.sort();
    assert_eq!(
        product_environment.environment_imports, 0,
        "product SSH transaction source must not import or alias environment APIs"
    );
    assert_eq!(
            product_environment.environment_paths,
            [
                "std::env::var".to_string(),
                "std::env::var".to_string(),
                "std::env::var".to_string(),
                "std::env::var_os".to_string(),
                "std::env::var_os".to_string(),
                "std::env::var_os".to_string(),
            ],
            "product transaction source may reference only the existing spike seam, DB reverify/restart-budget seams, exact wrapper override, host-proof seam, and exact late-failure seam environment APIs"
        );

    for (name, function) in [
        ("shared wrapper validator", validator),
        ("running SSH validator", running),
        ("pre-OAuth SSH validator", prevalidation),
        ("one-click product path", one_click),
    ] {
        reject_cfg(&function.attrs, name);
    }

    {
        let (name, function) = ("running SSH validator", running);
        let facts = function_facts(function);
        assert!(
            !facts.has_cfg,
            "{name} must not contain cfg-gated call sites"
        );
    }
    let prevalidation_facts = function_facts(prevalidation);
    assert!(
        !prevalidation_facts.has_cfg,
        "pre-OAuth SSH validation must not contain cfg-gated call sites"
    );
    assert_eq!(
        prevalidation_facts
            .calls
            .iter()
            .filter(|call| *call == "validate_system_ssh_wrapper_path")
            .count(),
        1,
        "enabled pre-OAuth validation must use the same shared wrapper validator exactly once"
    );
    for required in [
        "prevalidate_science_ssh_bridge",
        "prevalidate_sandbox_ssh_stub",
    ] {
        assert!(
                prevalidation_facts.calls.iter().any(|call| call == required),
                "pre-OAuth validation must preserve disabled-mode read-only conflict validation via {required}"
            );
    }

    {
        let (name, function) = ("running SSH validator", running);
        let shared_call_positions = function
            .block
            .stmts
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| {
                statement_directly_calls(
                    statement,
                    "crate::runtime::sandbox_session::validate_system_ssh_wrapper_path",
                )
                .then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            shared_call_positions,
            [0],
            "{name} must execute the shared wrapper validator exactly once as its first statement"
        );
        assert_eq!(
            statement_direct_call_arguments(&function.block.stmts[0]),
            Some(vec!["app".to_string()]),
            "{name} shared-validator call may receive only the simple app argument"
        );
        assert!(
            statement_propagates_direct_call(
                &function.block.stmts[0],
                "crate::runtime::sandbox_session::validate_system_ssh_wrapper_path",
            ),
            "{name} must propagate the shared-validator Result with an exact Try(Call)"
        );
        let mut call_exit = EarlyExitFacts::default();
        call_exit.visit_stmt(&function.block.stmts[0]);
        assert_eq!(
                call_exit.count, 0,
                "{name} shared-validator call statement must not hide early-exit control flow in its arguments"
            );
    }

    let prevalidate_statement = one_click
        .block
        .stmts
        .iter()
        .position(|statement| {
            matches!(statement, Stmt::Local(_))
                && statement_directly_calls(
                    statement,
                    "crate::runtime::sandbox_session::prevalidate_one_click_system_ssh",
                )
        })
        .expect("one-click must execute prevalidation in a top-level local statement");
    assert_eq!(
        one_click
            .block
            .stmts
            .iter()
            .filter(|statement| {
                statement_directly_calls(
                    statement,
                    "crate::runtime::sandbox_session::prevalidate_one_click_system_ssh",
                )
            })
            .count(),
        1,
        "one-click must execute exactly one direct prevalidation call"
    );
    assert_eq!(
        statement_direct_call_arguments(&one_click.block.stmts[prevalidate_statement]),
        Some(vec![
            "app".to_string(),
            "cfg".to_string(),
            "sbx_home".to_string(),
        ]),
        "one-click prevalidation may receive only simple app, cfg, and sbx_home arguments"
    );
    assert!(
        statement_propagates_direct_call(
            &one_click.block.stmts[prevalidate_statement],
            "crate::runtime::sandbox_session::prevalidate_one_click_system_ssh",
        ),
        "one-click must propagate the prevalidation Result with an exact Try(Call)"
    );
    let mut early_exit = EarlyExitFacts::default();
    for statement in &one_click.block.stmts[..prevalidate_statement] {
        early_exit.visit_stmt(statement);
    }
    early_exit.visit_stmt(&one_click.block.stmts[prevalidate_statement]);
    assert_eq!(
            early_exit.count, 0,
            "one-click prevalidation statement and its prefix must be reachable before explicit early-exit control flow"
        );
    let authority_snapshot_statement = one_click
        .block
        .stmts
        .iter()
        .position(|statement| {
            matches!(
                statement,
                Stmt::Local(local)
                    if local_name(local).is_some_and(|name| name == "authority_snapshot")
            )
        })
        .expect("one-click authority snapshot statement must exist");
    let transaction_statement = one_click
        .block
        .stmts
        .iter()
        .position(|statement| {
            matches!(
                statement,
                Stmt::Local(local)
                    if local_name(local).is_some_and(|name| name == "transaction_result")
            )
        })
        .expect("one-click transaction_result statement must exist");
    assert!(
        prevalidate_statement < authority_snapshot_statement
            && authority_snapshot_statement < transaction_statement,
        "SSH prevalidation must precede authority snapshot and the mutation transaction"
    );
    assert!(
        one_click.block.stmts[..transaction_statement]
            .iter()
            .all(|statement| !statement_facts(statement)
                .calls
                .iter()
                .any(|call| call == "ensure_virtual_login")),
        "one-click must not execute OAuth mutation before transaction_result"
    );
    let transaction_local = match &one_click.block.stmts[transaction_statement] {
        Stmt::Local(local) => local,
        _ => unreachable!(),
    };
    let transaction_body = direct_zero_arg_closure_body(transaction_local)
        .expect("transaction_result must directly invoke one zero-argument closure block");
    let oauth_statements = transaction_body
        .stmts
        .iter()
        .enumerate()
        .filter(|(_, statement)| {
            statement_facts(statement)
                .calls
                .iter()
                .any(|call| call == "ensure_virtual_login")
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(
        oauth_statements.len(),
        1,
        "transaction body must contain exactly one top-level OAuth mutation statement"
    );
    let mut one_click_cfg = CfgAttributes::default();
    one_click_cfg.visit_block(&one_click.block);
    assert_eq!(
        one_click_cfg.0,
        ["test".to_string(), "test".to_string()],
        "one-click may contain only the exact host-proof and late-failure cfg(test) seams"
    );
    let late_seam_statements = transaction_body
        .stmts
        .iter()
        .enumerate()
        .filter(|(_, statement)| {
            let facts = statement_facts(statement);
            facts.has_cfg
                && facts
                    .strings
                    .iter()
                    .any(|value| value == "CSSWITCH_TEST_SSH_LATE_FOREIGN_STUB")
                && matches!(
                    statement,
                    Stmt::Expr(Expr::If(expression), _)
                        if expression.attrs.len() == 1
                            && expression.attrs[0].path().is_ident("cfg")
                            && cfg_tokens(&expression.attrs[0]) == "test"
                )
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    assert_eq!(
        late_seam_statements.len(),
        1,
        "transaction body must contain exactly one top-level exact cfg(test) late-failure seam"
    );
    assert!(
        oauth_statements[0] < late_seam_statements[0],
        "the sole transaction cfg(test) seam must remain after OAuth mutation"
    );

    let wrapper_locals = validator
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            Stmt::Local(local)
                if local_name(local).is_some_and(|name| name == "wrapper_override") =>
            {
                Some(local)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut wrapper_local_count = WrapperLocalCount::default();
    wrapper_local_count.visit_block(&validator.block);
    assert_eq!(
        wrapper_local_count.0, 2,
        "shared validator must not hide extra wrapper_override locals in nested product code"
    );
    assert_eq!(
        wrapper_locals.len(),
        2,
        "shared validator must define test and release wrapper_override locals"
    );
    assert!(
            wrapper_locals.iter().any(|local| exact_test_override(local)),
            "test wrapper_override must be the sole cfg(test) var_os literal mapped through PathBuf::from"
        );
    assert!(
        wrapper_locals
            .iter()
            .any(|local| exact_release_override(local)),
        "release wrapper_override must be exactly cfg(not(test)) Option<PathBuf> = None"
    );
    let mut validator_facts = ValidatorFacts::default();
    validator_facts.visit_block(&validator.block);
    validator_facts.cfg_attributes.sort();
    assert_eq!(
        validator_facts.cfg_attributes,
        ["not (test)".to_string(), "test".to_string()],
        "shared validator may contain only the two exact wrapper_override cfg attributes"
    );
    assert_eq!(
        validator_facts.environment_reads,
        ["std::env::var_os".to_string()],
        "shared validator may perform only the guarded test var_os environment read"
    );
    assert_eq!(
        validator_facts.environment_paths,
        ["std::env::var_os".to_string()],
        "shared validator may reference only the guarded test var_os environment path"
    );
    assert_eq!(
        validator_facts.environment_imports, 0,
        "shared validator must not import or alias environment APIs"
    );
    let mut product_literals = ProductLiterals::default();
    product_literals.visit_file(&facade);
    product_literals.visit_file(&ssh);
    assert_eq!(
        product_literals
            .0
            .iter()
            .filter(|value| value.as_str() == "CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE")
            .count(),
        1,
        "the test wrapper environment variable may appear only in its guarded local"
    );

    struct RestoreWrapperOverride(Option<std::ffi::OsString>);
    impl Drop for RestoreWrapperOverride {
        fn drop(&mut self) {
            match self.0.take() {
                Some(value) => std::env::set_var("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", value),
                None => std::env::remove_var("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE"),
            }
        }
    }

    use std::os::unix::fs::PermissionsExt;
    let _override_guard =
        RestoreWrapperOverride(std::env::var_os("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE"));
    let root = std::env::temp_dir().join(format!(
        "csswitch-shared-ssh-validator-{}-{}",
        std::process::id(),
        crate::config::new_id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let missing = root.join("missing-wrapper");
    std::env::set_var("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &missing);
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    assert_eq!(
        super::validate_system_ssh_wrapper_path(app.handle()).unwrap_err(),
        "打包的 CSSwitch SSH bridge 缺失"
    );
    let wrapper = root.join("ssh");
    std::fs::write(&wrapper, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::env::set_var("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &wrapper);
    assert_eq!(
        super::validate_system_ssh_wrapper_path(app.handle()).unwrap_err(),
        "打包的 CSSwitch SSH bridge 不是安全的可执行文件"
    );
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        super::validate_system_ssh_wrapper_path(app.handle()).unwrap(),
        wrapper
    );
    let wrapper_link = root.join("ssh-link");
    std::os::unix::fs::symlink(&wrapper, &wrapper_link).unwrap();
    std::env::set_var("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &wrapper_link);
    assert_eq!(
        super::validate_system_ssh_wrapper_path(app.handle()).unwrap_err(),
        "打包的 CSSwitch SSH bridge 不是安全的可执行文件"
    );
    let wrapper_directory = root.join("ssh-directory");
    std::fs::create_dir(&wrapper_directory).unwrap();
    std::fs::set_permissions(&wrapper_directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::env::set_var("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &wrapper_directory);
    assert_eq!(
        super::validate_system_ssh_wrapper_path(app.handle()).unwrap_err(),
        "打包的 CSSwitch SSH bridge 不是安全的可执行文件"
    );
    let oversized_wrapper = root.join("ssh-oversized");
    std::fs::write(&oversized_wrapper, vec![b'x'; 128 * 1024 + 1]).unwrap();
    std::fs::set_permissions(&oversized_wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::env::set_var("CSSWITCH_TEST_SSH_WRAPPER_OVERRIDE", &oversized_wrapper);
    assert_eq!(
        super::validate_system_ssh_wrapper_path(app.handle()).unwrap_err(),
        "打包的 CSSwitch SSH bridge 不是安全的可执行文件"
    );
    std::fs::remove_dir_all(root).unwrap();
}
