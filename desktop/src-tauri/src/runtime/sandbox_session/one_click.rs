#[cfg(test)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::time::{Duration, Instant};

use csswitch_skill_install_core::{open_science_health_session_before, ScienceHealthSession};
use serde_json::{json, Value};
use tauri::{Manager, Runtime};

use crate::runtime::failure::{OneClickFailureKind, ProjectedRecovery, TypedOneClickFailure};
use crate::runtime::operation::{
    self, OperationKind, OperationStage, OperationTrace, POLL_INTERVAL_MS,
};
use crate::runtime::proxy::ProxyAction;
use crate::runtime::proxy_lifecycle::{
    current_skill_install_bridge_key, skill_install_bridge_dir, GatewayController,
};
use crate::runtime::science::{
    mark_science_runtime_adoption_finalized, reconcile_current_science_runtime_adoption,
    sandbox_home, select_science_runtime_cached, SandboxScienceState, ScienceEnvironmentExposure,
    ScienceHostAdapter, ScienceLaunchFailureKind, ScienceLaunchSpec, ScienceManagedLaunchToken,
    ScienceRuntimeIdentity, ScienceRuntimeSource, ScienceStopFailureKind,
    ScienceStopOwnershipReceipt, ScienceStopRequest,
};
use crate::runtime::skill_install_bridge::{
    inspect_while_science_running, register_before_science_start, RegistrationStatus,
};
use crate::runtime::system::{asset_root, log_path, open_in_browser, open_log, redact, tail_file};
use crate::{
    config, lifecycle, lock, oauth_forge, proc, HistoryRecoveryChoice,
    HistoryRecoveryScienceQuiescence, HistoryRecoverySession, SharedAppState,
};

// Sibling modules are owned by the sandbox_session facade.
#[cfg(test)]
use super::authority_snapshot::SANDBOX_SESSION_TEST_SEAMS;
use super::authority_transaction::AuthorityTransaction;
use super::catalog_verify::*;
use super::pending_cleanup::{
    cleanup_required_error, replay_compensation_snapshot_cleanup,
    replay_finalize_authority_cleanup, retry_pending_authority_cleanup, AuthorityCleanupFailure,
    AuthorityCleanupPhase, PendingCleanupRetryOutcome,
};
use super::recovery::{
    read_registered_private_manifest, AppAuthoritySnapshot, RuntimeTransactionRestoreExpectation,
};
use super::route_reconcile::configure_third_party_best_effort;
use super::ssh_preflight::*;
use super::transaction_science_stop::{
    execute_transaction_science_stop_with, TransactionScienceStopBoundary,
};

mod cold;
mod compensation_replay;
mod healthy_reopen;

use compensation_replay::persist_compensation_replay_manifest;
pub(super) use compensation_replay::replay_interrupted_one_click_compensation;
#[cfg(test)]
pub(super) fn test_replay_prior_restart_effect_without_outcome<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
) -> Result<config::RuntimeCompensationStepState, String> {
    compensation_replay::test_replay_prior_restart_effect_without_outcome(app, state, lifecycle)
}

#[cfg(test)]
pub(super) use cold::{
    CompensationCause, CompensationEnvironment, CompensationOutcome, CompensationSkipCause,
    CompensationStepOutcome,
};

use healthy_reopen::healthy_reopen_with_gateway_rollback;

pub(super) fn runtime_environment_fingerprint_changed(
    prior_runtime_fingerprint: Option<&str>,
    launch_runtime_fingerprint: &str,
) -> bool {
    prior_runtime_fingerprint.is_some_and(|fingerprint| fingerprint != launch_runtime_fingerprint)
}

#[derive(Clone, PartialEq)]
struct OneClickGatewayPreflightSnapshot {
    child_pid: u32,
    proxy_port: u16,
    secret: String,
    provider: String,
    gateway_kind: String,
    shim_mode: String,
    launch_id: String,
    key_fp: u64,
    launch_context: crate::runtime::proxy_lifecycle::GatewayLaunchRecipe,
}

impl OneClickGatewayPreflightSnapshot {
    fn capture(state: &SharedAppState) -> Result<Option<Self>, String> {
        let mut current = lock(state);
        let Some(child) = current.proxy.as_mut() else {
            return Ok(None);
        };
        if child
            .try_wait()
            .map_err(|_| {
                "gateway_identity_retry：无法确认先前 Gateway child 状态，请重试。".to_string()
            })?
            .is_some()
        {
            return Err(
                "gateway_identity_retry：先前 Gateway child 已退出，请先恢复运行态后重试。".into(),
            );
        }
        let child_pid = child.id();
        let launch_context = current.gateway_launch_context.clone().ok_or(
            "gateway_identity_retry：先前 Gateway 缺少完整启动上下文，请先恢复运行态后重试。",
        )?;
        Ok(Some(Self {
            child_pid,
            proxy_port: current.proxy_port,
            secret: current.secret.clone(),
            provider: current.provider.clone(),
            gateway_kind: current.gateway_kind.clone(),
            shim_mode: current.shim_mode.clone(),
            launch_id: current.launch_id.clone(),
            key_fp: current.key_fp,
            launch_context,
        }))
    }

    fn needs_codex_proof(&self) -> Result<bool, String> {
        Ok(
            crate::runtime::provider::resolve_launch_plan(&self.launch_context.profile)?.adapter
                == "codex",
        )
    }

    fn verify_unchanged(&self, state: &SharedAppState) -> Result<(), String> {
        let mut current = lock(state);
        let child_matches = match current.proxy.as_mut() {
            Some(child) if child.id() == self.child_pid => child
                .try_wait()
                .map_err(|_| {
                    "gateway_identity_retry：无法复核先前 Gateway child 状态，请重试。".to_string()
                })?
                .is_none(),
            _ => false,
        };
        let exact_context = child_matches
            && current.proxy_port == self.proxy_port
            && current.secret == self.secret
            && current.provider == self.provider
            && current.gateway_kind == self.gateway_kind
            && current.shim_mode == self.shim_mode
            && current.launch_id == self.launch_id
            && current.key_fp == self.key_fp
            && current.gateway_launch_context.as_ref() == Some(&self.launch_context);
        if exact_context {
            Ok(())
        } else {
            Err(
                "gateway_context_changed_retry：先前 Gateway 身份或完整启动上下文在认证检查期间发生变化，请重试。"
                    .into(),
            )
        }
    }
}

/// Immutable command-to-runtime proof used only to keep lock-free auth preflight
/// outside the destructive lease. Runtime entry owns capture and verification.
pub(crate) struct OneClickEntryPreflight {
    config: Option<config::Config>,
    runtime_transaction: Option<config::RuntimeTransactionRecord>,
    runtime_compensation: Option<config::RuntimeCompensationJournal>,
    prior_gateway: Option<OneClickGatewayPreflightSnapshot>,
    auth_adapter: String,
}

impl OneClickEntryPreflight {
    pub(crate) fn capture(state: &SharedAppState) -> Result<Self, TypedOneClickFailure> {
        let cfg = config::load_from(&config::default_dir()).map_err(|error| {
            typed_one_click_err(OneClickFailureKind::ConfigLoad, error.to_string())
        })?;
        let active = cfg.active_profile().ok_or_else(|| {
            typed_one_click_err(
                OneClickFailureKind::NoActiveProfile,
                "未配置生效 profile，请先在面板选择或新建一条配置。",
            )
        })?;
        let adapter = crate::runtime::provider::resolve_launch_plan(active)
            .map_err(|message| typed_one_click_err(OneClickFailureKind::LaunchPlan, message))?
            .adapter;
        let prior_gateway =
            OneClickGatewayPreflightSnapshot::capture(state).map_err(|message| {
                typed_one_click_err(OneClickFailureKind::PreflightSnapshot, message)
            })?;
        let needs_codex_proof = adapter == "codex"
            || prior_gateway
                .as_ref()
                .map(OneClickGatewayPreflightSnapshot::needs_codex_proof)
                .transpose()
                .map_err(|message| {
                    typed_one_click_err(OneClickFailureKind::PreflightSnapshot, message)
                })?
                .unwrap_or(false);
        Ok(Self {
            runtime_transaction: cfg.runtime_transaction.clone(),
            runtime_compensation: cfg.runtime_compensation.clone(),
            config: (adapter != "codex").then_some(cfg),
            prior_gateway,
            auth_adapter: if needs_codex_proof {
                "codex".into()
            } else {
                adapter
            },
        })
    }

    pub(crate) fn auth_adapter(&self) -> &str {
        &self.auth_adapter
    }

    pub(crate) fn verify_unchanged(
        &self,
        state: &SharedAppState,
    ) -> Result<(), TypedOneClickFailure> {
        if let Some(expected) = self.config.as_ref() {
            let current = config::load_from(&config::default_dir()).map_err(|_| {
                typed_one_click_err(
                    OneClickFailureKind::PreflightSnapshot,
                    "config_changed_retry：无法复核候选启动配置，请重试。",
                )
            })?;
            if &current != expected {
                return Err(typed_one_click_err(
                    OneClickFailureKind::PreflightSnapshot,
                    "config_changed_retry：候选启动配置在认证检查期间发生变化，请重试。",
                ));
            }
        } else {
            let current = config::load_from(&config::default_dir()).map_err(|_| {
                typed_one_click_err(
                    OneClickFailureKind::PreflightSnapshot,
                    "config_changed_retry：无法复核认证期间的 runtime transaction，请重试。",
                )
            })?;
            if current.runtime_transaction != self.runtime_transaction
                || current.runtime_compensation != self.runtime_compensation
            {
                return Err(typed_one_click_err(
                    OneClickFailureKind::PreflightSnapshot,
                    "config_changed_retry：runtime transaction 在认证检查期间发生变化，请重试。",
                ));
            }
        }
        if let Some(expected) = self.prior_gateway.as_ref() {
            expected.verify_unchanged(state).map_err(|message| {
                typed_one_click_err(OneClickFailureKind::PreflightSnapshot, message)
            })?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OneClickEntryRecoveryDecision {
    ReplayFinalize,
    ReplayFinalizeCleanup,
    RecoverGateway,
    Route,
}

pub(super) fn decide_one_click_entry_recovery(
    finalize_pending: bool,
    finalize_replayed: bool,
    finalize_cleanup_replayed: bool,
    gateway_recovery_complete: bool,
) -> OneClickEntryRecoveryDecision {
    if finalize_pending {
        OneClickEntryRecoveryDecision::ReplayFinalize
    } else if finalize_replayed && !finalize_cleanup_replayed {
        OneClickEntryRecoveryDecision::ReplayFinalizeCleanup
    } else if !gateway_recovery_complete {
        OneClickEntryRecoveryDecision::RecoverGateway
    } else {
        OneClickEntryRecoveryDecision::Route
    }
}

fn one_click_finalize_pending(cfg: &config::Config) -> bool {
    cfg.runtime_transaction.as_ref().is_some_and(|record| {
        matches!(
            record,
            config::RuntimeTransactionRecord::V2(transaction)
                if transaction.finalize != config::RuntimeFinalizeState::NotStarted
        )
    })
}

fn history_resume_handoff(cfg: &config::Config) -> Option<config::RuntimeTransactionV2> {
    match cfg.runtime_transaction.as_ref() {
        Some(config::RuntimeTransactionRecord::V2(record))
            if record.operation == config::RuntimeTransactionOperation::HistoryRecovery
                && record.phase == config::RuntimeTransactionPhase::ResumeAfterHistoryRestore
                && record.snapshot_ticket.is_none()
                && record.finalize == config::RuntimeFinalizeState::NotStarted
                && record.target_profile_id == cfg.active_id
                && record.previous_binding.as_ref() == cfg.runtime_binding.as_ref() =>
        {
            super::history_recovery::history_config_authority_matches(cfg, record)
                .then(|| record.clone())
        }
        _ => None,
    }
}

pub(super) fn pending_cleanup_requires_recapture(outcome: PendingCleanupRetryOutcome) -> bool {
    outcome == PendingCleanupRetryOutcome::Cleared
}

pub(crate) fn typed_interrupted_gateway_recovery_error(
    error: crate::runtime::proxy_lifecycle::InterruptedGatewayRecoveryError,
) -> TypedOneClickFailure {
    use crate::runtime::proxy_lifecycle::{
        InterruptedGatewayRecoveryDisposition, InterruptedGatewayRecoveryErrorKind,
    };

    let kind = match error.kind() {
        InterruptedGatewayRecoveryErrorKind::AuthoritySnapshot => {
            OneClickFailureKind::AuthoritySnapshot
        }
        InterruptedGatewayRecoveryErrorKind::GatewayStart
        | InterruptedGatewayRecoveryErrorKind::NotManaged
        | InterruptedGatewayRecoveryErrorKind::StopUnknown(_) => OneClickFailureKind::GatewayStart,
    };
    let recovery = match error.recovery() {
        InterruptedGatewayRecoveryDisposition::Degraded => ProjectedRecovery::DEGRADED,
        InterruptedGatewayRecoveryDisposition::ManualRecoveryRequired => {
            ProjectedRecovery::MANUAL_RECOVERY_REQUIRED
        }
    };
    TypedOneClickFailure::new(kind, error.to_string()).with_recovery(recovery)
}

fn open_science_surface<R: Runtime>(
    app: &tauri::AppHandle<R>,
    url: &str,
) -> Result<&'static str, String> {
    if std::env::var("CSSWITCH_SCIENCE_WEBVIEW_SPIKE")
        .ok()
        .as_deref()
        == Some("1")
    {
        if let Some(win) = app.get_webview_window("science") {
            let _ = win.close();
        }
        let parsed = url
            .parse()
            .map_err(|e| format!("Science URL 解析失败：{e}"))?;
        match tauri::WebviewWindowBuilder::new(app, "science", tauri::WebviewUrl::External(parsed))
            .title("Claude Science")
            .inner_size(1100.0, 800.0)
            .build()
        {
            Ok(win) => {
                let _ = win.set_focus();
                return Ok("webview");
            }
            Err(_) => {
                // Spike-only path: construction failure falls through to the existing browser surface.
            }
        }
    }
    open_in_browser(url)?;
    Ok("browser")
}

fn installer_status_json(status: &RegistrationStatus) -> Value {
    match status {
        RegistrationStatus::Warning(message) => {
            json!({"status": status.code(), "message": message})
        }
        _ => json!({"status": status.code()}),
    }
}

fn append_installer_note(mut message: String, status: &RegistrationStatus) -> String {
    if let Some(note) = status.user_note() {
        message.push_str(&format!(" {note}"));
    }
    message
}

/// One-click session startup: active proxy, virtual login, sandbox, browser.
///
/// Callers must hold the command serializer lock.
#[cfg(test)]
pub(crate) fn one_click_login<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    runtime_choice: Option<&str>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
) -> Result<Value, TypedOneClickFailure> {
    one_click_login_with_options(
        app,
        state,
        lifecycle,
        runtime_choice,
        auth_proof,
        true,
        None,
        None,
        OneClickEntryProgress::initial(None),
    )
}

fn one_click_login_after_gateway_recovery<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    runtime_choice: Option<&str>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    recovery: crate::runtime::proxy_lifecycle::InterruptedGatewayRecoveryOutcome,
) -> Result<Value, TypedOneClickFailure> {
    let terminal = recovery.into_terminal_record();
    one_click_login_with_options(
        app,
        state,
        lifecycle,
        runtime_choice,
        auth_proof,
        true,
        None,
        None,
        OneClickEntryProgress::initial(terminal.as_ref()),
    )
}

