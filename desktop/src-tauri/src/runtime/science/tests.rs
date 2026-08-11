use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus, Output};
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::{
    bind_selected_science_runtime_attempt, classify_known_runtime_state, classify_sandbox_state,
    fingerprint_sha256_hex, first_http_url, managed_launch_id, managed_launch_path,
    managed_launch_record_for, mark_science_runtime_adoption_finalized,
    mark_science_runtime_adoption_launch_committed,
    official_updated_embedded_identity_metadata_matches, official_updated_science_bin_for_home,
    official_updated_snapshot_for_home, official_updated_snapshot_from_process_paths,
    parse_unique_listener_pid, prior_restart_receipt_is_absent_at, probe_sandbox_runtime_cached,
    read_managed_launch_record, read_science_adoption_ledger_at,
    record_deferred_science_runtime_candidate, restore_unmatched_managed_launch_tombstone,
    runtime_identity, runtime_identity_is_current, runtime_status_value,
    runtime_status_with_timeout, safe_science_version_with_timeout, sandbox_data_dir, sandbox_home,
    sandbox_running_ours, sandbox_url, sandbox_url_with_timeout, science_executable_fingerprint,
    science_post_term_action, science_runtime_preflight_for_paths,
    science_runtime_preflight_for_paths_cached, science_runtime_preflight_for_paths_with_updated,
    science_status_running, secure_runtime_snapshot_root, select_science_runtime_for_paths,
    select_science_runtime_for_paths_cached, select_science_runtime_for_paths_with_updated,
    settings_change_needs_teardown, stop_runtime_from_probe, test_process_start_identity_for_pid,
    test_runtime_identity, trusted_science_status, SandboxScienceState, ScienceAdoptionDecision,
    ScienceAdoptionMilestone, ScienceObservationField, SciencePostTermAction,
    ScienceRuntimeIdentity, ScienceRuntimeSource, ScienceStopCommandOutcome, ScienceStopFailure,
    ScienceStopFailureKind, ScienceVersionCache, VerifiedScienceStop, CACHED_ONCE_CHOICE,
    MANAGED_LAUNCH_LAST_READ_BYTES, MAX_MANAGED_LAUNCH_BYTES, MAX_SCIENCE_ADOPTION_LEDGER_BYTES,
    SCIENCE_ADOPTION_LEDGER_FILE, SCIENCE_ADOPTION_STORE_DIR,
};

#[test]
fn managed_process_start_identity_preserves_the_legacy_receipt_format() {
    const CHILD_ENV: &str = "CSSWITCH_TEST_PROCESS_START_TZ_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "runtime::science::tests::managed_process_start_identity_preserves_the_legacy_receipt_format",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .env("TZ", "UTC")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("hostile-TZ test child should start");
        assert!(status.success());
        return;
    }
    let pid = std::process::id();
    let direct = test_process_start_identity_for_pid(pid)
        .expect("proc_pidinfo should identify the current process");
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "lstart="])
        .env_clear()
        .output()
        .expect("ps should provide the legacy receipt representation");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(direct, String::from_utf8(output.stdout).unwrap().trim());
    assert!(test_process_start_identity_for_pid(0).is_none());
}

#[test]
fn o1_e3_prior_restart_receipt_uses_the_durable_launch_id() -> Result<(), Box<dyn std::error::Error>>
{
    let durable_launch_id = "o1-e3-durable-restart-identity";
    assert_eq!(
        managed_launch_id(Some(durable_launch_id)),
        durable_launch_id
    );
    assert_ne!(managed_launch_id(None), durable_launch_id);
    Ok(())
}

