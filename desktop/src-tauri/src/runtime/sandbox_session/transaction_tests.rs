use super::{
    begin_one_click_compensation, begin_one_click_compensation_step, begin_one_click_finalize,
    begin_prior_stop_intent, cleanup_tombstone_path, clear_one_click_transaction,
    commit_healthy_reopen_binding, commit_runtime_binding, complete_one_click_finalize,
    finalize_registered_authority_cleanup, finish_one_click_authority_restore_step,
    finish_one_click_compensation, finish_one_click_compensation_step,
    gateway_model_catalog_timeout_ms, healthy_reopen_transaction_matches, one_click_phase_exposure,
    parse_pending_cleanup_manifest, prepare_registered_authority_cleanup,
    prevalidate_one_click_system_ssh, publish_prior_stop_outcome,
    replay_interrupted_one_click_compensation, resolve_gateway_terminal_handoff,
    resolve_profile_switch_handoff, retry_pending_authority_cleanup, science_health_control_error,
    test_arm_authority_cleanup_parent_sync_failure, test_arm_authority_snapshot_capture_failure,
    test_arm_authority_snapshot_cleanup_fault, test_arm_authority_snapshot_clone_errno,
    test_arm_authority_snapshot_completion_sync_failure,
    test_arm_authority_snapshot_directory_barrier,
    test_arm_authority_snapshot_fallback_create_failure,
    test_arm_authority_snapshot_parent_barrier, test_begin_replayable_compensation,
    test_compensate_one_click_failure, validate_system_ssh_wrapper_path,
    verify_gateway_model_catalog, write_one_click_checkpoint, AuthorityCleanupOutcome,
    AuthorityCleanupPhase, AuthorityCopyBudget, AuthoritySnapshotCategory, AuthoritySnapshotScope,
    AuthorityTransaction, AuthorityTreeSnapshot, OneClickAuthoritySnapshot,
    OneClickJournalProgress, OneClickTransactionIdentity, PendingCleanupEntry,
    RegisteredAuthorityCleanup, MAX_AUTHORITY_FULL_COPY_FILE_BYTES,
    MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES, MAX_AUTHORITY_SNAPSHOT_ENTRIES,
    MAX_AUTHORITY_SNAPSHOT_FILE_BYTES, MAX_AUTHORITY_SNAPSHOT_TOTAL_BYTES,
    PENDING_CLEANUP_MARKER_FILE, SCIENCE_OWNED_OPAQUE_ROOTS,
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

mod authority_snapshot;
mod cleanup_recovery;
mod gateway_catalog;
mod runtime_journal;
mod ssh_behavior;
mod ssh_contract;
mod transaction_contract;