pub(super) fn one_click_login_after_history_handoff<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    runtime_choice: Option<&str>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    expected: config::RuntimeTransactionV2,
) -> Result<Value, TypedOneClickFailure> {
    let dir = config::default_dir();
    config::update_result(&dir, |current| {
        if history_resume_handoff(current).as_ref() != Some(&expected)
            || !super::history_recovery::history_config_authority_matches(current, &expected)
            || current.runtime_transaction.as_ref()
                != Some(&config::RuntimeTransactionRecord::V2(expected.clone()))
        {
            return Err(
                "history resume handoff disappeared, drifted, or retargeted; preserved current state"
                    .into(),
            );
        }
        current.runtime_transaction = None;
        Ok(((), true))
    })
    .map_err(|error| {
        TypedOneClickFailure::new(OneClickFailureKind::Prepare, error)
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
    })?;
    one_click_login_entry(app, state, lifecycle, runtime_choice, auth_proof)
}

pub(crate) fn replay_interrupted_compensation_before_auth<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
) -> Result<bool, String> {
    let cfg = config::load_from(&config::default_dir()).map_err(|error| error.to_string())?;
    replay_interrupted_one_click_compensation(app, state, lifecycle, None, &cfg)
}

pub(crate) fn interrupted_compensation_requires_pre_auth_replay() -> Result<bool, String> {
    config::load_from(&config::default_dir())
        .map(|cfg| cfg.runtime_compensation.is_some())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
pub(super) fn test_begin_replayable_compensation(
    dir: &Path,
    authority: &AuthorityTransaction,
    state: &SharedAppState,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    launch_runtime: ScienceRuntimeIdentity,
    ssh_stub_transaction: Option<crate::runtime::settings::ManagedSshStubTransaction>,
    prior_science_present: bool,
) -> Result<(), String> {
    let rollback = OneClickRollbackContext {
        proxy_action: ProxyAction::Reused,
        sandbox_port: config::load_from(dir)
            .map_err(|error| error.to_string())?
            .sandbox_port,
        launch_runtime,
        launch_token: None,
        launch_environment: ScienceEnvironmentExposure::NotExposed,
        launch_confirmed_stopped: false,
        candidate_stop_proof: ManagedScienceCandidateStopProof::NotRequired,
        ssh_stub_transaction,
        current_kind: OneClickFailureKind::Prepare,
    };
    let (compensation_id, science_adoption_attempt_ids) = persist_compensation_replay_manifest(
        authority,
        state,
        identity,
        &rollback,
        prior_science_present,
        progress,
    )?;
    begin_one_click_compensation_with_id(
        dir,
        identity,
        progress,
        authority.captured_runtime_transaction(),
        compensation_id,
        science_adoption_attempt_ids,
    )
}

/// Sole production one-click runtime entry owner. Recovery is an explicit
/// recapture/decide/effect loop; the affine Gateway handoff is consumed only
/// after the post-effect facts have been recaptured.
pub(crate) fn one_click_login_entry<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    runtime_choice: Option<&str>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
) -> Result<Value, TypedOneClickFailure> {
    let mut finalize_replayed = false;
    let mut finalize_cleanup_replayed = false;
    let mut gateway_recovery = None;
    loop {
        let facts = config::load_from(&config::default_dir()).map_err(|error| {
            typed_one_click_err(OneClickFailureKind::ConfigLoad, error.to_string())
        })?;
        if replay_interrupted_one_click_compensation(&app, &state, lifecycle, auth_proof, &facts)
            .map_err(|error| {
                TypedOneClickFailure::new(
                    OneClickFailureKind::AuthoritySnapshot,
                    format!(
                        "检测到未完成的 durable compensation，安全重放失败并已保留事务：{error}"
                    ),
                )
                .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
            })?
        {
            continue;
        }
        if super::history_recovery::replay_interrupted_history_recovery(&state, &facts).map_err(
            |error| {
                TypedOneClickFailure::new(
                    OneClickFailureKind::AuthoritySnapshot,
                    format!("检测到未完成的 history recovery，安全重放失败并已保留事务：{error}"),
                )
                .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
            },
        )? {
            continue;
        }
        if let Some(handoff) = history_resume_handoff(&facts) {
            return one_click_login_after_history_handoff(
                app,
                state,
                lifecycle,
                runtime_choice,
                auth_proof,
                handoff,
            );
        }
        match decide_one_click_entry_recovery(
            one_click_finalize_pending(&facts),
            finalize_replayed,
            finalize_cleanup_replayed,
            gateway_recovery.is_some(),
        ) {
            OneClickEntryRecoveryDecision::ReplayFinalize => {
                replay_interrupted_one_click_finalize(&state).map_err(|error| {
                    TypedOneClickFailure::new(
                        OneClickFailureKind::AuthoritySnapshot,
                        format!(
                            "检测到未完成的 success finalize，安全重放失败并已保留事务：{error}"
                        ),
                    )
                    .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
                })?;
                finalize_replayed = true;
            }
            OneClickEntryRecoveryDecision::ReplayFinalizeCleanup => {
                retry_pending_authority_cleanup(&state).map_err(|failure| {
                    typed_authority_cleanup_err(
                        OneClickFailureKind::AuthoritySnapshot,
                        failure,
                        false,
                    )
                })?;
                finalize_cleanup_replayed = true;
            }
            OneClickEntryRecoveryDecision::RecoverGateway => {
                gateway_recovery = Some(
                    crate::runtime::proxy_lifecycle::recover_interrupted_gateway(&app, &state)
                        .map_err(typed_interrupted_gateway_recovery_error)?,
                );
            }
            OneClickEntryRecoveryDecision::Route => {
                let recovery = gateway_recovery.ok_or_else(|| {
                    typed_one_click_err(
                        OneClickFailureKind::GatewayStart,
                        "runtime entry lost the typed Gateway recovery outcome",
                    )
                })?;
                return one_click_login_after_gateway_recovery(
                    app,
                    state,
                    lifecycle,
                    runtime_choice,
                    auth_proof,
                    recovery,
                );
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum PriorScienceDisposition {
    #[default]
    RestartRequired,
    Restored,
    EnvironmentUncertain,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum ReconcileScienceError {
    PriorScienceRestored { cause: String },
    EnvironmentUncertain { cause: String },
    RestartRequired { cause: String },
}

#[allow(dead_code)]
impl ReconcileScienceError {
    pub(crate) fn cause(&self) -> &str {
        match self {
            Self::PriorScienceRestored { cause }
            | Self::EnvironmentUncertain { cause }
            | Self::RestartRequired { cause } => cause,
        }
    }

    pub(crate) fn prior_science_restored(&self) -> bool {
        matches!(self, Self::PriorScienceRestored { .. })
    }

    pub(crate) fn environment_uncertain(&self) -> bool {
        matches!(self, Self::EnvironmentUncertain { .. })
    }
}

#[allow(dead_code)]
pub(crate) fn reconcile_science_for_active<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    profile_switch_transaction: &config::RuntimeTransactionV2,
) -> Result<Value, ReconcileScienceError> {
    let mut disposition = PriorScienceDisposition::RestartRequired;
    one_click_login_with_options(
        app,
        state,
        lifecycle,
        None,
        auth_proof,
        false,
        Some(profile_switch_transaction),
        Some(&mut disposition),
        OneClickEntryProgress::initial(None),
    )
    .map_err(|failure| {
        let cause = failure.safe_detail;
        match disposition {
            PriorScienceDisposition::Restored => {
                ReconcileScienceError::PriorScienceRestored { cause }
            }
            PriorScienceDisposition::EnvironmentUncertain => {
                ReconcileScienceError::EnvironmentUncertain { cause }
            }
            PriorScienceDisposition::RestartRequired => {
                ReconcileScienceError::RestartRequired { cause }
            }
        }
    })
}

/// Rollback-only recovery path. The persisted config is already the old,
/// authoritative profile. Do not trust its previous runtime binding to decide
/// reuse: a healthy process may actually have loaded the failed candidate
/// catalog. Stop only the exact in-memory Science identity and start the
/// committed chain again from a clean process.
#[allow(clippy::result_large_err)]
pub(crate) fn force_restart_science_for_active<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
) -> Result<Value, TypedOneClickFailure> {
    let cfg = config::load_from(&config::default_dir())
        .map_err(|error| typed_one_click_err(OneClickFailureKind::ConfigLoad, error.to_string()))?;
    let (remembered, confirmed_stopped) = {
        let current = lock(&state);
        (
            current.science_runtime.clone(),
            current.science_confirmed_stopped.clone(),
        )
    };
    match remembered {
        Some(runtime) => match ScienceHostAdapter::probe_known(cfg.sandbox_port, &runtime) {
            SandboxScienceState::RunningHealthy => {
                execute_transaction_science_stop_with(
                    &state,
                    lifecycle,
                    TransactionScienceStopBoundary::ProfileSwitchRollback,
                    &runtime,
                    cfg.sandbox_port,
                    || {
                        let receipt =
                            ScienceHostAdapter::managed_receipt(cfg.sandbox_port, &runtime)
                                .ok_or_else(|| {
                                    crate::runtime::science::ScienceStopFailure::request_rejected(
                                        "回滚时无法取得候选 Science 的精确受管启动身份。",
                                    )
                                })?;
                        Ok(ScienceStopRequest::exact(
                            &runtime,
                            ScienceStopOwnershipReceipt::from_managed_launch(&receipt),
                        ))
                    },
                    |request| ScienceHostAdapter::execute_stop(&app, request).into_parts(),
                    |_state, _confirmed_runtime| {},
                )
                .map_err(|error| {
                    typed_one_click_err(
                        OneClickFailureKind::ScienceStop,
                        format!(
                            "回滚时停止候选 Science 失败，未猜测 PID 或按端口结束进程：{error}"
                        ),
                    )
                })?;
            }
            SandboxScienceState::Stopped => {
                return Err(typed_one_click_err(
                    OneClickFailureKind::ScienceStop,
                    "回滚时仅确认 Science 端口已关闭，未取得精确停止 receipt；已拒绝继续恢复 authority。",
                ));
            }
            SandboxScienceState::Unknown => {
                return Err(typed_one_click_err(
                    OneClickFailureKind::ScienceStop,
                    "回滚时 Science 可能正在运行，但身份无法确认；已拒绝猜测 PID 或按端口结束进程。",
                ));
            }
        },
        None if confirmed_stopped.is_some()
            && !proc::loopback_port_in_use(
                cfg.sandbox_port,
                operation::LOCAL_HEALTH_TIMEOUT_MS,
            ) => {}
        None if proc::loopback_port_in_use(
            cfg.sandbox_port,
            operation::LOCAL_HEALTH_TIMEOUT_MS,
        ) =>
        {
            return Err(typed_one_click_err(
                OneClickFailureKind::ScienceStop,
                "回滚时 Science 端口仍被占用，但没有可确认的 runtime 身份；已拒绝强制结束。",
            ));
        }
        None => {
            return Err(typed_one_click_err(
                OneClickFailureKind::ScienceStop,
                "回滚时 Science 端口已关闭，但没有 verified-stopped receipt；已拒绝继续恢复 authority。",
            ));
        }
    }
    one_click_login_with_options(
        app,
        state,
        lifecycle,
        None,
        auth_proof,
        false,
        None,
        None,
        OneClickEntryProgress::initial(None),
    )
}

fn typed_one_click_err(
    kind: OneClickFailureKind,
    message: impl Into<String>,
) -> TypedOneClickFailure {
    TypedOneClickFailure::new(kind, message)
}

#[derive(Debug)]
pub(super) struct InterruptedScienceRecoveryError {
    safe_detail: &'static str,
    recovery: ProjectedRecovery,
}

impl InterruptedScienceRecoveryError {
    fn new(safe_detail: &'static str, recovery: ProjectedRecovery) -> Self {
        Self {
            safe_detail,
            recovery,
        }
    }

    #[allow(dead_code)]
    pub(super) fn projected_recovery(&self) -> ProjectedRecovery {
        self.recovery
    }
}

impl std::fmt::Display for InterruptedScienceRecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.safe_detail)
    }
}

