mod authority_snapshot;
mod catalog_verify;
mod one_click;
mod pending_cleanup;
mod recovery;
mod route_reconcile;
mod ssh_preflight;

// Internal modules stay private; only re-export the historical crate-facing surface.
#[cfg(test)]
use authority_snapshot::{
    test_arm_authority_cleanup_parent_sync_failure, test_arm_authority_snapshot_clone_errno,
    test_arm_authority_snapshot_completion_sync_failure,
    test_arm_authority_snapshot_fallback_create_failure,
    test_arm_authority_snapshot_parent_barrier, AuthorityCopyBudget, AuthoritySnapshotCategory,
    AuthoritySnapshotScope, AuthorityTreeSnapshot, MAX_AUTHORITY_FULL_COPY_FILE_BYTES,
    MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES, MAX_AUTHORITY_SNAPSHOT_ENTRIES,
    MAX_AUTHORITY_SNAPSHOT_FILE_BYTES, MAX_AUTHORITY_SNAPSHOT_TOTAL_BYTES,
    SCIENCE_OWNED_OPAQUE_ROOTS,
};
#[cfg(test)]
use catalog_verify::*;
#[cfg(test)]
use one_click::{
    clear_one_click_transaction, commit_healthy_reopen_binding, commit_runtime_binding,
    healthy_reopen_transaction_matches, one_click_phase_exposure, resolve_profile_switch_handoff,
    science_health_control_error, test_compensate_one_click_failure, write_one_click_checkpoint,
    OneClickJournalProgress, OneClickTransactionIdentity,
};
#[allow(unused_imports)]
pub(crate) use one_click::{
    force_restart_science_for_active, one_click_login, reconcile_science_for_active,
    ReconcileScienceError,
};
#[cfg(test)]
use pending_cleanup::retry_pending_authority_cleanup;
#[cfg(test)]
use pending_cleanup::{
    cleanup_tombstone_path, finalize_registered_authority_cleanup, parse_pending_cleanup_manifest,
    AuthorityCleanupOutcome, AuthorityCleanupPhase, PendingCleanupEntry,
    RegisteredAuthorityCleanup, PENDING_CLEANUP_MARKER_FILE,
};
#[cfg(test)]
use recovery::OneClickAuthoritySnapshot;
pub(crate) use route_reconcile::force_third_party_reconcile;
use ssh_preflight::*;

#[cfg(test)]
pub(crate) use authority_snapshot::{
    test_arm_authority_snapshot_capture_failure, test_arm_authority_snapshot_cleanup_fault,
    test_arm_authority_snapshot_directory_barrier, test_arm_gateway_catalog_bypass,
    test_arm_healthy_reopen_catalog_failure, test_arm_one_click_exit_after_snapshot_capture,
    test_arm_one_click_first_journal_failure, test_arm_one_click_snapshot_capture,
    test_arm_prior_restart_post_spawn_failure, test_arm_rollback_diagnostic_canary,
    test_prior_restart_post_spawn_identity, test_rollback_diagnostic_snapshot,
    SCIENCE_PROTECTED_AUTHORITY_ENTRIES,
};

#[cfg(test)]
mod transaction_tests;
