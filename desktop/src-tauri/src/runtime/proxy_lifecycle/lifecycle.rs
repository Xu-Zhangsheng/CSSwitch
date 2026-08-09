#[derive(Clone, PartialEq)]
struct GatewayProcessLocalOwner {
    generation: u64,
    child_pid: u32,
    proxy_port: u16,
    secret: String,
    provider: String,
    gateway_kind: String,
    shim_mode: String,
    launch_id: String,
    key_fp: u64,
    launch_context: Option<GatewayLaunchRecipe>,
}

impl GatewayProcessLocalOwner {
    fn claim(st: &AppState, generation: u64) -> Option<Self> {
        Some(Self {
            generation,
            child_pid: st.proxy.as_ref()?.id(),
            proxy_port: st.proxy_port,
            secret: st.secret.clone(),
            provider: st.provider.clone(),
            gateway_kind: st.gateway_kind.clone(),
            shim_mode: st.shim_mode.clone(),
            launch_id: st.launch_id.clone(),
            key_fp: st.key_fp,
            launch_context: st.gateway_launch_context.clone(),
        })
    }

    fn metadata_matches(&self, st: &AppState, current_generation: u64) -> bool {
        self.generation == current_generation
            && st.proxy_port == self.proxy_port
            && st.secret == self.secret
            && st.provider == self.provider
            && st.gateway_kind == self.gateway_kind
            && st.shim_mode == self.shim_mode
            && st.launch_id == self.launch_id
            && st.key_fp == self.key_fp
            && st.gateway_launch_context == self.launch_context
    }

    fn still_owns(&self, st: &AppState, current_generation: u64) -> bool {
        st.proxy.as_ref().map(std::process::Child::id) == Some(self.child_pid)
            && self.metadata_matches(st, current_generation)
    }

    fn owns_cleanup_marker(&self, st: &AppState, current_generation: u64) -> bool {
        st.proxy.is_none() && self.metadata_matches(st, current_generation)
    }
}

struct GatewayTrackedCleanupOwner {
    identity: GatewayProcessLocalOwner,
    child: std::process::Child,
}

fn app_state_gateway_slot_is_empty(st: &AppState) -> bool {
    st.proxy.is_none()
        && st.secret.is_empty()
        && st.provider.is_empty()
        && st.gateway_kind.is_empty()
        && st.shim_mode.is_empty()
        && st.launch_id.is_empty()
        && st.key_fp == 0
        && st.gateway_launch_context.is_none()
}

fn stop_tracked_gateway_child(child: &mut std::process::Child) -> Result<(), String> {
    match child.try_wait() {
        Ok(Some(_)) => return Ok(()),
        Ok(None) => {}
        Err(error) => return Err(format!("无法确认旧 Gateway 子进程状态：{error}")),
    }
    if let Err(error) = child.kill() {
        return match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            _ => Err(format!("无法停止旧 Gateway 子进程：{error}")),
        };
    }
    child
        .wait()
        .map(|_| ())
        .map_err(|error| format!("等待旧 Gateway 子进程退出失败：{error}"))
}

fn cleanup_tracked_gateway_with<'a, Stop>(
    state: &'a SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    mut state_guard: std::sync::MutexGuard<'a, AppState>,
    generation: u64,
    stop: Stop,
) -> Result<std::sync::MutexGuard<'a, AppState>, String>
where
    Stop: FnOnce(&mut std::process::Child) -> Result<(), String>,
{
    let Some(identity) = GatewayProcessLocalOwner::claim(&state_guard, generation) else {
        state_guard.clear_proxy_identity();
        return Ok(state_guard);
    };
    let child = state_guard
        .proxy
        .take()
        .expect("Gateway cleanup claim must own the tracked child");
    let mut owner = GatewayTrackedCleanupOwner { identity, child };
    drop(state_guard);

    let stopped = stop(&mut owner.child);
    let mut current = lock(state);
    if !owner
        .identity
        .owns_cleanup_marker(&current, lifecycle.current_generation())
    {
        return Err(
            "旧 Gateway 清理完成时 process-local owner 已变化；已拒绝覆盖 replacement，请重试。"
                .into(),
        );
    }
    match stopped {
        Ok(()) => {
            current.clear_proxy_identity();
            Ok(current)
        }
        Err(error) => {
            current.proxy = Some(owner.child);
            Err(format!(
                "旧 Gateway 清理未确认完成；已保留 process-local owner 以便安全重试：{error}"
            ))
        }
    }
}

