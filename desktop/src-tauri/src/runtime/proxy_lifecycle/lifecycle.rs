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

#[derive(Clone, PartialEq)]
struct GatewaySpawnCandidateOwner {
    generation: u64,
    proxy_port: u16,
    secret: String,
    provider: String,
    gateway_kind: String,
    shim_mode: String,
    launch_id: String,
    key_fp: u64,
    launch_context: GatewayLaunchRecipe,
}

impl GatewaySpawnCandidateOwner {
    #[allow(clippy::too_many_arguments)]
    fn reserve(
        st: &mut AppState,
        generation: u64,
        proxy_port: u16,
        secret: String,
        provider: String,
        gateway_kind: String,
        shim_mode: String,
        launch_id: String,
        key_fp: u64,
        launch_context: GatewayLaunchRecipe,
    ) -> Result<Self, String> {
        if !app_state_gateway_slot_is_empty(st) {
            return Err(
                "Gateway spawn claim 发现 runtime slot 已被 replacement 占用；已拒绝启动。".into(),
            );
        }
        let owner = Self {
            generation,
            proxy_port,
            secret,
            provider,
            gateway_kind,
            shim_mode,
            launch_id,
            key_fp,
            launch_context,
        };
        st.proxy_port = owner.proxy_port;
        st.secret = owner.secret.clone();
        st.provider = owner.provider.clone();
        st.gateway_kind = owner.gateway_kind.clone();
        st.shim_mode = owner.shim_mode.clone();
        st.launch_id = owner.launch_id.clone();
        st.key_fp = owner.key_fp;
        st.gateway_launch_context = Some(owner.launch_context.clone());
        Ok(owner)
    }

    fn marker_matches(&self, st: &AppState) -> bool {
        st.proxy.is_none()
            && st.proxy_port == self.proxy_port
            && st.secret == self.secret
            && st.provider == self.provider
            && st.gateway_kind == self.gateway_kind
            && st.shim_mode == self.shim_mode
            && st.launch_id == self.launch_id
            && st.key_fp == self.key_fp
            && st.gateway_launch_context.as_ref() == Some(&self.launch_context)
    }

    fn still_owns(&self, st: &AppState, current_generation: u64) -> bool {
        self.generation == current_generation && self.marker_matches(st)
    }
}

struct GatewaySpawnCandidate {
    child: Option<std::process::Child>,
    rejected: crate::RejectedGatewayRegistry,
}

impl GatewaySpawnCandidate {
    fn new(child: std::process::Child, state: &SharedAppState) -> Self {
        Self {
            child: Some(child),
            rejected: lock(state).rejected_gateway_candidates.clone(),
        }
    }

    fn child_mut(&mut self) -> &mut std::process::Child {
        self.child
            .as_mut()
            .expect("Gateway spawn candidate must own its child")
    }

    fn accept_child(&mut self) -> std::process::Child {
        self.child
            .take()
            .expect("accepted Gateway candidate must own its child")
    }

    fn reject(self) -> Result<(), String> {
        self.reject_with(crate::runtime::system::stop_child_confirmed)
    }

    fn reject_with<Stop>(mut self, stop: Stop) -> Result<(), String>
    where
        Stop: FnOnce(&mut std::process::Child) -> Result<(), String>,
    {
        let stopped = stop(
            self.child
                .as_mut()
                .expect("rejected Gateway candidate must own its child"),
        );
        match stopped {
            Ok(()) => {
                self.child.take();
                Ok(())
            }
            Err(error) => {
                let child = self
                    .child
                    .take()
                    .expect("uncertain Gateway candidate must retain its child");
                self.rejected.retain(child);
                Err(format!(
                    "新 Gateway candidate 停止结果未确认；已保留独立 cleanup owner，未覆盖 replacement：{error}"
                ))
            }
        }
    }
}

impl Drop for GatewaySpawnCandidate {
    fn drop(&mut self) {
        let Some(child) = self.child.take() else {
            return;
        };
        // Unwind/early-return fallback only restores affine ownership. It must
        // not invoke another fallible stop callback while already unwinding.
        self.rejected.retain(child);
    }
}

struct GatewayCandidateLog {
    candidate_path: Option<PathBuf>,
    canonical_path: PathBuf,
}

