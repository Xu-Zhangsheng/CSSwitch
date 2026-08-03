use super::*;

pub(crate) fn stop_sandbox_state<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    st: &mut AppState,
) -> crate::runtime::science::ScienceStopOutcome {
    let runtime = st.science_runtime.clone();
    let request = crate::runtime::science::ScienceStopRequest::recover(runtime.as_ref());
    let result = ScienceHostAdapter::stop(app, &mut st.sandbox, &mut st.sandbox_url, request);
    if let Ok(verified) = result.as_ref() {
        st.science_confirmed_stopped = verified.confirmed_runtime().cloned();
        st.science_runtime = None;
    }
    result
}

/// 切换运行模式（"proxy" 第三方 / "official" 官方）。切官方要先拆第三方链路成功再落盘。
pub(super) async fn set_mode_command(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    mode: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || set_mode_inner(app, state, lifecycle, mode)).await
}

pub(super) fn set_mode_inner<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
    mode: String,
) -> Result<(), String> {
    if mode != "proxy" && mode != "official" {
        return Err(format!("未知模式：{mode}（只支持 proxy / official）。"));
    }
    // 经串行器（修 P1-b）：切官方的「拆链路 + 落盘」必须与「一键开始」等互斥，否则一键起到一半时
    // 切官方会先停链路、一键随后又把沙箱/OAuth 起起来 → 显示官方却有第三方沙箱在跑。bump_generation
    // 作废任何在途启动，防被停后又拿旧配置写回运行态。
    lifecycle.with_mutation(RuntimeMutationDomain::Destructive, |_| {
        let dir = config::default_dir();
        if mode == "official" {
            lifecycle.bump_generation();
            let mut st = lock(&state);
            stop_sandbox_state(&app, &mut st).map_err(|e| {
                format!("停止沙箱失败，未切换到官方模式：{e}（真实实例 8765 未受影响）")
            })?;
            st.stop_proxy();
        }
        config::update(&dir, {
            let mode = mode.clone();
            move |c| c.mode = mode
        })
        .map_err(|e| e.to_string())?;
        {
            let mut app_state = lock(&state);
            app_state.history_recovery = None;
            app_state.boot_attention = None;
        }
        Ok(())
    })
}

#[derive(Deserialize)]
pub(crate) struct UiSettings {
    pub(super) proxy_port: u16,
    pub(super) sandbox_port: u16,
    #[serde(default)]
    pub(super) reuse_system_ssh: bool,
}

/// 运行设置（端口 + 系统 SSH 配置授权；provider/连接改走 profile CRUD + set_active_profile）。
/// 经串行器（修 P1-c）：端口或 SSH 授权一旦变化，正在跑的沙箱都必须拆掉，
/// 与新端口不一致；此处把这条陈旧链路拆掉（只停我们的沙箱、绝不碰 8765），逼下次「一键开始」按新端口重建，
/// 杜绝「复用旧沙箱指向死端口、UI 却报沿用不变」。
pub(super) async fn set_settings_command(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    cfg: UiSettings,
) -> Result<(), String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || set_settings_inner(app, state, lifecycle, cfg)).await
}

pub(super) fn set_settings_inner<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
    cfg: UiSettings,
) -> Result<(), String> {
    validate_runtime_ports(cfg.proxy_port, cfg.sandbox_port)?;
    if cfg.reuse_system_ssh {
        system_ssh_config_path()?;
        system_ssh_hosts()?;
    }
    lifecycle.with_mutation(RuntimeMutationDomain::Destructive, |_| {
        let dir = config::default_dir();
        let old = config::load_from(&dir).map_err(|e| e.to_string())?;
        let teardown = settings_change_needs_teardown(
            old.proxy_port,
            cfg.proxy_port,
            old.sandbox_port,
            cfg.sandbox_port,
        ) || old.reuse_system_ssh != cfg.reuse_system_ssh;
        // 拆链路【先】于落盘，且停沙箱结果必须据实处理（修增量 P1）：停不掉就【不改端口】——
        // 否则会留下「config 已是新端口、旧沙箱仍在旧端口指向旧代理」的不一致态，下次一键还会复用这条死链路。
        // 保持端口不变则一切仍自洽（旧沙箱指旧代理端口、下次一键在旧端口重建代理，链路照通）。
        if teardown {
            let mut st = lock(&state);
            stop_sandbox_state(&app, &mut st).map_err(|e| {
                format!(
                    "设置未更改：无法停止仍使用旧端口或旧 SSH 授权的沙箱（{e}）。请手动停止沙箱或重启 app 后重试。（真实实例 8765 未受影响）"
                )
            })?;
            lifecycle.bump_generation(); // 停成功后作废在途启动
            st.stop_proxy();
        }
        if !cfg.reuse_system_ssh {
            revoke_science_ssh_bridge(&crate::runtime::science::sandbox_home())?;
            remove_managed_sandbox_ssh_stub(&crate::runtime::science::sandbox_home())?;
        }
        // 拆链路成功（或无需拆）→ 才落盘新端口，保证 config 与运行态一致。
        config::update(&dir, move |c| {
            c.proxy_port = cfg.proxy_port;
            c.sandbox_port = cfg.sandbox_port;
            c.reuse_system_ssh = cfg.reuse_system_ssh;
        })
        .map_err(|e| e.to_string())?;
        {
            let mut app_state = lock(&state);
            app_state.history_recovery = None;
            app_state.boot_attention = None;
        }
        Ok(())
    })
}

