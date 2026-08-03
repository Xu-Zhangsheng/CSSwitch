use super::*;

pub(super) async fn one_click_login_command<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    runtime_choice: Option<String>,
) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking_typed(move || one_click_login_cmd(app, state, lifecycle, runtime_choice)).await
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

pub(super) fn typed_interrupted_gateway_recovery_error(
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
        Ok(resolve_launch_plan(&self.launch_context.profile)?.adapter == "codex")
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

#[derive(Clone)]
struct OneClickCandidateConfigSnapshot {
    config: config::Config,
}

impl OneClickCandidateConfigSnapshot {
    fn capture(config: &config::Config, adapter: &str) -> Option<Self> {
        (adapter != "codex").then(|| Self {
            config: config.clone(),
        })
    }

    fn verify_unchanged(&self) -> Result<(), String> {
        let current = config::load_from(&config::default_dir())
            .map_err(|_| "config_changed_retry：无法复核候选启动配置，请重试。".to_string())?;
        if current == self.config {
            Ok(())
        } else {
            Err("config_changed_retry：候选启动配置在认证检查期间发生变化，请重试。".into())
        }
    }
}

pub(crate) fn one_click_login_cmd<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
    runtime_choice: Option<String>,
) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError> {
    let cfg = match config::load_from(&config::default_dir()) {
        Ok(cfg) => cfg,
        Err(error) => {
            return Ok(project_one_click_failure(TypedOneClickFailure::new(
                OneClickFailureKind::ConfigLoad,
                error.to_string(),
            )))
        }
    };
    let active = match cfg.active_profile() {
        Some(active) => active,
        None => {
            return Ok(project_one_click_failure(TypedOneClickFailure::new(
                OneClickFailureKind::NoActiveProfile,
                "未配置生效 profile，请先在面板选择或新建一条配置。",
            )))
        }
    };
    let adapter = match resolve_launch_plan(active) {
        Ok(plan) => plan.adapter,
        Err(message) => {
            return Ok(project_one_click_failure(TypedOneClickFailure::new(
                OneClickFailureKind::LaunchPlan,
                message,
            )))
        }
    };
    let candidate_config = OneClickCandidateConfigSnapshot::capture(&cfg, &adapter);
    let prior_gateway = match OneClickGatewayPreflightSnapshot::capture(&state) {
        Ok(snapshot) => snapshot,
        Err(message) => {
            return Ok(project_one_click_failure(TypedOneClickFailure::new(
                OneClickFailureKind::PreflightSnapshot,
                message,
            )))
        }
    };
    let needs_codex_proof = if adapter == "codex" {
        true
    } else {
        match prior_gateway
            .as_ref()
            .map(OneClickGatewayPreflightSnapshot::needs_codex_proof)
            .transpose()
        {
            Ok(Some(required)) => required,
            Ok(None) => false,
            Err(message) => {
                return Ok(project_one_click_failure(TypedOneClickFailure::new(
                    OneClickFailureKind::PreflightSnapshot,
                    message,
                )))
            }
        }
    };
    let preflight_adapter = if needs_codex_proof {
        "codex"
    } else {
        adapter.as_str()
    };
    let prepared = match crate::commands::codex::prepare_provider_auth(
        &app,
        preflight_adapter,
        crate::commands::codex::CodexPreflightTarget::ActiveProfile,
    ) {
        Ok(prepared) => prepared,
        Err(crate::commands::codex::RuntimeCommandError::Message(message)) => {
            return Ok(project_one_click_failure(TypedOneClickFailure::new(
                OneClickFailureKind::AuthPreflight,
                message,
            )))
        }
        Err(auth @ crate::commands::codex::RuntimeCommandError::Auth(_)) => return Err(auth),
    };
    match lifecycle.with_mutation(
        RuntimeMutationDomain::Destructive,
        |_| -> Result<_, TypedOneClickFailure> {
            if let Some(candidate_config) = candidate_config.as_ref() {
                candidate_config.verify_unchanged().map_err(|message| {
                    TypedOneClickFailure::new(OneClickFailureKind::PreflightSnapshot, message)
                })?;
            }
            if let Some(prepared) = prepared.as_ref() {
                prepared.verify_unchanged().map_err(|message| {
                    TypedOneClickFailure::new(OneClickFailureKind::AuthPreflight, message)
                })?;
            }
            if let Some(prior_gateway) = prior_gateway.as_ref() {
                prior_gateway.verify_unchanged(&state).map_err(|message| {
                    TypedOneClickFailure::new(OneClickFailureKind::PreflightSnapshot, message)
                })?;
            }
            crate::runtime::proxy_lifecycle::recover_interrupted_gateway(&app, &state)
                .map_err(typed_interrupted_gateway_recovery_error)?;
            crate::runtime::sandbox_session::one_click_login(
                app,
                state,
                lifecycle.as_ref(),
                runtime_choice.as_deref(),
                prepared.as_ref().map(|prepared| prepared.proof()),
            )
        },
    ) {
        Ok(value) => Ok(value),
        Err(failure) => Ok(project_one_click_failure(failure)),
    }
}

