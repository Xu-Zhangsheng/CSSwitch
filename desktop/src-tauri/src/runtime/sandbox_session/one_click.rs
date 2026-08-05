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
    config, lifecycle, lock, oauth_forge, proc, AppState, HistoryRecoveryChoice,
    HistoryRecoveryScienceQuiescence, HistoryRecoverySession, SharedAppState,
};

// Sibling modules are owned by the sandbox_session facade.
#[cfg(test)]
use super::authority_snapshot::SANDBOX_SESSION_TEST_SEAMS;
use super::authority_transaction::AuthorityTransaction;
use super::catalog_verify::*;
use super::pending_cleanup::{
    cleanup_required_error, replay_finalize_authority_cleanup, retry_pending_authority_cleanup,
    AuthorityCleanupFailure, AuthorityCleanupPhase, PendingCleanupRetryOutcome,
};
use super::recovery::{AppAuthoritySnapshot, RuntimeTransactionRestoreExpectation};
use super::route_reconcile::configure_third_party_best_effort;
use super::ssh_preflight::*;

mod healthy_reopen;

use healthy_reopen::healthy_reopen_with_gateway_rollback;

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
            if current.runtime_transaction != self.runtime_transaction {
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

#[allow(dead_code, clippy::result_large_err)]
fn stop_sandbox_state<R: Runtime>(
    app: &tauri::AppHandle<R>,
    st: &mut AppState,
) -> crate::runtime::science::ScienceStopOutcome {
    let runtime = st.science_runtime.clone();
    let result = ScienceHostAdapter::stop(
        app,
        &mut st.sandbox,
        &mut st.sandbox_url,
        ScienceStopRequest::recover(runtime.as_ref()),
    );
    if let Ok(verified) = result.as_ref() {
        st.science_confirmed_stopped = verified.confirmed_runtime().cloned();
        st.science_runtime = None;
    }
    result
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
#[allow(dead_code)]
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
                let mut st = lock(&state);
                let receipt = ScienceHostAdapter::managed_receipt(cfg.sandbox_port, &runtime)
                    .ok_or_else(|| {
                        typed_one_click_err(
                            OneClickFailureKind::ScienceStop,
                            "回滚时无法取得候选 Science 的精确受管启动身份。",
                        )
                    })?;
                let AppState {
                    sandbox,
                    sandbox_url,
                    ..
                } = &mut *st;
                let verified = ScienceHostAdapter::stop(
                    &app,
                    sandbox,
                    sandbox_url,
                    ScienceStopRequest::exact(
                        &runtime,
                        ScienceStopOwnershipReceipt::from_managed_launch(&receipt),
                    ),
                )
                .and_then(|verified| verified.require_exact_stop_of(&runtime))
                .map_err(|error| {
                    typed_one_click_err(
                        OneClickFailureKind::ScienceStop,
                        format!(
                            "回滚时停止候选 Science 失败，未猜测 PID 或按端口结束进程：{error}"
                        ),
                    )
                })?;
                st.science_confirmed_stopped = verified.confirmed_runtime().cloned();
                st.science_runtime = None;
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
    },
    Journaled {
        record: config::RuntimeTransactionV2,
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
            Self::PreJournalAbort { registered_ticket }
            | Self::Journaled {
                registered_ticket, ..
            }
            | Self::Finalized {
                registered_ticket, ..
            } => registered_ticket,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn transaction_id(&self) -> Option<&str> {
        match self {
            Self::PreJournalAbort { .. } => None,
            Self::Journaled { record, .. } | Self::Finalized { record, .. } => {
                Some(&record.transaction_id)
            }
        }
    }

    pub(super) fn journaled_record(&self) -> Option<&config::RuntimeTransactionV2> {
        match self {
            Self::PreJournalAbort { .. } | Self::Finalized { .. } => None,
            Self::Journaled { record, .. } => Some(record),
        }
    }

    fn restore_expectation(&self) -> RuntimeTransactionRestoreExpectation {
        match self {
            Self::PreJournalAbort { .. } => RuntimeTransactionRestoreExpectation::Unchecked,
            Self::Journaled { record, .. } => RuntimeTransactionRestoreExpectation::Exact(Some(
                config::RuntimeTransactionRecord::V2(record.clone()),
            )),
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
    restart_managed_science_with_budget(
        app,
        state,
        lifecycle,
        auth_proof,
        prior,
        operation::SANDBOX_HEALTH_BUDGET_MS,
    )
}

fn restart_managed_science_with_budget<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    _lifecycle: &lifecycle::Lifecycle,
    _auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    prior: &PriorScienceContext,
    health_budget_ms: u64,
) -> Result<(), ManagedScienceRestartError> {
    let dir = config::default_dir();
    let cfg = config::load_from(&dir).map_err(|error| error.to_string())?;
    if cfg.sandbox_port != prior.port {
        return Err("恢复 prior Science 时沙箱端口已变化".into());
    }
    if ScienceHostAdapter::validate_launch_runtime(&prior.runtime).is_err() {
        return Err("恢复 prior Science 时 runtime 身份已变化".into());
    }
    if proc::loopback_port_in_use(prior.port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
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
            &prior.runtime,
            prior.port,
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
        if seams.prior_restart_post_spawn_failure_port == Some(prior.port) {
            let listener_pid = crate::runtime::science::test_unique_listener_pid(prior.port)
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
                    &prior.runtime,
                    ScienceStopOwnershipReceipt::from_managed_launch(&_candidate_token),
                ),
            );
            return Err(ManagedScienceRestartError::test_post_spawn_validation(
                &prior.runtime,
                cleanup,
            ));
        }
    }
    let _token = match ScienceHostAdapter::commit_launch(verified) {
        Ok(receipt) => receipt.ownership().clone(),
        Err(error) => {
            let mut sandbox = None;
            let mut url = None;
            let token_present = error.ownership().is_some();
            let request = error
                .ownership()
                .map(|token| {
                    ScienceStopRequest::exact(
                        &prior.runtime,
                        ScienceStopOwnershipReceipt::from_managed_launch(token),
                    )
                })
                .unwrap_or_else(|| ScienceStopRequest::recover(Some(&prior.runtime)));
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
                ManagedScienceRestartError::after_exact_cleanup(message, &prior.runtime, cleanup)
            } else {
                ManagedScienceRestartError::after_spawn_unproven(message)
            });
        }
    };
    let url = ScienceHostAdapter::url(prior.port, &prior.runtime);
    let mut current = lock(state);
    current.sandbox_port = prior.port;
    current.sandbox_url = Some(url);
    current.science_runtime = Some(prior.runtime.clone());
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
        if let config::RuntimeFinalizeAction::CommitBinding { binding } = &action {
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
            config::RuntimeFinalizeAction::CommitBinding { binding },
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
            config::RuntimeFinalizeAction::CommitBinding { binding } => {
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
    let _ = cleanup;
    Ok(())
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

#[derive(Debug)]
pub(super) enum CompensationCause {
    ScienceCleanup { safe_detail: String },
    SshCleanup,
    AuthorityRestore,
    PriorScienceRestart(ManagedScienceRestartError),
    SnapshotCleanup(AuthorityCleanupFailure),
}

impl CompensationCause {
    fn render_diagnostic(&self) -> String {
        match self {
            Self::ScienceCleanup { safe_detail } => format!(
                "compensation_science_cleanup_failed；compensation_restore_blocked_science_candidate；{safe_detail}"
            ),
            Self::SshCleanup => "compensation_ssh_cleanup_failed".into(),
            Self::AuthorityRestore => "compensation_restore_failed".into(),
            Self::PriorScienceRestart(error) => match error.diagnostic {
                #[cfg(test)]
                PriorScienceRestartDiagnostic::TestPostSpawnValidationFailed => {
                    "test-only prior Science post-spawn validation failure".into()
                }
                PriorScienceRestartDiagnostic::Failed => {
                    "compensation_prior_science_restart_failed".into()
                }
            },
            Self::SnapshotCleanup(error) if error.cleanup_requirement().is_some() => {
                error.to_string()
            }
            Self::SnapshotCleanup(_) => "compensation_snapshot_register_failed".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompensationSkipCause {
    NoScienceCandidate,
    NoPriorScience,
    CrossRuntimeEnvironment,
    BlockedByScienceCleanup,
    BlockedByAuthorityRestore,
    SnapshotPreserved,
}

#[derive(Debug)]
pub(super) enum CompensationStepOutcome {
    Succeeded,
    Skipped(CompensationSkipCause),
    Failed(CompensationCause),
}

impl CompensationStepOutcome {
    fn succeeded(&self) -> bool {
        matches!(self, Self::Succeeded)
    }

    fn completed_or_not_required(&self) -> bool {
        matches!(
            self,
            Self::Succeeded
                | Self::Skipped(CompensationSkipCause::NoScienceCandidate)
                | Self::Skipped(CompensationSkipCause::NoPriorScience)
                | Self::Skipped(CompensationSkipCause::CrossRuntimeEnvironment)
        )
    }

    fn cause(&self) -> Option<&CompensationCause> {
        match self {
            Self::Failed(cause) => Some(cause),
            Self::Succeeded | Self::Skipped(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CompensationEnvironment {
    Quiescent,
    Candidate,
    CrossRuntime,
}

impl CompensationEnvironment {
    fn from_launch(environment: ScienceEnvironmentExposure, cross_runtime: bool) -> Self {
        if cross_runtime {
            Self::CrossRuntime
        } else {
            match environment {
                ScienceEnvironmentExposure::NotExposed => Self::Quiescent,
                ScienceEnvironmentExposure::Exposed | ScienceEnvironmentExposure::Uncertain => {
                    Self::Candidate
                }
            }
        }
    }

    fn is_uncertain(self) -> bool {
        self != Self::Quiescent
    }

    fn append_diagnostics(self, diagnostics: &mut Vec<String>) {
        if self.is_uncertain() {
            diagnostics.push("environment_uncertain".into());
        }
        if self == Self::CrossRuntime {
            diagnostics.push("newer_runtime_required".into());
        }
    }
}

#[derive(Debug)]
pub(super) struct CompensationOutcome {
    pub(super) science_cleanup: CompensationStepOutcome,
    pub(super) ssh_cleanup: CompensationStepOutcome,
    pub(super) authority_restore: CompensationStepOutcome,
    pub(super) prior_science_restart: CompensationStepOutcome,
    pub(super) snapshot_cleanup: CompensationStepOutcome,
    pub(super) environment: CompensationEnvironment,
}

impl CompensationOutcome {
    fn blocked_by_science_cleanup(
        safe_detail: String,
        environment: CompensationEnvironment,
    ) -> Self {
        Self {
            science_cleanup: CompensationStepOutcome::Failed(CompensationCause::ScienceCleanup {
                safe_detail,
            }),
            ssh_cleanup: CompensationStepOutcome::Skipped(
                CompensationSkipCause::BlockedByScienceCleanup,
            ),
            authority_restore: CompensationStepOutcome::Skipped(
                CompensationSkipCause::BlockedByScienceCleanup,
            ),
            prior_science_restart: CompensationStepOutcome::Skipped(
                CompensationSkipCause::BlockedByScienceCleanup,
            ),
            snapshot_cleanup: CompensationStepOutcome::Skipped(
                CompensationSkipCause::SnapshotPreserved,
            ),
            environment,
        }
    }

    pub(super) fn authorities_restored(&self) -> bool {
        self.science_cleanup.completed_or_not_required()
            && self.ssh_cleanup.succeeded()
            && self.authority_restore.succeeded()
            && self.prior_science_restart.completed_or_not_required()
    }

    pub(super) fn prior_science_restored(&self) -> bool {
        self.authorities_restored() && self.prior_science_restart.succeeded()
    }

    fn cleanup_required(&self) -> bool {
        matches!(
            &self.snapshot_cleanup,
            CompensationStepOutcome::Failed(CompensationCause::SnapshotCleanup(error))
                if error.cleanup_requirement().is_some()
        ) || matches!(
            self.science_cleanup,
            CompensationStepOutcome::Failed(CompensationCause::ScienceCleanup { .. })
        )
    }

    pub(super) fn projected_recovery(&self) -> ProjectedRecovery {
        if self.cleanup_required() && self.environment.is_uncertain() {
            ProjectedRecovery::cleanup_required_uncertain()
        } else if self.cleanup_required() {
            ProjectedRecovery::CLEANUP_REQUIRED
        } else if self.environment.is_uncertain() {
            ProjectedRecovery::ENVIRONMENT_UNCERTAIN
        } else if self.authorities_restored() {
            ProjectedRecovery::NOT_NEEDED
        } else {
            ProjectedRecovery::DEGRADED
        }
    }

    fn render_failure_message(&self, primary: &str) -> String {
        let mut diagnostics = [
            &self.science_cleanup,
            &self.ssh_cleanup,
            &self.authority_restore,
        ]
        .into_iter()
        .filter_map(CompensationStepOutcome::cause)
        .map(CompensationCause::render_diagnostic)
        .collect::<Vec<_>>();
        self.environment.append_diagnostics(&mut diagnostics);
        diagnostics.extend(
            [&self.prior_science_restart, &self.snapshot_cleanup]
                .into_iter()
                .filter_map(CompensationStepOutcome::cause)
                .map(CompensationCause::render_diagnostic),
        );
        if diagnostics.is_empty() {
            primary.to_string()
        } else {
            format!("{primary}；{}", diagnostics.join("; "))
        }
    }
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
fn compensate_one_click_failure<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    dir: &Path,
    trace: &OperationTrace,
    authority_transaction: &mut AuthorityTransaction,
    journal_progress: &OneClickJournalProgress,
    prior_science: Option<&PriorScienceContext>,
    failure: OneClickFailure,
    mut reconcile_disposition: Option<&mut PriorScienceDisposition>,
) -> Result<Value, TypedOneClickFailure> {
    let original_kind = failure.typed.kind();
    if let OneClickJournalProgress::PreJournalAbort { registered_ticket } = journal_progress {
        let in_memory_ticket = authority_transaction.registered_snapshot_ticket();
        if in_memory_ticket.as_ref() != Ok(registered_ticket) {
            authority_transaction.preserve_recovery();
            trace.finish("error=pre_journal_abort_ticket_unverified");
            return Err(TypedOneClickFailure::new(
                original_kind,
                "首个 one-click V2 checkpoint 失败，且同进程 registered snapshot ticket 无法重新验证；已保留 ActiveRecovery 并要求人工恢复。",
            )
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED));
        }
    }
    let cross_runtime_environment = failure.rollback.launch_environment.may_be_exposed()
        && prior_science.is_some_and(|prior| prior.runtime != failure.rollback.launch_runtime);
    let environment = CompensationEnvironment::from_launch(
        failure.rollback.launch_environment,
        cross_runtime_environment,
    );
    let science_cleanup_required = failure.rollback.launch_environment.may_be_exposed()
        || failure.rollback.launch_token.is_some();
    let cleanup = if failure.rollback.candidate_stop_proof
        == ManagedScienceCandidateStopProof::Unproven
    {
        Err("code=science_candidate_stop_unproven".into())
    } else if !failure.rollback.launch_environment.may_be_exposed()
        && failure.rollback.launch_token.is_none()
    {
        Ok(())
    } else if failure.rollback.launch_confirmed_stopped {
        let receipt = dir.join("science-managed-launch.v1.json");
        if proc::loopback_port_in_use(
            failure.rollback.sandbox_port,
            operation::LOCAL_HEALTH_TIMEOUT_MS,
        ) || failure
            .rollback
            .launch_token
            .as_ref()
            .is_some_and(ScienceHostAdapter::receipt_process_is_alive)
            || receipt.exists()
        {
            Err("DB recovery restart 前已停止的 Science 身份重新出现；拒绝恢复 authority".into())
        } else {
            Ok(())
        }
    } else {
        let mut current = lock(state);
        let AppState {
            sandbox,
            sandbox_url,
            ..
        } = &mut *current;
        let result = ScienceHostAdapter::stop(
            app,
            sandbox,
            sandbox_url,
            failure
                .rollback
                .launch_token
                .as_ref()
                .map(|token| {
                    ScienceStopRequest::exact(
                        &failure.rollback.launch_runtime,
                        ScienceStopOwnershipReceipt::from_managed_launch(token),
                    )
                })
                .unwrap_or_else(|| {
                    ScienceStopRequest::recover(Some(&failure.rollback.launch_runtime))
                }),
        );
        let result = result
            .and_then(|verified| verified.require_exact_stop_of(&failure.rollback.launch_runtime));
        if let Ok(verified) = result.as_ref() {
            current.science_runtime = None;
            current.science_confirmed_stopped = verified.confirmed_runtime().cloned();
        }
        result.map(|_| ()).map_err(|error| error.to_string())
    };
    if let Err(cleanup_error) = cleanup.as_ref() {
        let outcome =
            CompensationOutcome::blocked_by_science_cleanup(cleanup_error.to_string(), environment);
        if outcome.environment.is_uncertain() {
            if let Some(disposition) = reconcile_disposition.as_deref_mut() {
                *disposition = PriorScienceDisposition::EnvironmentUncertain;
            }
        }
        authority_transaction.preserve_recovery();
        trace.finish("error=compensation_restore_blocked_science_cleanup_unproven");
        let cleanup_failure = cleanup_required_error(
            AuthorityCleanupPhase::Cleanup,
            &outcome.render_failure_message(failure.message()),
            authority_transaction.recovery_path(),
            "science_candidate_stop_unproven",
        );
        let phase = cleanup_failure.phase();
        return Err(
            TypedOneClickFailure::new(original_kind, cleanup_failure.to_string())
                .with_recovery(outcome.projected_recovery())
                .with_safe_cause(phase.cause_code(), "authority cleanup typed failure"),
        );
    }
    let ssh_cleanup = match failure.rollback.ssh_stub_transaction.as_ref() {
        Some(transaction) => transaction.compensate(&sandbox_home()),
        None => crate::runtime::settings::remove_managed_sandbox_ssh_stub(&sandbox_home()),
    };
    let runtime_transaction_restore = journal_progress.restore_expectation();
    let rollback = authority_transaction.restore(
        app,
        dir,
        state,
        lifecycle,
        auth_proof,
        failure.rollback.proxy_action,
        &runtime_transaction_restore,
    );
    let prior_restart = if rollback.is_ok() && !cross_runtime_environment {
        match prior_science {
            Some(prior) => match restart_prior_science(app, state, lifecycle, auth_proof, prior) {
                Ok(()) => CompensationStepOutcome::Succeeded,
                Err(error) => {
                    CompensationStepOutcome::Failed(CompensationCause::PriorScienceRestart(error))
                }
            },
            None => CompensationStepOutcome::Skipped(CompensationSkipCause::NoPriorScience),
        }
    } else if cross_runtime_environment {
        CompensationStepOutcome::Skipped(CompensationSkipCause::CrossRuntimeEnvironment)
    } else {
        CompensationStepOutcome::Skipped(CompensationSkipCause::BlockedByAuthorityRestore)
    };
    let mut outcome = CompensationOutcome {
        science_cleanup: if science_cleanup_required {
            CompensationStepOutcome::Succeeded
        } else {
            CompensationStepOutcome::Skipped(CompensationSkipCause::NoScienceCandidate)
        },
        ssh_cleanup: match ssh_cleanup {
            Ok(_) => CompensationStepOutcome::Succeeded,
            Err(_) => CompensationStepOutcome::Failed(CompensationCause::SshCleanup),
        },
        authority_restore: match rollback {
            Ok(()) => CompensationStepOutcome::Succeeded,
            Err(_) => CompensationStepOutcome::Failed(CompensationCause::AuthorityRestore),
        },
        prior_science_restart: prior_restart,
        snapshot_cleanup: CompensationStepOutcome::Skipped(
            CompensationSkipCause::SnapshotPreserved,
        ),
        environment,
    };
    let authorities_restored = outcome.authorities_restored();
    let prior_science_restored = outcome.prior_science_restored();
    if let Some(disposition) = reconcile_disposition {
        if outcome.environment.is_uncertain() {
            *disposition = PriorScienceDisposition::EnvironmentUncertain;
        } else if prior_science_restored {
            *disposition = PriorScienceDisposition::Restored;
        }
    }
    outcome.snapshot_cleanup = if authorities_restored {
        match authority_transaction.cleanup_when_expendable() {
            Ok(_) => CompensationStepOutcome::Succeeded,
            Err(error) => {
                CompensationStepOutcome::Failed(CompensationCause::SnapshotCleanup(error))
            }
        }
    } else {
        authority_transaction.preserve_recovery();
        CompensationStepOutcome::Skipped(CompensationSkipCause::SnapshotPreserved)
    };
    trace.finish(
        if authorities_restored && outcome.environment.is_uncertain() {
            "error=one_click_transaction_compensated environment=uncertain"
        } else if authorities_restored {
            "error=one_click_transaction_compensated environment=not_exposed"
        } else {
            "error=one_click_compensation_incomplete"
        },
    );
    let message = outcome.render_failure_message(failure.message());
    let recovery = outcome.projected_recovery();
    Err(TypedOneClickFailure::new(original_kind, message).with_recovery(recovery))
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn test_compensate_one_click_failure<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    dir: &Path,
    authority_transaction: &mut AuthorityTransaction,
    journal_progress: &OneClickJournalProgress,
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
    compensate_one_click_failure(
        app,
        state,
        lifecycle,
        None,
        dir,
        &trace,
        authority_transaction,
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
    mut reconcile_disposition: Option<&mut PriorScienceDisposition>,
    entry_progress: OneClickEntryProgress<'_>,
) -> Result<Value, TypedOneClickFailure> {
    let trace = OperationTrace::start(OperationKind::OneClickLogin, "command=one_click_login");
    let dir = config::default_dir();
    let cfg = config::load_from(&dir)
        .map_err(|e| typed_one_click_err(OneClickFailureKind::ConfigLoad, e.to_string()))?;
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
    let ssh_prevalidation =
        crate::runtime::sandbox_session::prevalidate_one_click_system_ssh(&app, &cfg, &sbx_home)
            .map_err(|message| typed_one_click_err(OneClickFailureKind::Prepare, message))?;
    let ssh_stub_transaction = cfg
        .reuse_system_ssh
        .then(|| {
            crate::runtime::settings::ManagedSshStubTransaction::capture(
                &sbx_home,
                &ssh_prevalidation,
            )
        })
        .transpose()
        .map_err(|message| typed_one_click_err(OneClickFailureKind::Prepare, message))?;
    validate_interrupted_science_environment_runtime(
        interrupted_environment_runtime_id,
        &launch_runtime,
    )
    .map_err(|error| typed_interrupted_science_err(OneClickFailureKind::Prepare, error))?;
    let candidate_fingerprint = launch_runtime.environment_transaction_id();
    let mut rollback_context = OneClickRollbackContext {
        proxy_action: ProxyAction::Reused,
        sandbox_port: sport,
        launch_runtime: launch_runtime.clone(),
        launch_token: None,
        launch_environment: ScienceEnvironmentExposure::NotExposed,
        launch_confirmed_stopped: false,
        candidate_stop_proof: ManagedScienceCandidateStopProof::NotRequired,
        ssh_stub_transaction,
        current_kind: OneClickFailureKind::Prepare,
    };
    let prior_science = match running_runtime_to_stop.as_ref() {
        Some(runtime) => Some(PriorScienceContext {
            runtime: runtime.clone(),
            port: sport,
            launch_token: ScienceHostAdapter::managed_receipt(sport, runtime).ok_or_else(|| {
                typed_one_click_err(
                    OneClickFailureKind::ScienceStop,
                    "prior Science managed launch 身份无法确认，拒绝停止或快照",
                )
            })?,
        }),
        None => None,
    };
    let mut prior_stop_record = None;
    if let Some(prior) = prior_science.as_ref() {
        let recipe = prior
            .launch_token
            .durable_prior_stop_recipe(&prior.runtime, prior.port)
            .map_err(|error| typed_one_click_err(OneClickFailureKind::ScienceStop, error))?;
        let intent = begin_prior_stop_intent(
            &dir,
            &active_profile.id,
            &candidate_fingerprint,
            cfg.runtime_binding.as_ref(),
            profile_switch_handoff.as_ref(),
            gateway_terminal_handoff.as_ref(),
            recipe,
        )
        .map_err(|error| typed_one_click_err(OneClickFailureKind::ScienceStop, error))?;
        let stop_result = {
            let mut current = lock(&state);
            let AppState {
                sandbox,
                sandbox_url,
                ..
            } = &mut *current;
            let result = ScienceHostAdapter::stop(
                &app,
                sandbox,
                sandbox_url,
                ScienceStopRequest::exact(
                    &prior.runtime,
                    ScienceStopOwnershipReceipt::from_managed_launch(&prior.launch_token),
                ),
            )
            .and_then(|verified| verified.require_exact_stop_of(&prior.runtime));
            if let Ok(verified) = &result {
                current.science_runtime = None;
                current.science_confirmed_stopped = verified.confirmed_runtime().cloned();
            }
            result
        };
        let durable_outcome = match &stop_result {
            Ok(_) => config::RuntimePriorStopOutcome::ExactStopped,
            Err(error) if error.confirmed_runtime() == Some(&prior.runtime) => {
                let confirmed_runtime = error.confirmed_runtime().cloned();
                let mut current = lock(&state);
                current.science_runtime = None;
                current.science_confirmed_stopped = confirmed_runtime;
                config::RuntimePriorStopOutcome::ExactStopped
            }
            Err(error) => match error.kind() {
                ScienceStopFailureKind::RequestRejected => {
                    config::RuntimePriorStopOutcome::NotStopped
                }
                ScienceStopFailureKind::IdentityDrift
                | ScienceStopFailureKind::StopCommandFailed
                | ScienceStopFailureKind::SignalFailure
                | ScienceStopFailureKind::ExitUnconfirmed
                | ScienceStopFailureKind::ReceiptCleanupFailure
                | ScienceStopFailureKind::OutcomePublicationFailure => {
                    config::RuntimePriorStopOutcome::Unknown
                }
            },
        };
        let published = publish_prior_stop_outcome(&dir, &intent, durable_outcome).map_err(|error| {
            TypedOneClickFailure::new(
                OneClickFailureKind::ScienceStop,
                format!(
                    "prior Science stop outcome 无法持久化；durable intent 已保留并要求人工恢复：{error}"
                ),
            )
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
        })?;
        if let Err(error) = stop_result {
            return Err(TypedOneClickFailure::new(
                OneClickFailureKind::ScienceStop,
                error.to_string(),
            )
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED));
        }
        prior_stop_record = Some(published);
        let receipt = dir.join("science-managed-launch.v1.json");
        if proc::loopback_port_in_use(sport, operation::LOCAL_HEALTH_TIMEOUT_MS)
            || ScienceHostAdapter::receipt_process_is_alive(&prior.launch_token)
            || receipt.exists()
        {
            let restart = restart_prior_science(&app, &state, lifecycle, auth_proof, prior);
            if restart.is_ok() {
                if let Some(expected) = prior_stop_record.as_ref() {
                    clear_prior_stop_transition(&dir, expected).map_err(|error| {
                        TypedOneClickFailure::new(
                            OneClickFailureKind::ScienceStop,
                            format!(
                                "prior Science 已恢复，但 durable stop outcome 无法清除：{error}"
                            ),
                        )
                        .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
                    })?;
                }
            }
            return Err(typed_one_click_err(
                OneClickFailureKind::ScienceStop,
                format!(
                    "prior Science 未完成 verified stop，拒绝建立 authority 快照；restart={restart:?}"
                ),
            ));
        }
    }
    let prior_science_for_compensation = prior_science.as_ref();
    trace.stage(
        OperationStage::AuthoritySnapshot,
        "phase=capture_begin scope=protected_state",
    );
    let mut authority_transaction = match capture_authority_after_science_quiesce(
        &app,
        &state,
        lifecycle,
        auth_proof,
        &dir,
        &sbx_home,
        &auth_dir,
        &cfg,
        prior_science_for_compensation,
    ) {
        Ok(snapshot) => {
            trace.stage(
                OperationStage::AuthoritySnapshot,
                "phase=capture_end outcome=ok",
            );
            snapshot
        }
        Err(AuthorityCaptureAfterQuiesceError::PriorScienceRestored(cause)) => {
            trace.stage(
                OperationStage::AuthoritySnapshot,
                "phase=capture_end outcome=error prior_science=restored",
            );
            if let Some(disposition) = reconcile_disposition.as_deref_mut() {
                *disposition = PriorScienceDisposition::Restored;
            }
            if let Some(expected) = prior_stop_record.as_ref() {
                clear_prior_stop_transition(&dir, expected).map_err(|error| {
                    TypedOneClickFailure::new(
                        OneClickFailureKind::AuthoritySnapshot,
                        format!("prior Science 已恢复，但 durable stop outcome 无法清除：{error}"),
                    )
                    .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)
                })?;
            }
            return Err(typed_one_click_err(
                OneClickFailureKind::AuthoritySnapshot,
                cause,
            ));
        }
        Err(AuthorityCaptureAfterQuiesceError::RestartRequired(cause)) => {
            trace.stage(
                OperationStage::AuthoritySnapshot,
                "phase=capture_end outcome=error prior_science=restart_required",
            );
            return Err(typed_one_click_err(
                OneClickFailureKind::AuthoritySnapshot,
                cause,
            ));
        }
    };
    let snapshot_ticket = match authority_transaction.registered_snapshot_ticket() {
        Ok(ticket) => ticket,
        Err(cause) => {
            authority_transaction.preserve_recovery();
            return Err(TypedOneClickFailure::new(
                OneClickFailureKind::AuthoritySnapshot,
                format!(
                    "authority snapshot registered ticket 无法验证，已保留 ActiveRecovery 并要求人工恢复：{cause}"
                ),
            )
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED));
        }
    };
    let transaction_identity = OneClickTransactionIdentity {
        target_profile_id: active_profile.id.clone(),
        runtime_fingerprint: candidate_fingerprint,
        snapshot_ticket: snapshot_ticket.clone(),
        previous_binding: cfg.runtime_binding.clone(),
        profile_switch_handoff,
        gateway_terminal_handoff,
        prior_stop: prior_stop_record
            .as_ref()
            .map(|record| record.prior_stop.clone())
            .unwrap_or(config::RuntimePriorStopState::NotRequired),
    };
    let mut journal_progress = match prior_stop_record {
        Some(record) => OneClickJournalProgress::Journaled {
            record,
            registered_ticket: snapshot_ticket,
        },
        None => OneClickJournalProgress::PreJournalAbort {
            registered_ticket: snapshot_ticket,
        },
    };
    let transaction_result = (|| -> Result<Value, OneClickFailure> {
        if running_runtime_to_stop.is_some() {
            rollback_context.set_kind(OneClickFailureKind::ScienceStop);
            one_click_step(
                write_one_click_checkpoint(
                    &dir,
                    &transaction_identity,
                    &mut journal_progress,
                    config::RuntimeTransactionPhase::StopOldScience,
                ),
                &rollback_context,
            )?;
        }
        rollback_context.set_kind(OneClickFailureKind::Prepare);
        let _transaction_cfg = one_click_step(config::load_from(&dir), &rollback_context)?;
        one_click_step(
            write_one_click_checkpoint(
                &dir,
                &transaction_identity,
                &mut journal_progress,
                config::RuntimeTransactionPhase::StartGateway,
            ),
            &rollback_context,
        )?;
        let preview_port = match sport.checked_add(1) {
            Some(port) => port,
            None => {
                return Err(
                    rollback_context.failure("沙箱端口必须小于 65535，才能分配隔离预览端口。")
                )
            }
        };
        if proc::loopback_port_in_use(preview_port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
            return Err(rollback_context.failure(format!(
                "隔离 Science 预览端口 {preview_port} 已被占用；未启动或结束任何占用者。请修改沙箱端口后重试。"
            )));
        }
        let verified_stopped_runtime = {
            let mut current = lock(&state);
            let verified = current.science_confirmed_stopped.clone();
            current.science_confirmed_stopped = None;
            verified
        };
        let history_science_quiescence = match verified_stopped_runtime.as_ref() {
            Some(runtime) => HistoryRecoveryScienceQuiescence::ExactStopped(runtime.clone()),
            None if running_runtime_to_stop.is_none()
                && !remembered_runtime_was_present
                && science_state == SandboxScienceState::Stopped =>
            {
                HistoryRecoveryScienceQuiescence::NoManagedRuntimeObserved
            }
            None => {
                return Err(rollback_context
                    .failure("历史恢复前缺少 typed Science quiescence proof；已拒绝发布选择会话"));
            }
        };
        rollback_context.set_kind(OneClickFailureKind::AuthoritySnapshot);
        one_click_step(
            authority_transaction.validate_science_restore_root(),
            &rollback_context,
        )?;
        one_click_step(
            write_one_click_checkpoint(
                &dir,
                &transaction_identity,
                &mut journal_progress,
                config::RuntimeTransactionPhase::AuthoritySnapshotActive,
            ),
            &rollback_context,
        )?;

        rollback_context.set_kind(OneClickFailureKind::SandboxLogin);
        trace.stage(OperationStage::SandboxLogin, "ensure_virtual_login");
        let (forged, login_action) = match oauth_forge::ensure_virtual_login(
            &auth_dir,
            "virtual@localhost.invalid",
            &sbx_home,
        ) {
            Ok(result) => result,
            Err(oauth_forge::EnsureVirtualLoginError::HistoryChoiceRequired(candidates)) => {
                let (choices, visible_choices) =
                    one_click_step(history_recovery_choices(candidates), &rollback_context)?;
                {
                    let mut app_state = lock(&state);
                    app_state.science_confirmed_stopped = verified_stopped_runtime.clone();
                    app_state.history_recovery = Some(HistoryRecoverySession {
                        active_profile_id: active_profile.id.clone(),
                        sandbox_port: sport,
                        auth_dir: auth_dir.clone(),
                        sandbox_root: sbx_home.clone(),
                        science_quiescence: history_science_quiescence.clone(),
                        choices,
                    });
                }
                trace.finish("attention=history_choice_required");
                let mut value = json!({
                    "msg": "检测到多份旧历史记录。请选择要恢复的一份；CSSwitch 不会删除其他记录。",
                    "action": "history_choice_required",
                    "stage": "history_recovery",
                    "status": "attention",
                    "recovery_status": "choice_required",
                    "choices": visible_choices,
                    "fallback_url": null
                });
                one_click_step(
                    begin_one_click_finalize(
                        &dir,
                        &transaction_identity,
                        &mut journal_progress,
                        config::RuntimeFinalizeAction::ClearJournal,
                    ),
                    &rollback_context,
                )?;
                if authority_transaction.prepare_success(&mut value).is_err() {
                    preserve_interrupted_success_finalize(&mut authority_transaction, &mut value);
                    trace.finish("degraded=success_finalize_pending");
                    return Ok(value);
                }
                if complete_one_click_finalize(&dir, &mut journal_progress).is_err() {
                    preserve_interrupted_success_finalize(&mut authority_transaction, &mut value);
                    trace.finish("degraded=success_finalize_pending");
                    return Ok(value);
                }
                return Ok(value);
            }
            Err(oauth_forge::EnsureVirtualLoginError::Message(message)) => {
                return Err(rollback_context.failure(format!("写虚拟登录失败：{message}")));
            }
        };
        let _validated_login_identity = (
            &forged.auth_dir,
            &forged.account_uuid,
            &forged.org_uuid,
            &forged.enc_file,
        );
        // Resource discovery is prepare-class; only login failures keep SandboxLogin.
        rollback_context.set_kind(OneClickFailureKind::Prepare);
        let root = match asset_root(&app) {
            Some(root) => root,
            None => {
                return Err(rollback_context.failure(
                    "找不到 scripts/launch-virtual-sandbox.sh（打包资源或仓库根均未命中）。",
                ))
            }
        };
        let launch = root.join("scripts/launch-virtual-sandbox.sh");
        if !launch.is_file() {
            return Err(rollback_context.failure("找不到 scripts/launch-virtual-sandbox.sh。"));
        }
        #[cfg(test)]
        if let Some(hosts) = std::env::var_os("CSSWITCH_TEST_SSH_HOSTS_AFTER_CAPTURE") {
            let config = one_click_step(
                crate::runtime::settings::system_ssh_config_path(),
                &rollback_context,
            )?;
            one_click_step(
                std::fs::write(&config, format!("Host {}\n", hosts.to_string_lossy())),
                &rollback_context,
            )?;
            one_click_step(
                std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o600)),
                &rollback_context,
            )?;
        }
        rollback_context.set_kind(OneClickFailureKind::Prepare);
        let ssh_hosts = if cfg.reuse_system_ssh {
            one_click_step(
                crate::runtime::ssh_bridge::prepare_science_ssh_bridge(&sbx_home),
                &rollback_context,
            )?
        } else {
            one_click_step(
                crate::runtime::ssh_bridge::revoke_science_ssh_bridge(&sbx_home),
                &rollback_context,
            )?;
            Vec::new()
        };
        if cfg.reuse_system_ssh {
            if let Some(transaction) = rollback_context.ssh_stub_transaction.as_ref() {
                one_click_step(
                    transaction.validate_prepared_hosts(&ssh_hosts),
                    &rollback_context,
                )?;
            }
        }
        rollback_context.set_kind(OneClickFailureKind::GatewayStart);
        let gateway = one_click_step(
            GatewayController::ensure_active(
                &app,
                &state,
                lifecycle,
                Some(&launch_runtime),
                Some(&trace),
                auth_proof,
            ),
            &rollback_context,
        )?;
        let pport = gateway.port;
        let secret = gateway.route_secret;
        let proxy_action = gateway.action;
        rollback_context.proxy_action = proxy_action;
        rollback_context.set_kind(OneClickFailureKind::CatalogVerify);
        one_click_step(
            verify_gateway_model_catalog_traced(&trace, pport, &secret, active_profile),
            &rollback_context,
        )?;
        rollback_context.set_kind(OneClickFailureKind::SandboxLaunch);
        one_click_step(
            write_one_click_checkpoint(
                &dir,
                &transaction_identity,
                &mut journal_progress,
                config::RuntimeTransactionPhase::StartScienceEnvironmentPending,
            ),
            &rollback_context,
        )?;
        let installer_bridge =
            one_click_step(skill_install_bridge_dir(&secret), &rollback_context)?;
        let installer = match current_skill_install_bridge_key() {
            Ok(installer_key) => {
                register_before_science_start(&app, &auth_dir, &installer_bridge, &installer_key)
            }
            Err(error) => RegistrationStatus::Warning(error),
        };
        let proxy_url = format!("http://127.0.0.1:{pport}/{secret}");
        let logf = match open_log("sandbox.log") {
            Ok(file) => file,
            Err(error) => return Err(rollback_context.failure(format!("建日志失败：{error}"))),
        };
        {
            use std::io::Write;
            let mut writer = &logf;
            let _ = writeln!(
                writer,
                "[oauth] 虚拟登录已就绪（Rust，零 node；action={:?}；isolated=true）",
                login_action
            );
        }
        let logf2 = one_click_step(logf.try_clone(), &rollback_context)?;
        trace.stage(OperationStage::SandboxLaunch, format!("port={sport}"));
        if ScienceHostAdapter::validate_launch_runtime(&launch_runtime).is_err() {
            return Err(
                rollback_context.failure("Science runtime 在预检后发生变化；已拒绝启动，请重试")
            );
        }
        one_click_step(
            authority_transaction.validate_science_restore_root(),
            &rollback_context,
        )?;
        #[cfg(test)]
        if let Some(foreign_stub) =
            std::env::var_os("CSSWITCH_TEST_SSH_LATE_FOREIGN_STUB").map(std::path::PathBuf::from)
        {
            let parent = match foreign_stub.parent() {
                Some(parent) => parent,
                None => {
                    return Err(rollback_context.failure("SSH late-failure test stub has no parent"))
                }
            };
            one_click_step(std::fs::create_dir_all(parent), &rollback_context)?;
            one_click_step(
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)),
                &rollback_context,
            )?;
            let mut file = one_click_step(
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&foreign_stub),
                &rollback_context,
            )?;
            one_click_step(
                std::io::Write::write_all(&mut file, b"foreign-test-stub-must-survive\n"),
                &rollback_context,
            )?;
            one_click_step(file.sync_all(), &rollback_context)?;
        }
        one_click_step(
            authority_transaction.validate_science_restore_root(),
            &rollback_context,
        )?;
        let opaque_bindings = authority_transaction.science_opaque_bindings_env();
        let ssh_hosts = ssh_hosts.join(" ");
        let attempt = match ScienceHostAdapter::spawn_launch(
            ScienceLaunchSpec::one_click(
                &launch,
                &launch_runtime,
                sport,
                &proxy_url,
                cfg.reuse_system_ssh,
                &ssh_hosts,
                Some(opaque_bindings.as_str()),
                operation::SANDBOX_HEALTH_BUDGET_MS,
                POLL_INTERVAL_MS,
                operation::LOCAL_HEALTH_TIMEOUT_MS,
            ),
            logf,
            logf2,
        ) {
            Ok(attempt) => attempt,
            Err(error) => {
                rollback_context.launch_environment = error.environment();
                if error.kind() == ScienceLaunchFailureKind::WaitFailed {
                    rollback_context.candidate_stop_proof =
                        ManagedScienceCandidateStopProof::Unproven;
                    return Err(rollback_context.failure(format!(
                        "起沙箱状态未知：{}；code=science_candidate_stop_unproven",
                        error.message()
                    )));
                }
                return Err(rollback_context.failure(format!("起沙箱失败：{}", error.message())));
            }
        };
        rollback_context.launch_environment = attempt.environment();
        if let Some(transaction) = rollback_context.ssh_stub_transaction.as_mut() {
            transaction.observe_after_launch(&sbx_home);
        }
        let attempt = match ScienceHostAdapter::accept_launch_script(attempt) {
            Ok(attempt) => attempt,
            Err(_) => {
                let tail = redact(&tail_file(&log_path("sandbox.log"), 600), &secret);
                return Err(rollback_context.failure(format!("起沙箱脚本失败。\n{tail}")));
            }
        };
        {
            let mut current = lock(&state);
            current.sandbox_port = sport;
            current.science_runtime = Some(launch_runtime.clone());
            current.science_confirmed_stopped = None;
        }
        rollback_context.set_kind(OneClickFailureKind::SandboxHealth);
        let healthy = match ScienceHostAdapter::verify_health(attempt) {
            Ok(healthy) => {
                trace.stage(OperationStage::SandboxHealth, "ready");
                healthy
            }
            Err(_) => {
                trace.stage(OperationStage::SandboxHealth, "not_ready");
                let tail = redact(&tail_file(&log_path("sandbox.log"), 600), &secret);
                return Err(
                    rollback_context.failure(format!("沙箱起后探活超时（端口 {sport}）。\n{tail}"))
                );
            }
        };
        let verified = match ScienceHostAdapter::verify_identity(healthy) {
            Ok(verified) => verified,
            Err(_) => {
                return Err(rollback_context.failure(format!(
                "端口 {sport} 有服务响应，但按 data-dir 确认不是本沙箱 Science（疑似被其它服务占用）。"
            )));
            }
        };
        match ScienceHostAdapter::commit_launch(verified) {
            Ok(receipt) => rollback_context.launch_token = Some(receipt.ownership().clone()),
            Err(error) => {
                rollback_context.launch_token = error.ownership().cloned();
                return Err(rollback_context.failure(format!(
                    "Science 已启动但受管启动身份无法安全提交：{}",
                    error.message()
                )));
            }
        }
        rollback_context.set_kind(OneClickFailureKind::ScienceDbReverify);
        one_click_step(
            write_one_click_checkpoint(
                &dir,
                &transaction_identity,
                &mut journal_progress,
                config::RuntimeTransactionPhase::WaitScienceDbReverify,
            ),
            &rollback_context,
        )?;
        let first_token = match rollback_context.launch_token.clone() {
            Some(token) => token,
            None => {
                return Err(rollback_context.failure("Science DB 检查缺少受管启动身份"));
            }
        };
        match one_click_step(
            wait_for_science_db_reverify(sport, &launch_runtime, &first_token),
            &rollback_context,
        )? {
            proc::ScienceDbHealth::Ready => {}
            proc::ScienceDbHealth::ReverifyPending => unreachable!(),
            proc::ScienceDbHealth::RestartRequired => {
                {
                    let mut current = lock(&state);
                    let AppState {
                        sandbox,
                        sandbox_url,
                        ..
                    } = &mut *current;
                    let verified = one_click_step(
                        ScienceHostAdapter::stop(
                            &app,
                            sandbox,
                            sandbox_url,
                            ScienceStopRequest::exact(
                                &launch_runtime,
                                ScienceStopOwnershipReceipt::from_managed_launch(&first_token),
                            ),
                        ),
                        &rollback_context,
                    )?;
                    let verified = one_click_step(
                        verified.require_exact_stop_of(&launch_runtime),
                        &rollback_context,
                    )?;
                    current.science_runtime = None;
                    current.science_confirmed_stopped = verified.confirmed_runtime().cloned();
                }
                rollback_context.launch_confirmed_stopped = true;
                if proc::loopback_port_in_use(sport, operation::LOCAL_HEALTH_TIMEOUT_MS)
                    || ScienceHostAdapter::receipt_process_is_alive(&first_token)
                    || dir.join("science-managed-launch.v1.json").exists()
                {
                    return Err(rollback_context
                        .failure("Science DB recovery 的首次受管进程未完成 verified stop"));
                }
                one_click_step(
                    write_one_click_checkpoint(
                        &dir,
                        &transaction_identity,
                        &mut journal_progress,
                        config::RuntimeTransactionPhase::RestartScienceAfterDbHeal,
                    ),
                    &rollback_context,
                )?;
                let recovery = PriorScienceContext {
                    runtime: launch_runtime.clone(),
                    port: sport,
                    // restart_managed_science_with_budget does not consume the
                    // stopped token; retaining it gives compensation an exact
                    // absence proof until the fresh receipt is committed.
                    launch_token: first_token.clone(),
                };
                if let Err(error) = restart_managed_science_with_budget(
                    &app,
                    &state,
                    lifecycle,
                    auth_proof,
                    &recovery,
                    science_db_recovery_restart_budget_ms(),
                ) {
                    rollback_context.candidate_stop_proof = error.candidate_stop_proof;
                    return Err(rollback_context.failure(error.to_string()));
                }
                let second_token = one_click_step(
                    ScienceHostAdapter::managed_receipt(sport, &launch_runtime)
                        .ok_or("Science DB recovery restart 缺少 fresh managed receipt"),
                    &rollback_context,
                )?;
                rollback_context.launch_token = Some(second_token.clone());
                rollback_context.launch_confirmed_stopped = false;
                one_click_step(
                    write_one_click_checkpoint(
                        &dir,
                        &transaction_identity,
                        &mut journal_progress,
                        config::RuntimeTransactionPhase::VerifyScienceDbAfterRestart,
                    ),
                    &rollback_context,
                )?;
                let second_state = one_click_step(
                    wait_for_science_db_reverify(sport, &launch_runtime, &second_token),
                    &rollback_context,
                )?;
                if !ScienceHostAdapter::receipt_is_current(&second_token, &launch_runtime)
                    || second_state != proc::ScienceDbHealth::Ready
                {
                    return Err(rollback_context.failure(format!(
                        "Science DB recovery 的第二次启动未达到 clear/clear：{second_state:?}"
                    )));
                }
            }
        }
        rollback_context.set_kind(OneClickFailureKind::Prepare);
        one_click_step(
            write_one_click_checkpoint(
                &dir,
                &transaction_identity,
                &mut journal_progress,
                config::RuntimeTransactionPhase::VerifyScienceCatalog,
            ),
            &rollback_context,
        )?;
        let installer = configure_third_party_best_effort(
            &app,
            installer,
            &auth_dir,
            sport,
            &launch_runtime,
            false,
        );
        let url = ScienceHostAdapter::url(sport, &launch_runtime);
        {
            let mut current = lock(&state);
            current.sandbox_port = sport;
            current.sandbox_url = Some(url.clone());
            current.science_runtime = Some(launch_runtime.clone());
            current.science_confirmed_stopped = None;
        }
        let started = match login_action {
            oauth_forge::LoginAction::Created => "已启动",
            _ => "沙箱已重新启动，沿用原有对话",
        };
        let refreshed_cfg = one_click_step(config::load_from(&dir), &rollback_context)?;
        let refreshed_profile = match refreshed_cfg.active_profile() {
            Some(profile) => profile,
            None => return Err(rollback_context.failure("生效 profile 在启动期间消失")),
        };
        let committed = one_click_step(
            crate::runtime::provider::desired_runtime_binding(
                &refreshed_cfg,
                refreshed_profile,
                &launch_runtime,
            ),
            &rollback_context,
        )?;
        one_click_step(
            begin_one_click_finalize(
                &dir,
                &transaction_identity,
                &mut journal_progress,
                config::RuntimeFinalizeAction::CommitBinding { binding: committed },
            ),
            &rollback_context,
        )?;
        rollback_context.set_kind(OneClickFailureKind::OpenSurface);
        let (message, fallback_url) = if open_surface {
            match open_science_surface(&app, &url) {
                Ok("webview") => (format!("{started}，已打开 Science 窗口。"), None),
                Ok(_) => (format!("{started}，已向系统浏览器发送打开请求。"), None),
                Err(_) => (
                    format!("{started}，服务已就绪；自动打开失败。"),
                    Some(url.clone()),
                ),
            }
        } else {
            (format!("{started}，Science 已按新模型目录刷新。"), None)
        };
        let message = append_installer_note(message, &installer);
        trace.stage(OperationStage::OpenBrowser, "done");
        trace.finish(format!(
            "ok action=started proxy_action={}",
            proxy_action.as_str()
        ));
        let mut value = json!({
            "msg": message,
            "action": "started",
            "stage": "complete",
            "status": "ok",
            "recovery_status": "not_needed",
            "fallback_url": fallback_url,
            "external_skill_installer": installer_status_json(&installer)
        });
        if interrupted_environment_runtime_id.is_some() {
            value["recovery_status"] = json!("environment_uncertain");
            value["environment_status"] = json!("uncertain");
        }
        rollback_context.set_kind(OneClickFailureKind::AuthoritySnapshot);
        if authority_transaction.prepare_success(&mut value).is_err() {
            preserve_interrupted_success_finalize(&mut authority_transaction, &mut value);
            trace.finish("degraded=success_finalize_pending");
            return Ok(value);
        }
        if complete_one_click_finalize(&dir, &mut journal_progress).is_err() {
            preserve_interrupted_success_finalize(&mut authority_transaction, &mut value);
            trace.finish("degraded=success_finalize_pending");
            return Ok(value);
        }
        Ok(value)
    })();
    match transaction_result {
        Ok(value) => {
            authority_transaction.commit();
            Ok(value)
        }
        Err(failure) => compensate_one_click_failure(
            &app,
            &state,
            lifecycle,
            auth_proof,
            &dir,
            &trace,
            &mut authority_transaction,
            &journal_progress,
            prior_science_for_compensation,
            failure,
            reconcile_disposition,
        ),
    }
}