fn run_legacy_cleanup_outside_state_with<'a, Cleanup>(
    state: &'a SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    state_guard: std::sync::MutexGuard<'a, AppState>,
    generation: u64,
    cleanup: Cleanup,
) -> Result<(std::sync::MutexGuard<'a, AppState>, Option<u32>), String>
where
    Cleanup: FnOnce() -> Result<Option<u32>, String>,
{
    drop(state_guard);
    let result = cleanup();
    let current = lock(state);
    if lifecycle.current_generation() != generation || !app_state_gateway_slot_is_empty(&current) {
        return Err(
            "旧 Gateway listener 清理完成时 runtime owner 已变化；已拒绝覆盖 replacement，请重试。"
                .into(),
        );
    }
    result.map(|stopped_pid| (current, stopped_pid))
}

fn probe_gateway_reuse_health_with<'a, Probe>(
    state: &'a SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    state_guard: std::sync::MutexGuard<'a, AppState>,
    owner: &GatewayProcessLocalOwner,
    probe: Probe,
) -> Result<
    (
        std::sync::MutexGuard<'a, AppState>,
        Option<proc::GatewayHealth>,
    ),
    String,
>
where
    Probe: FnOnce(&GatewayProcessLocalOwner) -> Option<proc::GatewayHealth>,
{
    drop(state_guard);
    let accepted_health = probe(owner);
    let current = lock(state);
    if !owner.still_owns(&current, lifecycle.current_generation()) {
        return Err(
            "Gateway reuse 探活完成时 process-local owner 已变化；已拒绝过期结果，请重试。".into(),
        );
    }
    Ok((current, accepted_health))
}

