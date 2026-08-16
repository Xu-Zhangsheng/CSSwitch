use super::*;

/// Rebind the Gateway around an already healthy managed Science runtime.
///
/// This path owns its own config/Gateway compensation because it does not
/// mutate the authority snapshot or restart Science.
#[allow(clippy::too_many_arguments)]
pub(super) fn healthy_reopen_with_gateway_rollback<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    trace: &OperationTrace,
    dir: &Path,
    cfg: &config::Config,
    active_profile: &config::Profile,
    auth_dir: &Path,
    sport: u16,
    running_runtime: &ScienceRuntimeIdentity,
    open_surface: bool,
    expected_gateway_intent: &config::RuntimeTransactionV2,
) -> Result<Value, TypedOneClickFailure> {
    let app_snapshot = AppAuthoritySnapshot::capture(state);
    let prior_config = cfg.clone();
    let attempt = (|| -> Result<Value, TypedOneClickFailure> {
        let _ = reconcile_current_science_runtime_adoption(
            running_runtime,
            cfg.runtime_binding.as_ref(),
        );
        if cfg.reuse_system_ssh {
            validate_running_system_ssh_bridge(app, &sandbox_home())
                .map_err(|message| typed_one_click_err(OneClickFailureKind::Prepare, message))?;
        }
        oauth_forge::bootstrap_marker_for_intact_login(
            auth_dir,
            "virtual@localhost.invalid",
            &sandbox_home(),
        )
        .map_err(|error| {
            typed_one_click_err(
                OneClickFailureKind::SandboxLogin,
                format!("补齐历史恢复标记失败：{error}"),
            )
        })?;
        let gateway = GatewayController::ensure_active(
            app,
            state,
            lifecycle,
            Some(running_runtime),
            Some(trace),
            auth_proof,
        )
        .map_err(|message| typed_one_click_err(OneClickFailureKind::GatewayStart, message))?;
        let secret = gateway.route_secret;
        let proxy_action = gateway.action;
        verify_gateway_model_catalog_traced(trace, cfg.proxy_port, &secret, active_profile)
            .map_err(|message| typed_one_click_err(OneClickFailureKind::CatalogVerify, message))?;
        let installer_bridge = skill_install_bridge_dir(&secret)
            .map_err(|message| typed_one_click_err(OneClickFailureKind::Prepare, message))?;
        let refreshed_cfg = config::load_from(dir).map_err(|error| {
            typed_one_click_err(OneClickFailureKind::ConfigLoad, error.to_string())
        })?;
        if refreshed_cfg.runtime_transaction.as_ref()
            != Some(&config::RuntimeTransactionRecord::V2(
                expected_gateway_intent.clone(),
            ))
            || refreshed_cfg.active_id != expected_gateway_intent.target_profile_id
            || refreshed_cfg.runtime_binding.as_ref()
                != expected_gateway_intent.previous_binding.as_ref()
        {
            return Err(TypedOneClickFailure::new(
                OneClickFailureKind::Prepare,
                "runtime journal retargeted healthy reopen; preserved the current transaction and refused the binding commit",
            )
            .with_recovery(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED));
        }
        let committed = crate::runtime::provider::desired_runtime_binding(
            &refreshed_cfg,
            refreshed_cfg.active_profile().ok_or_else(|| {
                typed_one_click_err(
                    OneClickFailureKind::NoActiveProfile,
                    "生效 profile 在启动期间消失",
                )
            })?,
            running_runtime,
        )
        .map_err(|message| typed_one_click_err(OneClickFailureKind::Prepare, message))?;
        commit_healthy_reopen_binding(dir, expected_gateway_intent, &committed).map_err(
            |error| typed_one_click_err(OneClickFailureKind::Prepare, error.to_string()),
        )?;
        let installer = match current_skill_install_bridge_key() {
            Ok(installer_key) => {
                inspect_while_science_running(app, auth_dir, &installer_bridge, &installer_key)
            }
            Err(error) => RegistrationStatus::Warning(error),
        };
        let installer = configure_third_party_best_effort(
            app,
            installer,
            auth_dir,
            sport,
            running_runtime,
            false,
        );
        let url = ScienceHostAdapter::url(sport, running_runtime);
        {
            let mut current = lock(state);
            current.sandbox_port = sport;
            current.sandbox_url = Some(url.clone());
            current.science_runtime = Some(running_runtime.clone());
            current.science_confirmed_stopped = None;
        }
        let base = match proxy_action {
            ProxyAction::Reused => "已在运行",
            ProxyAction::Restarted => "已用新配置重启代理，Science 沿用不变",
        };
        let (message, fallback_url) = if open_surface {
            match open_science_surface(app, &url) {
                Ok("webview") => (format!("{base}，已重新打开 Science 窗口。"), None),
                Ok(_) => (format!("{base}，已向系统浏览器发送打开请求。"), None),
                Err(_) => (
                    format!("{base}，服务已就绪；自动打开失败。"),
                    Some(url.clone()),
                ),
            }
        } else {
            (format!("{base}，Science 绑定保持不变。"), None)
        };
        let message = append_installer_note(message, &installer);
        trace.finish(format!(
            "ok action=reopened proxy_action={}",
            proxy_action.as_str()
        ));
        Ok(json!({
            "msg": message,
            "action": "reopened",
            "stage": "complete",
            "status": "ok",
            "recovery_status": "not_needed",
            "fallback_url": fallback_url,
            "external_skill_installer": installer_status_json(&installer)
        }))
    })();
    match attempt {
        Ok(value) => Ok(value),
        Err(primary) => {
            let mut recovery_errors = Vec::new();
            let rollback_admission =
                admit_healthy_reopen_gateway_rollback(dir, expected_gateway_intent);
            if let Err(error) = rollback_admission.as_ref() {
                recovery_errors.push(format!("config_admission={error}"));
            }
            if let Ok(expected_during_rollback) = rollback_admission {
                let gateway_restore = app_snapshot.restore_with_gateway(
                    app,
                    state,
                    lifecycle,
                    auth_proof,
                    ProxyAction::Restarted,
                );
                if let Err(error) = gateway_restore.as_ref() {
                    recovery_errors.push(format!("gateway={error}"));
                }
                if gateway_restore.is_ok() {
                    if let Err(error) = complete_healthy_reopen_gateway_rollback(
                        dir,
                        &expected_during_rollback,
                        &prior_config,
                    ) {
                        recovery_errors.push(format!("config_complete={error}"));
                    }
                } else {
                    recovery_errors.push("config=retained_gateway_intent".into());
                }
            } else {
                recovery_errors.push("gateway=skipped_after_transaction_retarget".into());
            }
            if recovery_errors.is_empty() {
                Err(primary)
            } else {
                Err(TypedOneClickFailure::new(
                    primary.kind(),
                    format!(
                        "{}；healthy_reopen_recovery={}",
                        primary.safe_detail,
                        recovery_errors.join("; ")
                    ),
                )
                .with_recovery(primary.projected_recovery()))
            }
        }
    }
}