fn typed_interrupted_science_err(
    kind: OneClickFailureKind,
    error: InterruptedScienceRecoveryError,
) -> TypedOneClickFailure {
    TypedOneClickFailure::new(kind, error.safe_detail).with_recovery(error.recovery)
}

fn typed_authority_cleanup_err(
    kind: OneClickFailureKind,
    failure: AuthorityCleanupFailure,
    environment_uncertain: bool,
) -> TypedOneClickFailure {
    let cleanup_required = failure.cleanup_requirement().is_some();
    let phase = failure.phase();
    let message = failure.to_string();
    let recovery = if cleanup_required && environment_uncertain {
        ProjectedRecovery::cleanup_required_uncertain()
    } else if cleanup_required {
        ProjectedRecovery::CLEANUP_REQUIRED
    } else if environment_uncertain {
        ProjectedRecovery::ENVIRONMENT_UNCERTAIN
    } else {
        ProjectedRecovery::NOT_NEEDED
    };
    TypedOneClickFailure::new(kind, message)
        .with_recovery(recovery)
        .with_safe_cause(phase.cause_code(), "authority cleanup typed failure")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OneClickTransactionIdentity {
    pub(super) target_profile_id: String,
    pub(super) runtime_fingerprint: String,
    pub(super) snapshot_ticket: config::RuntimeSnapshotTicket,
    pub(super) previous_binding: Option<config::RuntimeBindingCommit>,
    pub(super) profile_switch_handoff: Option<config::RuntimeTransactionV2>,
    pub(super) gateway_terminal_handoff: Option<config::RuntimeTransactionV2>,
    pub(super) prior_stop: config::RuntimePriorStopState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum OneClickJournalProgress {
    PreJournalAbort {
        registered_ticket: config::RuntimeSnapshotTicket,
        runtime_transaction: Box<Option<config::RuntimeTransactionRecord>>,
    },
    Journaled {
        record: config::RuntimeTransactionV2,
        registered_ticket: config::RuntimeSnapshotTicket,
    },
    Compensating {
        active_runtime_transaction: Box<Option<config::RuntimeTransactionRecord>>,
        restored_runtime_transaction: Box<Option<config::RuntimeTransactionRecord>>,
        expected_runtime_transaction: Box<Option<config::RuntimeTransactionRecord>>,
        compensation: Box<config::RuntimeCompensationJournal>,
        registered_ticket: config::RuntimeSnapshotTicket,
    },
    CompensationFinished {
        registered_ticket: config::RuntimeSnapshotTicket,
    },
    Finalized {
        record: config::RuntimeTransactionV2,
        registered_ticket: config::RuntimeSnapshotTicket,
    },
}

impl OneClickJournalProgress {
    fn registered_ticket(&self) -> &config::RuntimeSnapshotTicket {
        match self {
            Self::PreJournalAbort {
                registered_ticket, ..
            }
            | Self::Journaled {
                registered_ticket, ..
            }
            | Self::Compensating {
                registered_ticket, ..
            }
            | Self::CompensationFinished { registered_ticket }
            | Self::Finalized {
                registered_ticket, ..
            } => registered_ticket,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn transaction_id(&self) -> Option<&str> {
        match self {
            Self::PreJournalAbort {
                runtime_transaction,
                ..
            } => runtime_transaction
                .as_ref()
                .as_ref()
                .map(config::RuntimeTransactionRecord::transaction_id),
            Self::Journaled { record, .. } | Self::Finalized { record, .. } => {
                Some(&record.transaction_id)
            }
            Self::Compensating {
                active_runtime_transaction,
                ..
            } => active_runtime_transaction
                .as_ref()
                .as_ref()
                .map(config::RuntimeTransactionRecord::transaction_id),
            Self::CompensationFinished { .. } => None,
        }
    }

    pub(super) fn journaled_record(&self) -> Option<&config::RuntimeTransactionV2> {
        match self {
            Self::PreJournalAbort { .. }
            | Self::Compensating { .. }
            | Self::CompensationFinished { .. }
            | Self::Finalized { .. } => None,
            Self::Journaled { record, .. } => Some(record),
        }
    }

    fn restore_expectation(&self) -> RuntimeTransactionRestoreExpectation {
        match self {
            Self::PreJournalAbort {
                runtime_transaction,
                ..
            } => RuntimeTransactionRestoreExpectation::Exact(runtime_transaction.as_ref().clone()),
            Self::Journaled { record, .. } => RuntimeTransactionRestoreExpectation::Exact(Some(
                config::RuntimeTransactionRecord::V2(record.clone()),
            )),
            Self::Compensating {
                expected_runtime_transaction,
                compensation,
                ..
            } => RuntimeTransactionRestoreExpectation::ExactPreservingCompensation {
                runtime_transaction: expected_runtime_transaction.as_ref().clone(),
                compensation: compensation.as_ref().clone(),
            },
            Self::CompensationFinished { .. } => RuntimeTransactionRestoreExpectation::Exact(None),
            Self::Finalized { .. } => RuntimeTransactionRestoreExpectation::Exact(None),
        }
    }
}

pub(super) fn one_click_phase_exposure(
    phase: config::RuntimeTransactionPhase,
) -> config::RuntimeEnvironmentExposure {
    match phase {
        config::RuntimeTransactionPhase::StartScienceEnvironmentPending => {
            config::RuntimeEnvironmentExposure::Possible
        }
        config::RuntimeTransactionPhase::WaitScienceDbReverify
        | config::RuntimeTransactionPhase::RestartScienceAfterDbHeal
        | config::RuntimeTransactionPhase::VerifyScienceDbAfterRestart
        | config::RuntimeTransactionPhase::VerifyScienceCatalog => {
            config::RuntimeEnvironmentExposure::Exposed
        }
        _ => config::RuntimeEnvironmentExposure::NotExposed,
    }
}

fn one_click_journal_matches(
    journal: &config::RuntimeTransactionV2,
    identity: &OneClickTransactionIdentity,
    transaction_id: &str,
) -> bool {
    journal.schema_version == config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2
        && journal.transaction_id == transaction_id
        && journal.operation == config::RuntimeTransactionOperation::OneClick
        && journal.target_profile_id == identity.target_profile_id
        && journal.runtime_fingerprint.as_deref() == Some(&identity.runtime_fingerprint)
        && journal.snapshot_ticket.as_ref() == Some(&identity.snapshot_ticket)
        && journal.previous_binding.as_ref() == identity.previous_binding.as_ref()
        && journal.previous_gateway.is_none()
        && journal.compensation == config::RuntimeCompensationState::NotStarted
        && journal.gateway_stop_outcome == config::RuntimeGatewayStopOutcome::NotAttempted
        && journal.environment_exposure == one_click_phase_exposure(journal.phase)
        && journal.prior_stop == identity.prior_stop
        && journal.finalize == config::RuntimeFinalizeState::NotStarted
}

fn one_click_prior_stop_record_matches(
    journal: &config::RuntimeTransactionV2,
    identity: &OneClickTransactionIdentity,
    transaction_id: &str,
) -> bool {
    journal.schema_version == config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2
        && journal.transaction_id == transaction_id
        && journal.operation == config::RuntimeTransactionOperation::OneClick
        && journal.target_profile_id == identity.target_profile_id
        && journal.phase == config::RuntimeTransactionPhase::StopOldScience
        && journal.runtime_fingerprint.as_deref() == Some(&identity.runtime_fingerprint)
        && journal.snapshot_ticket.is_none()
        && journal.previous_binding.as_ref() == identity.previous_binding.as_ref()
        && journal.previous_gateway.is_none()
        && journal.compensation == config::RuntimeCompensationState::NotStarted
        && journal.gateway_stop_outcome == config::RuntimeGatewayStopOutcome::NotAttempted
        && journal.environment_exposure == config::RuntimeEnvironmentExposure::NotExposed
        && journal.prior_stop == identity.prior_stop
        && matches!(
            journal.prior_stop,
            config::RuntimePriorStopState::Outcome {
                outcome: config::RuntimePriorStopOutcome::ExactStopped,
                ..
            }
        )
        && journal.finalize == config::RuntimeFinalizeState::NotStarted
}

fn profile_switch_handoff_matches(
    journal: &config::RuntimeTransactionV2,
    expected: &config::RuntimeTransactionV2,
    active_profile_id: &str,
    current_binding: Option<&config::RuntimeBindingCommit>,
) -> bool {
    journal == expected
        && expected.schema_version == config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2
        && expected.operation == config::RuntimeTransactionOperation::ProfileSwitch
        && expected.target_profile_id == active_profile_id
        && expected.phase == config::RuntimeTransactionPhase::StartFormalGateway
        && expected.runtime_fingerprint.is_none()
        && expected.environment_exposure == config::RuntimeEnvironmentExposure::NotExposed
        && expected.snapshot_ticket.is_none()
        && expected.previous_binding.as_ref() == current_binding
        && expected.compensation == config::RuntimeCompensationState::NotStarted
        && expected.gateway_stop_outcome == config::RuntimeGatewayStopOutcome::NotAttempted
}

fn config_authority_matches(
    current: &config::Config,
    target_profile_id: &str,
    previous_binding: Option<&config::RuntimeBindingCommit>,
) -> bool {
    current.active_id == target_profile_id && current.runtime_binding.as_ref() == previous_binding
}

fn gateway_terminal_handoff_matches(
    journal: &config::RuntimeTransactionV2,
    expected: &config::RuntimeTransactionV2,
    active_profile_id: &str,
    current_binding: Option<&config::RuntimeBindingCommit>,
) -> bool {
    journal == expected
        && expected.schema_version == config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2
        && expected.operation == config::RuntimeTransactionOperation::ProfileSwitch
        && expected.target_profile_id == active_profile_id
        && expected.phase == config::RuntimeTransactionPhase::RecoverInterruptedGateway
        && expected.runtime_fingerprint.is_none()
        && expected.snapshot_ticket.is_none()
        && expected.previous_binding.as_ref() == current_binding
        && expected.environment_exposure == config::RuntimeEnvironmentExposure::NotExposed
        && expected.compensation == config::RuntimeCompensationState::NotStarted
        && matches!(
            expected.gateway_stop_outcome,
            config::RuntimeGatewayStopOutcome::Stopped
                | config::RuntimeGatewayStopOutcome::AbsentAfterAttempt
        )
        && expected.prior_stop == config::RuntimePriorStopState::NotRequired
        && expected.finalize == config::RuntimeFinalizeState::NotStarted
}

pub(super) fn resolve_gateway_terminal_handoff(
    journal: Option<&config::RuntimeTransactionRecord>,
    expected: Option<&config::RuntimeTransactionV2>,
    active_profile_id: &str,
    current_binding: Option<&config::RuntimeBindingCommit>,
) -> Result<Option<config::RuntimeTransactionV2>, &'static str> {
    match expected {
        Some(expected) => match journal {
            Some(config::RuntimeTransactionRecord::V2(typed))
                if gateway_terminal_handoff_matches(
                    typed,
                    expected,
                    active_profile_id,
                    current_binding,
                ) =>
            {
                Ok(Some(expected.clone()))
            }
            _ => Err("interrupted-Gateway terminal handoff disappeared, drifted, or retargeted"),
        },
        None => Ok(None),
    }
}

pub(super) fn healthy_reopen_transaction_matches(
    journal: Option<&config::RuntimeTransactionRecord>,
    expected_profile_switch_transaction: Option<&config::RuntimeTransactionV2>,
    expected_gateway_terminal_handoff: Option<&config::RuntimeTransactionV2>,
    active_profile_id: &str,
    current_binding: Option<&config::RuntimeBindingCommit>,
) -> bool {
    if let Some(expected) = expected_gateway_terminal_handoff {
        return expected_profile_switch_transaction.is_none()
            && matches!(
                journal,
                Some(config::RuntimeTransactionRecord::V2(typed))
                    if gateway_terminal_handoff_matches(
                        typed,
                        expected,
                        active_profile_id,
                        current_binding,
                    )
            );
    }
    match expected_profile_switch_transaction {
        Some(expected) => matches!(
            journal,
            Some(config::RuntimeTransactionRecord::V2(typed))
                if profile_switch_handoff_matches(
                    typed,
                    expected,
                    active_profile_id,
                    current_binding,
                )
        ),
        None => !journal.is_some_and(config::RuntimeTransactionRecord::is_v2),
    }
}

pub(super) fn resolve_profile_switch_handoff(
    journal: Option<&config::RuntimeTransactionRecord>,
    expected_profile_switch_transaction: Option<&config::RuntimeTransactionV2>,
    reconcile_expected: bool,
    active_profile_id: &str,
    current_binding: Option<&config::RuntimeBindingCommit>,
) -> Result<Option<config::RuntimeTransactionV2>, &'static str> {
    match expected_profile_switch_transaction {
        Some(expected) if reconcile_expected => match journal {
            Some(config::RuntimeTransactionRecord::V2(typed))
                if profile_switch_handoff_matches(
                    typed,
                    expected,
                    active_profile_id,
                    current_binding,
                ) =>
            {
                Ok(Some(expected.clone()))
            }
            _ => Err(
                "profile-switch handoff journal disappeared, regressed, or retargeted before reconcile",
            ),
        },
        Some(_) => Err("profile-switch handoff was supplied outside the reconcile path"),
        None if journal.is_some_and(config::RuntimeTransactionRecord::is_v2) => {
            Err("interrupted typed runtime journal requires manual recovery")
        }
        None => Ok(None),
    }
}

pub(super) fn commit_healthy_reopen_binding(
    dir: &Path,
    expected_profile_switch_transaction: Option<&config::RuntimeTransactionV2>,
    expected_gateway_terminal_handoff: Option<&config::RuntimeTransactionV2>,
    committed: &config::RuntimeBindingCommit,
) -> Result<(), String> {
    config::update_result(dir, |config| {
        if !healthy_reopen_transaction_matches(
            config.runtime_transaction.as_ref(),
            expected_profile_switch_transaction,
            expected_gateway_terminal_handoff,
            &config.active_id,
            config.runtime_binding.as_ref(),
        ) {
            return Err(
                "runtime journal retargeted healthy reopen; preserved the current transaction"
                    .into(),
            );
        }
        config.runtime_binding = Some(committed.clone());
        config.runtime_transaction = None;
        Ok(((), true))
    })
}

fn new_one_click_journal(
    identity: &OneClickTransactionIdentity,
    transaction_id: String,
    phase: config::RuntimeTransactionPhase,
) -> config::RuntimeTransactionV2 {
    config::RuntimeTransactionV2 {
        schema_version: config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2,
        transaction_id,
        operation: config::RuntimeTransactionOperation::OneClick,
        target_profile_id: identity.target_profile_id.clone(),
        phase,
        runtime_fingerprint: Some(identity.runtime_fingerprint.clone()),
        environment_exposure: one_click_phase_exposure(phase),
        snapshot_ticket: Some(identity.snapshot_ticket.clone()),
        previous_binding: identity.previous_binding.clone(),
        previous_gateway: None,
        compensation: config::RuntimeCompensationState::NotStarted,
        gateway_stop_outcome: config::RuntimeGatewayStopOutcome::NotAttempted,
        prior_stop: identity.prior_stop.clone(),
        finalize: config::RuntimeFinalizeState::NotStarted,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn begin_prior_stop_intent(
    dir: &Path,
    target_profile_id: &str,
    runtime_fingerprint: &str,
    previous_binding: Option<&config::RuntimeBindingCommit>,
    profile_switch_handoff: Option<&config::RuntimeTransactionV2>,
    gateway_terminal_handoff: Option<&config::RuntimeTransactionV2>,
    recipe: config::RuntimePriorScienceRecipe,
) -> Result<config::RuntimeTransactionV2, String> {
    config::update_result(dir, |current| {
        let current_handoff_matches = match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal)) => {
                profile_switch_handoff.is_some_and(|expected| {
                    profile_switch_handoff_matches(
                        journal,
                        expected,
                        target_profile_id,
                        previous_binding,
                    )
                }) || gateway_terminal_handoff.is_some_and(|expected| {
                    gateway_terminal_handoff_matches(
                        journal,
                        expected,
                        &current.active_id,
                        current.runtime_binding.as_ref(),
                    )
                })
            }
            _ => false,
        };
        let config_authority_matches =
            config_authority_matches(current, target_profile_id, previous_binding);
        let no_handoff_expected =
            profile_switch_handoff.is_none() && gateway_terminal_handoff.is_none();
        let replace_allowed = config_authority_matches
            && (current_handoff_matches
                || (no_handoff_expected
                    && matches!(
                        current.runtime_transaction.as_ref(),
                        None | Some(config::RuntimeTransactionRecord::V1(_))
                    )));
        if !replace_allowed {
            return Err(
                "prior Science stop intent found a drifted runtime handoff; preserved the current transaction"
                    .into(),
            );
        }
        let record = config::RuntimeTransactionV2 {
            schema_version: config::RUNTIME_TRANSACTION_SCHEMA_VERSION_V2,
            transaction_id: config::new_id(),
            operation: config::RuntimeTransactionOperation::OneClick,
            target_profile_id: target_profile_id.to_string(),
            phase: config::RuntimeTransactionPhase::StopOldScience,
            runtime_fingerprint: Some(runtime_fingerprint.to_string()),
            environment_exposure: config::RuntimeEnvironmentExposure::NotExposed,
            snapshot_ticket: None,
            previous_binding: previous_binding.cloned(),
            previous_gateway: None,
            compensation: config::RuntimeCompensationState::NotStarted,
            gateway_stop_outcome: config::RuntimeGatewayStopOutcome::NotAttempted,
            prior_stop: config::RuntimePriorStopState::Intent { recipe },
            finalize: config::RuntimeFinalizeState::NotStarted,
        };
        current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(record.clone()));
        Ok((record, true))
    })
}

