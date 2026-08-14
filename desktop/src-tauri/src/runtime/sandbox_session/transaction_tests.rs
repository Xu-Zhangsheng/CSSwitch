use super::{
    begin_one_click_compensation, begin_one_click_compensation_step, begin_one_click_finalize,
    begin_prior_stop_intent, cleanup_tombstone_path, clear_one_click_transaction,
    commit_runtime_binding, complete_one_click_finalize, execute_transaction_science_stop_with,
    finalize_registered_authority_cleanup, finish_one_click_authority_restore_step,
    finish_one_click_compensation, finish_one_click_compensation_step,
    gateway_model_catalog_timeout_ms, one_click_phase_exposure, parse_pending_cleanup_manifest,
    prepare_registered_authority_cleanup, prevalidate_one_click_system_ssh,
    publish_prior_stop_outcome, replay_interrupted_one_click_compensation,
    resolve_gateway_terminal_handoff, retry_pending_authority_cleanup,
    runtime_environment_fingerprint_changed, science_health_control_error,
    test_arm_authority_cleanup_parent_sync_failure, test_arm_authority_snapshot_capture_failure,
    test_arm_authority_snapshot_cleanup_fault, test_arm_authority_snapshot_clone_errno,
    test_arm_authority_snapshot_completion_sync_failure,
    test_arm_authority_snapshot_directory_barrier,
    test_arm_authority_snapshot_fallback_create_failure,
    test_arm_authority_snapshot_parent_barrier, test_arm_durable_authority_crash_after_boundary,
    test_arm_durable_authority_outcome_crash_after_promotion,
    test_arm_durable_authority_stage_crash_after_copy,
    test_arm_durable_authority_tombstone_crash_after_rename, test_begin_replayable_compensation,
    test_compensate_one_click_failure, test_replay_prior_restart_effect_without_outcome,
    validate_system_ssh_wrapper_path, verify_gateway_model_catalog, write_one_click_checkpoint,
    AuthorityCleanupOutcome, AuthorityCleanupPhase, AuthorityCopyBudget, AuthoritySnapshotCategory,
    AuthoritySnapshotScope, AuthorityTransaction, AuthorityTreeSnapshot, OneClickAuthoritySnapshot,
    OneClickJournalProgress, OneClickTransactionIdentity, PendingCleanupEntry,
    RegisteredAuthorityCleanup, TransactionScienceStopBoundary, TransactionScienceStopTarget,
    MAX_AUTHORITY_FULL_COPY_FILE_BYTES, MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES,
    MAX_AUTHORITY_SNAPSHOT_ENTRIES, MAX_AUTHORITY_SNAPSHOT_FILE_BYTES,
    MAX_AUTHORITY_SNAPSHOT_TOTAL_BYTES, PENDING_CLEANUP_MARKER_FILE, SCIENCE_OWNED_OPAQUE_ROOTS,
};
use crate::config::{self, Config, RuntimeBindingCommit};
use crate::provider_contracts::ModelPolicy;
use crate::runtime::proxy::ProxyAction;
use crate::{lock, AppState, SharedAppState};
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