#[test]
fn o1_e3_prior_restart_requires_the_stopped_receipt_to_be_absent() {
    let root = unique_temp_dir("o1-e3-prior-receipt-absence").unwrap();
    let runtime_path = root.join("science");
    fs::write(&runtime_path, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&runtime_path, fs::Permissions::from_mode(0o700)).unwrap();
    let runtime = test_runtime_identity(runtime_path.clone());
    let recipe = crate::config::RuntimePriorScienceRecipe {
        port: 18992,
        runtime_path,
        runtime_source: runtime.source.code().to_string(),
        runtime_version: runtime.version.clone(),
        runtime_fingerprint: runtime.environment_transaction_id(),
        runtime_adoption_attempt_id: None,
        launch_receipt_digest: "a".repeat(64),
    };
    let receipt = root.join("science-managed-launch.v1.json");
    assert!(prior_restart_receipt_is_absent_at(
        &recipe, &runtime, &receipt
    ));
    fs::write(&receipt, b"replacement").unwrap();
    assert!(!prior_restart_receipt_is_absent_at(
        &recipe, &runtime, &receipt
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn official_updater_identity_parser_accepts_only_known_exact_variants() {
    let standalone = "Identifier=com.anthropic.operon\nTeamIdentifier=Q6L2SF6YDW\n";
    assert!(official_updated_embedded_identity_metadata_matches(
        standalone
    ));

    let dmg_seed = "Identifier=com.anthropic.operon.cli\nTeamIdentifier=Q6L2SF6YDW\n";
    assert!(official_updated_embedded_identity_metadata_matches(
        dmg_seed
    ));
    assert!(!official_updated_embedded_identity_metadata_matches(
        "Identifier=com.anthropic.operon.other\nTeamIdentifier=Q6L2SF6YDW\n",
    ));
    assert!(!official_updated_embedded_identity_metadata_matches(
        "Identifier=com.anthropic.operon\nTeamIdentifier=WRONG\n",
    ));
    assert!(!official_updated_embedded_identity_metadata_matches(
        "prefix-Identifier=com.anthropic.operon\nTeamIdentifier=Q6L2SF6YDW\n",
    ));
    assert!(!official_updated_embedded_identity_metadata_matches(
        "Identifier=com.anthropic.operon\nTeamIdentifier=Q6L2SF6YDW-suffix\n",
    ));
}

#[test]
fn science_runtime_adoption_record_is_private_bounded_and_milestone_ordered(
) -> Result<(), Box<dyn std::error::Error>> {
    const CHILD_ENV: &str = "CSSWITCH_TEST_SCIENCE_ADOPTION_RECORD_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let root = unique_temp_dir("science-adoption-record")?;
        let home = root.join("home");
        fs::create_dir_all(&home)?;
        let output = Command::new(std::env::current_exe()?)
            .arg("--exact")
            .arg("runtime::science::tests::science_runtime_adoption_record_is_private_bounded_and_milestone_ordered")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env("HOME", &home)
            .env_remove("SCIENCE_BIN")
            .output()?;
        let _ = fs::remove_dir_all(&root);
        assert!(
            output.status.success(),
            "isolated Science adoption oracle failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    let config_dir = crate::config::default_dir();
    fs::create_dir_all(&config_dir)?;
    fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700))?;
    let data_dir = sandbox_data_dir();
    fs::create_dir_all(&data_dir)?;
    fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o700))?;
    let running_bin = config_dir.join("running-science");
    let candidate_bin = config_dir.join("candidate-science");
    write_fake_version_bin(&running_bin, 0o700, "claude-science 1.0")?;
    write_fake_version_bin(&candidate_bin, 0o700, "claude-science 2.0")?;
    let cache = ScienceVersionCache::default();
    let running = runtime_identity(
        running_bin.clone(),
        ScienceRuntimeSource::InstalledApp,
        &cache,
    )
    .ok_or("running test runtime should be observable")?;

    std::env::set_var("SCIENCE_BIN", &candidate_bin);
    record_deferred_science_runtime_candidate(&running, &cache)?;
    record_deferred_science_runtime_candidate(&running, &cache)?;
    let store = config_dir.join(SCIENCE_ADOPTION_STORE_DIR);
    let ledger = read_science_adoption_ledger_at(&store)?;
    assert_eq!(
        ledger.attempts.len(),
        1,
        "identical healthy defer must deduplicate"
    );
    assert_eq!(
        ledger.attempts[0].decision,
        ScienceAdoptionDecision::DeferredHealthy
    );
    assert_eq!(
        ledger.attempts[0].predecessor.as_ref().unwrap().version,
        "claude-science 1.0"
    );
    assert_eq!(
        ledger.attempts[0].candidate.as_ref().unwrap().version,
        "claude-science 2.0"
    );

    std::env::set_var("SCIENCE_BIN", "relative-science");
    record_deferred_science_runtime_candidate(&running, &cache)?;
    let ledger = read_science_adoption_ledger_at(&store)?;
    assert_eq!(ledger.attempts.len(), 2);
    assert_eq!(
        ledger.attempts[1].decision,
        ScienceAdoptionDecision::Rejected
    );
    assert!(ledger.attempts[1].candidate.is_none());
    assert_eq!(
        ledger.attempts[1].rejected_source.as_deref(),
        Some("explicit")
    );
    std::env::remove_var("SCIENCE_BIN");

    let mut selected_runtime = runtime_identity(
        candidate_bin.clone(),
        ScienceRuntimeSource::Explicit,
        &cache,
    )
    .ok_or("selected test runtime should be observable")?;
    bind_selected_science_runtime_attempt(&mut selected_runtime)?;
    let attempt_id = selected_runtime
        .adoption_attempt_id()
        .ok_or("selected runtime should carry its adoption attempt")?
        .to_string();
    assert!(mark_science_runtime_adoption_finalized(&selected_runtime).is_err());
    mark_science_runtime_adoption_launch_committed(&attempt_id, &selected_runtime)?;
    mark_science_runtime_adoption_finalized(&selected_runtime)?;
    let ledger = read_science_adoption_ledger_at(&store)?;
    let selected = ledger
        .attempts
        .iter()
        .find(|attempt| attempt.attempt_id == attempt_id)
        .ok_or("selected attempt should remain in the ledger")?;
    assert_eq!(selected.decision, ScienceAdoptionDecision::Selected);
    assert_eq!(selected.milestone, ScienceAdoptionMilestone::Finalized);
    assert_eq!(
        selected
            .predecessor
            .as_ref()
            .map(|value| value.version.as_str()),
        Some("claude-science 1.0"),
        "selected adoption should carry forward the healthy-defer predecessor"
    );

    let receipt_v2 = managed_launch_record_for(
        8990,
        std::process::id(),
        &selected_runtime,
        Some("science-adoption-test-launch"),
        Some(&attempt_id),
    )
    .ok_or("managed receipt v2 should be constructible")?;
    assert_eq!(receipt_v2.schema_version, 2);
    assert_eq!(receipt_v2.runtime_source.as_deref(), Some("explicit"));
    assert_eq!(
        receipt_v2.runtime_version.as_deref(),
        Some("claude-science 2.0")
    );
    assert_eq!(
        receipt_v2.adoption_attempt_id.as_deref(),
        Some(attempt_id.as_str())
    );
    assert!(super::record_matches_runtime(
        &receipt_v2,
        8990,
        &selected_runtime
    ));

    let selected_count = ledger
        .attempts
        .iter()
        .filter(|attempt| attempt.decision == ScienceAdoptionDecision::Selected)
        .count();
    let mut same_runtime_restart = runtime_identity(
        candidate_bin.clone(),
        ScienceRuntimeSource::Explicit,
        &cache,
    )
    .ok_or("same-runtime restart should remain observable")?;
    bind_selected_science_runtime_attempt(&mut same_runtime_restart)?;
    assert_eq!(
        same_runtime_restart.adoption_attempt_id(),
        Some(attempt_id.as_str()),
        "ordinary restart of the latest finalized runtime must reuse its adoption attempt"
    );
    assert_eq!(
        read_science_adoption_ledger_at(&store)?
            .attempts
            .iter()
            .filter(|attempt| attempt.decision == ScienceAdoptionDecision::Selected)
            .count(),
        selected_count,
        "same-runtime restart must not append a duplicate selected attempt"
    );

    let mut intermediate_runtime = runtime_identity(
        running_bin.clone(),
        ScienceRuntimeSource::InstalledApp,
        &cache,
    )
    .ok_or("intermediate runtime should remain observable")?;
    bind_selected_science_runtime_attempt(&mut intermediate_runtime)?;
    let intermediate_attempt_id = intermediate_runtime
        .adoption_attempt_id()
        .ok_or("intermediate runtime should carry an adoption attempt")?
        .to_string();
    let mut crash_retry_runtime = runtime_identity(
        running_bin.clone(),
        ScienceRuntimeSource::InstalledApp,
        &cache,
    )
    .ok_or("crash retry runtime should remain observable")?;
    bind_selected_science_runtime_attempt(&mut crash_retry_runtime)?;
    assert_eq!(
        crash_retry_runtime.adoption_attempt_id(),
        Some(intermediate_attempt_id.as_str()),
        "unfinished retry of the latest selected runtime must reuse the same attempt"
    );
    mark_science_runtime_adoption_launch_committed(
        &intermediate_attempt_id,
        &intermediate_runtime,
    )?;
    mark_science_runtime_adoption_finalized(&intermediate_runtime)?;

    let mut readopted_runtime = runtime_identity(
        candidate_bin.clone(),
        ScienceRuntimeSource::Explicit,
        &cache,
    )
    .ok_or("re-adopted runtime should remain observable")?;
    bind_selected_science_runtime_attempt(&mut readopted_runtime)?;
    let readopted_attempt_id = readopted_runtime
        .adoption_attempt_id()
        .ok_or("re-adopted runtime should carry a new attempt")?
        .to_string();
    assert_ne!(
        readopted_attempt_id, attempt_id,
        "A→B→A must not reuse the historical finalized A attempt"
    );
    let ledger = read_science_adoption_ledger_at(&store)?;
    let readopted = ledger
        .attempts
        .iter()
        .find(|attempt| attempt.attempt_id == readopted_attempt_id)
        .ok_or("re-adoption attempt should be appended")?;
    assert_eq!(
        readopted
            .predecessor
            .as_ref()
            .map(|value| value.version.as_str()),
        Some("claude-science 1.0"),
        "A→B→A must use the latest finalized B as predecessor"
    );
    assert!(
        readopted
            .normalized_diff
            .contains(&ScienceObservationField::Version),
        "A→B→A must retain the B→A normalized version diff"
    );

    let receipt_v1 = managed_launch_record_for(
        8990,
        std::process::id(),
        &selected_runtime,
        Some("science-adoption-test-launch"),
        None,
    )
    .ok_or("legacy managed receipt should remain constructible")?;
    let legacy_bytes = serde_json::to_vec(&receipt_v1)?;
    let legacy_roundtrip: super::ScienceManagedLaunchRecord =
        serde_json::from_slice(&legacy_bytes)?;
    assert_eq!(legacy_roundtrip.schema_version, 1);
    assert!(legacy_roundtrip.adoption_attempt_id.is_none());
    assert!(super::record_matches_runtime(
        &legacy_roundtrip,
        8990,
        &selected_runtime
    ));

    let ledger_path = store.join(SCIENCE_ADOPTION_LEDGER_FILE);
    let ledger_bytes = fs::read(&ledger_path)?;
    let ledger_text = String::from_utf8(ledger_bytes)?;
    assert!(!ledger_text.contains(running_bin.to_string_lossy().as_ref()));
    assert!(!ledger_text.contains(candidate_bin.to_string_lossy().as_ref()));
    assert!(!ledger_text.contains("orgs/"));
    assert_eq!(
        store.metadata()?.permissions().mode() & 0o777,
        0o700,
        "adoption store must be owner-only"
    );
    assert_eq!(
        ledger_path.metadata()?.permissions().mode() & 0o777,
        0o600,
        "adoption ledger must be owner-only"
    );
    assert!(ledger_path.metadata()?.len() <= MAX_SCIENCE_ADOPTION_LEDGER_BYTES);
    Ok(())
}