pub(super) fn publish_prior_stop_outcome(
    dir: &Path,
    expected: &config::RuntimeTransactionV2,
    outcome: config::RuntimePriorStopOutcome,
) -> Result<config::RuntimeTransactionV2, String> {
    let recipe = match &expected.prior_stop {
        config::RuntimePriorStopState::Intent { recipe } => recipe.clone(),
        _ => return Err("prior Science stop outcome has no matching durable intent".into()),
    };
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &expected.target_profile_id,
            expected.previous_binding.as_ref(),
        ) || current.runtime_transaction.as_ref()
            != Some(&config::RuntimeTransactionRecord::V2(expected.clone()))
        {
            return Err(
                "prior Science stop outcome found a drifted intent; preserved the current transaction"
                    .into(),
            );
        }
        let mut record = expected.clone();
        record.prior_stop = config::RuntimePriorStopState::Outcome { recipe, outcome };
        current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(record.clone()));
        Ok((record, true))
    })
}

fn clear_prior_stop_transition(
    dir: &Path,
    expected: &config::RuntimeTransactionV2,
) -> Result<(), String> {
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &expected.target_profile_id,
            expected.previous_binding.as_ref(),
        ) || current.runtime_transaction.as_ref()
            != Some(&config::RuntimeTransactionRecord::V2(expected.clone()))
        {
            return Err(
                "prior Science restart found a drifted stop outcome; preserved the current transaction"
                    .into(),
            );
        }
        current.runtime_transaction = None;
        Ok(((), true))
    })
}

pub(super) fn write_one_click_checkpoint(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    phase: config::RuntimeTransactionPhase,
) -> Result<(), String> {
    if progress.registered_ticket() != &identity.snapshot_ticket {
        return Err("one-click checkpoint rejected a replaced in-memory snapshot ticket".into());
    }
    #[cfg(test)]
    if progress.journaled_record().is_none()
        || progress
            .journaled_record()
            .is_some_and(|record| record.snapshot_ticket.is_none())
    {
        let mut seams = SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if seams.one_click_fail_first_journal.as_deref() == Some(dir) {
            seams.one_click_fail_first_journal = None;
            return Err("test-only one-click first journal write failure".into());
        }
    }
    let expected_record = progress.journaled_record().cloned();
    let next_record = config::update_result(dir, |current| {
        let profile_switch_handoff_matches = match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal)) => identity
                .profile_switch_handoff
                .as_ref()
                .is_some_and(|expected| {
                    profile_switch_handoff_matches(
                        journal,
                        expected,
                        &identity.target_profile_id,
                        identity.previous_binding.as_ref(),
                    )
                }),
            _ => false,
        };
        let gateway_terminal_handoff_matches = match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal)) => identity
                .gateway_terminal_handoff
                .as_ref()
                .is_some_and(|expected| {
                    gateway_terminal_handoff_matches(
                        journal,
                        expected,
                        &current.active_id,
                        current.runtime_binding.as_ref(),
                    )
                }),
            _ => false,
        };
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) {
            return Err(
                "one-click checkpoint config authority drifted; preserved the typed transaction"
                    .into(),
            );
        }
        match (
            current.runtime_transaction.as_ref(),
            expected_record.as_ref(),
        ) {
            (Some(config::RuntimeTransactionRecord::V2(journal)), Some(expected))
                if journal == expected
                    && (one_click_journal_matches(journal, identity, &expected.transaction_id)
                        || one_click_prior_stop_record_matches(
                            journal,
                            identity,
                            &expected.transaction_id,
                        )) =>
            {
                let mut next = expected.clone();
                next.phase = phase;
                next.environment_exposure = one_click_phase_exposure(phase);
                next.snapshot_ticket = Some(identity.snapshot_ticket.clone());
                current.runtime_transaction =
                    Some(config::RuntimeTransactionRecord::V2(next.clone()));
                Ok((next, true))
            }
            (Some(config::RuntimeTransactionRecord::V2(_)), None)
                if profile_switch_handoff_matches || gateway_terminal_handoff_matches =>
            {
                let transaction_id = config::new_id();
                let next = new_one_click_journal(identity, transaction_id, phase);
                current.runtime_transaction =
                    Some(config::RuntimeTransactionRecord::V2(next.clone()));
                Ok((next, true))
            }
            (Some(config::RuntimeTransactionRecord::V2(_)), _) => {
                Err("one-click checkpoint identity changed; preserved the typed transaction".into())
            }
            (Some(config::RuntimeTransactionRecord::V1(_)) | None, None)
                if identity.profile_switch_handoff.is_none()
                    && identity.gateway_terminal_handoff.is_none() =>
            {
                let transaction_id = config::new_id();
                let next = new_one_click_journal(identity, transaction_id, phase);
                current.runtime_transaction =
                    Some(config::RuntimeTransactionRecord::V2(next.clone()));
                Ok((next, true))
            }
            (Some(config::RuntimeTransactionRecord::V1(_)) | None, None) => {
                Err("runtime handoff journal disappeared or regressed; refused replacement".into())
            }
            (Some(config::RuntimeTransactionRecord::V1(_)) | None, Some(_)) => Err(
                "one-click checkpoint journal disappeared or regressed; refused replacement".into(),
            ),
        }
    })?;
    *progress = OneClickJournalProgress::Journaled {
        record: next_record,
        registered_ticket: identity.snapshot_ticket.clone(),
    };
    Ok(())
}

#[cfg(test)]
pub(super) fn begin_one_click_compensation(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    restored_runtime_transaction: Option<config::RuntimeTransactionRecord>,
) -> Result<(), String> {
    begin_one_click_compensation_with_id(
        dir,
        identity,
        progress,
        restored_runtime_transaction,
        config::new_id(),
        Vec::new(),
    )
}

fn begin_one_click_compensation_with_id(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    restored_runtime_transaction: Option<config::RuntimeTransactionRecord>,
    compensation_id: String,
    science_adoption_attempt_ids: Vec<String>,
) -> Result<(), String> {
    let registered_ticket = progress.registered_ticket().clone();
    let active_runtime_transaction = match progress {
        OneClickJournalProgress::PreJournalAbort {
            runtime_transaction,
            ..
        } => runtime_transaction.as_ref().clone(),
        OneClickJournalProgress::Journaled { record, .. } => {
            let snapshot_identity_matches = record.snapshot_ticket.as_ref()
                == Some(&registered_ticket)
                || (record.snapshot_ticket.is_none()
                    && one_click_prior_stop_record_matches(
                        record,
                        identity,
                        &record.transaction_id,
                    ));
            if record.operation != config::RuntimeTransactionOperation::OneClick
                || record.runtime_fingerprint.as_deref()
                    != Some(identity.runtime_fingerprint.as_str())
                || !snapshot_identity_matches
                || record.previous_gateway.is_some()
                || record.compensation != config::RuntimeCompensationState::NotStarted
                || record.gateway_stop_outcome != config::RuntimeGatewayStopOutcome::NotAttempted
                || record.environment_exposure != one_click_phase_exposure(record.phase)
                || record.finalize != config::RuntimeFinalizeState::NotStarted
            {
                return Err("one-click compensation intent rejected an ineligible journal".into());
            }
            Some(config::RuntimeTransactionRecord::V2(record.clone()))
        }
        OneClickJournalProgress::Compensating { .. }
        | OneClickJournalProgress::CompensationFinished { .. }
        | OneClickJournalProgress::Finalized { .. } => {
            return Err("one-click compensation intent was already published".into())
        }
    };
    if identity.snapshot_ticket != registered_ticket {
        return Err("one-click compensation intent rejected a replaced snapshot ticket".into());
    }
    let _publication_lease = config::acquire_runtime_compensation_publication_lease(dir)
        .map_err(|error| format!("one-click compensation publication fence failed: {error}"))?;
    let compensation = config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) || current.runtime_transaction != active_runtime_transaction
            || current.runtime_compensation.is_some()
        {
            return Err(
                "one-click compensation intent found a drifted runtime journal; preserved the current transaction"
                    .into(),
            );
        }
        let next = config::RuntimeCompensationJournal {
            schema_version: config::RUNTIME_COMPENSATION_SCHEMA_VERSION_V2,
            compensation_id: compensation_id.clone(),
            target_profile_id: identity.target_profile_id.clone(),
            runtime_fingerprint: identity.runtime_fingerprint.clone(),
            snapshot_ticket: identity.snapshot_ticket.clone(),
            state: config::RuntimeCompensationState::InProgress,
            steps: config::pending_one_click_compensation_steps(),
            science_adoption_attempt_ids: science_adoption_attempt_ids.clone(),
        };
        current.runtime_compensation = Some(next.clone());
        Ok((next, true))
    })?;
    *progress = OneClickJournalProgress::Compensating {
        expected_runtime_transaction: Box::new(active_runtime_transaction.clone()),
        active_runtime_transaction: Box::new(active_runtime_transaction),
        restored_runtime_transaction: Box::new(restored_runtime_transaction),
        compensation: Box::new(compensation),
        registered_ticket,
    };
    Ok(())
}