impl GatewayCandidateLog {
    fn open(launch_id: &str) -> Result<(std::fs::File, Self), String> {
        let name = format!("proxy-candidate-{launch_id}.log");
        let file = open_log(&name).map_err(|error| format!("建日志失败：{error}"))?;
        Ok((
            file,
            Self {
                candidate_path: Some(log_path(&name)),
                canonical_path: log_path("proxy.log"),
            },
        ))
    }

    fn tail(&self) -> String {
        self.candidate_path
            .as_deref()
            .map(|path| tail_file(path, 500))
            .unwrap_or_default()
    }

    fn publish(&mut self) -> Result<(), String> {
        let candidate = self
            .candidate_path
            .as_ref()
            .ok_or("Gateway candidate log 已发布")?;
        config::assert_not_symlink(&self.canonical_path)
            .map_err(|error| format!("无法安全发布 Gateway 日志：{error}"))?;
        fs::rename(candidate, &self.canonical_path)
            .map_err(|error| format!("无法发布 Gateway 日志：{error}"))?;
        self.candidate_path = None;
        Ok(())
    }
}

impl Drop for GatewayCandidateLog {
    fn drop(&mut self) {
        if let Some(path) = self.candidate_path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

fn app_state_gateway_slot_is_empty(st: &AppState) -> bool {
    st.proxy.is_none()
        && st.rejected_gateway_candidates.is_idle()
        && st.secret.is_empty()
        && st.provider.is_empty()
        && st.gateway_kind.is_empty()
        && st.shim_mode.is_empty()
        && st.launch_id.is_empty()
        && st.key_fp == 0
        && st.gateway_launch_context.is_none()
}

fn finish_failed_gateway_candidate(
    state: &SharedAppState,
    lifecycle: &lifecycle::Lifecycle,
    owner: &GatewaySpawnCandidateOwner,
    candidate_log: Option<&mut GatewayCandidateLog>,
    skill_host: Option<&mut PreparedSkillInstallHost>,
) -> bool {
    let mut current = lock(state);
    let current_owner = owner.still_owns(&current, lifecycle.current_generation());
    if current_owner {
        // Preserve the existing diagnostic/optional-bridge publication on an
        // ordinary current-owner launch failure. Stale candidates never publish.
        if let Some(candidate_log) = candidate_log {
            let _ = candidate_log.publish();
        }
        if let Some(skill_host) = skill_host {
            let _ = skill_host.publish();
        }
    }
    if owner.marker_matches(&current) {
        current.clear_proxy_identity();
    }
    current_owner
}

fn retry_rejected_gateway_candidates_outside_state(state: &SharedAppState) -> Result<(), String> {
    let rejected = lock(state).rejected_gateway_candidates.clone();
    rejected.retry_with(crate::runtime::system::stop_child_confirmed)
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
    retry_rejected_gateway_candidates_outside_state(state)?;
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

    let launch_id = proc::gen_secret().map_err(|e| format!("无法生成 gateway launch_id：{e}"))?;
    let spawn_owner = {
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

        st = cleanup_tracked_gateway_with(
            state,
            lifecycle,
            st,
            gen,
            crate::runtime::system::stop_child_confirmed,
        )?;
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
        GatewaySpawnCandidateOwner::reserve(
            &mut st,
            gen,
            port,
            secret.clone(),
            launch.adapter.clone(),
            gateway_kind.to_string(),
            shim_mode.to_string(),
            launch_id.clone(),
            key_fp,
            recipe.clone(),
        )?
    };

    let mut candidate_log = None;
    let mut prepared_skill_host = None;
    let spawn_result = (|| -> Result<std::process::Child, String> {
        let (logf, log_owner) = GatewayCandidateLog::open(&launch_id)?;
        let logf2 = logf.try_clone().map_err(|error| error.to_string())?;
        candidate_log = Some(log_owner);
        if let Some(t) = trace {
            t.stage(
                OperationStage::ProxySpawn,
                format!(
                    "port={port} adapter={} gateway={gateway_kind}",
                    launch.adapter
                ),
            );
        }
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
        // Bridge preparation is candidate-scoped and optional. The canonical
        // key is published only after this candidate still owns AppState.
        prepared_skill_host = prepare_skill_install_host(
            &mut cmd,
            &crate::runtime::science::sandbox_home().join(".claude-science"),
            &secret,
            &launch_id,
            science_context.as_ref(),
        )
        .ok();
        for (k, v) in formal_proxy_env(&launch)? {
            cmd.env(k, v);
        }
        cmd.stdout(Stdio::from(logf))
            .stderr(Stdio::from(logf2))
            .spawn()
            .map_err(|error| format!("启动代理失败：{error}"))
    })();
    let child = match spawn_result {
        Ok(child) => child,
        Err(error) => {
            finish_failed_gateway_candidate(
                state,
                lifecycle,
                &spawn_owner,
                candidate_log.as_mut(),
                prepared_skill_host.as_mut(),
            );
            return Err(error);
        }
    };
    let mut candidate = GatewaySpawnCandidate::new(child, state);

    let mut accepted_health = None;
    let mut early_exit = None;
    for _ in 0..(operation::PROXY_HEALTH_BUDGET_MS / POLL_INTERVAL_MS) {
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        match proc::poll_child_liveness(candidate.child_mut()) {
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
        let tail = redact(
            &candidate_log
                .as_ref()
                .map(GatewayCandidateLog::tail)
                .unwrap_or_default(),
            &secret,
        );
        let cleanup_error = candidate.reject().err();
        let current_owner = finish_failed_gateway_candidate(
            state,
            lifecycle,
            &spawn_owner,
            candidate_log.as_mut(),
            prepared_skill_host.as_mut(),
        );
        if !current_owner {
            let mut error =
                "Gateway candidate 探活期间 owner 已变化；已拒绝覆盖 replacement。".to_string();
            if let Some(cleanup_error) = cleanup_error {
                error.push_str(&format!("\n{cleanup_error}"));
            }
            return Err(error);
        }
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
        if let Some(cleanup_error) = cleanup_error {
            details.push(cleanup_error);
        }
        return Err(details.join("\n"));
    }
    let accepted_health = accepted_health.expect("checked accepted Gateway health");

    {
        let mut st = lock(state);
        if !spawn_owner.still_owns(&st, lifecycle.current_generation()) {
            drop(st);
            let cleanup_error = candidate.reject().err();
            finish_failed_gateway_candidate(
                state,
                lifecycle,
                &spawn_owner,
                candidate_log.as_mut(),
                prepared_skill_host.as_mut(),
            );
            let mut error =
                "代理启动期间 owner 已变化（被更晚的操作取代），本次启动未生效。".to_string();
            if let Some(cleanup_error) = cleanup_error {
                error.push_str(&format!("\n{cleanup_error}"));
            }
            return Err(error);
        }
        if let Err(error) = proc::require_child_running(
            candidate.child_mut(),
            &format!("新启动的 {gateway_kind} gateway 在发布 AppState 前"),
        ) {
            drop(st);
            let cleanup_error = candidate.reject().err();
            finish_failed_gateway_candidate(
                state,
                lifecycle,
                &spawn_owner,
                candidate_log.as_mut(),
                prepared_skill_host.as_mut(),
            );
            return Err(
                cleanup_error.map_or(error.clone(), |cleanup| format!("{error}\n{cleanup}"))
            );
        }
        if let Err(error) = candidate_log
            .as_mut()
            .expect("spawned Gateway candidate must own its log")
            .publish()
        {
            drop(st);
            let cleanup_error = candidate.reject().err();
            finish_failed_gateway_candidate(
                state,
                lifecycle,
                &spawn_owner,
                candidate_log.as_mut(),
                prepared_skill_host.as_mut(),
            );
            return Err(
                cleanup_error.map_or(error.clone(), |cleanup| format!("{error}\n{cleanup}"))
            );
        }
        if let Some(skill_host) = prepared_skill_host.as_mut() {
            // The bridge remains optional. Publication failure degrades only the
            // bridge and the staged file is removed on drop.
            let _ = skill_host.publish();
        }
        #[cfg(test)]
        if let Some(path) = std::env::var_os("CSSWITCH_TEST_GATEWAY_PUBLISH_LOG") {
            if let Ok(mut log) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(
                    log,
                    "{} {} {} {} {}",
                    candidate.child_mut().id(),
                    launch_id,
                    key_fp,
                    port,
                    launch.adapter
                );
            }
        }
        st.proxy = Some(candidate.accept_child());
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
