use super::*;

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
        assert_eq!(error.phase(), AuthorityCleanupPhase::Cleanup);
        let (recovery_path, cleanup_code) = error
            .cleanup_requirement()
            .expect("cleanup failure must carry typed recovery authority");
        assert_eq!(recovery_path, cleanup_sync_root);
        assert_eq!(cleanup_code, "cleanup_remove_failed");
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
    assert_eq!(
        finalize_registered_authority_cleanup(
            &cleanup_sync_snapshot.cleanup_context,
            &retry_ticket
        )
        .unwrap(),
        AuthorityCleanupOutcome::Cleared
    );
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
    assert_eq!(
        rebound_error.phase(),
        AuthorityCleanupPhase::IdentityValidation
    );
    assert!(rebound_error.cleanup_requirement().is_none());
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
    drop(_seam);

    let degraded_log = tmp.join("degraded-cleanup.log");
    let _degraded_seam =
        test_arm_authority_snapshot_cleanup_fault(tmp.clone(), "persistent", degraded_log);
    let mut degraded_snapshot =
        OneClickAuthoritySnapshot::capture(&config_dir, &sandbox_home, &auth_dir, &config, &state)
            .unwrap();
    let degraded_root = degraded_snapshot.backup_root.clone();
    let mut success_dto = serde_json::json!({
        "msg": "ready",
        "action": "started",
        "stage": "complete",
        "status": "ok",
        "recovery_status": "not_needed",
        "fallback_url": null
    });
    degraded_snapshot
        .prepare_success(&mut success_dto)
        .expect("cleanup-required must remain a successful degraded DTO");
    assert_eq!(success_dto["msg"], "ready");
    assert_eq!(success_dto["action"], "started");
    assert_eq!(success_dto["stage"], "complete");
    assert_eq!(success_dto["status"], "degraded");
    assert_eq!(success_dto["recovery_status"], "cleanup_required");
    assert_eq!(
        success_dto["cleanup_recovery_path"],
        degraded_root.to_string_lossy().as_ref()
    );
    assert_eq!(
        success_dto["cleanup_message"],
        "one-click 已完成，但私有事务快照需要稍后安全清理。"
    );
    assert!(success_dto["fallback_url"].is_null());
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
    assert_eq!(retry_error.phase(), AuthorityCleanupPhase::Retry);
    let (retry_path, retry_code) = retry_error
        .cleanup_requirement()
        .expect("active recovery refusal must carry typed retry authority");
    assert_eq!(retry_path, backup_root);
    assert_eq!(retry_code, "authority_snapshot_recovery_required");
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