fn update_one_click_compensation_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    step: config::RuntimeCompensationStep,
    expected_outcome: config::RuntimeCompensationStepState,
    next_outcome: config::RuntimeCompensationStepState,
) -> Result<(), String> {
    let (
        active_runtime_transaction,
        restored_runtime_transaction,
        expected_runtime_transaction,
        expected,
        registered_ticket,
    ) = match progress {
        OneClickJournalProgress::Compensating {
            active_runtime_transaction,
            restored_runtime_transaction,
            expected_runtime_transaction,
            compensation,
            registered_ticket,
        } => (
            active_runtime_transaction.as_ref().clone(),
            restored_runtime_transaction.as_ref().clone(),
            expected_runtime_transaction.as_ref().clone(),
            compensation.as_ref().clone(),
            registered_ticket.clone(),
        ),
        _ => return Err("one-click compensation step has no durable intent".into()),
    };
    if expected.schema_version != config::RUNTIME_COMPENSATION_SCHEMA_VERSION_V2
        || expected.state != config::RuntimeCompensationState::InProgress
    {
        return Err("one-click compensation step requires an active V2 journal".into());
    }
    let transition_valid = matches!(
        (expected_outcome, next_outcome),
        (
            config::RuntimeCompensationStepState::Pending,
            config::RuntimeCompensationStepState::InProgress
        )
    ) || (expected_outcome
        == config::RuntimeCompensationStepState::InProgress
        && next_outcome.is_terminal());
    if !transition_valid {
        return Err("one-click compensation step transition is invalid".into());
    }
    let Some(step_index) = expected
        .steps
        .iter()
        .position(|candidate| candidate.step == step)
    else {
        return Err("one-click compensation step is not part of the durable plan".into());
    };
    if expected.steps[step_index].outcome != expected_outcome
        || expected.steps[..step_index]
            .iter()
            .any(|candidate| !candidate.outcome.is_terminal())
        || expected.steps[step_index + 1..]
            .iter()
            .any(|candidate| candidate.outcome != config::RuntimeCompensationStepState::Pending)
    {
        return Err("one-click compensation step rejected non-canonical progress".into());
    }
    let next = config::update_result(dir, |current| {
        if current.runtime_compensation.as_ref() != Some(&expected)
            || current.runtime_transaction != expected_runtime_transaction
        {
            return Err(
                "one-click compensation step found a drifted runtime journal; preserved the current transaction"
                    .into(),
            );
        }
        let mut record = expected.clone();
        record.steps[step_index].outcome = next_outcome;
        current.runtime_compensation = Some(record.clone());
        Ok((record, true))
    })?;
    *progress = OneClickJournalProgress::Compensating {
        active_runtime_transaction: Box::new(active_runtime_transaction),
        restored_runtime_transaction: Box::new(restored_runtime_transaction),
        expected_runtime_transaction: Box::new(expected_runtime_transaction),
        compensation: Box::new(next),
        registered_ticket,
    };
    Ok(())
}

pub(super) fn begin_one_click_compensation_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    step: config::RuntimeCompensationStep,
) -> Result<(), String> {
    update_one_click_compensation_step(
        dir,
        progress,
        step,
        config::RuntimeCompensationStepState::Pending,
        config::RuntimeCompensationStepState::InProgress,
    )
}

pub(super) fn finish_one_click_compensation_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    step: config::RuntimeCompensationStep,
    outcome: config::RuntimeCompensationStepState,
) -> Result<(), String> {
    if step == config::RuntimeCompensationStep::AuthorityRestore {
        return Err(
            "authority restore outcome requires the dedicated business-record transition".into(),
        );
    }
    update_one_click_compensation_step(
        dir,
        progress,
        step,
        config::RuntimeCompensationStepState::InProgress,
        outcome,
    )
}

pub(super) fn finish_one_click_authority_restore_step(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
    outcome: config::RuntimeCompensationStepState,
) -> Result<(), String> {
    if !outcome.is_terminal() {
        return Err("authority restore outcome must be terminal".into());
    }
    let (
        active_runtime_transaction,
        restored_runtime_transaction,
        expected_runtime_transaction,
        expected,
        registered_ticket,
    ) = match progress {
        OneClickJournalProgress::Compensating {
            active_runtime_transaction,
            restored_runtime_transaction,
            expected_runtime_transaction,
            compensation,
            registered_ticket,
        } => (
            active_runtime_transaction.as_ref().clone(),
            restored_runtime_transaction.as_ref().clone(),
            expected_runtime_transaction.as_ref().clone(),
            compensation.as_ref().clone(),
            registered_ticket.clone(),
        ),
        _ => return Err("authority restore outcome has no durable intent".into()),
    };
    if expected.schema_version != config::RUNTIME_COMPENSATION_SCHEMA_VERSION_V2
        || expected.state != config::RuntimeCompensationState::InProgress
        || expected_runtime_transaction != active_runtime_transaction
    {
        return Err("authority restore outcome rejected a crossed business-record boundary".into());
    }
    let Some(step_index) = expected
        .steps
        .iter()
        .position(|candidate| candidate.step == config::RuntimeCompensationStep::AuthorityRestore)
    else {
        return Err("authority restore step is not part of the durable plan".into());
    };
    if expected.steps[step_index].outcome != config::RuntimeCompensationStepState::InProgress
        || expected.steps[..step_index]
            .iter()
            .any(|candidate| !candidate.outcome.is_terminal())
        || expected.steps[step_index + 1..]
            .iter()
            .any(|candidate| candidate.outcome != config::RuntimeCompensationStepState::Pending)
    {
        return Err("authority restore outcome rejected non-canonical progress".into());
    }
    let require_restored = outcome == config::RuntimeCompensationStepState::Succeeded;
    let (next, next_runtime_transaction) = config::update_result(dir, |current| {
        if current.runtime_compensation.as_ref() != Some(&expected) {
            return Err(
                "authority restore outcome found a drifted compensation journal; preserved the current transaction"
                    .into(),
            );
        }
        let observed_runtime_transaction = if current.runtime_transaction
            == restored_runtime_transaction
        {
            restored_runtime_transaction.clone()
        } else if !require_restored && current.runtime_transaction == expected_runtime_transaction {
            expected_runtime_transaction.clone()
        } else {
            return Err(
                "authority restore outcome found a drifted business record; preserved the current transaction"
                    .into(),
            );
        };
        let mut record = expected.clone();
        record.steps[step_index].outcome = outcome;
        current.runtime_compensation = Some(record.clone());
        Ok(((record, observed_runtime_transaction), true))
    })?;
    *progress = OneClickJournalProgress::Compensating {
        active_runtime_transaction: Box::new(active_runtime_transaction),
        restored_runtime_transaction: Box::new(restored_runtime_transaction),
        expected_runtime_transaction: Box::new(next_runtime_transaction),
        compensation: Box::new(next),
        registered_ticket,
    };
    Ok(())
}

pub(super) fn finish_one_click_compensation(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
) -> Result<(), String> {
    let (
        active_runtime_transaction,
        restored_runtime_transaction,
        expected_runtime_transaction,
        expected,
    ) = match progress {
        OneClickJournalProgress::PreJournalAbort { .. }
        | OneClickJournalProgress::CompensationFinished { .. }
        | OneClickJournalProgress::Finalized { .. } => return Ok(()),
        OneClickJournalProgress::Journaled { .. } => {
            return Err("one-click compensation completion has no durable intent".into())
        }
        OneClickJournalProgress::Compensating {
            active_runtime_transaction,
            restored_runtime_transaction,
            expected_runtime_transaction,
            compensation,
            ..
        } => (
            active_runtime_transaction.as_ref().clone(),
            restored_runtime_transaction.as_ref().clone(),
            expected_runtime_transaction.as_ref().clone(),
            compensation.as_ref().clone(),
        ),
    };
    if expected.state != config::RuntimeCompensationState::InProgress {
        return Err("one-click compensation completion has no durable intent".into());
    }
    if expected
        .steps
        .iter()
        .any(|progress| progress.outcome == config::RuntimeCompensationStepState::InProgress)
    {
        return Err("one-click compensation completion found an unfinished step".into());
    }
    let failed_steps = expected
        .steps
        .iter()
        .filter_map(|progress| {
            (progress.outcome == config::RuntimeCompensationStepState::Failed)
                .then_some(progress.step)
        })
        .collect::<Vec<_>>();
    if failed_steps.is_empty()
        && expected
            .steps
            .iter()
            .any(|progress| !progress.outcome.is_terminal())
    {
        return Err("one-click compensation completion found a pending step".into());
    }
    let registered_ticket = progress.registered_ticket().clone();
    let next = config::update_result(dir, |current| {
        if current.runtime_compensation.as_ref() != Some(&expected)
            || current.runtime_transaction != expected_runtime_transaction
        {
            return Err(
                "one-click compensation completion found a drifted runtime journal; preserved the current transaction"
                    .into(),
            );
        }
        if failed_steps.is_empty() {
            current.runtime_compensation = None;
            Ok((None, true))
        } else {
            let mut record = expected.clone();
            record.state = config::RuntimeCompensationState::Incomplete {
                failed_steps: failed_steps.clone(),
            };
            current.runtime_compensation = Some(record.clone());
            Ok((Some(record), true))
        }
    })?;
    *progress = match next {
        Some(compensation) => OneClickJournalProgress::Compensating {
            active_runtime_transaction: Box::new(active_runtime_transaction),
            restored_runtime_transaction: Box::new(restored_runtime_transaction),
            expected_runtime_transaction: Box::new(expected_runtime_transaction),
            compensation: Box::new(compensation),
            registered_ticket,
        },
        None => OneClickJournalProgress::CompensationFinished { registered_ticket },
    };
    Ok(())
}

pub(super) fn validate_interrupted_science_transaction_entry(
    environment_state: Option<config::RuntimeTransactionV1EnvironmentState<'_>>,
) -> Result<(), InterruptedScienceRecoveryError> {
    if environment_state
        .is_some_and(config::RuntimeTransactionV1EnvironmentState::is_authority_snapshot_active)
    {
        return Err(InterruptedScienceRecoveryError::new(
            "检测到 authority 快照已登记但受保护状态写入未完成；已保留恢复快照并拒绝把部分写入态作为新基线；recovery_status=manual_recovery_required",
            ProjectedRecovery::MANUAL_RECOVERY_REQUIRED,
        ));
    }
    Ok(())
}

fn validate_interrupted_science_environment_runtime(
    expected_runtime_id: Option<&str>,
    runtime: &ScienceRuntimeIdentity,
) -> Result<(), InterruptedScienceRecoveryError> {
    let Some(expected_runtime_id) = expected_runtime_id else {
        return Ok(());
    };
    if runtime.environment_transaction_id() == expected_runtime_id {
        Ok(())
    } else {
        Err(InterruptedScienceRecoveryError::new(
            "上次启动在 Science 环境暴露边界中断，当前 executable 与中断事务不一致；已拒绝自动启动旧版或其他 runtime；environment_uncertain；newer_runtime_required；recovery_status=manual_recovery_required",
            ProjectedRecovery::environment_uncertain_manual(),
        ))
    }
}

#[derive(Clone)]
struct OneClickRollbackContext {
    proxy_action: ProxyAction,
    sandbox_port: u16,
    launch_runtime: ScienceRuntimeIdentity,
    launch_token: Option<ScienceManagedLaunchToken>,
    launch_environment: ScienceEnvironmentExposure,
    launch_confirmed_stopped: bool,
    candidate_stop_proof: ManagedScienceCandidateStopProof,
    ssh_stub_transaction: Option<crate::runtime::settings::ManagedSshStubTransaction>,
    /// Produce-site failure kind for UI projection; updated at phase boundaries.
    current_kind: OneClickFailureKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ManagedScienceCandidateStopProof {
    #[default]
    NotRequired,
    ConfirmedStopped,
    Unproven,
}

#[derive(Debug)]
pub(super) struct ManagedScienceRestartError {
    message: String,
    candidate_stop_proof: ManagedScienceCandidateStopProof,
    diagnostic: PriorScienceRestartDiagnostic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PriorScienceRestartDiagnostic {
    Failed,
    #[cfg(test)]
    TestPostSpawnValidationFailed,
}

impl ManagedScienceRestartError {
    fn before_spawn(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            candidate_stop_proof: ManagedScienceCandidateStopProof::NotRequired,
            diagnostic: PriorScienceRestartDiagnostic::Failed,
        }
    }

    fn after_spawn_unproven(message: impl Into<String>) -> Self {
        Self {
            message: format!("{}；code=science_candidate_stop_unproven", message.into()),
            candidate_stop_proof: ManagedScienceCandidateStopProof::Unproven,
            diagnostic: PriorScienceRestartDiagnostic::Failed,
        }
    }

    #[allow(clippy::result_large_err)]
    fn after_exact_cleanup(
        message: impl Into<String>,
        expected_runtime: &ScienceRuntimeIdentity,
        cleanup: crate::runtime::science::ScienceStopOutcome,
    ) -> Self {
        match cleanup.and_then(|verified| verified.require_exact_stop_of(expected_runtime)) {
            Ok(_) => Self {
                message: message.into(),
                candidate_stop_proof: ManagedScienceCandidateStopProof::ConfirmedStopped,
                diagnostic: PriorScienceRestartDiagnostic::Failed,
            },
            Err(error) => {
                Self::after_spawn_unproven(format!("{}；candidate_cleanup={error}", message.into()))
            }
        }
    }