#[test]
fn science_runtime_adoption_store_rejects_symlink_and_oversized_ledger(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-adoption-store-guards")?;
    let target = root.join("target");
    fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))?;
    let linked = root.join("linked");
    symlink(&target, &linked)?;
    assert!(read_science_adoption_ledger_at(&linked).is_err());

    let store = root.join("store");
    fs::create_dir(&store)?;
    fs::set_permissions(&store, fs::Permissions::from_mode(0o700))?;
    let ledger = store.join(SCIENCE_ADOPTION_LEDGER_FILE);
    fs::write(
        &ledger,
        vec![b' '; (MAX_SCIENCE_ADOPTION_LEDGER_BYTES + 1) as usize],
    )?;
    fs::set_permissions(&ledger, fs::Permissions::from_mode(0o600))?;
    assert!(read_science_adoption_ledger_at(&store).is_err());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn science_runtime_adoption_recovery_is_exact_and_retains_compensation_references(
) -> Result<(), Box<dyn std::error::Error>> {
    const CHILD_ENV: &str = "CSSWITCH_TEST_SCIENCE_ADOPTION_RECOVERY_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let root = unique_temp_dir("science-adoption-recovery")?;
        let home = root.join("home");
        fs::create_dir_all(&home)?;
        let output = Command::new(std::env::current_exe()?)
            .arg("--exact")
            .arg("runtime::science::tests::science_runtime_adoption_recovery_is_exact_and_retains_compensation_references")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env("HOME", &home)
            .env_remove("SCIENCE_BIN")
            .output()?;
        let _ = fs::remove_dir_all(&root);
        assert!(
            output.status.success(),
            "isolated Science adoption recovery oracle failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    let config_dir = crate::config::default_dir();
    fs::create_dir_all(&config_dir)?;
    fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700))?;
    let data_dir = sandbox_data_dir();
    fs::create_dir_all(&data_dir)?;
    fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o700))?;
    let runtime_bin = config_dir.join("recovery-science");
    write_fake_version_bin(&runtime_bin, 0o700, "claude-science 3.0")?;
    let cache = ScienceVersionCache::default();
    let mut runtime = runtime_identity(runtime_bin, ScienceRuntimeSource::Explicit, &cache)
        .ok_or("recovery test runtime should be observable")?;
    bind_selected_science_runtime_attempt(&mut runtime)?;
    let attempt_id = runtime
        .adoption_attempt_id()
        .ok_or("selected recovery runtime should carry an attempt")?
        .to_string();
    let receipt = managed_launch_record_for(
        8991,
        std::process::id(),
        &runtime,
        Some("science-adoption-recovery-launch"),
        Some(&attempt_id),
    )
    .ok_or("recovery receipt should be constructible")?;
    super::write_managed_launch_record(&receipt)?;

    let mut recovered_runtime = runtime.clone();
    recovered_runtime.adoption_attempt_id = None;
    let recovered_token = super::ScienceManagedLaunchToken {
        record: receipt.clone(),
        receipt_file: None,
    };
    super::hydrate_runtime_adoption_from_managed_launch(&recovered_token, &mut recovered_runtime)?;
    assert_eq!(
        recovered_runtime.adoption_attempt_id(),
        Some(attempt_id.as_str()),
        "fresh V2 receipt recovery must hydrate the exact adoption attempt"
    );

    super::reconcile_current_science_runtime_adoption(&recovered_runtime, None)?;
    let store = config_dir.join(SCIENCE_ADOPTION_STORE_DIR);
    let ledger = read_science_adoption_ledger_at(&store)?;
    assert_eq!(
        ledger
            .attempts
            .iter()
            .find(|attempt| attempt.attempt_id == attempt_id)
            .map(|attempt| attempt.milestone),
        Some(ScienceAdoptionMilestone::LaunchCommitted),
        "receipt-only crash recovery must not finalize before binding commit"
    );
    let legacy_binding = crate::config::RuntimeBindingCommit {
        profile_id: "recovery-profile".into(),
        route_fp: "recovery-route".into(),
        catalog_fp: "recovery-catalog".into(),
        binding_fp: "recovery-binding".into(),
        science_adoption_attempt_id: None,
    };
    super::reconcile_current_science_runtime_adoption(&recovered_runtime, Some(&legacy_binding))?;
    let wrong_binding = crate::config::RuntimeBindingCommit {
        science_adoption_attempt_id: Some("f".repeat(32)),
        ..legacy_binding.clone()
    };
    super::reconcile_current_science_runtime_adoption(&recovered_runtime, Some(&wrong_binding))?;
    let ledger = read_science_adoption_ledger_at(&store)?;
    assert_eq!(
        ledger
            .attempts
            .iter()
            .find(|attempt| attempt.attempt_id == attempt_id)
            .map(|attempt| attempt.milestone),
        Some(ScienceAdoptionMilestone::LaunchCommitted),
        "absent or different binding provenance must not authorize finalize"
    );
    let exact_binding = crate::config::RuntimeBindingCommit {
        science_adoption_attempt_id: Some(attempt_id.clone()),
        ..legacy_binding
    };
    super::reconcile_current_science_runtime_adoption(&recovered_runtime, Some(&exact_binding))?;
    let ledger = read_science_adoption_ledger_at(&store)?;
    assert_eq!(
        ledger
            .attempts
            .iter()
            .find(|attempt| attempt.attempt_id == attempt_id)
            .map(|attempt| attempt.milestone),
        Some(ScienceAdoptionMilestone::Finalized)
    );

    let v2_receipt_free_token = super::ScienceManagedLaunchToken {
        record: receipt.clone(),
        receipt_file: None,
    };
    super::clear_managed_launch_identity(&v2_receipt_free_token, &runtime)?;
    assert!(managed_launch_path().symlink_metadata().is_err());

    crate::config::save_to(
        &config_dir,
        &crate::config::Config {
            runtime_binding: Some(exact_binding.clone()),
            ..Default::default()
        },
    )?;
    super::mutate_science_adoption_ledger(&store, |ledger| {
        for _ in 0..(super::MAX_SCIENCE_ADOPTION_ATTEMPTS + 8) {
            super::append_science_update_attempt(
                ledger,
                None,
                None,
                ScienceAdoptionDecision::Rejected,
                Some(super::ScienceAdoptionRejectionCode::RuntimeUnavailable),
                Some("installed_app".into()),
            );
        }
        Ok(())
    })?;
    let ledger = read_science_adoption_ledger_at(&store)?;
    assert_eq!(ledger.attempts.len(), super::MAX_SCIENCE_ADOPTION_ATTEMPTS);
    assert!(
        ledger
            .attempts
            .iter()
            .any(|attempt| attempt.attempt_id == attempt_id),
        "compaction must retain the finalized attempt referenced only by runtime binding"
    );

    let compensation = crate::config::RuntimeCompensationJournal {
        schema_version: crate::config::RUNTIME_COMPENSATION_SCHEMA_VERSION_V2,
        compensation_id: "science-adoption-retention".into(),
        target_profile_id: "retention-profile".into(),
        runtime_fingerprint: "a".repeat(64),
        snapshot_ticket: crate::config::RuntimeSnapshotTicket::verified(format!(
            ".one-click-rollback-{}",
            "b".repeat(32)
        ))?,
        state: crate::config::RuntimeCompensationState::InProgress,
        steps: crate::config::pending_one_click_compensation_steps(),
        science_adoption_attempt_ids: vec![attempt_id.clone()],
    };
    crate::config::save_to(
        &config_dir,
        &crate::config::Config {
            runtime_compensation: Some(compensation),
            ..Default::default()
        },
    )?;
    super::mutate_science_adoption_ledger(&store, |ledger| {
        for _ in 0..(super::MAX_SCIENCE_ADOPTION_ATTEMPTS + 8) {
            super::append_science_update_attempt(
                ledger,
                None,
                None,
                ScienceAdoptionDecision::Rejected,
                Some(super::ScienceAdoptionRejectionCode::RuntimeUnavailable),
                Some("installed_app".into()),
            );
        }
        Ok(())
    })?;
    let ledger = read_science_adoption_ledger_at(&store)?;
    assert_eq!(ledger.attempts.len(), super::MAX_SCIENCE_ADOPTION_ATTEMPTS);
    assert!(
        ledger
            .attempts
            .iter()
            .any(|attempt| attempt.attempt_id == attempt_id),
        "compaction must retain the finalized attempt referenced only by active compensation"
    );
    let before_failed_compaction = fs::read(store.join(SCIENCE_ADOPTION_LEDGER_FILE))?;
    let mut unknown_schema = serde_json::to_value(&receipt)?;
    unknown_schema["schema_version"] = serde_json::json!(3);
    let mut missing_attempt = serde_json::to_value(&receipt)?;
    missing_attempt
        .as_object_mut()
        .unwrap()
        .remove("adoption_attempt_id");
    let mut invalid_attempt = serde_json::to_value(&receipt)?;
    invalid_attempt["adoption_attempt_id"] = serde_json::json!("A".repeat(32));
    let invalid_receipts = [
        b"{invalid-managed-receipt".to_vec(),
        serde_json::to_vec(&unknown_schema)?,
        serde_json::to_vec(&missing_attempt)?,
        serde_json::to_vec(&invalid_attempt)?,
    ];
    let receipt_path = managed_launch_path();
    for invalid_receipt in invalid_receipts {
        fs::write(&receipt_path, invalid_receipt)?;
        fs::set_permissions(&receipt_path, fs::Permissions::from_mode(0o600))?;
        let failed_compaction = super::mutate_science_adoption_ledger(&store, |ledger| {
            super::append_science_update_attempt(
                ledger,
                None,
                None,
                ScienceAdoptionDecision::Rejected,
                Some(super::ScienceAdoptionRejectionCode::RuntimeUnavailable),
                Some("installed_app".into()),
            );
            Ok(())
        });
        assert!(
            failed_compaction
                .as_ref()
                .is_err_and(|error| error.contains("live receipt")),
            "invalid present receipt must fail compaction closed: {failed_compaction:?}"
        );
        assert_eq!(
            fs::read(store.join(SCIENCE_ADOPTION_LEDGER_FILE))?,
            before_failed_compaction,
            "failed closed compaction must preserve the previous ledger bytes"
        );
    }
    Ok(())
}

#[test]
fn managed_launch_tombstone_restore_uses_no_clobber_primitive() {
    let source = include_str!("managed_launch.rs");
    let start = source
        .find("fn restore_unmatched_managed_launch_tombstone")
        .expect("managed launch tombstone restore implementation must exist");
    let remainder = &source[start..];
    let end = remainder
        .find("\nfn clear_managed_launch_identity")
        .expect("managed launch tombstone restore implementation must remain bounded");
    let implementation = &remainder[..end];

    assert!(
            implementation.contains("fs::hard_link(tombstone, path)"),
            "managed launch tombstone restore must use hard_link as its final atomic no-clobber destination commit"
        );
    assert!(
        !implementation.contains("rename("),
        "managed launch tombstone restore must not retain a check-then-overwriting-rename path"
    );
}

