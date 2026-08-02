#[cfg(test)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};
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
    current_skill_install_bridge_key, ensure_proxy, skill_install_bridge_dir,
};
use crate::runtime::science::{
    probe_known_runtime, probe_sandbox_runtime_cached, runtime_identity_is_current, sandbox_home,
    sandbox_listener_matches_runtime, sandbox_url, select_science_runtime_cached, stop_sandbox,
    stop_sandbox_with_launch_token, SandboxScienceState, ScienceManagedLaunchToken,
    ScienceRuntimeIdentity, ScienceRuntimeSource,
};
use crate::runtime::skill_install_bridge::{
    inspect_while_science_running, register_before_science_start, RegistrationStatus,
};
use crate::runtime::system::{asset_root, log_path, open_in_browser, open_log, redact, tail_file};
use crate::{
    config, lifecycle, lock, oauth_forge, proc, AppState, HistoryRecoveryChoice,
    HistoryRecoverySession, SharedAppState,
};

// Sibling modules are owned by the sandbox_session facade.
#[cfg(test)]
use super::authority_snapshot::SANDBOX_SESSION_TEST_SEAMS;
use super::catalog_verify::*;
use super::pending_cleanup::{
    cleanup_required_error, retry_pending_authority_cleanup, AuthorityCleanupFailure,
    AuthorityCleanupPhase,
};
use super::recovery::{AppAuthoritySnapshot, OneClickAuthoritySnapshot};
use super::route_reconcile::configure_third_party_best_effort;
use super::ssh_preflight::*;

mod healthy_reopen;

use healthy_reopen::healthy_reopen_with_gateway_rollback;