    #[cfg(test)]
    fn test_post_spawn_validation(
        expected_runtime: &ScienceRuntimeIdentity,
        cleanup: crate::runtime::science::ScienceStopOutcome,
    ) -> Self {
        let mut failure = Self::after_exact_cleanup(
            "test-only prior Science post-spawn validation failure",
            expected_runtime,
            cleanup,
        );
        failure.diagnostic = PriorScienceRestartDiagnostic::TestPostSpawnValidationFailed;
        failure
    }
}

impl std::fmt::Display for ManagedScienceRestartError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl From<String> for ManagedScienceRestartError {
    fn from(message: String) -> Self {
        Self::before_spawn(message)
    }
}

impl From<&str> for ManagedScienceRestartError {
    fn from(message: &str) -> Self {
        Self::before_spawn(message)
    }
}

struct OneClickFailure {
    typed: TypedOneClickFailure,
    rollback: OneClickRollbackContext,
}

impl OneClickFailure {
    fn message(&self) -> &str {
        &self.typed.safe_detail
    }
}

// Science 0.1.25 gives boot quick_check a 300s query timeout. Its own warning
// says the subsequently unblocked migration can take about 30 minutes, so the
// recovery restart has a separate finite ceiling instead of the ordinary 8s
// launch budget.
const SCIENCE_DB_REVERIFY_BUDGET_MS: u64 = 305_000;
const SCIENCE_DB_RECOVERY_RESTART_BUDGET_MS: u64 = 30 * 60 * 1_000 + 10_000;
const SCIENCE_HEALTH_BOOTSTRAP_BUDGET_MS: u64 = 20_000;

fn science_db_reverify_budget_ms() -> u64 {
    #[cfg(test)]
    if let Ok(value) = std::env::var("CSSWITCH_TEST_DB_REVERIFY_BUDGET_MS") {
        if let Ok(value) = value.parse::<u64>() {
            return value.max(POLL_INTERVAL_MS);
        }
    }
    SCIENCE_DB_REVERIFY_BUDGET_MS
}

fn science_db_recovery_restart_budget_ms() -> u64 {
    #[cfg(test)]
    if let Ok(value) = std::env::var("CSSWITCH_TEST_DB_RECOVERY_RESTART_BUDGET_MS") {
        if let Ok(value) = value.parse::<u64>() {
            return value.max(POLL_INTERVAL_MS);
        }
    }
    SCIENCE_DB_RECOVERY_RESTART_BUDGET_MS
}

fn open_authenticated_science_health_session(
    port: u16,
    runtime: &ScienceRuntimeIdentity,
    token: &ScienceManagedLaunchToken,
    deadline: Instant,
) -> Result<ScienceHealthSession, String> {
    if !ScienceHostAdapter::receipt_is_current(token, runtime) {
        return Err("science_db_listener_identity_changed".into());
    }
    let context = runtime
        .skill_install_host_context(port)
        .map_err(|_| "science_api_health_control_context_invalid".to_string())?;
    let session = open_science_health_session_before(&context, deadline)
        .map_err(science_health_control_error)?;
    if !ScienceHostAdapter::receipt_is_current(token, runtime) {
        return Err("science_db_listener_identity_changed".into());
    }
    Ok(session)
}

pub(super) fn science_health_control_error(
    error: csswitch_skill_install_core::AttachError,
) -> String {
    if error.retryable
        && matches!(
            error.code.as_str(),
            "SCIENCE_HEALTH_UNREACHABLE"
                | "SCIENCE_HEALTH_TIMEOUT"
                | "SCIENCE_CONTROL_TIMEOUT"
                | "SCIENCE_HEALTH_HTTP_STATUS"
        )
    {
        "science_api_health_unreachable".into()
    } else {
        format!("science_api_health_control_failed code={}", error.code)
    }
}

fn authenticated_science_db_health(
    session: &ScienceHealthSession,
    timeout: Duration,
) -> Result<proc::ScienceDbHealth, String> {
    let body = session
        .read_health_with_timeout(timeout)
        .map_err(science_health_control_error)?;
    let body =
        std::str::from_utf8(&body).map_err(|_| "science_api_health_malformed".to_string())?;
    proc::science_db_health_from_body(body)
}

fn wait_for_science_db_reverify(
    port: u16,
    runtime: &ScienceRuntimeIdentity,
    token: &ScienceManagedLaunchToken,
) -> Result<proc::ScienceDbHealth, String> {
    let deadline = Instant::now() + Duration::from_millis(science_db_reverify_budget_ms());
    let bootstrap_deadline =
        deadline.min(Instant::now() + Duration::from_millis(SCIENCE_HEALTH_BOOTSTRAP_BUDGET_MS));
    let session = loop {
        let remaining = bootstrap_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("science_api_health_bootstrap_timeout".into());
        }
        match open_authenticated_science_health_session(port, runtime, token, bootstrap_deadline) {
            Ok(session) => break session,
            Err(error)
                if error == "science_api_health_unreachable"
                    && Instant::now() < bootstrap_deadline =>
            {
                std::thread::sleep(
                    Duration::from_millis(POLL_INTERVAL_MS)
                        .min(bootstrap_deadline.saturating_duration_since(Instant::now())),
                );
            }
            Err(error) => return Err(error),
        }
    };
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("science_db_reverify_timeout".into());
        }
        if !ScienceHostAdapter::receipt_is_current(token, runtime) {
            return Err("science_db_listener_identity_changed".into());
        }
        match authenticated_science_db_health(&session, remaining.min(Duration::from_secs(5))) {
            Ok(state) => {
                if !ScienceHostAdapter::receipt_is_current(token, runtime) {
                    return Err("science_db_listener_identity_changed".into());
                }
                match state {
                    proc::ScienceDbHealth::ReverifyPending if Instant::now() < deadline => {
                        std::thread::sleep(
                            Duration::from_millis(POLL_INTERVAL_MS)
                                .min(deadline.saturating_duration_since(Instant::now())),
                        );
                    }
                    proc::ScienceDbHealth::ReverifyPending => {
                        return Err("science_db_reverify_timeout".into())
                    }
                    other => return Ok(other),
                }
            }
            Err(error)
                if matches!(
                    error.as_str(),
                    "science_api_health_unreachable" | "science_api_health_incomplete"
                ) && Instant::now() < deadline =>
            {
                std::thread::sleep(
                    Duration::from_millis(POLL_INTERVAL_MS)
                        .min(deadline.saturating_duration_since(Instant::now())),
                );
            }
            Err(error)
                if matches!(
                    error.as_str(),
                    "science_api_health_unreachable" | "science_api_health_incomplete"
                ) =>
            {
                return Err("science_db_reverify_timeout".into())
            }
            Err(error) => return Err(error),
        }
    }
}

#[derive(Clone)]
struct PriorScienceContext {
    runtime: ScienceRuntimeIdentity,
    port: u16,
    launch_token: ScienceManagedLaunchToken,
}

enum AuthorityCaptureAfterQuiesceError {
    PriorScienceRestored(String),
    RestartRequired(String),
}

impl OneClickRollbackContext {
    fn failure(&self, message: impl Into<String>) -> OneClickFailure {
        OneClickFailure {
            typed: TypedOneClickFailure::new(self.current_kind, message),
            rollback: self.clone(),
        }
    }

    fn set_kind(&mut self, kind: OneClickFailureKind) {
        self.current_kind = kind;
    }
}

#[allow(clippy::result_large_err)]
fn one_click_step<T, E: std::fmt::Display>(
    result: Result<T, E>,
    rollback: &OneClickRollbackContext,
) -> Result<T, OneClickFailure> {
    result.map_err(|error| rollback.failure(error.to_string()))
}

fn restart_prior_science<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    prior: &PriorScienceContext,
) -> Result<(), ManagedScienceRestartError> {
    restart_science_identity_with_budget(
        app,
        state,
        lifecycle,
        auth_proof,
        &prior.runtime,
        prior.port,
        operation::SANDBOX_HEALTH_BUDGET_MS,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn restart_science_identity_with_budget<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    _lifecycle: &lifecycle::Lifecycle,
    _auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    runtime: &ScienceRuntimeIdentity,
    port: u16,
    health_budget_ms: u64,
    durable_launch_id: Option<&str>,
) -> Result<(), ManagedScienceRestartError> {
    let dir = config::default_dir();
    let cfg = config::load_from(&dir).map_err(|error| error.to_string())?;
    if cfg.sandbox_port != port {
        return Err("恢复 prior Science 时沙箱端口已变化".into());
    }
    if ScienceHostAdapter::validate_launch_runtime(runtime).is_err() {
        return Err("恢复 prior Science 时 runtime 身份已变化".into());
    }
    if proc::loopback_port_in_use(port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
        return Err("恢复 prior Science 前端口仍被占用；拒绝接管未知 listener".into());
    }
    let (proxy_port, secret) = {
        let current = lock(state);
        if current.proxy.is_some() {
            (current.proxy_port, current.secret.clone())
        } else {
            (cfg.proxy_port, cfg.secret.clone())
        }
    };
    let ssh_hosts = if cfg.reuse_system_ssh {
        crate::runtime::ssh_bridge::validate_science_ssh_bridge(&sandbox_home())?
    } else {
        Vec::new()
    };
    let root = asset_root(app).ok_or("恢复 prior Science 时找不到打包资源")?;
    let launch = root.join("scripts/launch-virtual-sandbox.sh");
    if !launch.is_file() {
        return Err("恢复 prior Science 时启动脚本缺失".into());
    }
    let logf = open_log("sandbox.log").map_err(|error| error.to_string())?;
    let logf2 = logf.try_clone().map_err(|error| error.to_string())?;
    let proxy_url = format!("http://127.0.0.1:{proxy_port}/{secret}");
    let ssh_hosts = ssh_hosts.join(" ");
    let attempt = ScienceHostAdapter::spawn_launch(
        ScienceLaunchSpec::recovery(
            &launch,
            runtime,
            port,
            &proxy_url,
            cfg.reuse_system_ssh,
            &ssh_hosts,
            health_budget_ms,
            POLL_INTERVAL_MS,
            operation::LOCAL_HEALTH_TIMEOUT_MS,
        ),
        logf,
        logf2,
    )
    .map_err(|error| match error.kind() {
        ScienceLaunchFailureKind::RuntimeDrift | ScienceLaunchFailureKind::SpawnFailed => {
            ManagedScienceRestartError::before_spawn(format!(
                "恢复 prior Science 启动失败：{}",
                error.message()
            ))
        }
        _ => ManagedScienceRestartError::after_spawn_unproven(format!(
            "恢复 prior Science {}",
            error.message()
        )),
    })?;
    let attempt = ScienceHostAdapter::accept_launch_script(attempt).map_err(|error| {
        ManagedScienceRestartError::after_spawn_unproven(format!(
            "恢复 prior Science {}",
            error.message()
        ))
    })?;
    let healthy = ScienceHostAdapter::verify_health(attempt).map_err(|_| {
        ManagedScienceRestartError::after_spawn_unproven(
            "恢复 prior Science 后 listener 健康或 runtime 身份不一致",
        )
    })?;
    let verified =
        ScienceHostAdapter::verify_identity(healthy).map_err(|error| match error.kind() {
            ScienceLaunchFailureKind::OwnershipUnavailable => {
                ManagedScienceRestartError::after_spawn_unproven(
                    "恢复 prior Science 后无法建立精确的未提交启动身份",
                )
            }
            _ => ManagedScienceRestartError::after_spawn_unproven(
                "恢复 prior Science 后 listener 健康或 runtime 身份不一致",
            ),
        })?;
    let _candidate_token = verified
        .ownership()
        .expect("recovery launch must carry uncommitted ownership proof")
        .clone();
    #[cfg(test)]
    {
        let mut seams = SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if seams.prior_restart_post_spawn_failure_port == Some(port) {
            let listener_pid = crate::runtime::science::test_unique_listener_pid(port)
                .ok_or("test-only prior Science listener identity missing after verification")?;
            let process_start =
                crate::runtime::science::test_process_start_identity_for_pid(listener_pid)
                    .ok_or("test-only prior Science process-start identity missing")?;
            seams.prior_restart_post_spawn_identity = Some((listener_pid, process_start));
            drop(seams);
            let mut sandbox = None;
            let mut url = None;
            let cleanup = ScienceHostAdapter::stop(
                app,
                &mut sandbox,
                &mut url,
                ScienceStopRequest::exact(
                    runtime,
                    ScienceStopOwnershipReceipt::from_managed_launch(&_candidate_token),
                ),
            );
            return Err(ManagedScienceRestartError::test_post_spawn_validation(
                runtime, cleanup,
            ));
        }
    }
    let committed_launch = match durable_launch_id {
        Some(launch_id) => ScienceHostAdapter::commit_launch_with_launch_id(verified, launch_id),
        None => ScienceHostAdapter::commit_launch(verified),
    };
    let committed_runtime = match committed_launch {
        Ok(receipt) => receipt.runtime().clone(),
        Err(error) => {
            let mut sandbox = None;
            let mut url = None;
            let token_present = error.ownership().is_some();
            let request = error
                .ownership()
                .map(|token| {
                    ScienceStopRequest::exact(
                        runtime,
                        ScienceStopOwnershipReceipt::from_managed_launch(token),
                    )
                })
                .unwrap_or_else(|| ScienceStopRequest::recover(Some(runtime)));
            let cleanup = ScienceHostAdapter::stop(app, &mut sandbox, &mut url, request);
            let message = match error.kind() {
                ScienceLaunchFailureKind::ReceiptIdentityDrift => {
                    "恢复 prior Science 后 fresh managed receipt 回读不一致".to_string()
                }
                _ => format!(
                    "恢复 prior Science 时 fresh managed receipt 提交失败：{}",
                    error.message()
                ),
            };
            return Err(if token_present {
                ManagedScienceRestartError::after_exact_cleanup(message, runtime, cleanup)
            } else {
                ManagedScienceRestartError::after_spawn_unproven(message)
            });
        }
    };
    let url = ScienceHostAdapter::url(port, &committed_runtime);
    let mut current = lock(state);
    current.sandbox_port = port;
    current.sandbox_url = Some(url);
    current.science_runtime = Some(committed_runtime);
    current.science_confirmed_stopped = None;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn capture_authority_after_science_quiesce<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    config_dir: &Path,
    sandbox_home: &Path,
    auth_dir: &Path,
    config: &config::Config,
    prior_science: Option<&PriorScienceContext>,
) -> Result<AuthorityTransaction, AuthorityCaptureAfterQuiesceError> {
    match AuthorityTransaction::capture(config_dir, sandbox_home, auth_dir, config, state) {
        Ok(snapshot) => Ok(snapshot),
        Err(capture_error) => {
            if let Some(prior) = prior_science {
                match restart_prior_science(app, state, lifecycle, auth_proof, prior) {
                    Ok(()) => Err(AuthorityCaptureAfterQuiesceError::PriorScienceRestored(
                        capture_error,
                    )),
                    Err(restart_error) => Err(AuthorityCaptureAfterQuiesceError::RestartRequired(
                        format!("{capture_error}；prior_science_restart={restart_error}"),
                    )),
                }
            } else {
                Err(AuthorityCaptureAfterQuiesceError::RestartRequired(
                    capture_error,
                ))
            }
        }
    }
}

#[cfg(test)]
pub(super) fn clear_one_click_transaction(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
) -> Result<(), String> {
    let expected_record = progress
        .journaled_record()
        .cloned()
        .ok_or("one-click cannot clear a journal before its first durable checkpoint")?;
    let registered_ticket = progress.registered_ticket().clone();
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) {
            return Err(
                "one-click clear config authority drifted; preserved the runtime transaction"
                    .into(),
            );
        }
        match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal))
                if journal == &expected_record
                    && one_click_journal_matches(
                        journal,
                        identity,
                        &expected_record.transaction_id,
                    ) => {}
            _ => {
                return Err(
                    "one-click clear identity changed; preserved the runtime transaction".into(),
                )
            }
        }
        current.runtime_transaction = None;
        Ok(((), true))
    })?;
    *progress = OneClickJournalProgress::Finalized {
        record: expected_record,
        registered_ticket,
    };
    Ok(())
}