#[test]
fn managed_launch_tombstone_restore_is_atomic_no_clobber() -> Result<(), Box<dyn std::error::Error>>
{
    let root = unique_temp_dir("science-managed-launch-restore-race")?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let tombstone = root.join("science-managed-launch.old.stopped");
    let receipt = root.join("science-managed-launch.v1.json");
    let old_receipt = b"old-private-managed-launch".to_vec();
    let new_receipt = b"new-concurrent-managed-launch".to_vec();
    fs::write(&tombstone, &old_receipt)?;
    fs::set_permissions(&tombstone, fs::Permissions::from_mode(0o600))?;
    let barrier = root.join("restore-barrier");
    fs::create_dir(&barrier)?;
    std::env::set_var("CSSWITCH_TEST_TOMBSTONE_RESTORE_BARRIER", &barrier);

    let receipt_for_writer = receipt.clone();
    let barrier_for_writer = barrier.clone();
    let new_receipt_for_writer = new_receipt.clone();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for _ in 0..500 {
            if barrier_for_writer.join("ready").is_file() {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&receipt_for_writer)?;
                file.write_all(&new_receipt_for_writer)?;
                file.sync_all()?;
                fs::write(barrier_for_writer.join("continue"), b"continue")?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "tombstone restore never reached the deterministic race barrier",
        ))
    });

    let restore = restore_unmatched_managed_launch_tombstone(&tombstone, &receipt);
    writer
        .join()
        .map_err(|_| "managed launch race writer panicked")??;
    std::env::remove_var("CSSWITCH_TEST_TOMBSTONE_RESTORE_BARRIER");
    let receipt_after = fs::read(&receipt)?;
    let tombstone_after = fs::read(&tombstone).ok();
    fs::remove_dir_all(&root)?;

    assert_eq!(
            restore.as_ref().map_err(String::as_str),
            Err(
                "Science managed launch 原子 no-clobber 恢复冲突；新记录与旧 tombstone 均已保留"
            ),
            "atomic no-clobber restore must report the conflict from its final no-replace commit primitive, not from another check before an overwriting rename"
        );
    assert_eq!(
        receipt_after, new_receipt,
        "atomic no-clobber restore must preserve the concurrently committed receipt byte-for-byte"
    );
    assert_eq!(
        tombstone_after.as_deref(),
        Some(old_receipt.as_slice()),
        "atomic no-clobber restore must retain the old tombstone on conflict"
    );
    Ok(())
}

#[test]
fn managed_launch_receipt_growth_read_is_hard_bounded() -> Result<(), Box<dyn std::error::Error>> {
    const CHILD_ENV: &str = "CSSWITCH_TEST_MANAGED_LAUNCH_GROWTH_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let root = unique_temp_dir("science-managed-launch-growth")?;
        let home = root.join("home");
        fs::create_dir_all(&home)?;
        let output = Command::new(std::env::current_exe()?)
            .arg("--exact")
            .arg("runtime::science::tests::managed_launch_receipt_growth_read_is_hard_bounded")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env("HOME", &home)
            .output()?;
        let _ = fs::remove_dir_all(&root);
        assert!(
            output.status.success(),
            "isolated bounded receipt oracle failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    let config_dir = crate::config::default_dir();
    fs::create_dir_all(&config_dir)?;
    fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700))?;
    let receipt = managed_launch_path();
    fs::write(&receipt, b"{}")?;
    fs::set_permissions(&receipt, fs::Permissions::from_mode(0o600))?;
    let barrier = config_dir.join("managed-launch-read-barrier");
    fs::create_dir_all(&barrier)?;
    std::env::set_var("CSSWITCH_TEST_MANAGED_LAUNCH_READ_BARRIER", &barrier);
    MANAGED_LAUNCH_LAST_READ_BYTES.store(0, Ordering::SeqCst);

    let receipt_for_writer = receipt.clone();
    let barrier_for_writer = barrier.clone();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for _ in 0..500 {
            if barrier_for_writer.join("ready").is_file() {
                let mut file = OpenOptions::new().append(true).open(&receipt_for_writer)?;
                file.write_all(&vec![b' '; 1024 * 1024])?;
                file.sync_all()?;
                fs::write(barrier_for_writer.join("continue"), b"continue")?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "managed launch reader never reached the deterministic barrier",
        ))
    });
    let _ = read_managed_launch_record();
    writer
        .join()
        .map_err(|_| "managed launch writer panicked")??;
    std::env::remove_var("CSSWITCH_TEST_MANAGED_LAUNCH_READ_BARRIER");
    let observed = MANAGED_LAUNCH_LAST_READ_BYTES.load(Ordering::SeqCst);
    assert!(
        observed <= MAX_MANAGED_LAUNCH_BYTES + 1,
        "managed launch reader consumed {observed} bytes after concurrent growth; hard cap is {}",
        MAX_MANAGED_LAUNCH_BYTES + 1
    );
    Ok(())
}

#[test]
fn fresh_restart_rejects_listener_without_managed_launch_identity(
) -> Result<(), Box<dyn std::error::Error>> {
    const CHILD_ENV: &str = "CSSWITCH_TEST_SCIENCE_REATTACH_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let root = unique_temp_dir("science-unmanaged-reattach")?;
        let home = root.join("home");
        fs::create_dir_all(&home)?;
        let output = Command::new(std::env::current_exe()?)
                .arg("--exact")
                .arg("runtime::science::tests::fresh_restart_rejects_listener_without_managed_launch_identity")
                .arg("--nocapture")
                .env(CHILD_ENV, "1")
                .env("HOME", &home)
                .output()?;
        let _ = fs::remove_dir_all(&root);
        assert!(
            output.status.success(),
            "isolated reattach oracle failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or("isolated HOME is required")?;
    let bin = home.join("bin").join("claude-science");
    let data_dir = sandbox_home().join(".claude-science");
    let state_dir = data_dir.join("fake-science");
    fs::create_dir_all(bin.parent().ok_or("fake bin parent is required")?)?;
    fs::create_dir_all(&state_dir)?;
    fs::write(
        &bin,
        r#"#!/bin/sh
set -eu
cmd="${1:-}"
if [ "$#" -gt 0 ]; then shift; fi
if [ "$cmd" = "--version" ]; then
  echo "claude-science unmanaged-reattach-test"
  exit 0
fi
data_dir=""
port=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --data-dir) data_dir="$2"; shift 2 ;;
    --port) port="$2"; shift 2 ;;
    *) shift ;;
  esac
done
state="$data_dir/fake-science"
mkdir -p "$state"
case "$cmd" in
  serve)
    python3 - "$port" "$state/pid" >/dev/null 2>&1 <<'PY' &
import http.server
import os
import socketserver
import sys
port = int(sys.argv[1])
pidfile = sys.argv[2]
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass
    def do_GET(self):
        if self.path.startswith("/health"):
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"status":"ok"}')
        else:
            self.send_response(404)
            self.end_headers()
socketserver.TCPServer.allow_reuse_address = True
with open(pidfile, "w", encoding="utf-8") as f:
    f.write(str(os.getpid()))
with socketserver.TCPServer(("127.0.0.1", port), Handler) as httpd:
    httpd.serve_forever()
PY
    ;;
  status)
    pid="$(cat "$state/pid" 2>/dev/null || true)"
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
      echo '{"running":true}'
    else
      echo '{"running":false}'
      exit 1
    fi
    ;;
  stop)
    pid="$(cat "$state/pid" 2>/dev/null || true)"
    if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; fi
    rm -f "$state/pid"
    ;;
  *) exit 2 ;;
esac
"#,
    )?;
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o700))?;
    let bin = bin.canonicalize()?;
    std::env::set_var("SCIENCE_BIN", &bin);
    std::env::set_var("CSSWITCH_TEST_FAKE_SCIENCE_IDENTITY", "1");

    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    drop(listener);
    let launch = Command::new(&bin)
        .arg("serve")
        .arg("--data-dir")
        .arg(&data_dir)
        .arg("--port")
        .arg(port.to_string())
        .status()?;
    assert!(launch.success());
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let observed = probe_sandbox_runtime_cached(port, &ScienceVersionCache::default())?;
    let listener_still_alive = TcpStream::connect(("127.0.0.1", port)).is_ok();
    let stopped = Command::new(&bin)
        .arg("stop")
        .arg("--data-dir")
        .arg(&data_dir)
        .status()?;
    assert!(stopped.success());

    assert_eq!(
            observed,
            (SandboxScienceState::Unknown, None),
            "a fresh Desktop state must not adopt a listener that has no persisted managed-launch identity"
        );
    assert!(
        listener_still_alive,
        "rejected adoption must not signal the unproven listener"
    );
    Ok(())
}

