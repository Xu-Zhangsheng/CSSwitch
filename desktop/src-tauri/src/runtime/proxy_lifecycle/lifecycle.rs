/// Ensure the active profile's proxy is running and healthy.
pub(crate) fn ensure_proxy<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    science_runtime: Option<&crate::runtime::science::ScienceRuntimeIdentity>,
    trace: Option<&OperationTrace>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
) -> Result<(u16, String, ProxyAction), String> {
    let cfg = config::load_from(&config::default_dir()).map_err(|e| e.to_string())?;
    let profile = cfg
        .active_profile()
        .cloned()
        .ok_or("未配置生效 profile，请先在面板选择或新建一条配置。")?;
    start_proxy_for(
        app,
        state,
        lifecycle,
        &profile,
        science_runtime,
        trace,
        auth_proof,
    )
}

/// Start or reuse a proxy for a specific profile, without reading the active profile.
///
/// This function does not take the command serializer lock; callers own that boundary.
pub(crate) fn start_proxy_for<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    profile: &config::Profile,
    science_runtime: Option<&crate::runtime::science::ScienceRuntimeIdentity>,
    trace: Option<&OperationTrace>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
) -> Result<(u16, String, ProxyAction), String> {
    assert_format_supported(profile)?;
    let resolved = proxy_args_for(profile)?;
    let mut launch = resolved.formal();
    let dir = config::default_dir();
    let cfg = config::load_from(&dir).map_err(|e| e.to_string())?;
    config::require_template_enabled(&cfg, &profile.template_id)?;
    if launch.adapter == "codex" {
        launch.codex_network_route = Some(
            csswitch_codex_network::resolve_from_process(&cfg.codex_network)
                .map_err(|_| "proxy_config_invalid：Codex 网络代理配置非法。".to_string())?,
        );
    }
    crate::commands::codex::require_provider_auth_proof(&launch.adapter, auth_proof)?;
    if !launch.credential_configured() {
        return Err(format!(
            "「{}」还没配置凭据，请先在面板填写或登录。",
            profile.name
        ));
    }
    if launch.endpoint_policy == crate::provider_contracts::EndpointPolicy::ProfileRequired
        && launch.endpoint.is_empty()
    {
        return Err(
            "该配置需要填 base_url（如 https://your-relay/claude），请先在面板填写并保存。".into(),
        );
    }

    let shim_mode = current_shim_mode_for_adapter(&launch.adapter);
    let gateway_kind = "rust";
    let port = cfg.proxy_port;
    let science_context = match science_runtime {
        Some(runtime) => Some(runtime.skill_install_host_context(cfg.sandbox_port)?),
        None => {
            let remembered = {
                let st = lock(state);
                st.science_runtime.clone().map(|runtime| {
                    let port = if st.sandbox_port == 0 {
                        cfg.sandbox_port
                    } else {
                        st.sandbox_port
                    };
                    (runtime, port)
                })
            };
            remembered.and_then(|(runtime, sandbox_port)| {
                (sandbox_port == cfg.sandbox_port
                    && crate::runtime::science::ScienceHostAdapter::probe_known(
                        sandbox_port,
                        &runtime,
                    ) == crate::runtime::science::SandboxScienceState::RunningHealthy)
                    .then(|| runtime.skill_install_host_context(sandbox_port).ok())
                    .flatten()
            })
        }
    };
    let key_fp = proxy_fingerprint_with_science_context(
        proxy_fingerprint_with_runtime(profile, &launch, gateway_kind, shim_mode),
        science_context.as_ref(),
    );
    let expected_catalog_fp = launch
        .static_model_catalog
        .as_deref()
        .and_then(|payload| serde_json::from_str::<serde_json::Value>(payload).ok())
        .and_then(|value| {
            value
                .get("catalog_fp")
                .and_then(|fp| fp.as_str())
                .map(str::to_string)
        });

    let secret = if !cfg.secret.is_empty() {
        cfg.secret.clone()
    } else {
        let s = proc::gen_secret().map_err(|e| format!("无法生成安全 secret：{e}"))?;
        let s2 = s.clone();
        config::update(&dir, move |c| c.secret = s2).map_err(|e| e.to_string())?;
        s
    };

    let gen = lifecycle.current_generation();

    let (mut child, launch_id) = {
        let mut st = lock(state);
        let tracked_child_running = proc::tracked_child_is_running(&mut st.proxy);
        if tracked_child_running
            && st.proxy_port == port
            && st.provider == launch.adapter
            && st.gateway_kind == gateway_kind
            && st.shim_mode == shim_mode
            && st.key_fp == key_fp
            && proc::http_health_gateway(
                port,
                Some(&st.secret),
                operation::PROXY_REUSE_HEALTH_TIMEOUT_MS,
                proc::GatewayHealthExpectation {
                    gateway: gateway_kind,
                    provider: Some(&launch.adapter),
                    shim: Some(st.shim_mode.as_str()),
                    launch_id: Some(st.launch_id.as_str()),
                    provider_contract_id: Some(&launch.contract_id),
                    provider_contract_digest: Some(&launch.contract_digest),
                },
            )
            && proc::http_gateway_health(
                port,
                Some(&st.secret),
                operation::PROXY_REUSE_HEALTH_TIMEOUT_MS,
            )
            .is_some_and(|health| {
                health.intent == "formal"
                    && expected_catalog_fp
                        .as_deref()
                        .map(|expected| health.catalog_fp == expected)
                        .unwrap_or(health.catalog_fp.is_empty())
            })
        {
            if let Some(t) = trace {
                t.stage(
                    OperationStage::ProxyHealth,
                    format!(
                        "reused port={port} adapter={} gateway={gateway_kind}",
                        launch.adapter
                    ),
                );
            }
            return Ok((port, st.secret.clone(), ProxyAction::Reused));
        }

        st.stop_proxy();
        if proc::loopback_port_in_use(port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
            let legacy_script = asset_root(app).map(|root| root.join("proxy/csswitch_proxy.py"));
            let cleanup = legacy_script
                .as_deref()
                .map(|script| stop_legacy_csswitch_python_on_port(port, script))
                .unwrap_or(LegacyProxyCleanup::NotLegacy);
            match cleanup {
                LegacyProxyCleanup::Stopped(pid) => {
                    if let Some(t) = trace {
                        t.stage(
                            OperationStage::ProxySpawn,
                            format!("stopped legacy CSSwitch Python proxy pid={pid} port={port}"),
                        );
                    }
                }
                LegacyProxyCleanup::StopFailed(pid) => {
                    return Err(format!(
                        "已确认端口 {port} 由旧版 CSSwitch Python proxy（PID {pid}）占用，但安全停止失败。请退出旧版或重启电脑后重试；未发送鉴权信息，也未强制结束进程。"
                    ));
                }
                LegacyProxyCleanup::NotLegacy => {
                    return Err(format!(
                        "端口 {port} 已被未知或旧 listener 占用；为避免发送鉴权信息或接管/结束非本轮进程，已拒绝启动。请手工确认后改用空闲端口。"
                    ));
                }
            }
            if proc::loopback_port_in_use(port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
                return Err(format!(
                    "旧版 CSSwitch proxy 已停止，但端口 {port} 随即被其它 listener 占用；未发送鉴权信息，也未结束新占用者。请改用空闲端口。"
                ));
            }
        }
        st.secret = secret.clone();

        let logf = open_log("proxy.log").map_err(|e| format!("建日志失败：{e}"))?;
        let logf2 = logf.try_clone().map_err(|e| e.to_string())?;
        if let Some(t) = trace {
            t.stage(
                OperationStage::ProxySpawn,
                format!(
                    "port={port} adapter={} gateway={gateway_kind}",
                    launch.adapter
                ),
            );
        }
        let launch_id =
            proc::gen_secret().map_err(|e| format!("无法生成 gateway launch_id：{e}"))?;
        let bin = gateway_bin_path(app)
            .ok_or("找不到 csswitch-gateway 二进制；请重新安装完整应用，开发态可设置绝对 CSSWITCH_GATEWAY_BIN。")?;
        let mut cmd = Command::new(bin);
        configure_managed_proxy_command(
            &mut cmd,
            &launch.adapter,
            shim_mode,
            port,
            &secret,
            &launch_id,
        )?;
        // The external-Skill bridge is optional. Unsafe or unwritable bridge
        // state disables only that bridge; it must never prevent the proxy (and
        // therefore Science) from starting.
        let _skill_install_bridge_ready = configure_skill_install_host(
            &mut cmd,
            &crate::runtime::science::sandbox_home().join(".claude-science"),
            &secret,
            &launch_id,
            science_context.as_ref(),
        )
        .is_ok();
        for (k, v) in formal_proxy_env(&launch)? {
            cmd.env(k, v);
        }
        let child = cmd
            .stdout(Stdio::from(logf))
            .stderr(Stdio::from(logf2))
            .spawn()
            .map_err(|e| format!("启动代理失败：{e}"))?;
        (child, launch_id)
    };

    let mut ok = false;
    let mut early_exit = None;
    for _ in 0..(operation::PROXY_HEALTH_BUDGET_MS / POLL_INTERVAL_MS) {
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        match proc::poll_child_liveness(&mut child) {
            proc::ChildLiveness::Exited(status) => {
                early_exit = Some(format!(
                    "新启动的 {gateway_kind} gateway 提前退出（{status}）"
                ));
                break;
            }
            proc::ChildLiveness::Running => {}
            proc::ChildLiveness::Unknown(error) => {
                early_exit = Some(format!(
                    "无法确认新启动的 {gateway_kind} gateway 是否存活：{error}"
                ));
                break;
            }
        }
        if proc::http_health_gateway(
            port,
            Some(&secret),
            operation::LOCAL_HEALTH_TIMEOUT_MS,
            proc::GatewayHealthExpectation {
                gateway: gateway_kind,
                provider: Some(&launch.adapter),
                shim: Some(shim_mode),
                launch_id: Some(&launch_id),
                provider_contract_id: Some(&launch.contract_id),
                provider_contract_digest: Some(&launch.contract_digest),
            },
        ) && proc::http_gateway_health(port, Some(&secret), operation::LOCAL_HEALTH_TIMEOUT_MS)
            .is_some_and(|health| {
                health.intent == "formal"
                    && expected_catalog_fp
                        .as_deref()
                        .map(|expected| health.catalog_fp == expected)
                        .unwrap_or(health.catalog_fp.is_empty())
            })
        {
            ok = true;
            break;
        }
    }
    if let Some(t) = trace {
        t.stage(
            OperationStage::ProxyHealth,
            if ok { "ready" } else { "not_ready" },
        );
    }
    if !ok {
        let _ = child.kill();
        let _ = child.wait();
        let tail = redact(&tail_file(&log_path("proxy.log"), 500), &secret);
        // Never authenticate to an unowned listener during failure diagnosis.
        // A bare TCP connect carries no path secret and is enough to report the
        // occupied-port class while leaving the unknown process untouched.
        let listener = if proc::loopback_port_in_use(port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
            format!("端口 {port} 仍有未知或旧 listener；未发送鉴权信息、未接管且未结束该进程。")
        } else {
            String::new()
        };
        let primary = early_exit.unwrap_or_else(|| health_timeout_reason(port, &tail));
        let mut details = vec![primary];
        if !listener.is_empty() {
            details.push(listener);
        }
        if !tail.is_empty() {
            details.push(tail);
        }
        return Err(details.join("\n"));
    }

    {
        let mut st = lock(state);
        if !should_write_back(gen, lifecycle.current_generation(), &st.secret, &secret) {
            let mut c = child;
            let _ = c.kill();
            let _ = c.wait();
            return Err("代理启动期间配置已变更（被更晚的操作取代），本次启动未生效。".into());
        }
        if let Err(error) = proc::require_child_running(
            &mut child,
            &format!("新启动的 {gateway_kind} gateway 在发布 AppState 前"),
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        #[cfg(test)]
        if let Some(path) = std::env::var_os("CSSWITCH_TEST_GATEWAY_PUBLISH_LOG") {
            if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(
                    log,
                    "{} {} {} {} {}",
                    child.id(),
                    launch_id,
                    key_fp,
                    port,
                    launch.adapter
                );
            }
        }
        st.proxy = Some(child);
        st.proxy_port = port;
        st.secret = secret.clone();
        st.provider = launch.adapter.clone();
        st.gateway_kind = gateway_kind.to_string();
        st.shim_mode = shim_mode.to_string();
        st.launch_id = launch_id;
        st.key_fp = key_fp;
        st.gateway_launch_context = Some(crate::GatewayLaunchContext {
            profile: profile.clone(),
            science_runtime: science_runtime.cloned(),
        });
    }
    Ok((port, secret, ProxyAction::Restarted))
}