#[cfg(test)]
pub(super) fn commit_runtime_binding(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    binding: config::RuntimeBindingCommit,
) -> Result<(), String> {
    let expected_record = progress
        .journaled_record()
        .cloned()
        .ok_or("one-click cannot commit a binding before its first durable checkpoint")?;
    let registered_ticket = progress.registered_ticket().clone();
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) {
            return Err(
                "one-click binding commit config authority drifted; preserved the runtime transaction"
                    .into(),
            );
        }
        match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal))
                if journal == &expected_record
                    && one_click_journal_matches(
                        journal,
                        identity,
                        &expected_record.transaction_id,
                    ) => {}
            _ => {
                return Err(
                    "one-click binding commit identity changed; preserved the runtime transaction"
                        .into(),
                )
            }
        }
        current.runtime_binding = Some(binding.clone());
        current.runtime_transaction = None;
        Ok(((), true))
    })?;
    *progress = OneClickJournalProgress::Finalized {
        record: expected_record,
        registered_ticket,
    };
    Ok(())
}

pub(super) fn begin_one_click_finalize(
    dir: &Path,
    identity: &OneClickTransactionIdentity,
    progress: &mut OneClickJournalProgress,
    action: config::RuntimeFinalizeAction,
) -> Result<(), String> {
    let expected = progress
        .journaled_record()
        .cloned()
        .ok_or("one-click cannot begin finalize before its first durable checkpoint")?;
    let registered_ticket = progress.registered_ticket().clone();
    let next = config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &identity.target_profile_id,
            identity.previous_binding.as_ref(),
        ) {
            return Err(
                "one-click finalize intent config authority drifted; preserved the runtime transaction"
                    .into(),
            );
        }
        match current.runtime_transaction.as_ref() {
            Some(config::RuntimeTransactionRecord::V2(journal))
                if journal == &expected
                    && one_click_journal_matches(journal, identity, &expected.transaction_id) => {}
            _ => {
                return Err(
                    "one-click finalize intent identity changed; preserved the runtime transaction"
                        .into(),
                )
            }
        }
        let mut next = expected.clone();
        next.finalize = config::RuntimeFinalizeState::Intent {
            action: action.clone(),
        };
        current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(next.clone()));
        Ok((next, true))
    })?;
    *progress = OneClickJournalProgress::Journaled {
        record: next,
        registered_ticket,
    };
    Ok(())
}

pub(super) fn complete_one_click_finalize(
    dir: &Path,
    progress: &mut OneClickJournalProgress,
) -> Result<(), String> {
    let expected = progress
        .journaled_record()
        .cloned()
        .ok_or("one-click cannot complete finalize without a durable intent")?;
    let action = match &expected.finalize {
        config::RuntimeFinalizeState::Intent { action } => action.clone(),
        config::RuntimeFinalizeState::NotStarted => {
            return Err("one-click finalize intent is missing".into())
        }
    };
    let registered_ticket = progress.registered_ticket().clone();
    #[cfg(test)]
    if SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .one_click_fail_finalize_completion
        .as_deref()
        == Some(dir)
    {
        return Err("test-only one-click finalize config commit failure".into());
    }
    config::update_result(dir, |current| {
        if !config_authority_matches(
            current,
            &expected.target_profile_id,
            expected.previous_binding.as_ref(),
        ) || current.runtime_transaction.as_ref()
            != Some(&config::RuntimeTransactionRecord::V2(expected.clone()))
        {
            return Err(
                "one-click finalize completion identity changed; preserved the runtime transaction"
                    .into(),
            );
        }
        if let config::RuntimeFinalizeAction::CommitBinding { binding, .. } = &action {
            current.runtime_binding = Some(binding.clone());
        }
        current.runtime_transaction = None;
        Ok(((), true))
    })?;
    *progress = OneClickJournalProgress::Finalized {
        record: expected,
        registered_ticket,
    };
    Ok(())
}

fn preserve_interrupted_success_finalize(
    authority_transaction: &mut AuthorityTransaction,
    value: &mut Value,
) {
    authority_transaction.preserve_recovery();
    if let Some(object) = value.as_object_mut() {
        object.insert("status".into(), Value::String("degraded".into()));
        object.insert(
            "recovery_status".into(),
            Value::String("manual_recovery_required".into()),
        );
        object.insert(
            "cleanup_message".into(),
            Value::String(
                "启动已完成，但 success finalize 尚未持久化；下次一键操作会先安全重放。".into(),
            ),
        );
    }
}

struct OneClickEntryFacts {
    science_state: SandboxScienceState,
    running_runtime: Option<ScienceRuntimeIdentity>,
    selected_runtime: Option<ScienceRuntimeIdentity>,
    login_intact: bool,
    binding_matches: bool,
    remembered_runtime_was_present: bool,
}

enum OneClickEntryDecision {
    HealthyReopen {
        runtime: ScienceRuntimeIdentity,
    },
    Mutating {
        launch_runtime: ScienceRuntimeIdentity,
        running_runtime_to_stop: Option<ScienceRuntimeIdentity>,
        science_state: SandboxScienceState,
        remembered_runtime_was_present: bool,
    },
}

#[derive(Clone, Copy)]
struct OneClickEntryProgress<'a> {
    gateway_terminal_handoff: Option<&'a config::RuntimeTransactionV2>,
    cleanup_replayed: bool,
}

impl<'a> OneClickEntryProgress<'a> {
    fn initial(gateway_terminal_handoff: Option<&'a config::RuntimeTransactionV2>) -> Self {
        Self {
            gateway_terminal_handoff,
            cleanup_replayed: false,
        }
    }

    fn after_cleanup(self) -> Self {
        Self {
            cleanup_replayed: true,
            ..self
        }
    }
}

fn capture_one_click_entry_facts(
    state: &SharedAppState,
    cfg: &config::Config,
    active_profile: &config::Profile,
    auth_dir: &Path,
    runtime_choice: Option<&str>,
    interrupted_environment_runtime_id: Option<&str>,
) -> Result<OneClickEntryFacts, TypedOneClickFailure> {
    let version_cache = { lock(state).science_version_cache.clone() };
    let (remembered_runtime, confirmed_stopped) = {
        let current = lock(state);
        (
            current.science_runtime.clone(),
            current.science_confirmed_stopped.clone(),
        )
    };
    let remembered_runtime_was_present = remembered_runtime.is_some();
    let (science_state, running_runtime) = match remembered_runtime {
        Some(runtime) => {
            let observed = ScienceHostAdapter::probe_known(cfg.sandbox_port, &runtime);
            let running = (observed == SandboxScienceState::RunningHealthy).then_some(runtime);
            (observed, running)
        }
        None if confirmed_stopped
            .as_ref()
            .is_some_and(|runtime| runtime.source != ScienceRuntimeSource::CachedOnce)
            && !proc::loopback_port_in_use(cfg.sandbox_port, 100) =>
        {
            (SandboxScienceState::Stopped, None)
        }
        None => ScienceHostAdapter::probe_cached(cfg.sandbox_port, &version_cache)
            .map_err(|message| typed_one_click_err(OneClickFailureKind::ScienceStart, message))?,
    };

    let mut selected_runtime = None;
    let mut login_intact = false;
    let mut binding_matches = false;
    match science_state {
        SandboxScienceState::RunningHealthy => {
            let running = running_runtime.as_ref().ok_or_else(|| {
                typed_one_click_err(
                    OneClickFailureKind::ScienceStart,
                    "Science 状态为运行中，但无法确认其 binary 身份",
                )
            })?;
            validate_interrupted_science_environment_runtime(
                interrupted_environment_runtime_id,
                running,
            )
            .map_err(|error| typed_interrupted_science_err(OneClickFailureKind::Prepare, error))?;
            let desired_binding =
                crate::runtime::provider::desired_runtime_binding(cfg, active_profile, running)
                    .map_err(|message| {
                        typed_one_click_err(OneClickFailureKind::Prepare, message)
                    })?;
            binding_matches = !crate::runtime::provider::science_restart_required(
                cfg.runtime_binding.as_ref(),
                &desired_binding,
            );
            login_intact =
                oauth_forge::login_intact(auth_dir, "virtual@localhost.invalid", &sandbox_home());
            if !login_intact {
                selected_runtime = Some(
                    select_science_runtime_cached(runtime_choice, &version_cache).map_err(
                        |message| typed_one_click_err(OneClickFailureKind::ScienceStart, message),
                    )?,
                );
            }
        }
        SandboxScienceState::Stopped => {
            selected_runtime = Some(
                select_science_runtime_cached(runtime_choice, &version_cache).map_err(
                    |message| typed_one_click_err(OneClickFailureKind::ScienceStart, message),
                )?,
            );
        }
        SandboxScienceState::Unknown => {
            if interrupted_environment_runtime_id.is_some() {
                return Err(TypedOneClickFailure::new(
                    OneClickFailureKind::ScienceStart,
                    "上次启动在 Science 环境暴露边界中断，且当前 listener/runtime 身份无法确认；已拒绝自动恢复；environment_uncertain；recovery_status=manual_recovery_required",
                )
                .with_recovery(ProjectedRecovery::environment_uncertain_manual()));
            }
            let sandbox_port = cfg.sandbox_port;
            return Err(typed_one_click_err(
                OneClickFailureKind::ScienceStart,
                format!(
                    "无法确认隔离 Science 状态（端口 {sandbox_port} 或 data-dir 状态不一致）。请先停止占用该端口的进程后重试。"
                ),
            ));
        }
    }
    Ok(OneClickEntryFacts {
        science_state,
        running_runtime,
        selected_runtime,
        login_intact,
        binding_matches,
        remembered_runtime_was_present,
    })
}

fn decide_one_click_entry(
    facts: OneClickEntryFacts,
) -> Result<OneClickEntryDecision, TypedOneClickFailure> {
    if facts.science_state == SandboxScienceState::RunningHealthy
        && facts.login_intact
        && facts.binding_matches
    {
        let runtime = facts.running_runtime.ok_or_else(|| {
            typed_one_click_err(
                OneClickFailureKind::ScienceStart,
                "healthy entry facts lost the observed Science runtime",
            )
        })?;
        return Ok(OneClickEntryDecision::HealthyReopen { runtime });
    }
    let running_runtime_to_stop = facts.running_runtime.clone();
    let launch_runtime =
        if facts.science_state == SandboxScienceState::RunningHealthy && facts.login_intact {
            facts.running_runtime.ok_or_else(|| {
                typed_one_click_err(
                    OneClickFailureKind::ScienceStart,
                    "mutating entry facts lost the observed Science runtime",
                )
            })?
        } else {
            facts.selected_runtime.ok_or_else(|| {
                typed_one_click_err(
                    OneClickFailureKind::ScienceStart,
                    "mutating entry facts lost the selected Science runtime",
                )
            })?
        };
    Ok(OneClickEntryDecision::Mutating {
        launch_runtime,
        running_runtime_to_stop,
        science_state: facts.science_state,
        remembered_runtime_was_present: facts.remembered_runtime_was_present,
    })
}