#[allow(dead_code)]
fn stop_sandbox_state<R: Runtime>(
    app: &tauri::AppHandle<R>,
    st: &mut AppState,
) -> Result<(), String> {
    let runtime = st.science_runtime.clone();
    let result = stop_sandbox(app, &mut st.sandbox, &mut st.sandbox_url, runtime.as_ref());
    if result.is_ok() {
        st.science_confirmed_stopped = runtime;
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
    )
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
) -> Result<Value, ReconcileScienceError> {
    let mut disposition = PriorScienceDisposition::RestartRequired;
    one_click_login_with_options(
        app,
        state,
        lifecycle,
        None,
        auth_proof,
        false,
        Some(&mut disposition),
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
pub(crate) fn force_restart_science_for_active<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
) -> Result<Value, TypedOneClickFailure> {
    let cfg = config::load_from(&config::default_dir())
        .map_err(|error| typed_one_click_err(OneClickFailureKind::ConfigLoad, error.to_string()))?;
    let remembered = { lock(&state).science_runtime.clone() };
    match remembered {
        Some(runtime) => match probe_known_runtime(cfg.sandbox_port, &runtime) {
            SandboxScienceState::RunningHealthy => {
                let mut st = lock(&state);
                st.science_runtime = Some(runtime);
                stop_sandbox_state(&app, &mut st).map_err(|error| {
                    typed_one_click_err(
                        OneClickFailureKind::ScienceStop,
                        format!(
                            "回滚时停止候选 Science 失败，未猜测 PID 或按端口结束进程：{error}"
                        ),
                    )
                })?;
            }
            SandboxScienceState::Stopped => {
                let mut st = lock(&state);
                st.science_confirmed_stopped = Some(runtime);
                st.science_runtime = None;
            }
            SandboxScienceState::Unknown => {
                return Err(typed_one_click_err(
                    OneClickFailureKind::ScienceStop,
                    "回滚时 Science 可能正在运行，但身份无法确认；已拒绝猜测 PID 或按端口结束进程。",
                ));
            }
        },
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
        None => {}
    }
    one_click_login_with_options(app, state, lifecycle, None, auth_proof, false, None)
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

pub(super) fn advance_runtime_transaction(
    dir: &Path,
    active_profile_id: &str,
    previous_binding: Option<config::RuntimeBindingCommit>,
    stage: &str,
) -> Result<(), String> {
    config::update_result(dir, |current| {
        match current.runtime_transaction.as_mut() {
        Some(journal) if journal.is_v2() => Err(
            "typed runtime journal retargeted the V1 writer; preserved the typed transaction and refused the phase advance".into(),
        ),
        Some(journal) if journal.target_profile_id() == active_profile_id => {
            let journal = journal
                .as_v1_mut()
                .expect("V2 is rejected before the V1 phase advance");
            journal.stage = stage.to_string();
            Ok(((), true))
        }
        _ => {
            current.runtime_transaction = Some(
                config::RuntimeTransactionJournal {
                    transaction_id: config::new_id(),
                    target_profile_id: active_profile_id.to_string(),
                    stage: stage.to_string(),
                    previous_binding: previous_binding.clone(),
                    previous_gateway: None,
                }
                .into(),
            );
            Ok(((), true))
        }
    }
    })
}

pub(super) const SCIENCE_ENVIRONMENT_PENDING_STAGE_PREFIX: &str =
    "start_science_environment_pending:";
pub(super) const AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX: &str = "authority_snapshot_active:";

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
    launch_attempted: bool,
    launch_confirmed_stopped: bool,
    candidate_stop_proof: ManagedScienceCandidateStopProof,
    ssh_stub_transaction: Option<crate::runtime::settings::ManagedSshStubTransaction>,
    /// Produce-site failure kind for UI projection; updated at phase boundaries.
    current_kind: OneClickFailureKind,
}

const SCIENCE_LAUNCH_ENVIRONMENT_EXPOSED_EXIT_CODE: i32 = 70;

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

    fn after_exact_cleanup(message: impl Into<String>, cleanup: Result<(), String>) -> Self {
        match cleanup {
            Ok(()) => Self {
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
    fn test_post_spawn_validation(cleanup: Result<(), String>) -> Self {
        let mut failure = Self::after_exact_cleanup(
            "test-only prior Science post-spawn validation failure",
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
    if !crate::runtime::science::managed_launch_token_is_current_for_runtime(token, runtime) {
        return Err("science_db_listener_identity_changed".into());
    }
    let context = runtime
        .skill_install_host_context(port)
        .map_err(|_| "science_api_health_control_context_invalid".to_string())?;
    let session = open_science_health_session_before(&context, deadline)
        .map_err(science_health_control_error)?;
    if !crate::runtime::science::managed_launch_token_is_current_for_runtime(token, runtime) {
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
        if !crate::runtime::science::managed_launch_token_is_current_for_runtime(token, runtime) {
            return Err("science_db_listener_identity_changed".into());
        }
        match authenticated_science_db_health(&session, remaining.min(Duration::from_secs(5))) {
            Ok(state) => {
                if !crate::runtime::science::managed_launch_token_is_current_for_runtime(
                    token, runtime,
                ) {
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
    if !runtime_identity_is_current(&prior.runtime) {
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
    let deadline = Instant::now() + Duration::from_millis(health_budget_ms.max(POLL_INTERVAL_MS));
    let mut launch_cmd = Command::new("zsh");
    launch_cmd
        .arg(&launch)
        .arg("--port")
        .arg(prior.port.to_string())
        .arg("--skip-oauth-forge");
    crate::runtime::launch_env::configure_science_launch_script_command(
        &mut launch_cmd,
        &crate::runtime::launch_env::ScienceLaunchScriptEnv {
            sandbox_home: &sandbox_home(),
            science_bin: Path::new(&prior.runtime.path),
            proxy_url: &proxy_url,
            reuse_system_ssh: cfg.reuse_system_ssh,
            system_ssh_hosts: &ssh_hosts.join(" "),
            opaque_bindings: None,
            runtime_version_prechecked: true,
        },
    );
    let mut launch_child = launch_cmd
        .stdout(Stdio::from(logf))
        .stderr(Stdio::from(logf2))
        .spawn()
        .map_err(|error| {
            ManagedScienceRestartError::before_spawn(format!(
                "恢复 prior Science 启动失败：{error}"
            ))
        })?;
    let status = loop {
        match launch_child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(
                    Duration::from_millis(POLL_INTERVAL_MS)
                        .min(deadline.saturating_duration_since(Instant::now())),
                );
            }
            Ok(None) => {
                let _ = launch_child.kill();
                let _ = launch_child.wait();
                return Err(ManagedScienceRestartError::after_spawn_unproven(
                    "恢复 prior Science 启动脚本超过 absolute deadline",
                ));
            }
            Err(error) => {
                let _ = launch_child.kill();
                let _ = launch_child.wait();
                return Err(ManagedScienceRestartError::after_spawn_unproven(format!(
                    "恢复 prior Science 启动脚本状态未知：{error}"
                )));
            }
        }
    };
    if !status.success() {
        return Err(ManagedScienceRestartError::after_spawn_unproven(format!(
            "恢复 prior Science 启动脚本非零退出（{:?}）",
            status.code()
        )));
    }
    let mut healthy = false;
    while Instant::now() < deadline {
        std::thread::sleep(
            Duration::from_millis(POLL_INTERVAL_MS)
                .min(deadline.saturating_duration_since(Instant::now())),
        );
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let probe_timeout_ms = operation::LOCAL_HEALTH_TIMEOUT_MS.min(
            u64::try_from(remaining.as_millis())
                .unwrap_or(u64::MAX)
                .max(1),
        );
        if proc::http_health(prior.port, None, probe_timeout_ms) {
            healthy = true;
            break;
        }
    }
    if !healthy
        || Instant::now() > deadline
        || !sandbox_listener_matches_runtime(prior.port, &prior.runtime)
    {
        return Err(ManagedScienceRestartError::after_spawn_unproven(
            "恢复 prior Science 后 listener 健康或 runtime 身份不一致",
        ));
    }
    let _candidate_token = crate::runtime::science::uncommitted_managed_science_launch_token(
        prior.port,
        &prior.runtime,
    )
    .ok_or_else(|| {
        ManagedScienceRestartError::after_spawn_unproven(
            "恢复 prior Science 后无法建立精确的未提交启动身份",
        )
    })?;
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
            let cleanup = stop_sandbox_with_launch_token(
                app,
                &mut sandbox,
                &mut url,
                Some(&prior.runtime),
                Some(&_candidate_token),
            );
            return Err(ManagedScienceRestartError::test_post_spawn_validation(
                cleanup,
            ));
        }
    }
    let token =
        match crate::runtime::science::record_managed_science_launch(prior.port, &prior.runtime) {
            Ok(token) => token,
            Err(error) => {
                let mut sandbox = None;
                let mut url = None;
                let token_present = error.token().is_some();
                let cleanup = stop_sandbox_with_launch_token(
                    app,
                    &mut sandbox,
                    &mut url,
                    Some(&prior.runtime),
                    error.token(),
                );
                let message = format!(
                    "恢复 prior Science 时 fresh managed receipt 提交失败：{}",
                    error.message()
                );
                return Err(if token_present {
                    ManagedScienceRestartError::after_exact_cleanup(message, cleanup)
                } else {
                    ManagedScienceRestartError::after_spawn_unproven(message)
                });
            }
        };
    if !crate::runtime::science::managed_launch_token_is_current_for_runtime(&token, &prior.runtime)
    {
        let mut sandbox = None;
        let mut url = None;
        let cleanup = stop_sandbox_with_launch_token(
            app,
            &mut sandbox,
            &mut url,
            Some(&prior.runtime),
            Some(&token),
        );
        return Err(ManagedScienceRestartError::after_exact_cleanup(
            "恢复 prior Science 后 fresh managed receipt 回读不一致",
            cleanup,
        ));
    }
    let url = sandbox_url(prior.port, &prior.runtime);
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
) -> Result<OneClickAuthoritySnapshot, AuthorityCaptureAfterQuiesceError> {
    match OneClickAuthoritySnapshot::capture(config_dir, sandbox_home, auth_dir, config, state) {
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

fn mark_stop_old_science_transaction(
    dir: &Path,
    active_profile_id: &str,
    previous_binding: Option<config::RuntimeBindingCommit>,
) -> Result<(), String> {
    config::update_result(dir, |current| {
        if current
            .runtime_transaction
            .as_ref()
            .is_some_and(config::RuntimeTransactionRecord::is_v2)
        {
            return Err(
                "typed runtime journal retargeted the V1 stop checkpoint; preserved the typed transaction"
                    .into(),
            );
        }
        current.runtime_transaction = Some(
            config::RuntimeTransactionJournal {
                transaction_id: config::new_id(),
                target_profile_id: active_profile_id.to_string(),
                stage: "stop_old_science".into(),
                previous_binding: previous_binding.clone(),
                previous_gateway: None,
            }
            .into(),
        );
        Ok(((), true))
    })
}

pub(super) fn clear_runtime_transaction(dir: &Path) -> Result<(), String> {
    config::update_result(dir, |current| {
        if current
            .runtime_transaction
            .as_ref()
            .is_some_and(config::RuntimeTransactionRecord::is_v2)
        {
            return Err(
                "typed runtime journal retargeted the V1 clearer; preserved the typed transaction"
                    .into(),
            );
        }
        current.runtime_transaction = None;
        Ok(((), true))
    })
}

fn commit_runtime_binding(dir: &Path, binding: config::RuntimeBindingCommit) -> Result<(), String> {
    config::update_result(dir, |current| {
        if current
            .runtime_transaction
            .as_ref()
            .is_some_and(config::RuntimeTransactionRecord::is_v2)
        {
            return Err(
                "typed runtime journal retargeted the V1 binding commit; preserved the typed transaction"
                    .into(),
            );
        }
        current.runtime_binding = Some(binding.clone());
        current.runtime_transaction = None;
        Ok(((), true))
    })
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
    NotExposed,
    CandidateExposed,
    CrossRuntimeExposed,
}

impl CompensationEnvironment {
    fn from_launch(launch_attempted: bool, cross_runtime: bool) -> Self {
        if cross_runtime {
            Self::CrossRuntimeExposed
        } else if launch_attempted {
            Self::CandidateExposed
        } else {
            Self::NotExposed
        }
    }

    fn is_uncertain(self) -> bool {
        self != Self::NotExposed
    }

    fn append_diagnostics(self, diagnostics: &mut Vec<String>) {
        if self.is_uncertain() {
            diagnostics.push("environment_uncertain".into());
        }
        if self == Self::CrossRuntimeExposed {
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

#[allow(clippy::too_many_arguments)]
fn compensate_one_click_failure<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    dir: &Path,
    trace: &OperationTrace,
    authority_snapshot: &mut OneClickAuthoritySnapshot,
    prior_science: Option<&PriorScienceContext>,
    failure: OneClickFailure,
    mut reconcile_disposition: Option<&mut PriorScienceDisposition>,
) -> Result<Value, TypedOneClickFailure> {
    let original_kind = failure.typed.kind();
    let cross_runtime_environment = failure.rollback.launch_attempted
        && prior_science.is_some_and(|prior| prior.runtime != failure.rollback.launch_runtime);
    let environment = CompensationEnvironment::from_launch(
        failure.rollback.launch_attempted,
        cross_runtime_environment,
    );
    let science_cleanup_required =
        failure.rollback.launch_attempted || failure.rollback.launch_token.is_some();
    let cleanup = if failure.rollback.candidate_stop_proof
        == ManagedScienceCandidateStopProof::Unproven
    {
        Err("code=science_candidate_stop_unproven".into())
    } else if !failure.rollback.launch_attempted && failure.rollback.launch_token.is_none() {
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
            .is_some_and(crate::runtime::science::managed_launch_token_process_is_alive)
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
        let result = stop_sandbox_with_launch_token(
            app,
            sandbox,
            sandbox_url,
            Some(&failure.rollback.launch_runtime),
            failure.rollback.launch_token.as_ref(),
        );
        if result.is_ok() {
            current.science_runtime = None;
            current.science_confirmed_stopped = Some(failure.rollback.launch_runtime.clone());
        }
        result
    };
    if let Err(cleanup_error) = cleanup.as_ref() {
        let outcome =
            CompensationOutcome::blocked_by_science_cleanup(cleanup_error.to_string(), environment);
        if outcome.environment.is_uncertain() {
            if let Some(disposition) = reconcile_disposition.as_deref_mut() {
                *disposition = PriorScienceDisposition::EnvironmentUncertain;
            }
        }
        authority_snapshot.preserve_recovery = true;
        trace.finish("error=compensation_restore_blocked_science_cleanup_unproven");
        let cleanup_failure = cleanup_required_error(
            AuthorityCleanupPhase::Cleanup,
            &outcome.render_failure_message(failure.message()),
            &authority_snapshot.backup_root,
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
    let rollback = authority_snapshot.restore_with_gateway(
        app,
        dir,
        state,
        lifecycle,
        auth_proof,
        failure.rollback.proxy_action,
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
        match authority_snapshot.cleanup_when_expendable() {
            Ok(_) => CompensationStepOutcome::Succeeded,
            Err(error) => {
                CompensationStepOutcome::Failed(CompensationCause::SnapshotCleanup(error))
            }
        }
    } else {
        authority_snapshot.preserve_recovery = true;
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

#[allow(clippy::result_large_err)]
fn one_click_login_with_options<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    runtime_choice: Option<&str>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    open_surface: bool,
    mut reconcile_disposition: Option<&mut PriorScienceDisposition>,
) -> Result<Value, TypedOneClickFailure> {
    let trace = OperationTrace::start(OperationKind::OneClickLogin, "command=one_click_login");
    let dir = config::default_dir();
    let cfg = config::load_from(&dir)
        .map_err(|e| typed_one_click_err(OneClickFailureKind::ConfigLoad, e.to_string()))?;
    match cfg
        .runtime_transaction
        .as_ref()
        .is_some_and(config::RuntimeTransactionRecord::is_v2)
    {
        true => Err(TypedOneClickFailure::new(
                OneClickFailureKind::Prepare,
                "检测到 typed runtime journal；当前 R2-A 兼容层拒绝在 writer 迁移前改写或恢复该事务；recovery_status=manual_recovery_required",
            )
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED)),
        false => Ok(()),
    }?;
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
    retry_pending_authority_cleanup(&state).map_err(|failure| {
        typed_authority_cleanup_err(OneClickFailureKind::AuthoritySnapshot, failure, false)
    })?;
    let version_cache = { lock(&state).science_version_cache.clone() };

    let (remembered_runtime, confirmed_stopped) = {
        let st = lock(&state);
        (
            st.science_runtime.clone(),
            st.science_confirmed_stopped.clone(),
        )
    };
    let (science_state, running_runtime) = match remembered_runtime {
        Some(runtime) => {
            let science_state = probe_known_runtime(sport, &runtime);
            let running_runtime =
                (science_state == SandboxScienceState::RunningHealthy).then_some(runtime);
            (science_state, running_runtime)
        }
        None if confirmed_stopped
            .as_ref()
            .is_some_and(|runtime| runtime.source != ScienceRuntimeSource::CachedOnce)
            && !proc::loopback_port_in_use(sport, 100) =>
        {
            (SandboxScienceState::Stopped, None)
        }
        None => probe_sandbox_runtime_cached(sport, &version_cache)
            .map_err(|message| typed_one_click_err(OneClickFailureKind::ScienceStart, message))?,
    };
    let mut running_runtime_to_stop = None;
    let launch_runtime: ScienceRuntimeIdentity = match science_state {
        SandboxScienceState::RunningHealthy => {
            let running_runtime = running_runtime.ok_or_else(|| {
                typed_one_click_err(
                    OneClickFailureKind::ScienceStart,
                    "Science 状态为运行中，但无法确认其 binary 身份",
                )
            })?;
            validate_interrupted_science_environment_runtime(
                interrupted_environment_runtime_id,
                &running_runtime,
            )
            .map_err(|error| typed_interrupted_science_err(OneClickFailureKind::Prepare, error))?;
            let desired_binding = crate::runtime::provider::desired_runtime_binding(
                &cfg,
                active_profile,
                &running_runtime,
            )
            .map_err(|message| typed_one_click_err(OneClickFailureKind::Prepare, message))?;
            let science_binding_matches = !crate::runtime::provider::science_restart_required(
                cfg.runtime_binding.as_ref(),
                &desired_binding,
            );
            let login_intact =
                oauth_forge::login_intact(&auth_dir, "virtual@localhost.invalid", &sbx_home);
            if login_intact && science_binding_matches {
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
                )?;
                if interrupted_environment_runtime_id.is_some() {
                    reopened["recovery_status"] = json!("environment_uncertain");
                    reopened["environment_status"] = json!("uncertain");
                }
                return Ok(reopened);
            }
            let prior_runtime = running_runtime.clone();
            let selected = if login_intact {
                running_runtime
            } else {
                select_science_runtime_cached(runtime_choice, &version_cache).map_err(
                    |message| typed_one_click_err(OneClickFailureKind::ScienceStart, message),
                )?
            };
            running_runtime_to_stop = Some(prior_runtime);
            selected
        }
        SandboxScienceState::Stopped => {
            select_science_runtime_cached(runtime_choice, &version_cache).map_err(|message| {
                typed_one_click_err(OneClickFailureKind::ScienceStart, message)
            })?
        }
        SandboxScienceState::Unknown => {
            trace.finish("error=sandbox_state_unknown_before_start");
            if interrupted_environment_runtime_id.is_some() {
                return Err(TypedOneClickFailure::new(
                    OneClickFailureKind::ScienceStart,
                    "上次启动在 Science 环境暴露边界中断，且当前 listener/runtime 身份无法确认；已拒绝自动恢复；environment_uncertain；recovery_status=manual_recovery_required",
                )
                .with_recovery(ProjectedRecovery::environment_uncertain_manual()));
            }
            return Err(typed_one_click_err(
                OneClickFailureKind::ScienceStart,
                format!(
                    "无法确认隔离 Science 状态（端口 {sport} 或 data-dir 状态不一致）。请先停止占用该端口的进程后重试。"
                ),
            ));
        }
    };
    validate_interrupted_science_environment_runtime(
        interrupted_environment_runtime_id,
        &launch_runtime,
    )
    .map_err(|error| typed_interrupted_science_err(OneClickFailureKind::Prepare, error))?;
    let mut rollback_context = OneClickRollbackContext {
        proxy_action: ProxyAction::Reused,
        sandbox_port: sport,
        launch_runtime: launch_runtime.clone(),
        launch_token: None,
        launch_attempted: false,
        launch_confirmed_stopped: false,
        candidate_stop_proof: ManagedScienceCandidateStopProof::NotRequired,
        ssh_stub_transaction,
        current_kind: OneClickFailureKind::Prepare,
    };
    let prior_science = match running_runtime_to_stop.as_ref() {
        Some(runtime) => Some(PriorScienceContext {
            runtime: runtime.clone(),
            port: sport,
            launch_token: crate::runtime::science::managed_launch_token_for_runtime(sport, runtime)
                .ok_or_else(|| {
                    typed_one_click_err(
                        OneClickFailureKind::ScienceStop,
                        "prior Science managed launch 身份无法确认，拒绝停止或快照",
                    )
                })?,
        }),
        None => None,
    };
    if let Some(prior) = prior_science.as_ref() {
        {
            let mut current = lock(&state);
            let AppState {
                sandbox,
                sandbox_url,
                ..
            } = &mut *current;
            stop_sandbox_with_launch_token(
                &app,
                sandbox,
                sandbox_url,
                Some(&prior.runtime),
                Some(&prior.launch_token),
            )
            .map_err(|message| typed_one_click_err(OneClickFailureKind::ScienceStop, message))?;
            current.science_runtime = None;
            current.science_confirmed_stopped = Some(prior.runtime.clone());
        }
        let receipt = dir.join("science-managed-launch.v1.json");
        if proc::loopback_port_in_use(sport, operation::LOCAL_HEALTH_TIMEOUT_MS)
            || crate::runtime::science::managed_launch_token_process_is_alive(&prior.launch_token)
            || receipt.exists()
        {
            let restart = restart_prior_science(&app, &state, lifecycle, auth_proof, prior);
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
    let mut authority_snapshot = match capture_authority_after_science_quiesce(
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
    let transaction_result = (|| -> Result<Value, OneClickFailure> {
        if running_runtime_to_stop.is_some() {
            rollback_context.set_kind(OneClickFailureKind::ScienceStop);
            one_click_step(
                mark_stop_old_science_transaction(
                    &dir,
                    &active_profile.id,
                    cfg.runtime_binding.clone(),
                ),
                &rollback_context,
            )?;
        }
        rollback_context.set_kind(OneClickFailureKind::Prepare);
        let transaction_cfg = one_click_step(config::load_from(&dir), &rollback_context)?;
        one_click_step(
            advance_runtime_transaction(
                &dir,
                &active_profile.id,
                transaction_cfg.runtime_binding.clone(),
                "start_gateway",
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
        lock(&state).science_confirmed_stopped = None;
        rollback_context.set_kind(OneClickFailureKind::AuthoritySnapshot);
        one_click_step(
            authority_snapshot.validate_science_restore_root(),
            &rollback_context,
        )?;
        let authority_active_stage = format!(
            "{AUTHORITY_SNAPSHOT_ACTIVE_STAGE_PREFIX}{}",
            launch_runtime.environment_transaction_id()
        );
        one_click_step(
            advance_runtime_transaction(
                &dir,
                &active_profile.id,
                transaction_cfg.runtime_binding.clone(),
                &authority_active_stage,
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
                    app_state.science_confirmed_stopped = Some(launch_runtime.clone());
                    app_state.history_recovery = Some(HistoryRecoverySession {
                        active_profile_id: active_profile.id.clone(),
                        sandbox_port: sport,
                        auth_dir: auth_dir.clone(),
                        sandbox_root: sbx_home.clone(),
                        choices,
                    });
                }
                one_click_step(clear_runtime_transaction(&dir), &rollback_context)?;
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
                    authority_snapshot
                        .prepare_success(&mut value)
                        .map_err(String::from),
                    &rollback_context,
                )?;
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
        let (pport, secret, proxy_action) = one_click_step(
            ensure_proxy(
                &app,
                &state,
                lifecycle,
                Some(&launch_runtime),
                Some(&trace),
                auth_proof,
            ),
            &rollback_context,
        )?;
        rollback_context.proxy_action = proxy_action;
        rollback_context.set_kind(OneClickFailureKind::CatalogVerify);
        one_click_step(
            verify_gateway_model_catalog_traced(&trace, pport, &secret, active_profile),
            &rollback_context,
        )?;
        rollback_context.set_kind(OneClickFailureKind::SandboxLaunch);
        let environment_pending_stage = format!(
            "{SCIENCE_ENVIRONMENT_PENDING_STAGE_PREFIX}{}",
            launch_runtime.environment_transaction_id()
        );
        one_click_step(
            advance_runtime_transaction(
                &dir,
                &active_profile.id,
                transaction_cfg.runtime_binding.clone(),
                &environment_pending_stage,
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
        if !runtime_identity_is_current(&launch_runtime) {
            return Err(
                rollback_context.failure("Science runtime 在预检后发生变化；已拒绝启动，请重试")
            );
        }
        one_click_step(
            authority_snapshot.validate_science_restore_root(),
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
            authority_snapshot.validate_science_restore_root(),
            &rollback_context,
        )?;
        let mut launch_cmd = Command::new("zsh");
        launch_cmd
            .arg(&launch)
            .arg("--port")
            .arg(sport.to_string())
            .arg("--skip-oauth-forge");
        let opaque_bindings = authority_snapshot.science_opaque_bindings_env();
        crate::runtime::launch_env::configure_science_launch_script_command(
            &mut launch_cmd,
            &crate::runtime::launch_env::ScienceLaunchScriptEnv {
                sandbox_home: &sandbox_home(),
                science_bin: Path::new(&launch_runtime.path),
                proxy_url: &proxy_url,
                reuse_system_ssh: cfg.reuse_system_ssh,
                system_ssh_hosts: &ssh_hosts.join(" "),
                opaque_bindings: Some(opaque_bindings.as_str()),
                runtime_version_prechecked: true,
            },
        );
        let launch_child = launch_cmd
            .stdout(Stdio::from(logf))
            .stderr(Stdio::from(logf2))
            .spawn();
        let status = match launch_child {
            Ok(mut child) => match child.wait() {
                Ok(status) => status,
                Err(error) => {
                    rollback_context.launch_attempted = true;
                    rollback_context.candidate_stop_proof =
                        ManagedScienceCandidateStopProof::Unproven;
                    return Err(rollback_context.failure(format!(
                        "起沙箱状态未知：{error}；code=science_candidate_stop_unproven"
                    )));
                }
            },
            Err(error) => {
                return Err(rollback_context.failure(format!("起沙箱失败：{error}")));
            }
        };
        rollback_context.launch_attempted = status.success()
            || status.code() == Some(SCIENCE_LAUNCH_ENVIRONMENT_EXPOSED_EXIT_CODE)
            || status.code().is_none();
        if let Some(transaction) = rollback_context.ssh_stub_transaction.as_mut() {
            transaction.observe_after_launch(&sbx_home);
        }
        if !status.success() {
            let tail = redact(&tail_file(&log_path("sandbox.log"), 600), &secret);
            return Err(rollback_context.failure(format!("起沙箱脚本失败。\n{tail}")));
        }
        {
            let mut current = lock(&state);
            current.sandbox_port = sport;
            current.science_runtime = Some(launch_runtime.clone());
            current.science_confirmed_stopped = None;
        }
        let mut healthy = false;
        for _ in 0..(operation::SANDBOX_HEALTH_BUDGET_MS / POLL_INTERVAL_MS) {
            std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            if proc::http_health(sport, None, operation::LOCAL_HEALTH_TIMEOUT_MS) {
                healthy = true;
                break;
            }
        }
        rollback_context.set_kind(OneClickFailureKind::SandboxHealth);
        trace.stage(
            OperationStage::SandboxHealth,
            if healthy { "ready" } else { "not_ready" },
        );
        if !healthy {
            let tail = redact(&tail_file(&log_path("sandbox.log"), 600), &secret);
            return Err(
                rollback_context.failure(format!("沙箱起后探活超时（端口 {sport}）。\n{tail}"))
            );
        }
        if !sandbox_listener_matches_runtime(sport, &launch_runtime) {
            return Err(rollback_context.failure(format!(
                "端口 {sport} 有服务响应，但按 data-dir 确认不是本沙箱 Science（疑似被其它服务占用）。"
            )));
        }
        match crate::runtime::science::record_managed_science_launch(sport, &launch_runtime) {
            Ok(token) => rollback_context.launch_token = Some(token),
            Err(error) => {
                rollback_context.launch_token = error.token().cloned();
                return Err(rollback_context.failure(format!(
                    "Science 已启动但受管启动身份无法安全提交：{}",
                    error.message()
                )));
            }
        }
        rollback_context.set_kind(OneClickFailureKind::ScienceDbReverify);
        one_click_step(
            advance_runtime_transaction(
                &dir,
                &active_profile.id,
                transaction_cfg.runtime_binding.clone(),
                "wait_science_db_reverify",
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
                    one_click_step(
                        stop_sandbox_with_launch_token(
                            &app,
                            sandbox,
                            sandbox_url,
                            Some(&launch_runtime),
                            Some(&first_token),
                        ),
                        &rollback_context,
                    )?;
                    current.science_runtime = None;
                    current.science_confirmed_stopped = Some(launch_runtime.clone());
                }
                rollback_context.launch_confirmed_stopped = true;
                if proc::loopback_port_in_use(sport, operation::LOCAL_HEALTH_TIMEOUT_MS)
                    || crate::runtime::science::managed_launch_token_process_is_alive(&first_token)
                    || dir.join("science-managed-launch.v1.json").exists()
                {
                    return Err(rollback_context
                        .failure("Science DB recovery 的首次受管进程未完成 verified stop"));
                }
                one_click_step(
                    advance_runtime_transaction(
                        &dir,
                        &active_profile.id,
                        transaction_cfg.runtime_binding.clone(),
                        "restart_science_after_db_heal",
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
                    crate::runtime::science::managed_launch_token_for_runtime(
                        sport,
                        &launch_runtime,
                    )
                    .ok_or("Science DB recovery restart 缺少 fresh managed receipt"),
                    &rollback_context,
                )?;
                rollback_context.launch_token = Some(second_token.clone());
                rollback_context.launch_confirmed_stopped = false;
                one_click_step(
                    advance_runtime_transaction(
                        &dir,
                        &active_profile.id,
                        transaction_cfg.runtime_binding.clone(),
                        "verify_science_db_after_restart",
                    ),
                    &rollback_context,
                )?;
                let second_state = one_click_step(
                    wait_for_science_db_reverify(sport, &launch_runtime, &second_token),
                    &rollback_context,
                )?;
                if !crate::runtime::science::managed_launch_token_is_current_for_runtime(
                    &second_token,
                    &launch_runtime,
                ) || second_state != proc::ScienceDbHealth::Ready
                {
                    return Err(rollback_context.failure(format!(
                        "Science DB recovery 的第二次启动未达到 clear/clear：{second_state:?}"
                    )));
                }
            }
        }
        rollback_context.set_kind(OneClickFailureKind::Prepare);
        one_click_step(
            advance_runtime_transaction(
                &dir,
                &active_profile.id,
                transaction_cfg.runtime_binding.clone(),
                "verify_science_catalog",
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
        let url = sandbox_url(sport, &launch_runtime);
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
        one_click_step(commit_runtime_binding(&dir, committed), &rollback_context)?;
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
        one_click_step(
            authority_snapshot
                .prepare_success(&mut value)
                .map_err(String::from),
            &rollback_context,
        )?;
        Ok(value)
    })();
    match transaction_result {
        Ok(value) => {
            authority_snapshot.commit();
            Ok(value)
        }
        Err(failure) => compensate_one_click_failure(
            &app,
            &state,
            lifecycle,
            auth_proof,
            &dir,
            &trace,
            &mut authority_snapshot,
            prior_science_for_compensation,
            failure,
            reconcile_disposition,
        ),
    }
}
