use super::*;

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
    let (model_catalog, default_model_route_id, role_bindings) =
        crate::model_catalog::new_profile_catalog(
            "deepseek",
            "anthropic",
            Some("deepseek-v4-flash"),
        )
        .unwrap();
    let config = Config {
        profiles: vec![config::Profile {
            id: "snapshot-crash-fixture".into(),
            template_id: "deepseek".into(),
            api_format: "anthropic".into(),
            model: "deepseek-v4-flash".into(),
            model_catalog,
            default_model_route_id,
            role_bindings,
            model_policy: crate::provider_contracts::ModelPolicy::SavedCatalog,
            ..Default::default()
        }],
        active_id: "snapshot-crash-fixture".into(),
        ..Default::default()
    };
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
    let snapshot_ticket = snapshot.registered_snapshot_ticket().unwrap();
    let snapshot_identity = OneClickTransactionIdentity {
        target_profile_id: "snapshot-crash-fixture".into(),
        runtime_fingerprint: runtime_id.clone(),
        snapshot_ticket: snapshot_ticket.clone(),
        previous_binding: None,
        profile_switch_handoff: None,
        gateway_terminal_handoff: None,
        prior_stop: config::RuntimePriorStopState::NotRequired,
    };
    let mut snapshot_progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: snapshot_ticket,
    };
    write_one_click_checkpoint(
        &config_dir,
        &snapshot_identity,
        &mut snapshot_progress,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
    )
    .unwrap();
    let retry_error = retry_pending_authority_cleanup(&state)
        .expect_err("an active authority transaction must preserve its recovery snapshot");
    assert!(
        retry_error
            .to_string()
            .contains("code=authority_snapshot_recovery_required")
            && backup_root.is_dir(),
        "active crash recovery must preserve the exact registered root: {retry_error}"
    );
    clear_one_click_transaction(&config_dir, &snapshot_identity, &mut snapshot_progress).unwrap();
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
    let panic_ticket = panic_snapshot.registered_snapshot_ticket().unwrap();
    let panic_identity = OneClickTransactionIdentity {
        target_profile_id: "snapshot-crash-fixture".into(),
        runtime_fingerprint: runtime_id,
        snapshot_ticket: panic_ticket.clone(),
        previous_binding: None,
        profile_switch_handoff: None,
        gateway_terminal_handoff: None,
        prior_stop: config::RuntimePriorStopState::NotRequired,
    };
    let mut panic_progress = OneClickJournalProgress::PreJournalAbort {
        registered_ticket: panic_ticket,
    };
    write_one_click_checkpoint(
        &config_dir,
        &panic_identity,
        &mut panic_progress,
        config::RuntimeTransactionPhase::AuthoritySnapshotActive,
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
        panic_retry
            .to_string()
            .contains("cleanup_code=authority_snapshot_recovery_required")
            && panic_retry
                .to_string()
                .contains(&panic_recovery_root.to_string_lossy().to_string())
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