pub(crate) fn replay_interrupted_one_click_finalize(state: &SharedAppState) -> Result<(), String> {
    let dir = config::default_dir();
    let cfg = config::load_from(&dir).map_err(|error| error.to_string())?;
    let Some(config::RuntimeTransactionRecord::V2(expected)) = cfg.runtime_transaction.as_ref()
    else {
        return Ok(());
    };
    let action = match &expected.finalize {
        config::RuntimeFinalizeState::NotStarted => return Ok(()),
        config::RuntimeFinalizeState::Intent { action } => action.clone(),
    };
    let replay_adoption_proof = if let config::RuntimeFinalizeAction::CommitBinding {
        binding,
        science_adoption_attempt_id,
    } = &action
    {
        if binding.science_adoption_attempt_id.as_deref() != science_adoption_attempt_id.as_deref()
        {
            return Err(
                "interrupted finalize binding/adoption provenance mismatch; preserved current state"
                    .into(),
            );
        }
        science_adoption_attempt_id
            .as_deref()
            .map(|attempt_id| prove_finalize_science_adoption_runtime(state, &cfg, attempt_id))
            .transpose()?
    } else {
        None
    };
    let initial_authority_matches =
        if expected.operation == config::RuntimeTransactionOperation::HistoryRecovery {
            super::history_recovery::history_config_authority_matches(&cfg, expected)
        } else {
            config_authority_matches(
                &cfg,
                &expected.target_profile_id,
                expected.previous_binding.as_ref(),
            )
        };
    if !initial_authority_matches {
        return Err("interrupted finalize intent no longer targets the active profile".into());
    }
    match (&expected.operation, &action) {
        (
            config::RuntimeTransactionOperation::OneClick,
            config::RuntimeFinalizeAction::CommitBinding { binding, .. },
        ) if binding.profile_id != expected.target_profile_id => {
            return Err("interrupted finalize binding no longer matches its target profile".into())
        }
        (
            config::RuntimeTransactionOperation::OneClick,
            config::RuntimeFinalizeAction::ClearJournal
            | config::RuntimeFinalizeAction::CommitBinding { .. },
        )
        | (
            config::RuntimeTransactionOperation::HistoryRecovery,
            config::RuntimeFinalizeAction::ClearJournal
            | config::RuntimeFinalizeAction::ResumeOneClick,
        ) => {}
        _ => return Err("interrupted finalize action has invalid operation ownership".into()),
    }
    let ticket = expected
        .snapshot_ticket
        .as_ref()
        .ok_or("interrupted finalize intent has no authority snapshot ticket")?;
    let cleanup = replay_finalize_authority_cleanup(state, ticket).map_err(String::from)?;
    if replay_adoption_proof
        .as_ref()
        .is_some_and(|(runtime, receipt)| !ScienceHostAdapter::receipt_is_current(receipt, runtime))
    {
        return Err(
            "interrupted finalize Science adoption receipt drifted before config commit; preserved current state"
                .into(),
        );
    }
    config::update_result(&dir, |current| {
        let authority_matches =
            if expected.operation == config::RuntimeTransactionOperation::HistoryRecovery {
                super::history_recovery::history_config_authority_matches(current, expected)
            } else {
                config_authority_matches(
                    current,
                    &expected.target_profile_id,
                    expected.previous_binding.as_ref(),
                )
            };
        if !authority_matches
            || current.runtime_transaction.as_ref()
                != Some(&config::RuntimeTransactionRecord::V2(expected.clone()))
        {
            return Err(
                "interrupted finalize record drifted during authority replay; preserved current state"
                    .into(),
            );
        }
        match &action {
            config::RuntimeFinalizeAction::CommitBinding { binding, .. } => {
                current.runtime_binding = Some(binding.clone());
                current.runtime_transaction = None;
            }
            config::RuntimeFinalizeAction::ClearJournal => {
                current.runtime_transaction = None;
            }
            config::RuntimeFinalizeAction::ResumeOneClick => {
                let mut terminal = expected.clone();
                terminal.phase = config::RuntimeTransactionPhase::ResumeAfterHistoryRestore;
                terminal.snapshot_ticket = None;
                terminal.finalize = config::RuntimeFinalizeState::NotStarted;
                current.runtime_transaction = Some(config::RuntimeTransactionRecord::V2(terminal));
            }
        }
        Ok(((), true))
    })?;
    if let Some((runtime, _)) = replay_adoption_proof {
        let _ = mark_science_runtime_adoption_finalized(&runtime);
    }
    let _ = cleanup;
    Ok(())
}

fn prove_finalize_science_adoption_runtime(
    state: &SharedAppState,
    cfg: &config::Config,
    expected_attempt_id: &str,
) -> Result<(ScienceRuntimeIdentity, ScienceManagedLaunchToken), String> {
    let version_cache = { lock(state).science_version_cache.clone() };
    let (science_state, runtime) =
        ScienceHostAdapter::probe_cached(cfg.sandbox_port, &version_cache).map_err(|error| {
            format!(
                "interrupted finalize could not prove current Science adoption runtime: {error}"
            )
        })?;
    if science_state != SandboxScienceState::RunningHealthy {
        return Err(
            "interrupted finalize requires a healthy current Science runtime with an exact V2 receipt; preserved current state"
                .into(),
        );
    }
    let runtime = runtime.ok_or(
        "interrupted finalize lost the current Science runtime identity; preserved current state",
    )?;
    if runtime.adoption_attempt_id() != Some(expected_attempt_id) {
        return Err(
            "interrupted finalize Science runtime/receipt adoption provenance mismatch; preserved current state"
                .into(),
        );
    }
    let receipt = ScienceHostAdapter::managed_receipt(cfg.sandbox_port, &runtime).ok_or(
        "interrupted finalize could not recover the exact current Science V2 receipt; preserved current state",
    )?;
    if !ScienceHostAdapter::receipt_is_current(&receipt, &runtime) {
        return Err(
            "interrupted finalize Science V2 receipt changed during proof; preserved current state"
                .into(),
        );
    }
    Ok((runtime, receipt))
}

fn history_recovery_choices(
    candidates: Vec<oauth_forge::HistoryOrgCandidate>,
) -> Result<(Vec<HistoryRecoveryChoice>, Vec<Value>), String> {
    if candidates.len() > 64 {
        return Err("历史记录候选超过安全上限（64），已拒绝生成恢复会话".into());
    }
    let choices = candidates
        .into_iter()
        .map(|candidate| HistoryRecoveryChoice {
            reference: config::new_id(),
            candidate,
        })
        .collect::<Vec<_>>();
    let visible = choices
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            let label = if index < 26 {
                format!("历史记录 {}", (b'A' + index as u8) as char)
            } else {
                format!("历史记录 {}", index + 1)
            };
            json!({
                "reference": choice.reference,
                "label": label
            })
        })
        .collect();
    Ok((choices, visible))
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn test_compensate_one_click_failure<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    dir: &Path,
    authority_transaction: &mut AuthorityTransaction,
    transaction_identity: &OneClickTransactionIdentity,
    journal_progress: &mut OneClickJournalProgress,
    launch_runtime: ScienceRuntimeIdentity,
) -> Result<Value, TypedOneClickFailure> {
    let trace = OperationTrace::start(
        OperationKind::OneClickLogin,
        "test=one_click_journal_drift_compensation",
    );
    let failure = OneClickRollbackContext {
        proxy_action: ProxyAction::Reused,
        sandbox_port: 0,
        launch_runtime,
        launch_token: None,
        launch_environment: ScienceEnvironmentExposure::NotExposed,
        launch_confirmed_stopped: false,
        candidate_stop_proof: ManagedScienceCandidateStopProof::NotRequired,
        ssh_stub_transaction: None,
        current_kind: OneClickFailureKind::Prepare,
    }
    .failure("test-only one-click complete-record CAS rejection");
    cold::compensate_one_click_failure(
        app,
        state,
        lifecycle,
        None,
        dir,
        &trace,
        authority_transaction,
        transaction_identity,
        journal_progress,
        None,
        failure,
        None,
    )
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
fn one_click_login_with_options<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    runtime_choice: Option<&str>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    open_surface: bool,
    expected_profile_switch_transaction: Option<&config::RuntimeTransactionV2>,
    reconcile_disposition: Option<&mut PriorScienceDisposition>,
    entry_progress: OneClickEntryProgress<'_>,
) -> Result<Value, TypedOneClickFailure> {
    let trace = OperationTrace::start(OperationKind::OneClickLogin, "command=one_click_login");
    let dir = config::default_dir();
    let cfg = config::load_from(&dir)
        .map_err(|e| typed_one_click_err(OneClickFailureKind::ConfigLoad, e.to_string()))?;
    if cfg.runtime_compensation.is_some() {
        return Err(TypedOneClickFailure::new(
            OneClickFailureKind::Prepare,
            "durable compensation 尚未由 production entry owner 收敛；已保留 exact journal 与恢复快照。",
        )
        .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED));
    }
    let gateway_terminal_handoff = resolve_gateway_terminal_handoff(
        cfg.runtime_transaction.as_ref(),
        entry_progress.gateway_terminal_handoff,
        &cfg.active_id,
        cfg.runtime_binding.as_ref(),
    )
    .map_err(|detail| {
        TypedOneClickFailure::new(
            OneClickFailureKind::Prepare,
            format!(
                "检测到无法接管的 runtime journal；已保留当前事务并要求人工恢复：{detail}；recovery_status=manual_recovery_required"
            ),
        )
        .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
    })?;
    let profile_switch_handoff = if gateway_terminal_handoff.is_some() {
        None
    } else {
        resolve_profile_switch_handoff(
            cfg.runtime_transaction.as_ref(),
            expected_profile_switch_transaction,
            reconcile_disposition.is_some(),
            &cfg.active_id,
            cfg.runtime_binding.as_ref(),
        )
        .map_err(|detail| {
            TypedOneClickFailure::new(
                OneClickFailureKind::Prepare,
                format!(
                    "检测到无法接管的 runtime journal；已保留当前事务并要求人工恢复：{detail}；recovery_status=manual_recovery_required"
                ),
            )
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
        })?
    };
    let interrupted_environment_state = cfg
        .runtime_transaction
        .as_ref()
        .and_then(config::RuntimeTransactionRecord::v1_environment_state);
    let interrupted_environment_runtime_id = interrupted_environment_state
        .map(config::RuntimeTransactionV1EnvironmentState::runtime_fingerprint);
    validate_interrupted_science_transaction_entry(interrupted_environment_state)
        .map_err(|error| typed_interrupted_science_err(OneClickFailureKind::Prepare, error))?;
    let active_profile = cfg.active_profile().ok_or_else(|| {
        typed_one_click_err(
            OneClickFailureKind::NoActiveProfile,
            "未配置生效 profile，请先在面板选择或新建一条配置。",
        )
    })?;
    config::require_template_enabled(&cfg, &active_profile.template_id)
        .map_err(|message| typed_one_click_err(OneClickFailureKind::Prepare, message))?;
    let active_launch = crate::runtime::provider::resolve_launch_plan(active_profile)
        .map_err(|message| typed_one_click_err(OneClickFailureKind::LaunchPlan, message))?;
    crate::commands::codex::require_provider_auth_proof(&active_launch.adapter, auth_proof)
        .map_err(|message| typed_one_click_err(OneClickFailureKind::AuthPreflight, message))?;
    crate::runtime::settings::validate_runtime_ports(cfg.proxy_port, cfg.sandbox_port)
        .map_err(|message| typed_one_click_err(OneClickFailureKind::Prepare, message))?;
    let sport = cfg.sandbox_port;

    let sbx_home = sandbox_home();
    let auth_dir = sbx_home.join(".claude-science");
    let entry_facts = capture_one_click_entry_facts(
        &state,
        &cfg,
        active_profile,
        &auth_dir,
        runtime_choice,
        interrupted_environment_runtime_id,
    )?;
    let (launch_runtime, running_runtime_to_stop, science_state, remembered_runtime_was_present) =
        match decide_one_click_entry(entry_facts)? {
            OneClickEntryDecision::HealthyReopen {
                runtime: running_runtime,
            } => {
                let _ = reconcile_current_science_runtime_adoption(
                    &running_runtime,
                    cfg.runtime_binding.as_ref(),
                );
                if cfg.reuse_system_ssh {
                    validate_running_system_ssh_bridge(&app, &sbx_home).map_err(|message| {
                        typed_one_click_err(OneClickFailureKind::Prepare, message)
                    })?;
                }
                oauth_forge::bootstrap_marker_for_intact_login(
                    &auth_dir,
                    "virtual@localhost.invalid",
                    &sbx_home,
                )
                .map_err(|error| {
                    typed_one_click_err(
                        OneClickFailureKind::SandboxLogin,
                        format!("补齐历史恢复标记失败：{error}"),
                    )
                })?;
                let mut reopened = healthy_reopen_with_gateway_rollback(
                    &app,
                    &state,
                    lifecycle,
                    auth_proof,
                    &trace,
                    &dir,
                    &cfg,
                    active_profile,
                    &auth_dir,
                    sport,
                    &running_runtime,
                    open_surface,
                    profile_switch_handoff.as_ref(),
                    gateway_terminal_handoff.as_ref(),
                )?;
                if interrupted_environment_runtime_id.is_some() {
                    reopened["recovery_status"] = json!("environment_uncertain");
                    reopened["environment_status"] = json!("uncertain");
                }
                return Ok(reopened);
            }
            OneClickEntryDecision::Mutating {
                launch_runtime,
                running_runtime_to_stop,
                science_state,
                remembered_runtime_was_present,
            } => {
                if !entry_progress.cleanup_replayed {
                    let cleanup = retry_pending_authority_cleanup(&state).map_err(|failure| {
                        typed_authority_cleanup_err(
                            OneClickFailureKind::AuthoritySnapshot,
                            failure,
                            false,
                        )
                    })?;
                    if pending_cleanup_requires_recapture(cleanup) {
                        return one_click_login_with_options(
                            app,
                            state,
                            lifecycle,
                            runtime_choice,
                            auth_proof,
                            open_surface,
                            expected_profile_switch_transaction,
                            reconcile_disposition,
                            entry_progress.after_cleanup(),
                        );
                    }
                }
                (
                    launch_runtime,
                    running_runtime_to_stop,
                    science_state,
                    remembered_runtime_was_present,
                )
            }
        };
    cold::run_cold_one_click(
        app,
        state,
        lifecycle,
        auth_proof,
        open_surface,
        reconcile_disposition,
        trace,
        dir,
        &cfg,
        active_profile,
        sbx_home,
        auth_dir,
        sport,
        launch_runtime,
        running_runtime_to_stop,
        science_state,
        remembered_runtime_was_present,
        profile_switch_handoff,
        gateway_terminal_handoff,
        interrupted_environment_runtime_id,
    )
}
