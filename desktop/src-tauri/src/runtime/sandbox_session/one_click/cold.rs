use super::*;

mod compensation;
mod science_phase;

pub(super) use compensation::compensate_one_click_failure;
use science_phase::run_managed_science_launch_phase;

#[cfg(test)]
pub(in super::super) use compensation::{
    CompensationCause, CompensationEnvironment, CompensationOutcome, CompensationSkipCause,
    CompensationStepOutcome,
};

#[allow(clippy::result_large_err)]
fn stop_prior_science_with<Claim, Execute>(
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    prior_runtime: &ScienceRuntimeIdentity,
    prior_port: u16,
    claim: Claim,
    execute: Execute,
) -> crate::runtime::science::ScienceStopOutcome
where
    Claim: FnOnce() -> ScienceStopRequest,
    Execute: FnOnce(ScienceStopRequest) -> (crate::runtime::science::ScienceStopOutcome, bool),
{
    execute_transaction_science_stop_with(
        state,
        lifecycle,
        TransactionScienceStopBoundary::ColdPriorStop,
        prior_runtime,
        prior_port,
        || Ok(claim()),
        execute,
        |_state, _confirmed_runtime| {},
    )
}

#[allow(clippy::result_large_err, clippy::too_many_arguments)]
pub(super) fn run_cold_one_click<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    open_surface: bool,
    mut reconcile_disposition: Option<&mut PriorScienceDisposition>,
    trace: OperationTrace,
    dir: std::path::PathBuf,
    cfg: &config::Config,
    active_profile: &config::Profile,
    sbx_home: std::path::PathBuf,
    auth_dir: std::path::PathBuf,
    sport: u16,
    launch_runtime: ScienceRuntimeIdentity,
    running_runtime_to_stop: Option<ScienceRuntimeIdentity>,
    science_state: SandboxScienceState,
    remembered_runtime_was_present: bool,
    profile_switch_handoff: Option<config::RuntimeTransactionV2>,
    gateway_terminal_handoff: Option<config::RuntimeTransactionV2>,
    interrupted_environment_runtime_id: Option<&str>,
) -> Result<Value, TypedOneClickFailure> {
    let ssh_prevalidation =
        crate::runtime::sandbox_session::prevalidate_one_click_system_ssh(&app, cfg, &sbx_home)
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
        let stop_result = stop_prior_science_with(
            &state,
            lifecycle,
            &prior.runtime,
            prior.port,
            || {
                ScienceStopRequest::exact(
                    &prior.runtime,
                    ScienceStopOwnershipReceipt::from_managed_launch(&prior.launch_token),
                )
            },
            |request| {
                let (outcome, clear_tracking) =
                    ScienceHostAdapter::execute_stop(&app, request).into_parts();
                (
                    outcome.and_then(|verified| verified.require_exact_stop_of(&prior.runtime)),
                    clear_tracking,
                )
            },
        );
        let durable_outcome = match &stop_result {
            Ok(_) => config::RuntimePriorStopOutcome::ExactStopped,
            Err(error) if error.confirmed_runtime() == Some(&prior.runtime) => {
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
        cfg,
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
            runtime_transaction: Box::new(cfg.runtime_transaction.clone()),
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
        let installer = run_managed_science_launch_phase(
            &app,
            &state,
            lifecycle,
            auth_proof,
            &trace,
            &dir,
            cfg,
            &auth_dir,
            &sbx_home,
            &launch,
            &launch_runtime,
            &login_action,
            sport,
            pport,
            &secret,
            ssh_hosts,
            &mut authority_transaction,
            &transaction_identity,
            &mut journal_progress,
            &mut rollback_context,
        )?;
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
                config::RuntimeFinalizeAction::CommitBinding {
                    binding: committed,
                    science_adoption_attempt_id: launch_runtime
                        .adoption_attempt_id()
                        .map(str::to_string),
                },
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
        match mark_science_runtime_adoption_finalized(&launch_runtime) {
            Ok(()) => {
                value["adoption_record_status"] = json!("finalized");
            }
            Err(_) => {
                value["status"] = json!("degraded");
                value["adoption_record_status"] = json!("launch_committed");
                value["adoption_record_message"] = json!(
                    "Science 已启动且 binding 已提交，但 adoption finalize milestone 尚未收敛；下次 runtime preflight 会重试。"
                );
                trace.finish("degraded=science_adoption_finalize_pending");
            }
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
            &transaction_identity,
            &mut journal_progress,
            prior_science_for_compensation,
            failure,
            reconcile_disposition,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppState;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::path::PathBuf::from("/private/tmp").join(format!(
                "csswitch-cold-prior-stop-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn cold_prior_science_stop_wait_releases_read_model_and_stale_result_preserves_replacement() {
        let temp = TestDir::new("owner-cas");
        let prior_binary = temp.0.join("prior-science");
        let replacement_binary = temp.0.join("replacement-science");
        fs::write(&prior_binary, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::write(&replacement_binary, b"#!/bin/sh\nexit 0\n# replacement\n").unwrap();
        fs::set_permissions(&prior_binary, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&replacement_binary, fs::Permissions::from_mode(0o700)).unwrap();
        let prior =
            crate::runtime::science::test_runtime_identity(prior_binary.canonicalize().unwrap());
        let replacement = crate::runtime::science::test_runtime_identity(
            replacement_binary.canonicalize().unwrap(),
        );

        for (case, replace_identity, bump_generation) in [
            ("generation-only", false, true),
            ("identity-only", true, false),
        ] {
            let state: SharedAppState = Arc::new(Mutex::new(AppState::default()));
            {
                let mut current = lock(&state);
                current.science_runtime = Some(prior.clone());
                current.sandbox_port = 18765;
                current.sandbox_url = Some("http://127.0.0.1:18765/prior".into());
            }
            let lifecycle = Arc::new(lifecycle::Lifecycle::new());
            let (stop_started_tx, stop_started_rx) = std::sync::mpsc::channel();
            let (release_stop_tx, release_stop_rx) = std::sync::mpsc::channel();
            let worker_state = state.clone();
            let worker_lifecycle = lifecycle.clone();
            let claim_prior = prior.clone();
            let verified_prior = prior.clone();
            let worker = std::thread::spawn(move || {
                stop_prior_science_with(
                    &worker_state,
                    worker_lifecycle.as_ref(),
                    &claim_prior,
                    18765,
                    || ScienceStopRequest::recover(Some(&claim_prior)),
                    move |_| {
                        stop_started_tx.send(()).unwrap();
                        release_stop_rx.recv().unwrap();
                        (
                            Ok(crate::runtime::science::VerifiedScienceStop {
                                runtime: Some(verified_prior),
                                ownership_was_proven: true,
                            }),
                            true,
                        )
                    },
                )
            });

            stop_started_rx.recv().unwrap();
            {
                let mut read_model = state
                    .try_lock()
                    .expect("cold prior Science stop wait must not retain AppState");
                assert_eq!(read_model.science_runtime.as_ref(), Some(&prior), "{case}");
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
                outcome
                    .as_ref()
                    .is_err_and(|error| error.to_string().contains("process-local owner 已变化")),
                "{case}: {outcome:?}"
            );
            let current = lock(&state);
            assert_eq!(
                current.science_runtime.as_ref(),
                Some(if replace_identity {
                    &replacement
                } else {
                    &prior
                }),
                "{case}"
            );
            assert_eq!(
                current.sandbox_url.as_deref(),
                Some(if replace_identity {
                    "http://127.0.0.1:18765/replacement"
                } else {
                    "http://127.0.0.1:18765/prior"
                }),
                "{case}"
            );
        }
    }
}