#[test]
#[allow(clippy::result_large_err)]
fn transaction_scoped_science_stop_boundaries_release_read_model_and_cas_publication() {
    use crate::runtime::science::{ScienceStopFailure, ScienceStopRequest, VerifiedScienceStop};

    let tmp = isolated_tmpdir("transaction-science-stop-owner");
    let prior_binary = tmp.join("prior-science");
    let replacement_binary = tmp.join("replacement-science");
    std::fs::write(&prior_binary, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::write(&replacement_binary, b"#!/bin/sh\nexit 0\n# replacement\n").unwrap();
    std::fs::set_permissions(&prior_binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&replacement_binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let prior = crate::runtime::science::test_runtime_identity(prior_binary);
    let replacement = crate::runtime::science::test_runtime_identity(replacement_binary);
    let boundaries = [
        TransactionScienceStopBoundary::ColdPriorStop,
        TransactionScienceStopBoundary::ManagedDbRestart,
        TransactionScienceStopBoundary::HistoryRecoveryPriorStop,
        TransactionScienceStopBoundary::LiveCompensationCleanup,
        TransactionScienceStopBoundary::CompensationReplayCleanup,
    ];

    for boundary in boundaries {
        for (case, replace_identity, bump_generation) in
            [("replacement", true, false), ("generation", false, true)]
        {
            let mut authority = AppState::default();
            authority.science_runtime = Some(prior.clone());
            authority.sandbox_port = 18765;
            authority.sandbox_url = Some("http://127.0.0.1:18765/prior".into());
            let state: SharedAppState = Arc::new(Mutex::new(authority));
            let lifecycle = Arc::new(crate::lifecycle::Lifecycle::new());
            let (stop_started_tx, stop_started_rx) = std::sync::mpsc::channel();
            let (release_stop_tx, release_stop_rx) = std::sync::mpsc::channel();
            let worker_state = state.clone();
            let worker_lifecycle = lifecycle.clone();
            let claimed_runtime = prior.clone();
            let verified_runtime = prior.clone();
            let worker = std::thread::spawn(move || {
                execute_transaction_science_stop_with(
                    &worker_state,
                    worker_lifecycle.as_ref(),
                    TransactionScienceStopTarget::new(boundary, &claimed_runtime, 18765),
                    || Ok(ScienceStopRequest::recover(Some(&claimed_runtime))),
                    move |_request| {
                        stop_started_tx.send(()).unwrap();
                        release_stop_rx.recv().unwrap();
                        (
                            Ok(VerifiedScienceStop {
                                runtime: Some(verified_runtime),
                                ownership_was_proven: true,
                            }),
                            true,
                        )
                    },
                    |_state, _confirmed_runtime| {},
                )
            });

            stop_started_rx.recv().unwrap();
            {
                let mut read_model = state
                    .try_lock()
                    .unwrap_or_else(|_| panic!("{boundary:?}/{case}: stop wait retained AppState"));
                assert_eq!(read_model.science_runtime.as_ref(), Some(&prior));
                if replace_identity {
                    read_model.science_runtime = Some(replacement.clone());
                    read_model.sandbox_url = Some("http://127.0.0.1:18765/replacement".into());
                }
            }
            if bump_generation {
                lifecycle.bump_generation();
            }
            release_stop_tx.send(()).unwrap();

            let outcome = worker.join().unwrap();
            assert!(
                outcome.as_ref().is_err_and(|error| {
                    error.to_string().contains("process-local owner 已变化")
                }),
                "{boundary:?}/{case}: {outcome:?}"
            );
            let current = crate::lock(&state);
            assert_eq!(
                current.science_runtime.as_ref(),
                Some(if replace_identity {
                    &replacement
                } else {
                    &prior
                }),
                "{boundary:?}/{case}"
            );
            assert!(current.science_confirmed_stopped.is_none());
        }

        let mut authority = AppState::default();
        authority.science_runtime = Some(prior.clone());
        authority.sandbox_port = 18765;
        authority.sandbox_url = Some("http://127.0.0.1:18765/prior".into());
        let state: SharedAppState = Arc::new(Mutex::new(authority));
        let lifecycle = crate::lifecycle::Lifecycle::new();
        let failed = execute_transaction_science_stop_with(
            &state,
            &lifecycle,
            TransactionScienceStopTarget::new(boundary, &prior, 18765),
            || Ok(ScienceStopRequest::recover(Some(&prior))),
            |_request| {
                (
                    Err(ScienceStopFailure::stop_command_failed(
                        "fixture stop failure",
                    )),
                    false,
                )
            },
            |_state, _confirmed_runtime| {},
        );
        assert!(failed.is_err(), "{boundary:?}: stop failure was accepted");
        let current = crate::lock(&state);
        assert_eq!(current.science_runtime.as_ref(), Some(&prior));
        assert!(current.science_confirmed_stopped.is_none());
    }

    for boundary in [
        TransactionScienceStopBoundary::LiveCompensationCleanup,
        TransactionScienceStopBoundary::CompensationReplayCleanup,
    ] {
        let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
        let lifecycle = crate::lifecycle::Lifecycle::new();
        let stopped = execute_transaction_science_stop_with(
            &state,
            &lifecycle,
            TransactionScienceStopTarget::new(boundary, &prior, 18765),
            || Ok(ScienceStopRequest::recover(Some(&prior))),
            |_request| {
                (
                    Ok(VerifiedScienceStop {
                        runtime: Some(prior.clone()),
                        ownership_was_proven: true,
                    }),
                    false,
                )
            },
            |_state, _confirmed_runtime| {},
        );
        assert!(stopped.is_ok(), "{boundary:?}: durable owner was rejected");
        let current = crate::lock(&state);
        assert!(current.science_runtime.is_none());
        assert_eq!(current.science_confirmed_stopped.as_ref(), Some(&prior));
    }

    let _ = std::fs::remove_dir_all(tmp);
}

#[test]
fn transaction_science_stop_probe_is_lock_free_and_rechecks_owner_before_effect() {
    use crate::runtime::science::{ScienceStopRequest, VerifiedScienceStop};
    use std::sync::atomic::{AtomicBool, Ordering};

    let tmp = isolated_tmpdir("transaction-science-stop-probe");
    let prior_binary = tmp.join("prior-science");
    let replacement_binary = tmp.join("replacement-science");
    std::fs::write(&prior_binary, b"#!/bin/sh\nexit 0\n").unwrap();
    std::fs::write(&replacement_binary, b"#!/bin/sh\nexit 0\n# replacement\n").unwrap();
    std::fs::set_permissions(&prior_binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&replacement_binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let prior = crate::runtime::science::test_runtime_identity(prior_binary);
    let replacement = crate::runtime::science::test_runtime_identity(replacement_binary);
    let mut authority = AppState::default();
    authority.science_runtime = Some(prior.clone());
    authority.sandbox_port = 18765;
    authority.sandbox_url = Some("http://127.0.0.1:18765/prior".into());
    let state: SharedAppState = Arc::new(Mutex::new(authority));
    let lifecycle = Arc::new(crate::lifecycle::Lifecycle::new());
    let (probe_started_tx, probe_started_rx) = std::sync::mpsc::channel();
    let (release_probe_tx, release_probe_rx) = std::sync::mpsc::channel();
    let effect_ran = Arc::new(AtomicBool::new(false));
    let worker_effect_ran = effect_ran.clone();
    let worker_state = state.clone();
    let worker_lifecycle = lifecycle.clone();
    let claimed_runtime = prior.clone();
    let verified_runtime = prior.clone();
    let worker = std::thread::spawn(move || {
        execute_transaction_science_stop_with(
            &worker_state,
            worker_lifecycle.as_ref(),
            TransactionScienceStopTarget::new(
                TransactionScienceStopBoundary::ColdPriorStop,
                &claimed_runtime,
                18765,
            ),
            || {
                probe_started_tx.send(()).unwrap();
                release_probe_rx.recv().unwrap();
                Ok(ScienceStopRequest::recover(Some(&claimed_runtime)))
            },
            move |_request| {
                worker_effect_ran.store(true, Ordering::SeqCst);
                (
                    Ok(VerifiedScienceStop {
                        runtime: Some(verified_runtime),
                        ownership_was_proven: true,
                    }),
                    true,
                )
            },
            |_state, _confirmed_runtime| {},
        )
    });

    probe_started_rx.recv().unwrap();
    {
        let mut current = state
            .try_lock()
            .expect("external Science request probe retained AppState");
        current.science_runtime = Some(replacement.clone());
        current.sandbox_url = Some("http://127.0.0.1:18765/replacement".into());
    }
    release_probe_tx.send(()).unwrap();
    let outcome = worker.join().unwrap();
    assert!(outcome.is_err());
    assert!(!effect_ran.load(Ordering::SeqCst));
    assert_eq!(
        crate::lock(&state).science_runtime.as_ref(),
        Some(&replacement)
    );
    let _ = std::fs::remove_dir_all(tmp);
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

mod authority_snapshot;
mod cleanup_recovery;
mod gateway_catalog;
mod runtime_journal;
mod ssh_behavior;
mod ssh_contract;
mod transaction_contract;
