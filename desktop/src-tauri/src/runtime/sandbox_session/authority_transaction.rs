//! Behavior-preserving authority transaction façade.
//!
//! This interface owns the existing capture, verified snapshot-ticket,
//! restore, and cleanup surface used by the one-click coordinator. The
//! underlying snapshot and pending-cleanup implementations keep their
//! filesystem, identity, and typed-outcome contracts. Operation ordering,
//! journal checkpoints, compensation policy, DTO/text projection, and prior
//! Science handling remain coordinator responsibilities.

use std::path::Path;

use serde_json::Value;
use tauri::Runtime;

use crate::config;
use crate::runtime::proxy::ProxyAction;
use crate::{lifecycle, SharedAppState};

use super::pending_cleanup::{AuthorityCleanupFailure, AuthorityCleanupOutcome};
use super::recovery::{OneClickAuthoritySnapshot, RuntimeTransactionRestoreExpectation};

pub(super) struct AuthorityTransaction {
    snapshot: OneClickAuthoritySnapshot,
}

impl AuthorityTransaction {
    pub(super) fn capture(
        config_dir: &Path,
        sandbox_home: &Path,
        auth_dir: &Path,
        config: &config::Config,
        state: &SharedAppState,
    ) -> Result<Self, String> {
        OneClickAuthoritySnapshot::capture(config_dir, sandbox_home, auth_dir, config, state)
            .map(|snapshot| Self { snapshot })
    }

    pub(super) fn registered_snapshot_ticket(
        &self,
    ) -> Result<config::RuntimeSnapshotTicket, String> {
        self.snapshot.registered_snapshot_ticket()
    }

    pub(super) fn captured_runtime_transaction(&self) -> Option<config::RuntimeTransactionRecord> {
        self.snapshot.config.runtime_transaction.clone()
    }

    pub(super) fn preserve_recovery(&mut self) {
        self.snapshot.preserve_recovery = true;
    }

    pub(super) fn recovery_path(&self) -> &Path {
        &self.snapshot.backup_root
    }

    pub(super) fn persist_private_manifest(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        self.snapshot.persist_private_manifest(name, bytes)
    }

    pub(super) fn validate_science_restore_root(&self) -> Result<(), String> {
        self.snapshot.validate_science_restore_root()
    }

    pub(super) fn science_opaque_bindings_env(&self) -> String {
        self.snapshot.science_opaque_bindings_env()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore<R: Runtime>(
        &mut self,
        app: &tauri::AppHandle<R>,
        config_dir: &Path,
        state: &SharedAppState,
        lifecycle: &lifecycle::Lifecycle,
        auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
        proxy_action: ProxyAction,
        runtime_transaction: &RuntimeTransactionRestoreExpectation,
    ) -> Result<(), String> {
        self.snapshot.restore_with_gateway(
            app,
            config_dir,
            state,
            lifecycle,
            auth_proof,
            proxy_action,
            runtime_transaction,
        )
    }

    pub(super) fn cleanup_when_expendable(
        &mut self,
    ) -> Result<AuthorityCleanupOutcome, AuthorityCleanupFailure> {
        self.snapshot.cleanup_when_expendable()
    }

    pub(super) fn prepare_success(
        &mut self,
        value: &mut Value,
    ) -> Result<(), AuthorityCleanupFailure> {
        self.snapshot.prepare_success(value)
    }

    pub(super) fn commit(&mut self) {
        self.snapshot.commit();
    }
}