pub(super) async fn restore_history_choice_command<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    reference: String,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || {
        lifecycle.with_mutation(RuntimeMutationDomain::Destructive, |_| {
            let cfg = config::load_from(&config::default_dir()).map_err(|e| e.to_string())?;
            if cfg.mode != "proxy" {
                return Err("当前已不是第三方模型模式，本次历史恢复选择已作废".into());
            }
            if cfg.runtime_transaction.is_some() {
                return Err("当前有新的运行事务尚未完成，已拒绝覆盖其历史身份".into());
            }
            let active_profile_id = cfg
                .active_profile()
                .map(|profile| profile.id.clone())
                .ok_or("当前选择已变化，本次历史恢复选择已作废")?;
            let (auth_dir, sandbox_root, candidate, expected_port, science_quiescence) = {
                let app_state = lock(&state);
                let session = app_state
                    .history_recovery
                    .as_ref()
                    .ok_or("历史恢复选择已过期，请重新点击一键开始")?;
                if session.active_profile_id != active_profile_id
                    || session.sandbox_port != cfg.sandbox_port
                {
                    return Err("当前配置或端口已变化，本次历史恢复选择已作废".into());
                }
                let choice = session
                    .choices
                    .iter()
                    .find(|choice| choice.reference == reference)
                    .ok_or("历史恢复引用无效或已过期")?;
                (
                    session.auth_dir.clone(),
                    session.sandbox_root.clone(),
                    choice.candidate.clone(),
                    session.sandbox_port,
                    session.science_quiescence.clone(),
                )
            };

            // A user may discover after opening Science that A/B was the wrong
            // history. Keep the one-shot mapping in memory for this app session,
            // but stop only the exact managed runtime before changing credentials.
            {
                let mut app_state = lock(&state);
                if let Some(runtime) = app_state.science_runtime.clone() {
                    let receipt = ScienceHostAdapter::managed_receipt(expected_port, &runtime)
                        .ok_or("历史恢复前无法取得 Science 的精确受管启动身份")?;
                    let AppState {
                        sandbox,
                        sandbox_url,
                        ..
                    } = &mut *app_state;
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
                    .map_err(|error| error.to_string())?;
                    app_state.science_confirmed_stopped = verified.confirmed_runtime().cloned();
                    app_state.science_runtime = None;
                    let session = app_state
                        .history_recovery
                        .as_mut()
                        .ok_or("历史恢复会话已过期")?;
                    session.science_quiescence =
                        crate::HistoryRecoveryScienceQuiescence::ExactStopped(runtime);
                } else {
                    let version_cache = app_state.science_version_cache.clone();
                    let (current_science_state, current_runtime) =
                        ScienceHostAdapter::probe_cached(expected_port, &version_cache)
                            .map_err(|error| error.to_string())?;
                    if current_science_state != SandboxScienceState::Stopped
                        || current_runtime.is_some()
                    {
                        return Err(
                            "历史恢复前 Science typed quiescence 复核失败；已拒绝改写历史身份"
                                .into(),
                        );
                    }
                    match science_quiescence {
                        crate::HistoryRecoveryScienceQuiescence::ExactStopped(expected) => {
                            if app_state.science_confirmed_stopped.as_ref() != Some(&expected) {
                                return Err(
                                    "历史恢复的 verified-stopped receipt 已变化，本次选择已作废"
                                        .into(),
                                );
                            }
                        }
                        crate::HistoryRecoveryScienceQuiescence::NoManagedRuntimeObserved => {
                            if app_state.science_confirmed_stopped.is_some() {
                                return Err(
                                    "历史恢复的 Science quiescence 状态已变化，本次选择已作废"
                                        .into(),
                                );
                            }
                        }
                    }
                }
            }
            #[cfg(test)]
            apply_history_restore_post_stop_config_drift(expected_port)?;
            let current_cfg =
                config::load_from(&config::default_dir()).map_err(|e| e.to_string())?;
            if current_cfg.mode != "proxy"
                || current_cfg.sandbox_port != expected_port
                || current_cfg.runtime_transaction.is_some()
                || current_cfg
                    .active_profile()
                    .map(|profile| profile.id.as_str())
                    != Some(active_profile_id.as_str())
            {
                return Err("运行配置或事务在恢复前已变化，本次选择已作废".into());
            }
            let _ = crate::oauth_forge::restore_history_choice(
                &auth_dir,
                "virtual@localhost.invalid",
                &sandbox_root,
                &candidate,
            )?;
            // Consume every old reference after a successful selection. Fresh
            // references preserve the in-session "choose again" escape hatch
            // without making an invoke token replayable.
            let refreshed_choices = {
                let mut app_state = lock(&state);
                app_state.boot_attention = None;
                let session = app_state
                    .history_recovery
                    .as_mut()
                    .ok_or("历史恢复会话已过期")?;
                session
                    .choices
                    .iter_mut()
                    .enumerate()
                    .map(|(index, choice)| {
                        choice.reference = config::new_id();
                        let label = if index < 26 {
                            format!("历史记录 {}", (b'A' + index as u8) as char)
                        } else {
                            format!("历史记录 {}", index + 1)
                        };
                        json!({"reference": choice.reference, "label": label})
                    })
                    .collect::<Vec<_>>()
            };
            Ok(json!({
                "status": "ok",
                "action": "history_choice_restored",
                "message": "已恢复所选历史记录；其他历史记录未被删除。",
                "choices": refreshed_choices
            }))
        })
    })
    .await
}

