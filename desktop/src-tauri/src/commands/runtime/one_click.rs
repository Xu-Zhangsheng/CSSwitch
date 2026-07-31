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
    launch_context: crate::GatewayLaunchContext,
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
    match lifecycle.with_serialized(|| -> Result<_, TypedOneClickFailure> {
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
        crate::runtime::proxy_lifecycle::recover_interrupted_gateway(&app, &state).map_err(
            |message| {
                // recover_interrupted_gateway still returns String; map known
                // Science journal-preservation refuses away from gateway_start.
                let kind = if message.contains("Science authority/environment")
                    || message.contains("authority 快照")
                    || message.contains("Science 环境暴露")
                {
                    OneClickFailureKind::AuthoritySnapshot
                } else {
                    OneClickFailureKind::GatewayStart
                };
                TypedOneClickFailure::new(kind, message)
            },
        )?;
        crate::runtime::sandbox_session::one_click_login(
            app,
            state,
            lifecycle.as_ref(),
            runtime_choice.as_deref(),
            prepared.as_ref().map(|prepared| prepared.proof()),
        )
    }) {
        Ok(value) => Ok(value),
        Err(failure) => Ok(project_one_click_failure(failure)),
    }
}

pub(super) async fn restore_history_choice_command(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    reference: String,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || {
        lifecycle.with_serialized(|| {
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
            let (auth_dir, sandbox_root, candidate, expected_port) = {
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
                )
            };

            // A user may discover after opening Science that A/B was the wrong
            // history. Keep the one-shot mapping in memory for this app session,
            // but stop only the exact managed runtime before changing credentials.
            {
                let mut app_state = lock(&state);
                if app_state.science_runtime.is_some() {
                    stop_sandbox_state(&app, &mut app_state)?;
                } else if proc::loopback_port_in_use(
                    expected_port,
                    operation::LOCAL_HEALTH_TIMEOUT_MS,
                ) {
                    return Err("Science 端口被未知进程占用，已拒绝改写历史身份".into());
                }
            }
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

pub(super) fn project_one_click_failure(failure: TypedOneClickFailure) -> serde_json::Value {
    let journal_open = config::load_from(&config::default_dir())
        .ok()
        .and_then(|cfg| cfg.runtime_transaction)
        .is_some();
    let failure = if failure.recovery == ProjectedRecovery::NOT_NEEDED {
        if let Some(recovery) = recovery_from_diagnostic_codes(&failure.message) {
            failure.with_recovery(recovery)
        } else {
            failure.apply_open_journal_degraded(journal_open)
        }
    } else {
        failure
    };
    failure.project_dto()
}