fn start_proxy_for_inner<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    profile: &config::Profile,
    science_runtime: Option<&crate::runtime::science::ScienceRuntimeIdentity>,
    trace: Option<&OperationTrace>,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
) -> Result<GatewayReceipt, String> {
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
    let (science_context, effective_science_runtime) = match science_runtime {
        Some(runtime) => (
            Some(runtime.skill_install_host_context(cfg.sandbox_port)?),
            Some(runtime.clone()),
        ),
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
            remembered
                .and_then(|(runtime, sandbox_port)| {
                    (sandbox_port == cfg.sandbox_port
                        && crate::runtime::science::ScienceHostAdapter::probe_known(
                            sandbox_port,
                            &runtime,
                        ) == crate::runtime::science::SandboxScienceState::RunningHealthy)
                        .then(|| {
                            runtime
                                .skill_install_host_context(sandbox_port)
                                .ok()
                                .map(|context| (context, runtime))
                        })
                        .flatten()
                })
                .map_or((None, None), |(context, runtime)| {
                    (Some(context), Some(runtime))
                })
        }
    };
    let recipe = GatewayLaunchRecipe {
        profile: profile.clone(),
        science_runtime: effective_science_runtime,
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

    let (mut child, launch_id, gen) = {
        let mut st = lock(state);
        let gen = lifecycle.current_generation();
        let tracked_child_running = proc::tracked_child_is_running(&mut st.proxy);
        let reuse_owner = (tracked_child_running
            && st.proxy_port == port
            && st.provider == launch.adapter
            && st.gateway_kind == gateway_kind
            && st.shim_mode == shim_mode
            && st.key_fp == key_fp)
            .then(|| GatewayProcessLocalOwner::claim(&st, gen))
            .flatten();
        if let Some(owner) = reuse_owner {
            let (next_state, accepted_reuse_health) =
                probe_gateway_reuse_health_with(state, lifecycle, st, &owner, |owner| {
                    proc::http_gateway_health(
                        owner.proxy_port,
                        Some(&owner.secret),
                        operation::PROXY_REUSE_HEALTH_TIMEOUT_MS,
                    )
                    .filter(|health| {
                        accepted_gateway_health(
                            health,
                            proc::GatewayHealthExpectation {
                                gateway: owner.gateway_kind.as_str(),
                                provider: Some(owner.provider.as_str()),
                                shim: Some(owner.shim_mode.as_str()),
                                launch_id: Some(owner.launch_id.as_str()),
                                provider_contract_id: Some(&launch.contract_id),
                                provider_contract_digest: Some(&launch.contract_digest),
                            },
                            expected_catalog_fp.as_deref(),
                        )
                    })
                })?;
            st = next_state;
            if let Some(accepted_health) = accepted_reuse_health {
                if let Some(t) = trace {
                    t.stage(
                        OperationStage::ProxyHealth,
                        format!(
                            "reused port={port} adapter={} gateway={gateway_kind}",
                            launch.adapter
                        ),
                    );
                }
                st.gateway_launch_context = Some(recipe.clone());
                return Ok(GatewayReceipt::verified(
                    port,
                    owner.secret,
                    ProxyAction::Reused,
                    &accepted_health,
                    expected_catalog_fp.clone(),
                    recipe,
                ));
            }
        }

        st = cleanup_tracked_gateway_with(state, lifecycle, st, gen, stop_tracked_gateway_child)?;
        let legacy_script = asset_root(app).map(|root| root.join("proxy/csswitch_proxy.py"));
        let (next_state, _) = run_legacy_cleanup_outside_state_with(
            state,
            lifecycle,
            st,
            gen,
            || {
                if !proc::loopback_port_in_use(port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
                    return Ok(None);
                }
                let cleanup = legacy_script
                    .as_deref()
                    .map(|script| stop_legacy_csswitch_python_on_port(port, script))
                    .unwrap_or(LegacyProxyCleanup::NotLegacy);
                let stopped_pid = match cleanup {
                    LegacyProxyCleanup::Stopped(pid) => pid,
                    LegacyProxyCleanup::IdentityChanged(pid) => {
                        return Err(format!(
                            "旧版 CSSwitch Python proxy（原 PID {pid}）在发信号前身份已变化；为避免结束 replacement，已拒绝清理。请改用空闲端口。"
                        ));
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
                };
                if proc::loopback_port_in_use(port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
                    return Err(format!(
                        "旧版 CSSwitch proxy 已停止，但端口 {port} 随即被其它 listener 占用；未发送鉴权信息，也未结束新占用者。请改用空闲端口。"
                    ));
                }
                if let Some(t) = trace {
                    t.stage(
                        OperationStage::ProxySpawn,
                        format!(
                            "stopped legacy CSSwitch Python proxy pid={stopped_pid} port={port}"
                        ),
                    );
                }
                Ok(Some(stopped_pid))
            },
        )?;
        st = next_state;
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
        #[cfg(feature = "acceptance-build")]
        configure_acceptance_native_upstream_override(
            &mut cmd,
            &launch.adapter,
            std::env::var_os("CSSWITCH_UPSTREAM_URL").as_deref(),
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
        (child, launch_id, gen)
    };

    let mut accepted_health = None;
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
        accepted_health =
            proc::http_gateway_health(port, Some(&secret), operation::LOCAL_HEALTH_TIMEOUT_MS)
                .filter(|health| {
                    accepted_gateway_health(
                        health,
                        proc::GatewayHealthExpectation {
                            gateway: gateway_kind,
                            provider: Some(&launch.adapter),
                            shim: Some(shim_mode),
                            launch_id: Some(&launch_id),
                            provider_contract_id: Some(&launch.contract_id),
                            provider_contract_digest: Some(&launch.contract_digest),
                        },
                        expected_catalog_fp.as_deref(),
                    )
                });
        if accepted_health.is_some() {
            break;
        }
    }
    if let Some(t) = trace {
        t.stage(
            OperationStage::ProxyHealth,
            if accepted_health.is_some() {
                "ready"
            } else {
                "not_ready"
            },
        );
    }
    if accepted_health.is_none() {
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
    let accepted_health = accepted_health.expect("checked accepted Gateway health");

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
        st.launch_id = launch_id.clone();
        st.key_fp = key_fp;
        st.gateway_launch_context = Some(recipe.clone());
    }
    Ok(GatewayReceipt::verified(
        port,
        secret,
        ProxyAction::Restarted,
        &accepted_health,
        expected_catalog_fp,
        recipe,
    ))
}