// ---------- P1-c: 端口变更是否需拆链路（纯函数，4 组合） ----------
#[test]
fn settings_teardown_when_any_port_changes() {
    assert!(
        !settings_change_needs_teardown(18991, 18991, 8990, 8990),
        "端口未变 → 不拆链路"
    );
    assert!(
        settings_change_needs_teardown(18991, 19000, 8990, 8990),
        "代理端口变 → 拆（旧代理绑旧端口、沙箱烘旧 URL）"
    );
    assert!(
        settings_change_needs_teardown(18991, 18991, 8990, 9000),
        "沙箱端口变 → 拆（旧沙箱在旧端口成孤儿）"
    );
    assert!(
        settings_change_needs_teardown(18991, 19000, 8990, 9000),
        "都变 → 拆"
    );
}

#[test]
fn science_version_probe_times_out_and_reaps_its_child() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-version-timeout")?;
    let binary = root.join("hanging-science");
    fs::write(
        &binary,
        "#!/bin/sh\nif [ \"${1:-}\" = \"--version\" ]; then exec /bin/sleep 60; fi\nexit 0\n",
    )?;
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    let started = std::time::Instant::now();
    assert_eq!(
        safe_science_version_with_timeout(&binary, std::time::Duration::from_millis(100)),
        None
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn science_version_probe_kills_descendant_before_it_can_act_after_return(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-version-descendant")?;
    let binary = root.join("forking-science");
    let started_marker = root.join("descendant-started");
    let late_marker = root.join("descendant-late");
    fs::write(
            &binary,
            format!(
                "#!/bin/sh\nif [ \"${{1:-}}\" = \"--version\" ]; then\n  ( : > '{}'; /bin/sleep 1; : > '{}' ) &\n  while [ ! -f '{}' ]; do /bin/sleep 0.01; done\n  printf '%s\\n' 'descendant-safe-v1'\n  exit 0\nfi\nexit 0\n",
                started_marker.display(),
                late_marker.display(),
                started_marker.display()
            ),
        )?;
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    let started = std::time::Instant::now();
    let version = safe_science_version_with_timeout(&binary, std::time::Duration::from_secs(15));
    assert_eq!(version.as_deref(), Some("descendant-safe-v1"));
    assert!(started.elapsed() < std::time::Duration::from_secs(17));
    assert!(
        started_marker.exists(),
        "the descendant must actually start"
    );
    std::thread::sleep(std::time::Duration::from_millis(1_200));
    assert!(
        !late_marker.exists(),
        "the descendant must not remain executable after the probe returns"
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn science_version_probe_rejects_oversize_output() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-version-oversize")?;
    let binary = root.join("oversize-science");
    fs::write(
            &binary,
            "#!/bin/sh\nif [ \"${1:-}\" = \"--version\" ]; then printf '%s\\n' 'valid-first-line'; printf '%01030d' 0; exit 0; fi\nexit 0\n",
        )?;
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    assert_eq!(
        safe_science_version_with_timeout(&binary, std::time::Duration::from_secs(2)),
        None,
        "a legal first line must not hide oversized trailing output"
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn science_control_helpers_clear_ambient_sensitive_environment(
) -> Result<(), Box<dyn std::error::Error>> {
    const CHILD_ENV: &str = "CSSWITCH_CONTROL_ENV_TEST_CHILD";
    const HOSTILE_ENV: &str = "CSSWITCH_TEST_HOSTILE_API_KEY";
    if std::env::var_os(CHILD_ENV).is_none() {
        let output = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "runtime::science::tests::science_control_helpers_clear_ambient_sensitive_environment",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .env(HOSTILE_ENV, "must-not-reach-science")
            .output()?;
        assert!(
            output.status.success(),
            "isolated hostile-environment control probe failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }

    let root = unique_temp_dir("science-control-env-clear")?;
    let binary = root.join("science");
    fs::write(
        &binary,
        format!(
            "#!/bin/sh\nif [ -n \"${{{HOSTILE_ENV}:-}}\" ]; then printf '%s\\n' leaked; exit 0; fi\ncase \"${{1:-}}\" in\n  --version) printf '%s\\n' safe-version ;;\n  status) printf '%s\\n' '{{\"running\":false}}' ;;\n  url) printf '%s\\n' 'http://127.0.0.1:19090/safe' ;;\nesac\n"
        ),
    )?;
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    let runtime = test_runtime_identity(binary.clone());

    assert_eq!(
        safe_science_version_with_timeout(&binary, super::SCIENCE_VERSION_TIMEOUT).as_deref(),
        Some("safe-version")
    );
    assert_eq!(
        runtime_status_with_timeout(&runtime, super::SCIENCE_CONTROL_TIMEOUT),
        Some(false)
    );
    assert_eq!(
        sandbox_url_with_timeout(19090, &runtime, super::SCIENCE_CONTROL_TIMEOUT),
        "http://127.0.0.1:19090/safe"
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn science_status_probe_times_out_and_reaps_direct_child() -> Result<(), Box<dyn std::error::Error>>
{
    let root = unique_temp_dir("science-status-timeout")?;
    let binary = root.join("science");
    fs::write(
        &binary,
        "#!/bin/sh\nif [ \"${1:-}\" = status ]; then exec /bin/sleep 60; fi\nexit 0\n",
    )?;
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    let runtime = test_runtime_identity(binary);
    let started = std::time::Instant::now();
    assert_eq!(
        runtime_status_with_timeout(&runtime, Duration::from_millis(100)),
        None
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn science_url_probe_kills_descendant_after_direct_parent_exits(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-url-descendant")?;
    let binary = root.join("science");
    let started_marker = root.join("descendant-started");
    let late_marker = root.join("descendant-late");
    fs::write(
        &binary,
        format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = url ]; then\n  ( : > '{}'; /bin/sleep 1; : > '{}' ) &\n  while [ ! -f '{}' ]; do /bin/sleep 0.01; done\n  printf '%s\\n' 'http://127.0.0.1:19091/safe'\n  exit 0\nfi\nexit 0\n",
            started_marker.display(),
            late_marker.display(),
            started_marker.display(),
        ),
    )?;
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
    let runtime = test_runtime_identity(binary);
    assert_eq!(
        sandbox_url_with_timeout(19091, &runtime, super::SCIENCE_CONTROL_TIMEOUT),
        "http://127.0.0.1:19091/safe"
    );
    assert!(started_marker.exists());
    std::thread::sleep(Duration::from_millis(1_200));
    assert!(
        !late_marker.exists(),
        "the bounded runner must kill descendants before returning"
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn first_http_url_takes_only_first_valid_url() {
    let multi = "http://127.0.0.1:8990/setup?nonce=abc123\n\
                     This is a single-use link, expires in 60 seconds.";
    assert_eq!(
        first_http_url(multi).as_deref(),
        Some("http://127.0.0.1:8990/setup?nonce=abc123"),
    );
    let inline = "https://x.example/y?z=1  (single-use)";
    assert_eq!(
        first_http_url(inline).as_deref(),
        Some("https://x.example/y?z=1")
    );
    let lead = "Open this link in your browser:\nhttp://127.0.0.1:8990/a";
    assert_eq!(
        first_http_url(lead).as_deref(),
        Some("http://127.0.0.1:8990/a")
    );
    assert_eq!(first_http_url("no url here\nnor here"), None);
    assert_eq!(
        first_http_url("http://127.0.0.1:8990").as_deref(),
        Some("http://127.0.0.1:8990")
    );
}

#[test]
fn listener_pid_parser_requires_one_safe_identity() {
    assert_eq!(parse_unique_listener_pid("preamble\n"), None);
    assert_eq!(parse_unique_listener_pid("1\n"), None);
    assert_eq!(parse_unique_listener_pid("42\n42\n"), Some(42));
    assert_eq!(parse_unique_listener_pid("42\n43\n"), None);
    assert_eq!(parse_unique_listener_pid("42\ninvalid\n"), None);
}

#[test]
fn version_cache_is_shared_and_invalidates_when_binary_changes(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-version-cache")?;
    let app_bin = root.join("claude-science");
    let count = root.join("version-count");
    write_counted_version_bin(&app_bin, &count, "claude-science cache-v1")?;
    let data_dir = root.join("data");
    fs::create_dir_all(&data_dir)?;
    let cache = ScienceVersionCache::default();

    let preflight =
        science_runtime_preflight_for_paths_cached(&data_dir, None, None, &app_bin, &cache)?;
    assert_eq!(preflight["selected_version"], "claude-science cache-v1");
    let selected =
        select_science_runtime_for_paths_cached(&data_dir, None, None, &app_bin, None, &cache)?;
    assert_eq!(selected.version.as_deref(), Some("claude-science cache-v1"));
    assert_eq!(fs::read_to_string(&count)?, "1");

    write_counted_version_bin(&app_bin, &count, "claude-science cache-version-two")?;
    let selected =
        select_science_runtime_for_paths_cached(&data_dir, None, None, &app_bin, None, &cache)?;
    assert_eq!(
        selected.version.as_deref(),
        Some("claude-science cache-version-two")
    );
    assert_eq!(fs::read_to_string(&count)?, "2");

    assert_eq!(
        cache.force_refresh(&app_bin).as_deref(),
        Some("claude-science cache-version-two")
    );
    assert_eq!(fs::read_to_string(&count)?, "3");
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn science_status_running_accepts_compact_and_spaced_json() {
    assert!(science_status_running(&status_output(
        0,
        r#"{"running":true}"#
    )));
    assert!(science_status_running(&status_output(
        0,
        r#"{"running": true}"#
    )));
    assert!(!science_status_running(&status_output(
        0,
        r#"{"running":false}"#
    )));
    assert!(!science_status_running(&status_output(0, "running")));
    assert!(!science_status_running(&status_output(
        1,
        r#"{"running": true}"#
    )));
}

#[test]
fn science_status_running_accepts_json_with_cli_text() {
    assert!(science_status_running(&status_output(
        0,
        "Claude Science status:\n{\"running\": true, \"port\": 8990}\nready"
    )));
    assert!(science_status_running(&status_output(
        0,
        "warning: {not-json}\n{\"state\":\"ok\"}\n{\"running\": true}"
    )));
    assert!(!science_status_running(&status_output(
        0,
        "warning\n{\"running\": false}\n{\"running\": true}"
    )));
}

#[test]
fn sandbox_state_classification_fails_closed_on_probe_disagreement() {
    assert_eq!(
        classify_sandbox_state(Some(true), true, true),
        SandboxScienceState::RunningHealthy
    );
    assert_eq!(
        classify_sandbox_state(Some(false), false, false),
        SandboxScienceState::Stopped
    );
    for state in [
        classify_sandbox_state(None, false, false),
        classify_sandbox_state(Some(true), false, true),
        classify_sandbox_state(Some(true), false, false),
        classify_sandbox_state(Some(false), true, true),
        classify_sandbox_state(Some(false), false, true),
    ] {
        assert_eq!(state, SandboxScienceState::Unknown);
    }
    assert_eq!(
        trusted_science_status(&status_output(1, r#"{"running":false}"#)),
        Some(false),
        "a stopped daemon may be reported with a non-zero CLI exit"
    );
}

#[test]
fn stop_probe_is_idempotent_only_for_confirmed_stopped_state() {
    assert_eq!(
        stop_runtime_from_probe(SandboxScienceState::Stopped, None).unwrap(),
        None
    );
    assert!(stop_runtime_from_probe(SandboxScienceState::Unknown, None).is_err());
    assert!(stop_runtime_from_probe(SandboxScienceState::RunningHealthy, None).is_err());
    assert_eq!(
        science_post_term_action(ScienceStopCommandOutcome::Success, false, false),
        SciencePostTermAction::Complete,
        "a closed port completes without requiring a still-live ownership token"
    );
    assert_eq!(
        science_post_term_action(ScienceStopCommandOutcome::Success, true, true),
        SciencePostTermAction::KillExact,
        "a surviving exact listener retains the historical KILL fallback"
    );
    assert_eq!(
        science_post_term_action(ScienceStopCommandOutcome::Success, true, false),
        SciencePostTermAction::IdentityDrift,
        "a surviving replacement listener must never be collapsed into exit-unconfirmed"
    );
    assert_eq!(
        science_post_term_action(ScienceStopCommandOutcome::NonZero, true, true),
        SciencePostTermAction::KillExact,
        "a non-zero CLI may fall back only to the still-current exact launch token"
    );
    assert_eq!(
        science_post_term_action(ScienceStopCommandOutcome::NonZero, false, false),
        SciencePostTermAction::Complete,
        "a non-zero CLI that already closed the port may proceed to receipt cleanup"
    );
    assert_eq!(
        science_post_term_action(ScienceStopCommandOutcome::NonZero, true, false),
        SciencePostTermAction::IdentityDrift,
        "a non-zero CLI must not signal a replacement listener"
    );
    assert_eq!(
        science_post_term_action(ScienceStopCommandOutcome::Unavailable, true, true),
        SciencePostTermAction::PreserveCommandFailure,
        "a missing or unspawnable stop command must retain its typed failure"
    );
    assert_eq!(
        science_post_term_action(ScienceStopCommandOutcome::Unavailable, false, false),
        SciencePostTermAction::PreserveCommandFailure,
        "an unavailable stop command is not greened by an unrelated closed-port observation"
    );

    let typed_failures = [
        (
            ScienceStopFailure::request_rejected("request"),
            ScienceStopFailureKind::RequestRejected,
            "request",
        ),
        (
            ScienceStopFailure::identity_drift("identity"),
            ScienceStopFailureKind::IdentityDrift,
            "identity",
        ),
        (
            ScienceStopFailure::stop_command_failed("command"),
            ScienceStopFailureKind::StopCommandFailed,
            "command",
        ),
        (
            ScienceStopFailure::signal_failure("signal"),
            ScienceStopFailureKind::SignalFailure,
            "signal",
        ),
        (
            ScienceStopFailure::exit_unconfirmed("exit"),
            ScienceStopFailureKind::ExitUnconfirmed,
            "exit",
        ),
        (
            ScienceStopFailure::receipt_cleanup_failure("receipt"),
            ScienceStopFailureKind::ReceiptCleanupFailure,
            "receipt",
        ),
        (
            ScienceStopFailure::outcome_publication_failure("outcome"),
            ScienceStopFailureKind::OutcomePublicationFailure,
            "outcome",
        ),
    ];
    for (failure, expected_kind, expected_message) in typed_failures {
        assert_eq!(failure.kind(), expected_kind);
        assert_eq!(failure.message(), expected_message);
        assert_eq!(failure.to_string(), expected_message);
    }

    let root = unique_temp_dir("science-stop-proof-projection").unwrap();
    let bin = root.join("claude-science");
    write_fake_bin(&bin, 0o755).unwrap();
    let runtime = ScienceRuntimeIdentity {
        path: bin.clone(),
        source: ScienceRuntimeSource::InstalledApp,
        version: None,
        fingerprint: science_executable_fingerprint(&bin).unwrap(),
        adoption_attempt_id: None,
    };
    let unproven = VerifiedScienceStop {
        runtime: None,
        ownership_was_proven: false,
    };
    assert!(unproven.confirmed_runtime().is_none());
    assert_eq!(
        unproven.require_exact_stop_of(&runtime).unwrap_err().kind(),
        ScienceStopFailureKind::IdentityDrift,
        "a closed port without ownership proof must not satisfy an exact cleanup"
    );
    let proven = VerifiedScienceStop {
        runtime: Some(runtime.clone()),
        ownership_was_proven: true,
    };
    assert!(proven.proves_exact_stop_of(&runtime));
    assert_eq!(proven.confirmed_runtime(), Some(&runtime));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn known_runtime_state_requires_listener_binary_match() {
    assert_eq!(
        classify_known_runtime_state(Some(true), true, true, true),
        SandboxScienceState::RunningHealthy
    );
    assert_eq!(
        classify_known_runtime_state(Some(true), true, true, false),
        SandboxScienceState::Unknown
    );
    assert_eq!(
        runtime_status_value(&status_output(1, r#"{"running":false}"#)),
        Some(false),
        "a selected runtime may report stopped with a non-zero CLI exit"
    );
    assert_eq!(
        runtime_status_value(&status_output(1, r#"{"running":true}"#)),
        None,
        "a non-zero positive status is never trusted"
    );
}

#[test]
fn known_runtime_classifier_requires_listener_binary_identity() {
    assert_eq!(
        classify_known_runtime_state(Some(true), true, true, true),
        SandboxScienceState::RunningHealthy
    );
    assert_eq!(
        classify_known_runtime_state(Some(true), true, true, false),
        SandboxScienceState::Unknown
    );
}

#[test]
fn runtime_selection_requires_explicit_one_shot_cache_choice(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-bin-selection")?;
    let data_dir = root.join("home").join(".claude-science");
    let explicit_bin = root.join("explicit-claude-science");
    let cached_bin = data_dir.join("bin").join("claude-science");
    let app_bin = root.join("app-claude-science");

    write_fake_version_bin(&explicit_bin, 0o755, "fake-explicit-1")?;
    write_fake_version_bin(&cached_bin, 0o755, "fake-cache-1")?;
    write_fake_version_bin(&app_bin, 0o755, "fake-app-1")?;
    let preflight = science_runtime_preflight_for_paths(&data_dir, Some(&explicit_bin), &app_bin)?;
    assert_eq!(preflight["status"], "installed_ready");
    assert_eq!(preflight["selected_source"], "explicit");
    assert_eq!(preflight["selected_version"], "fake-explicit-1");
    assert_eq!(
        select_science_runtime_for_paths(
            &data_dir,
            Some(&explicit_bin),
            &app_bin,
            Some(CACHED_ONCE_CHOICE),
        )?
        .path,
        explicit_bin,
        "a valid explicit development override wins even if cache was authorized"
    );

    fs::set_permissions(&explicit_bin, fs::Permissions::from_mode(0o644))?;
    assert!(
        select_science_runtime_for_paths(&data_dir, Some(&explicit_bin), &app_bin, None).is_err(),
        "an invalid explicit override must not fall through to sandbox or system Science"
    );

    let app =
        select_science_runtime_for_paths(&data_dir, None, &app_bin, Some(CACHED_ONCE_CHOICE))?;
    assert_eq!(
        app.path, app_bin,
        "the installed Science app always wins over an old cache"
    );
    assert_eq!(app.source, ScienceRuntimeSource::InstalledApp);
    assert_eq!(app.version.as_deref(), Some("fake-app-1"));

    let explicit_link = root.join("explicit-link");
    symlink(&app_bin, &explicit_link)?;
    assert!(
        select_science_runtime_for_paths(&data_dir, Some(&explicit_link), &app_bin, None).is_err(),
        "an explicit symlink must fail closed"
    );

    let real_parent = root.join("real-parent");
    let linked_parent = root.join("linked-parent");
    let parent_bin = real_parent.join("claude-science");
    write_fake_version_bin(&parent_bin, 0o755, "fake-parent-1")?;
    symlink(&real_parent, &linked_parent)?;
    assert!(
        select_science_runtime_for_paths(
            &data_dir,
            Some(&linked_parent.join("claude-science")),
            &app_bin,
            None,
        )
        .is_err(),
        "an explicit path with a symlinked parent must fail closed"
    );

    write_fake_bin(&app_bin, 0o755)?;
    let failed_app_preflight = science_runtime_preflight_for_paths(&data_dir, None, &app_bin)?;
    assert_eq!(failed_app_preflight["status"], "cached_choice_required");
    assert!(
        select_science_runtime_for_paths(&data_dir, None, &app_bin, None)
            .expect_err("failed App preflight must offer, not implicitly use, cache")
            .contains("SCIENCE_RUNTIME_CHOICE_REQUIRED")
    );

    fs::set_permissions(&app_bin, fs::Permissions::from_mode(0o644))?;
    let preflight = science_runtime_preflight_for_paths(&data_dir, None, &app_bin)?;
    assert_eq!(preflight["status"], "cached_choice_required");
    assert_eq!(preflight["cached_version"], "fake-cache-1");
    let no_choice = select_science_runtime_for_paths(&data_dir, None, &app_bin, None)
        .expect_err("cache must not launch without one-shot authorization");
    assert!(no_choice.contains("SCIENCE_RUNTIME_CHOICE_REQUIRED"));
    let cached =
        select_science_runtime_for_paths(&data_dir, None, &app_bin, Some(CACHED_ONCE_CHOICE))?;
    assert_eq!(cached.path, cached_bin);
    assert_eq!(cached.source, ScienceRuntimeSource::CachedOnce);
    assert_eq!(cached.version.as_deref(), Some("fake-cache-1"));

    write_fake_bin(&cached_bin, 0o755)?;
    let preflight = science_runtime_preflight_for_paths(&data_dir, None, &app_bin)?;
    assert_eq!(preflight["status"], "missing");
    assert!(
        select_science_runtime_for_paths(&data_dir, None, &app_bin, Some(CACHED_ONCE_CHOICE),)
            .is_err()
    );

    fs::set_permissions(&cached_bin, fs::Permissions::from_mode(0o644))?;
    assert_eq!(
        science_runtime_preflight_for_paths(&data_dir, None, &app_bin)?["status"],
        "missing"
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn locally_validated_updated_runtime_wins_over_app_and_is_fingerprint_bound(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-official-updated")?;
    let home = root.join("home");
    let data_dir = home.join(".csswitch/sandbox/home/.claude-science");
    let updated_bin = home.join(".claude-science/bin/claude-science");
    let app_bin = root.join("app-claude-science");
    let explicit_bin = root.join("explicit-claude-science");
    write_fake_version_bin(&updated_bin, 0o755, "fake-official-updated-v2")?;
    {
        let mut file = fs::OpenOptions::new().append(true).open(&updated_bin)?;
        file.write_all(b"\n#")?;
        file.write_all(&vec![b' '; super::MIN_SCIENCE_BINARY_SIZE as usize])?;
    }
    write_fake_version_bin(&app_bin, 0o755, "fake-app-v1")?;
    write_fake_version_bin(&explicit_bin, 0o755, "fake-explicit-v3")?;

    assert_eq!(
        official_updated_science_bin_for_home(&home, false).as_deref(),
        Some(updated_bin.as_path())
    );
    assert!(official_updated_science_bin_for_home(&home, true).is_none());

    let snapshot =
        official_updated_snapshot_for_home(&home, &root.join("runtime-snapshots/science"), false)?
            .expect("updated snapshot");
    assert_ne!(snapshot, updated_bin);

    let preflight = science_runtime_preflight_for_paths_with_updated(
        &data_dir,
        None,
        Some(&snapshot),
        &app_bin,
    )
    .expect("the locally validated updater snapshot must pass preflight");
    assert_eq!(preflight["selected_source"], "official_updated");
    assert_eq!(preflight["selected_version"], "fake-official-updated-v2");

    let selected = select_science_runtime_for_paths_with_updated(
        &data_dir,
        None,
        Some(&snapshot),
        &app_bin,
        None,
    )
    .expect("the locally validated updater snapshot must be selected");
    assert_eq!(selected.path, snapshot);
    assert_eq!(selected.source, ScienceRuntimeSource::OfficialUpdated);
    assert!(runtime_identity_is_current(&selected));

    let explicit = select_science_runtime_for_paths_with_updated(
        &data_dir,
        Some(&explicit_bin),
        Some(&selected.path),
        &app_bin,
        None,
    )
    .expect("a valid explicit override must still take priority");
    assert_eq!(explicit.source, ScienceRuntimeSource::Explicit);

    write_fake_version_bin(&updated_bin, 0o755, "fake-official-updated-v3")?;
    assert!(
        runtime_identity_is_current(&selected),
        "an updater replacement must not change the running snapshot identity"
    );
    fs::set_permissions(&selected.path, fs::Permissions::from_mode(0o700))?;
    write_fake_version_bin(&selected.path, 0o755, "fake-snapshot-tampered")?;
    assert!(!runtime_identity_is_current(&selected));

    write_fake_bin(&selected.path, 0o755)?;
    let preflight_error = science_runtime_preflight_for_paths_with_updated(
        &data_dir,
        None,
        Some(&selected.path),
        &app_bin,
    )
    .expect_err("an invalid updater snapshot must not fall through to the App seed");
    assert!(preflight_error.contains("已拒绝回退旧 App"));
    let selection_error = select_science_runtime_for_paths_with_updated(
        &data_dir,
        None,
        Some(&selected.path),
        &app_bin,
        None,
    )
    .expect_err("an invalid updater snapshot must not select the App seed");
    assert!(selection_error.contains("已拒绝回退旧 App"));

    fs::set_permissions(
        home.join(".claude-science/bin"),
        fs::Permissions::from_mode(0o775),
    )?;
    assert!(official_updated_science_bin_for_home(&home, false).is_none());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn historical_updated_snapshots_remain_recoverable_after_source_replacement(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-updated-snapshot-recovery")?;
    let home = root.join("home");
    let candidate = home.join(".claude-science/bin/claude-science");
    let snapshots = root.join("runtime-snapshots/science");
    write_padded_fake_version_bin(&candidate, "fake-updater-a")?;
    let snapshot_a =
        official_updated_snapshot_for_home(&home, &snapshots, false)?.expect("snapshot A");

    write_padded_fake_version_bin(&candidate, "fake-updater-b")?;
    let snapshot_b =
        official_updated_snapshot_for_home(&home, &snapshots, false)?.expect("snapshot B");
    assert_ne!(snapshot_a, snapshot_b);

    let unrelated = root.join("not-a-runtime");
    write_padded_fake_version_bin(&unrelated, "unrelated")?;
    assert_eq!(
        official_updated_snapshot_from_process_paths(
            &snapshots,
            &[unrelated, snapshot_a.clone()],
            false,
        )?,
        Some(snapshot_a),
        "recovery validates only the executable reported for the live listener"
    );
    assert_eq!(
        official_updated_snapshot_from_process_paths(
            &snapshots,
            std::slice::from_ref(&snapshot_b),
            false,
        )?,
        Some(snapshot_b)
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn snapshot_root_symlink_is_rejected_without_mutating_target(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-snapshot-root-symlink")?;
    let target = root.join("target");
    fs::create_dir(&target)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
    let link = root.join("snapshot-root");
    symlink(&target, &link)?;

    assert!(secure_runtime_snapshot_root(&link).is_err());
    assert_eq!(
        fs::symlink_metadata(&target)?.permissions().mode() & 0o777,
        0o755
    );
    assert!(target.read_dir()?.next().is_none());

    let ancestor_target = root.join("ancestor-target");
    fs::create_dir(&ancestor_target)?;
    fs::set_permissions(&ancestor_target, fs::Permissions::from_mode(0o755))?;
    let linked_ancestor = root.join("linked-ancestor");
    symlink(&ancestor_target, &linked_ancestor)?;
    assert!(secure_runtime_snapshot_root(&linked_ancestor.join("science")).is_err());
    assert_eq!(
        fs::symlink_metadata(&ancestor_target)?.permissions().mode() & 0o777,
        0o755
    );
    assert!(ancestor_target.read_dir()?.next().is_none());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
#[ignore = "requires CSSWITCH_REAL_SCIENCE_BIN plus expected version and SHA-256"]
fn real_updated_runtime_candidate_is_eligible_without_reading_real_science_data(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = std::env::var_os("CSSWITCH_REAL_SCIENCE_BIN")
        .map(std::path::PathBuf::from)
        .ok_or("CSSWITCH_REAL_SCIENCE_BIN is required")?;
    let expected_version = std::env::var("CSSWITCH_EXPECTED_SCIENCE_VERSION")?;
    let expected_sha256 = std::env::var("CSSWITCH_EXPECTED_SCIENCE_SHA256")?;
    let root = unique_temp_dir("science-real-updated")?;
    let home = root.join("home");
    let candidate = home.join(".claude-science/bin/claude-science");
    fs::create_dir_all(candidate.parent().expect("candidate parent"))?;
    fs::copy(&source, &candidate)?;
    fs::set_permissions(&candidate, fs::Permissions::from_mode(0o755))?;

    assert_eq!(
            official_updated_science_bin_for_home(&home, true).as_deref(),
            Some(candidate.as_path()),
            "the fixed updater executable should pass the same local identity guards used in production"
        );
    let snapshot =
        official_updated_snapshot_for_home(&home, &root.join("runtime-snapshots/science"), true)?
            .expect("real updater snapshot");
    assert_ne!(snapshot, candidate);
    assert_eq!(fs::read(&snapshot)?, fs::read(&candidate)?);
    let expected_snapshot_name = format!("claude-science-{expected_sha256}");
    assert_eq!(
        snapshot.file_name().and_then(|name| name.to_str()),
        Some(expected_snapshot_name.as_str())
    );
    let isolated_home = root.join("isolated-home");
    fs::create_dir_all(&isolated_home)?;
    let output = std::process::Command::new(&snapshot)
        .arg("--version")
        .env("HOME", &isolated_home)
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?.trim(),
        format!("claude-science {expected_version} (release, public)")
    );

    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
#[ignore = "requires CSSWITCH_REAL_SCIENCE_APP_BIN plus expected version and SHA-256"]
fn real_installed_app_seed_is_selected_without_mutating_data_dir(
) -> Result<(), Box<dyn std::error::Error>> {
    let app_bin = std::env::var_os("CSSWITCH_REAL_SCIENCE_APP_BIN")
        .map(std::path::PathBuf::from)
        .ok_or("CSSWITCH_REAL_SCIENCE_APP_BIN is required")?;
    let expected_version = std::env::var("CSSWITCH_EXPECTED_SCIENCE_VERSION")?;
    let expected_sha256 = std::env::var("CSSWITCH_EXPECTED_SCIENCE_SHA256")?;
    let root = unique_temp_dir("science-real-installed-app")?;
    let data_dir = root.join("home/.claude-science");
    fs::create_dir_all(&data_dir)?;
    let marker = data_dir.join("state-marker");
    fs::write(&marker, "keep-me")?;

    let selected = select_science_runtime_for_paths(&data_dir, None, &app_bin, None)?;
    assert_eq!(selected.source, ScienceRuntimeSource::InstalledApp);
    assert_eq!(selected.path, app_bin);
    let expected_version_output = format!("claude-science {expected_version} (release, public)");
    assert_eq!(
        selected.version.as_deref(),
        Some(expected_version_output.as_str())
    );
    assert_eq!(
        fingerprint_sha256_hex(&selected.fingerprint),
        expected_sha256
    );
    assert_eq!(fs::read_to_string(&marker)?, "keep-me");
    assert_eq!(data_dir.read_dir()?.count(), 1);

    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn cached_runtime_symlink_is_never_offered_or_executed() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-cache-symlink")?;
    let data_dir = root.join("home").join(".claude-science");
    let cached_bin = data_dir.join("bin").join("claude-science");
    let target = root.join("target-claude-science");
    let missing_app = root.join("missing-app-claude-science");
    write_fake_version_bin(&target, 0o755, "fake-target-1")?;
    fs::create_dir_all(cached_bin.parent().expect("cached parent"))?;
    symlink(&target, &cached_bin)?;

    let preflight = science_runtime_preflight_for_paths(&data_dir, None, &missing_app)?;
    assert_eq!(preflight["status"], "missing");
    assert!(select_science_runtime_for_paths(
        &data_dir,
        None,
        &missing_app,
        Some(CACHED_ONCE_CHOICE),
    )
    .is_err());

    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn replacing_installed_app_uses_new_version_without_mutating_cache_or_data_dir(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("science-app-upgrade")?;
    let data_dir = root.join("home").join(".claude-science");
    let cached_bin = data_dir.join("bin").join("claude-science");
    let app_bin = root.join("app-claude-science");
    let state_marker = data_dir.join("persistent-state.txt");
    write_fake_version_bin(&cached_bin, 0o755, "fake-cache-old")?;
    fs::write(&state_marker, "keep-me")?;
    let cached_before = fs::read(&cached_bin)?;

    write_fake_version_bin(&app_bin, 0o755, "fake-app-v1")?;
    let first = select_science_runtime_for_paths(&data_dir, None, &app_bin, None)?;
    assert_eq!(first.version.as_deref(), Some("fake-app-v1"));

    write_fake_version_bin(&app_bin, 0o755, "fake-app-v2")?;
    let second = select_science_runtime_for_paths(&data_dir, None, &app_bin, None)?;
    assert_eq!(second.version.as_deref(), Some("fake-app-v2"));
    assert_eq!(second.path, app_bin);
    assert_eq!(fs::read_to_string(&state_marker)?, "keep-me");
    assert_eq!(fs::read(&cached_bin)?, cached_before);
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn sandbox_home_is_writable_under_config_dir() {
    let h = sandbox_home();
    assert!(h.ends_with("sandbox/home"), "应以 sandbox/home 结尾：{h:?}");
    assert!(
        h.to_string_lossy().contains(".csswitch"),
        "应在 .csswitch 下：{h:?}"
    );
}

#[test]
fn sandbox_url_falls_back_to_localhost_when_cli_absent() {
    let root = unique_temp_dir("science-url-fallback").unwrap();
    let bin = root.join("claude-science");
    write_fake_bin(&bin, 0o755).unwrap();
    let runtime = ScienceRuntimeIdentity {
        path: bin,
        source: ScienceRuntimeSource::InstalledApp,
        version: None,
        fingerprint: science_executable_fingerprint(&root.join("claude-science")).unwrap(),
        adoption_attempt_id: None,
    };
    assert_eq!(sandbox_url(8990, &runtime), "http://127.0.0.1:8990");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn sandbox_identity_does_not_trust_health_when_cli_absent() {
    let root = unique_temp_dir("science-identity-fallback").unwrap();
    let bin = root.join("claude-science");
    write_fake_bin(&bin, 0o755).unwrap();
    let runtime = ScienceRuntimeIdentity {
        path: bin,
        source: ScienceRuntimeSource::InstalledApp,
        version: None,
        fingerprint: science_executable_fingerprint(&root.join("claude-science")).unwrap(),
        adoption_attempt_id: None,
    };
    assert!(!sandbox_running_ours(9, &runtime));
    fs::remove_dir_all(root).unwrap();
}

fn status_output(code: i32, stdout: &str) -> Output {
    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

fn unique_temp_dir(name: &str) -> std::io::Result<std::path::PathBuf> {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "csswitch-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&p)?;
    p.canonicalize()
}

fn write_fake_bin(path: &std::path::Path, mode: u32) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, "#!/bin/sh\nexit 0\n")?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

fn write_padded_fake_version_bin(path: &std::path::Path, version: &str) -> std::io::Result<()> {
    write_fake_version_bin(path, 0o755, version)?;
    let mut file = fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(b"\n#")?;
    file.write_all(&vec![b' '; super::MIN_SCIENCE_BINARY_SIZE as usize])?;
    Ok(())
}

fn write_fake_version_bin(path: &std::path::Path, mode: u32, version: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
            path,
            format!(
                "#!/bin/sh\nif [ \"${{1:-}}\" = \"--version\" ]; then printf '%s\\n' '{}'; exit 0; fi\nexit 0\n",
                version
            ),
        )?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

fn write_counted_version_bin(
    path: &std::path::Path,
    count: &std::path::Path,
    version: &str,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
            path,
            format!(
                "#!/bin/sh\nif [ \"${{1:-}}\" = \"--version\" ]; then count=$(cat '{}' 2>/dev/null || echo 0); count=$((count + 1)); printf '%s' \"$count\" > '{}'; printf '%s\\n' '{}'; exit 0; fi\nexit 0\n",
                count.display(),
                count.display(),
                version
            ),
        )?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
}