pub(super) async fn stop_all_command(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
) -> Result<(), String> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking(move || stop_all_inner_cmd(app, state, lifecycle)).await
}

pub(super) fn stop_all_inner_cmd<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
) -> Result<(), String> {
    stop_all_inner_with(
        app,
        state,
        lifecycle,
        RuntimeMutationDomain::Destructive,
        ScienceHostAdapter::claim_stop,
        |app, request| ScienceHostAdapter::execute_stop(app, request).into_parts(),
    )
}

#[derive(Clone)]
struct ScienceProcessLocalOwner {
    generation: u64,
    runtime: Option<crate::runtime::science::ScienceRuntimeIdentity>,
    confirmed_stopped: Option<crate::runtime::science::ScienceRuntimeIdentity>,
    sandbox_child_pid: Option<u32>,
    sandbox_port: u16,
    sandbox_url: Option<String>,
}

impl ScienceProcessLocalOwner {
    fn claim(st: &AppState, generation: u64) -> Self {
        Self {
            generation,
            runtime: st.science_runtime.clone(),
            confirmed_stopped: st.science_confirmed_stopped.clone(),
            sandbox_child_pid: st.sandbox.as_ref().map(std::process::Child::id),
            sandbox_port: st.sandbox_port,
            sandbox_url: st.sandbox_url.clone(),
        }
    }

    fn still_owns(&self, st: &AppState, current_generation: u64) -> bool {
        self.generation == current_generation
            && self.runtime == st.science_runtime
            && self.confirmed_stopped == st.science_confirmed_stopped
            && self.sandbox_child_pid == st.sandbox.as_ref().map(std::process::Child::id)
            && self.sandbox_port == st.sandbox_port
            && self.sandbox_url == st.sandbox_url
    }
}

pub(super) fn stop_all_inner_with<R, Claim, Execute>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
    domain: RuntimeMutationDomain,
    claim_science: Claim,
    execute_science: Execute,
) -> Result<(), String>
where
    R: tauri::Runtime,
    Claim: FnOnce(
        Option<&crate::runtime::science::ScienceRuntimeIdentity>,
    ) -> Result<
        crate::runtime::science::ScienceStopRequest,
        crate::runtime::science::ScienceStopFailure,
    >,
    Execute: FnOnce(
        &tauri::AppHandle<R>,
        crate::runtime::science::ScienceStopRequest,
    ) -> (crate::runtime::science::ScienceStopOutcome, bool),
{
    lifecycle.with_mutation(domain, |_| {
        let generation = lifecycle.bump_generation(); // 作废任何在途启动（防被停后又拿旧 key 复活）
        let (owner, request) = {
            let st = lock(&state);
            let owner = ScienceProcessLocalOwner::claim(&st, generation);
            let request = claim_science(owner.runtime.as_ref());
            (owner, request)
        };
        // The stop script and bounded TERM/KILL waits intentionally run without
        // the AppState mutex so high-frequency status can copy its read model.
        let execution = request.map(|request| execute_science(&app, request));
        let mut st = lock(&state);
        let owner_is_current = owner.still_owns(&st, lifecycle.current_generation());
        let sandbox_res = match execution {
            Err(error) => Err(error),
            Ok((_outcome, _clear_tracking)) if !owner_is_current => Err(
                crate::runtime::science::ScienceStopFailure::outcome_publication_failure(
                    "Science stop 完成时 process-local owner 已变化；已保留 replacement runtime。",
                ),
            ),
            Ok((outcome, clear_tracking)) => {
                if clear_tracking {
                    crate::runtime::system::kill_child(&mut st.sandbox);
                    st.sandbox_url = None;
                }
                if let Ok(verified) = outcome.as_ref() {
                    st.science_confirmed_stopped = verified.confirmed_runtime().cloned();
                    st.science_runtime = None;
                }
                outcome
            }
        };
        st.stop_proxy();
        sandbox_res
            .map(|_| ())
            .map_err(|e| format!("代理已停；但{e}真实实例 8765 未受影响。"))
    })
}

pub(super) async fn quit_app_command(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
) -> Result<(), String> {
    let exit_app = app.clone();
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    let stopped = run_blocking(move || {
        stop_all_inner_with(
            app,
            state,
            lifecycle,
            RuntimeMutationDomain::Terminal,
            ScienceHostAdapter::claim_stop,
            |app, request| ScienceHostAdapter::execute_stop(app, request).into_parts(),
        )
    })
    .await;
    exit_after_stop_success(stopped, || exit_app.exit(0))
}

pub(super) fn exit_after_stop_success(
    stopped: Result<(), String>,
    exit: impl FnOnce(),
) -> Result<(), String> {
    stopped?;
    exit();
    Ok(())
}