#[cfg(test)]
static HISTORY_RESTORE_POST_STOP_CONFIG_DRIFT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(super) struct HistoryRestorePostStopConfigDriftGuard;

#[cfg(test)]
impl Drop for HistoryRestorePostStopConfigDriftGuard {
    fn drop(&mut self) {
        HISTORY_RESTORE_POST_STOP_CONFIG_DRIFT.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
pub(super) fn test_arm_history_restore_post_stop_config_drift(
) -> HistoryRestorePostStopConfigDriftGuard {
    HISTORY_RESTORE_POST_STOP_CONFIG_DRIFT.store(true, std::sync::atomic::Ordering::SeqCst);
    HistoryRestorePostStopConfigDriftGuard
}

#[cfg(test)]
fn apply_history_restore_post_stop_config_drift(expected_port: u16) -> Result<(), String> {
    if !HISTORY_RESTORE_POST_STOP_CONFIG_DRIFT.swap(false, std::sync::atomic::Ordering::SeqCst) {
        return Ok(());
    }
    config::update(&config::default_dir(), |current| {
        current.sandbox_port = expected_port
            .checked_add(1)
            .filter(|port| *port != 8765)
            .unwrap_or(expected_port.saturating_sub(1));
    })
    .map(|_| ())
    .map_err(|error| error.to_string())
}

pub(super) fn project_one_click_failure(failure: TypedOneClickFailure) -> serde_json::Value {
    let journal_open = config::load_from(&config::default_dir())
        .ok()
        .and_then(|cfg| cfg.runtime_transaction)
        .is_some();
    failure
        .apply_open_journal_degraded(journal_open)
        .project_dto()
}
