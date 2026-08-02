//! CSSwitch 桌面 app 后端（进程管家）。
//!
//! 职责：管理「翻译代理」与「沙箱 Science」两个子进程的生命周期；读写
//! `~/.csswitch/config.json`（多 profile 形态）；把第三方 key 以【环境变量】注入代理子进程
//! （绝不进 argv）；探活；把沙箱 URL 交系统浏览器打开。推理与协议转换由随包交付的
//! Rust `csswitch-gateway` 完成；沙箱脚本仍作为受管子进程保留铁律护栏。
//!
//! 运行行为由 typed provider-contract catalog 与生效 profile 合并成受限 launch plan，
//! 再投影给 formal gateway、scratch 和前端；展示模板不再决定 adapter/鉴权/transport。
//!
//! 铁律相关：API key 只在内存与 0600 的 config.json，OAuth token 只在 CSSwitch 私有认证文件；回显前端只给掩码/脱敏状态；沙箱端口/目录护栏
//! 由被调脚本负责（对 8765 与真实目录失败关闭）；关窗只隐藏，显式退出停代理与沙箱。

mod codex_auth_supervisor;
mod commands;
mod config;
mod config_legacy;
mod lifecycle;
mod model_catalog;
mod oauth_forge;
mod opencode_go_models;
mod proc;
mod provider_contracts;
mod runtime;
mod scratch;
mod templates;

use std::process::Child;
use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

use runtime::{
    science::{stop_sandbox, ScienceStopRequest},
    system::kill_child,
};

use codex_auth_supervisor::{CodexAuthSupervisor, SharedCodexAuthSupervisor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaunchPath {
    ShowPanel,
    OpenOfficial,
    BootScience,
}

fn decide_launch_with_auto_boot(cfg: &config::Config, auto_boot: bool) -> LaunchPath {
    if !auto_boot {
        return LaunchPath::ShowPanel;
    }
    if cfg.mode == "official" {
        return LaunchPath::OpenOfficial;
    }
    match cfg.active_profile() {
        Some(p)
            if config::require_template_enabled(cfg, &p.template_id).is_ok()
                && p.template_id != "codex"
                && runtime::provider::resolve_launch_plan(p)
                    .map(|plan| plan.public().credential_configured)
                    .unwrap_or(false) =>
        {
            LaunchPath::BootScience
        }
        _ => LaunchPath::ShowPanel,
    }
}

fn decide_launch(cfg: &config::Config) -> LaunchPath {
    let auto_boot = std::env::var("CSSWITCH_AUTO_BOOT_ON_LAUNCH")
        .ok()
        .as_deref()
        == Some("1");
    decide_launch_with_auto_boot(cfg, auto_boot)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum BootState {
    #[default]
    Idle,
    Starting,
    Ready,
    Failed,
}

fn should_begin_boot(state: BootState) -> bool {
    matches!(state, BootState::Idle | BootState::Failed)
}

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) proxy: Option<Child>,
    pub(crate) proxy_port: u16,
    pub(crate) secret: String,
    /// 当前代理进程所用 adapter 名（deepseek | qwen | relay | openai-custom | openai-responses）；用于健康复用判定。
    pub(crate) provider: String,
    /// 当前代理进程的 gateway 实现身份（生产值固定为 rust）。
    pub(crate) gateway_kind: String,
    /// 当前代理进程的 DeepSeek tool-use shim 模式（off | detect | rewrite）。
    pub(crate) shim_mode: String,
    /// Tauri 每次启动 managed Rust gateway 时生成的唯一实例身份。
    pub(crate) launch_id: String,
    /// 当前代理进程所用 key 的非加密指纹（仅内存、绝不落盘/打印）。
    /// 换 key/换上游后指纹变化 → 触发重启，避免复用带旧配置的代理。
    pub(crate) key_fp: u64,
    /// The exact in-memory launch recipe for the currently tracked Gateway.
    /// It may contain profile credentials, so it is never serialized or logged.
    pub(crate) gateway_launch_context: Option<GatewayLaunchContext>,
    pub(crate) sandbox: Option<Child>,
    pub(crate) sandbox_port: u16,
    pub(crate) sandbox_url: Option<String>,
    /// 当前 CSSwitch Science daemon 的实际 binary 身份，仅存内存；绝不形成版本偏好。
    pub(crate) science_runtime: Option<runtime::science::ScienceRuntimeIdentity>,
    /// CSSwitch 自己成功停止后的单次快速启动令牌；下一次启动消费，App 重启即丢弃。
    pub(crate) science_confirmed_stopped: Option<runtime::science::ScienceRuntimeIdentity>,
    /// Science 版本探测缓存与 daemon 身份分离；停止 daemon 后仍可复用未变化二进制的版本。
    pub(crate) science_version_cache: runtime::science::ScienceVersionCache,
    /// One-shot, backend-only mapping for the v0.8.0 multi-history recovery UI.
    /// Neither organization UUIDs nor filesystem paths cross the invoke boundary.
    pub(crate) history_recovery: Option<HistoryRecoverySession>,
    /// Exact private rollback roots whose cleanup previously failed. Paths are
    /// backend-only and retried before the next one-click mutation.
    pub(crate) pending_authority_cleanup: Vec<std::path::PathBuf>,
    boot: BootState,
    /// Structured one-click failure DTO (`action/stage/status/message/...`) or a
    /// legacy plain-message object; never used for stage inference from text.
    pub(crate) boot_error: Option<serde_json::Value>,
    pub(crate) boot_attention: Option<serde_json::Value>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct GatewayLaunchContext {
    pub(crate) profile: config::Profile,
    pub(crate) science_runtime: Option<runtime::science::ScienceRuntimeIdentity>,
}

#[derive(Clone)]
pub(crate) struct HistoryRecoverySession {
    pub(crate) active_profile_id: String,
    pub(crate) sandbox_port: u16,
    pub(crate) auth_dir: std::path::PathBuf,
    pub(crate) sandbox_root: std::path::PathBuf,
    pub(crate) choices: Vec<HistoryRecoveryChoice>,
}

#[derive(Clone)]
pub(crate) struct HistoryRecoveryChoice {
    pub(crate) reference: String,
    pub(crate) candidate: oauth_forge::HistoryOrgCandidate,
}

impl AppState {
    pub(crate) fn clear_proxy_identity(&mut self) {
        self.secret.clear();
        self.provider.clear();
        self.gateway_kind.clear();
        self.shim_mode.clear();
        self.launch_id.clear();
        self.key_fp = 0;
        self.gateway_launch_context = None;
    }

    pub(crate) fn stop_proxy(&mut self) {
        kill_child(&mut self.proxy);
        self.clear_proxy_identity();
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        // `std::process::Child` does not kill on drop. Keep a final owned-child
        // safety net in addition to the Tauri exit events so a graceful app
        // teardown cannot orphan the managed gateway.
        self.stop_proxy();
    }
}

pub(crate) type SharedAppState = Arc<Mutex<AppState>>;
pub(crate) type SharedLifecycle = Arc<lifecycle::Lifecycle>;

/// 取锁并从 poison 中恢复：某线程持锁时 panic 不应把整个 app 卡死。
pub(crate) fn lock(m: &Mutex<AppState>) -> std::sync::MutexGuard<'_, AppState> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) async fn run_blocking<T>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("后台任务失败：{e}"))?
}

pub(crate) async fn run_blocking_typed<T, E>(
    f: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, E>
where
    T: Send + 'static,
    E: From<String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| E::from(format!("后台任务失败：{e}")))?
}

fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn install_menu(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};

    let preferences = MenuItemBuilder::with_id("preferences", "偏好设置...")
        .accelerator("CmdOrCtrl+,")
        .build(app)?;
    let app_menu = SubmenuBuilder::new(app, "CSSwitch")
        .item(&preferences)
        .separator()
        .quit()
        .build()?;
    let menu_builder = MenuBuilder::new(app).item(&app_menu);
    #[cfg(target_os = "macos")]
    let menu_builder = {
        // Native predefined edit items are what wires the standard macOS
        // Command-X/C/V/A/Z shortcuts into the focused WebView field.
        let edit_menu = SubmenuBuilder::new(app, "编辑")
            .undo_with_text("撤销")
            .redo_with_text("重做")
            .separator()
            .cut_with_text("剪切")
            .copy_with_text("复制")
            .paste_with_text("粘贴")
            .select_all_with_text("全选")
            .build()?;
        menu_builder.item(&edit_menu)
    };
    let menu = menu_builder.build()?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id().as_ref() == "preferences" {
            show_main_window(app);
        }
    });
    Ok(())
}

fn cleanup_for_exit_with<
    R,
    Cancel,
    Wait,
    Term,
    Kill,
    StopScience,
    StopGateway,
    StopValue,
    StopError,
>(
    app: &tauri::AppHandle<R>,
    mut cancel_codex: Cancel,
    mut wait_codex: Wait,
    mut term_codex: Term,
    mut kill_codex: Kill,
    mut stop_science: StopScience,
    mut stop_gateway: StopGateway,
) where
    R: tauri::Runtime,
    Cancel: FnMut(),
    Wait: FnMut(std::time::Duration) -> Vec<u32>,
    Term: FnMut(u32),
    Kill: FnMut(u32),
    StopScience: FnMut(
        &tauri::AppHandle<R>,
        &mut AppState,
        &runtime::science::ScienceRuntimeIdentity,
    ) -> Result<StopValue, StopError>,
    StopGateway: FnMut(&mut AppState),
{
    // First give login its protocol-level cancel path and read-only preflight
    // its cancellation token. Do not signal a possibly committing login child
    // until the bounded waiter has had a chance to reap it normally.
    cancel_codex();
    let remaining = wait_codex(std::time::Duration::from_secs(2));
    for pid in remaining {
        term_codex(pid);
    }
    let remaining = wait_codex(std::time::Duration::from_millis(500));
    for pid in remaining {
        kill_codex(pid);
    }
    let _ = wait_codex(std::time::Duration::from_millis(500));
    let state = app.state::<SharedAppState>().inner().clone();
    let lifecycle = app.state::<SharedLifecycle>().inner().clone();
    lifecycle.with_serialized(|| {
        let mut st = lock(&state);
        if let Some(runtime) = st.science_runtime.clone() {
            let stop_result = stop_science(app, &mut st, &runtime);
            if stop_result.is_ok() {
                st.science_runtime = None;
            }
        }
        stop_gateway(&mut st);
    });
}

fn cleanup_for_exit<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let supervisor = app.state::<SharedCodexAuthSupervisor>().inner().clone();
    cleanup_for_exit_with(
        app,
        || {
            let _ = supervisor.cancel_for_exit();
        },
        |timeout| supervisor.wait_for_auth_children_exit(timeout),
        |pid| {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
        },
        |pid| {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        },
        |app, st, runtime| {
            stop_sandbox(
                app,
                &mut st.sandbox,
                &mut st.sandbox_url,
                ScienceStopRequest::recover(Some(runtime)),
            )
        },
        AppState::stop_proxy,
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeExitEvent {
    ExitRequested,
    Exit,
}

type NativeExitCleanup<R> = fn(&tauri::AppHandle<R>);

fn production_native_exit_cleanup<R: tauri::Runtime>() -> NativeExitCleanup<R> {
    cleanup_for_exit::<R>
}

fn run_native_exit_event_with<R, Cleanup>(
    app: &tauri::AppHandle<R>,
    event: NativeExitEvent,
    cleanup: Cleanup,
) where
    R: tauri::Runtime,
    Cleanup: FnOnce(&tauri::AppHandle<R>, NativeExitEvent),
{
    cleanup(app, event);
}

fn run_native_exit_event<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: NativeExitEvent) {
    let cleanup = production_native_exit_cleanup();
    run_native_exit_event_with(app, event, |app, _| cleanup(app));
}

fn mark_boot_failed<R: tauri::Runtime>(app: &tauri::AppHandle<R>, failure: serde_json::Value) {
    let state = app.state::<SharedAppState>();
    {
        let mut st = lock(state.inner());
        st.boot = BootState::Failed;
        st.boot_error = Some(failure.clone());
        st.boot_attention = None;
    }
    show_main_window(app);
    let _ = app.emit("boot://failed", failure);
}

fn mark_boot_attention<R: tauri::Runtime>(app: &tauri::AppHandle<R>, value: serde_json::Value) {
    let state = app.state::<SharedAppState>();
    {
        let mut st = lock(state.inner());
        st.boot = BootState::Idle;
        st.boot_error = None;
        st.boot_attention = Some(value.clone());
    }
    show_main_window(app);
    let _ = app.emit("boot://attention", value);
}

fn boot_result_error(value: &serde_json::Value) -> Option<serde_json::Value> {
    (value.get("status").and_then(serde_json::Value::as_str) == Some("error")).then(|| {
        // Preserve the full failed one-click DTO so auto-boot matches manual invoke.
        value.clone()
    })
}

fn boot_prepare_failure(message: impl Into<String>) -> serde_json::Value {
    crate::runtime::failure::TypedOneClickFailure::new(
        crate::runtime::failure::OneClickFailureKind::Prepare,
        message,
    )
    .project_dto()
}

fn load_boot_config(dir: &std::path::Path) -> Result<config::Config, serde_json::Value> {
    config::load_from(dir).map_err(|error| boot_prepare_failure(format!("读取配置失败：{error}")))
}

fn run_startup_config_sequence<W, B>(dir: &std::path::Path, install_window_hook: W, boot: B)
where
    W: FnOnce(),
    B: FnOnce(),
{
    // Setup migration is intentionally best-effort; boot performs a fresh load
    // and owns the user-visible typed prepare failure.
    let _ = config::load_from(dir);
    install_window_hook();
    boot();
}

fn boot_result_needs_attention(value: &serde_json::Value) -> bool {
    value.get("status").and_then(serde_json::Value::as_str) == Some("attention")
}

type BootScienceCommand<R> =
    fn(
        tauri::AppHandle<R>,
        SharedAppState,
        SharedLifecycle,
        Option<String>,
    ) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError>;

fn production_boot_science_command<R: tauri::Runtime>() -> BootScienceCommand<R> {
    commands::runtime::one_click_login_cmd::<R>
}

fn run_boot_decision_with<R, Load, Decide, Open, Boot, Show>(
    app: tauri::AppHandle<R>,
    load_config: Load,
    decide: Decide,
    open_official: Open,
    boot_science: Boot,
    show: Show,
) where
    R: tauri::Runtime,
    Load: FnOnce() -> Result<config::Config, serde_json::Value>,
    Decide: FnOnce(&config::Config) -> LaunchPath,
    Open: FnOnce() -> Result<(), String>,
    Boot: FnOnce(
        tauri::AppHandle<R>,
        SharedAppState,
        SharedLifecycle,
        Option<String>,
    ) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError>,
    Show: Fn(&tauri::AppHandle<R>),
{
    let cfg = match load_config() {
        Ok(cfg) => cfg,
        Err(failure) => {
            mark_boot_failed(&app, failure);
            return;
        }
    };
    let state = app.state::<SharedAppState>();
    match decide(&cfg) {
        LaunchPath::ShowPanel => {
            let mut st = lock(state.inner());
            st.boot = BootState::Idle;
            st.boot_error = None;
            st.boot_attention = None;
            show(&app);
        }
        LaunchPath::OpenOfficial => match open_official() {
            Ok(()) => {
                let mut st = lock(state.inner());
                st.boot = BootState::Idle;
                st.boot_error = None;
                st.boot_attention = None;
            }
            Err(e) => mark_boot_failed(&app, boot_prepare_failure(e)),
        },
        LaunchPath::BootScience => {
            let state_inner = state.inner().clone();
            let lifecycle = app.state::<SharedLifecycle>().inner().clone();
            match boot_science(app.clone(), state_inner, lifecycle, None) {
                Ok(value) => {
                    if boot_result_needs_attention(&value) {
                        mark_boot_attention(&app, value);
                    } else if let Some(failure) = boot_result_error(&value) {
                        mark_boot_failed(&app, failure);
                    } else {
                        let mut st = lock(state.inner());
                        st.boot = BootState::Ready;
                        st.boot_error = None;
                        st.boot_attention = None;
                    }
                }
                Err(e) => mark_boot_failed(&app, boot_prepare_failure(e.to_string())),
            }
        }
    }
}

fn run_boot_decision(app: tauri::AppHandle) {
    run_boot_decision_with(
        app,
        || load_boot_config(&config::default_dir()),
        decide_launch,
        commands::runtime::open_official,
        production_boot_science_command(),
        show_main_window,
    );
}

fn run_boot_coordinator_with<R, Execute, Show>(
    app: tauri::AppHandle<R>,
    execute: Execute,
    show: Show,
) where
    R: tauri::Runtime,
    Execute: FnOnce(tauri::AppHandle<R>),
    Show: FnOnce(&tauri::AppHandle<R>),
{
    {
        let state = app.state::<SharedAppState>();
        let mut st = lock(state.inner());
        if !should_begin_boot(st.boot) {
            show(&app);
            return;
        }
        st.boot = BootState::Starting;
    }

    execute(app);
}

fn run_boot_coordinator(app: tauri::AppHandle) {
    run_boot_coordinator_with(
        app,
        |app| {
            tauri::async_runtime::spawn_blocking(move || run_boot_decision(app));
        },
        show_main_window,
    );
}

fn run_second_instance_callback_with<R, Execute, Show>(
    app: &tauri::AppHandle<R>,
    execute: Execute,
    show: Show,
) where
    R: tauri::Runtime,
    Execute: FnOnce(tauri::AppHandle<R>),
    Show: FnOnce(&tauri::AppHandle<R>),
{
    run_boot_coordinator_with(app.clone(), execute, show);
}

fn run_second_instance_callback(app: &tauri::AppHandle) {
    run_second_instance_callback_with(
        app,
        |app| {
            tauri::async_runtime::spawn_blocking(move || run_boot_decision(app));
        },
        show_main_window,
    );
}

// ---------- 入口 ----------
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            run_second_instance_callback(app);
        }))
        .manage(Arc::new(Mutex::new(AppState::default())))
        .manage(Arc::new(lifecycle::Lifecycle::new()))
        .manage(Arc::new(CodexAuthSupervisor::default()))
        .invoke_handler(tauri::generate_handler![
            commands::codex::set_experimental_codex_enabled,
            commands::codex::set_codex_network,
            commands::codex::codex_auth_status,
            commands::codex::codex_auth_start,
            commands::codex::codex_auth_cancel,
            commands::codex::codex_auth_operation_status,
            commands::codex::codex_ensure_profile,
            commands::codex::codex_auth_logout,
            commands::codex::codex_downgrade_preview,
            commands::codex::codex_downgrade_export_all,
            commands::profiles::get_config,
            commands::profiles::list_templates,
            commands::runtime::set_settings,
            commands::runtime::set_mode,
            commands::runtime::open_official,
            commands::profiles::create_profile,
            commands::profiles::update_profile_metadata,
            commands::profiles::update_profile_connection,
            commands::profiles::validate_profile_catalog_model,
            commands::profiles::preview_profile_preset_sync,
            commands::profiles::apply_profile_preset_sync,
            commands::profiles::clear_profile_key,
            commands::profiles::delete_profile,
            commands::profiles::set_active_profile,
            commands::runtime::start_proxy,
            commands::runtime::fetch_models,
            commands::runtime::stop_all,
            commands::runtime::one_click_login,
            commands::runtime::restore_history_choice,
            commands::runtime::science_runtime_preflight,
            commands::runtime::open_science_download_page,
            commands::runtime::status,
            commands::runtime::boot_error,
            commands::runtime::boot_attention,
            commands::runtime::open_url,
            commands::skill_install::install_local_skill_package,
            commands::skill_listing::list_installed_skills,
            commands::diagnostics::run_doctor,
            commands::diagnostics::app_version,
            commands::diagnostics::open_release_page,
            commands::diagnostics::report_bug,
            commands::diagnostics::open_logs,
            commands::runtime::quit_app
        ])
        .setup(|app| {
            install_menu(app)?;

            let config_dir = config::default_dir();
            run_startup_config_sequence(
                &config_dir,
                || {
                    // 关窗隐藏配置面板，不销毁窗口、不停止后台链路。显式退出清理代理与沙箱。
                    if let Some(win) = app.get_webview_window("main") {
                        let w = win.clone();
                        win.on_window_event(move |ev| {
                            if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                                api.prevent_close();
                                let _ = w.hide();
                            }
                        });
                    }
                },
                || run_boot_coordinator(app.handle().clone()),
            );
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app, event| match event {
        tauri::RunEvent::Reopen { .. } => show_main_window(app),
        tauri::RunEvent::ExitRequested { .. } => {
            run_native_exit_event(app, NativeExitEvent::ExitRequested)
        }
        tauri::RunEvent::Exit => run_native_exit_event(app, NativeExitEvent::Exit),
        _ => {}
    });
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        env, fs,
        net::{TcpListener, TcpStream},
        os::unix::fs::PermissionsExt,
        process::Command,
        sync::{Arc, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::config::{self, Config, Profile};
    use crate::runtime::system::redact;
    use crate::{
        boot_result_error, boot_result_needs_attention, cleanup_for_exit, cleanup_for_exit_with,
        decide_launch_with_auto_boot, load_boot_config, lock, production_boot_science_command,
        production_native_exit_cleanup, run_boot_decision_with, run_native_exit_event_with,
        run_second_instance_callback_with, run_startup_config_sequence, should_begin_boot,
        AppState, BootScienceCommand, BootState, LaunchPath, NativeExitCleanup, NativeExitEvent,
        SharedAppState, SharedLifecycle,
    };

    #[test]
    fn auto_boot_rejects_structured_runtime_failure() {
        let failed = serde_json::json!({
            "action": "failed",
            "stage": "gateway_start",
            "status": "error",
            "recovery_status": "degraded",
            "environment_status": "not_exposed",
            "message": "gateway recovery degraded",
            "fallback_url": null,
        });
        let projected = boot_result_error(&failed).expect("error dto");
        assert_eq!(projected["message"], "gateway recovery degraded");
        assert_eq!(projected["stage"], "gateway_start");
        assert_eq!(projected["recovery_status"], "degraded");
        assert!(boot_result_error(&serde_json::json!({"status": "ok"})).is_none());
        assert!(boot_result_needs_attention(
            &serde_json::json!({"status": "attention", "action": "history_choice_required"})
        ));
        assert!(!boot_result_needs_attention(
            &serde_json::json!({"status": "ok"})
        ));
    }

    #[test]
    fn r0_boot_surfaces_prepare_failure_after_setup_load_error() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!(
            "csswitch-r0-g-startup-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.json"), b"{invalid-config").unwrap();

        let events = std::cell::RefCell::new(Vec::new());
        let failure = std::cell::RefCell::new(None);
        run_startup_config_sequence(
            &dir,
            || events.borrow_mut().push("window-hook"),
            || {
                events.borrow_mut().push("boot-load");
                failure.replace(Some(load_boot_config(&dir).unwrap_err()));
            },
        );
        assert_eq!(*events.borrow(), vec!["window-hook", "boot-load"]);
        let failure = failure.borrow_mut().take().unwrap();
        assert_eq!(failure["status"], "error");
        assert_eq!(failure["stage"], "prepare");
        assert!(failure["message"]
            .as_str()
            .unwrap()
            .contains("读取配置失败"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn app_state_clear_proxy_identity_removes_runtime_credentials() {
        let mut st = AppState::default();
        st.secret = "secret".into();
        st.provider = "deepseek".into();
        st.gateway_kind = "rust".into();
        st.shim_mode = "off".into();
        st.launch_id = "launch-old".into();
        st.key_fp = 42;
        st.clear_proxy_identity();
        assert!(st.secret.is_empty());
        assert!(st.provider.is_empty());
        assert!(st.gateway_kind.is_empty());
        assert!(st.shim_mode.is_empty());
        assert!(st.launch_id.is_empty());
        assert_eq!(st.key_fp, 0);
    }

    #[test]
    fn r0_app_state_drop_stops_tracked_gateway() {
        let child = Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("spawn owned test child");
        let pid = child.id();
        {
            let mut st = AppState::default();
            st.proxy = Some(child);
        }
        let status = Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()
            .expect("inspect owned test child");
        assert!(
            String::from_utf8_lossy(&status.stdout).trim().is_empty(),
            "AppState drop left owned proxy child {pid} alive"
        );
    }

    #[test]
    fn r0_native_exit_events_share_repeatable_best_effort_cleanup() {
        let child_name =
            "tests::isolated_r0_native_exit_events_share_repeatable_best_effort_cleanup";
        let output = Command::new(env::current_exe().unwrap())
            .arg("--exact")
            .arg(child_name)
            .arg("--ignored")
            .arg("--nocapture")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success()
                && stdout.lines().any(|line| line == "running 1 test")
                && stdout
                    .lines()
                    .any(|line| line == format!("test {child_name} ... ok")),
            "isolated native-exit characterization failed:\nstdout={}\nstderr={}",
            stdout,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn r0_native_exit_requested_and_exit_repeat_full_terminal_cleanup() {
        type MockNativeExitCleanup = NativeExitCleanup<tauri::test::MockRuntime>;
        let production: MockNativeExitCleanup = production_native_exit_cleanup();
        let expected: MockNativeExitCleanup = cleanup_for_exit::<tauri::test::MockRuntime>;
        assert_eq!(
            production as usize, expected as usize,
            "native exit events must remain bound to cleanup_for_exit"
        );

        let gateway = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let gateway_pid = gateway.id();
        let runtime = crate::runtime::science::test_runtime_identity(env::current_exe().unwrap());
        let mut authority = AppState::default();
        authority.proxy = Some(gateway);
        authority.science_runtime = Some(runtime.clone());
        let state: SharedAppState = Arc::new(Mutex::new(authority));
        let lifecycle: SharedLifecycle = Arc::new(crate::lifecycle::Lifecycle::new());
        let generation = lifecycle.current_generation();
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .manage(lifecycle.clone())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();

        let actions = RefCell::new(Vec::new());
        let science_attempts = Cell::new(0);
        for event in [NativeExitEvent::ExitRequested, NativeExitEvent::Exit] {
            let term_child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            let term_pid = term_child.id();
            let kill_child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            let kill_pid = kill_child.id();
            let children = RefCell::new(vec![term_child, kill_child]);
            let wait_index = Cell::new(0);

            run_native_exit_event_with(app.handle(), event, |app, observed_event| {
                assert_eq!(observed_event, event);
                actions.borrow_mut().push(format!("event:{event:?}"));
                cleanup_for_exit_with(
                    app,
                    || actions.borrow_mut().push("codex:cancel".into()),
                    |timeout| {
                        actions
                            .borrow_mut()
                            .push(format!("codex:wait:{}", timeout.as_millis()));
                        let index = wait_index.get();
                        wait_index.set(index + 1);
                        match index {
                            0 => vec![term_pid],
                            1 => vec![kill_pid],
                            2 => {
                                for child in children.borrow_mut().iter_mut() {
                                    child.wait().unwrap();
                                }
                                Vec::new()
                            }
                            _ => panic!("unexpected Codex wait round: {index}"),
                        }
                    },
                    |pid| {
                        actions.borrow_mut().push("codex:term".into());
                        assert_eq!(pid, term_pid);
                        assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGTERM) }, 0);
                    },
                    |pid| {
                        actions.borrow_mut().push("codex:kill".into());
                        assert_eq!(pid, kill_pid);
                        assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
                    },
                    |_, _, observed_runtime| {
                        assert_eq!(observed_runtime, &runtime);
                        let attempt = science_attempts.get() + 1;
                        science_attempts.set(attempt);
                        if attempt == 1 {
                            actions.borrow_mut().push("science:error".into());
                            Err("controlled first Science stop failure".to_string())
                        } else {
                            actions.borrow_mut().push("science:stopped".into());
                            Ok(())
                        }
                    },
                    |app_state| {
                        actions.borrow_mut().push("gateway:stop".into());
                        app_state.stop_proxy();
                    },
                );
                actions.borrow_mut().push(format!("completed:{event:?}"));
            });

            assert_eq!(wait_index.get(), 3);
            assert_eq!(lifecycle.current_generation(), generation);
            let current = lock(&state);
            assert!(current.proxy.is_none());
            if event == NativeExitEvent::ExitRequested {
                assert_eq!(current.science_runtime.as_ref(), Some(&runtime));
            } else {
                assert!(current.science_runtime.is_none());
            }
        }

        assert!(unsafe { libc::kill(gateway_pid as i32, 0) } != 0);
        assert_eq!(science_attempts.get(), 2);
        assert_eq!(lifecycle.current_generation(), generation);
        assert_eq!(
            *actions.borrow(),
            vec![
                "event:ExitRequested",
                "codex:cancel",
                "codex:wait:2000",
                "codex:term",
                "codex:wait:500",
                "codex:kill",
                "codex:wait:500",
                "science:error",
                "gateway:stop",
                "completed:ExitRequested",
                "event:Exit",
                "codex:cancel",
                "codex:wait:2000",
                "codex:term",
                "codex:wait:500",
                "codex:kill",
                "codex:wait:500",
                "science:stopped",
                "gateway:stop",
                "completed:Exit",
            ]
        );
    }

    #[test]
    #[ignore = "source-gate parent executes exact isolated native-exit cleanup with temp HOME, fake process identity, and a dynamic loopback port"]
    fn isolated_r0_native_exit_events_share_repeatable_best_effort_cleanup() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "csswitch-r0-d-native-exit-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let home = root.join("home");
        fs::create_dir_all(&home).unwrap();
        env::set_var("HOME", &home);
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let sandbox_port = listener.local_addr().unwrap().port();
        assert_ne!(sandbox_port, 8765);
        let proxy_port = TcpListener::bind(("127.0.0.1", 0))
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert_ne!(proxy_port, 8765);
        let config_dir = config::default_dir();
        fs::create_dir_all(&config_dir).unwrap();
        let mut cfg = Config::default();
        cfg.sandbox_port = sandbox_port;
        cfg.proxy_port = proxy_port;
        config::save_to(&config_dir, &cfg).unwrap();

        let fake_science = root.join("fake-science");
        fs::write(&fake_science, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&fake_science, fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = crate::runtime::science::test_runtime_identity(fake_science);
        let first_proxy = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let first_pid = first_proxy.id();
        let mut authority = AppState::default();
        authority.proxy = Some(first_proxy);
        authority.science_runtime = Some(runtime.clone());
        let state: SharedAppState = Arc::new(Mutex::new(authority));
        let lifecycle: SharedLifecycle = Arc::new(crate::lifecycle::Lifecycle::new());
        let supervisor = Arc::new(crate::codex_auth_supervisor::CodexAuthSupervisor::default());
        let app = tauri::test::mock_builder()
            .manage(state.clone())
            .manage(lifecycle.clone())
            .manage(supervisor)
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let generation = lifecycle.current_generation();

        cleanup_for_exit(app.handle());
        assert!(lock(&state).proxy.is_none());
        assert!(unsafe { libc::kill(first_pid as i32, 0) } != 0);
        assert_eq!(lock(&state).science_runtime.as_ref(), Some(&runtime));
        assert_eq!(lifecycle.current_generation(), generation);
        assert!(TcpStream::connect(("127.0.0.1", sandbox_port)).is_ok());

        let second_proxy = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let second_pid = second_proxy.id();
        lock(&state).proxy = Some(second_proxy);
        cleanup_for_exit(app.handle());
        assert!(lock(&state).proxy.is_none());
        assert!(unsafe { libc::kill(second_pid as i32, 0) } != 0);
        assert_eq!(lock(&state).science_runtime.as_ref(), Some(&runtime));
        assert_eq!(lifecycle.current_generation(), generation);
        assert!(TcpStream::connect(("127.0.0.1", sandbox_port)).is_ok());

        drop(app);
        drop(listener);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn redact_scrubs_secret_and_is_noop_when_empty() {
        assert_eq!(
            redact("推理指向 http://127.0.0.1:18991/abcd1234 尾巴", "abcd1234"),
            "推理指向 http://127.0.0.1:18991/**** 尾巴"
        );
        assert_eq!(redact("原样返回", ""), "原样返回");
        assert!(!redact("leak abcd1234 leak abcd1234", "abcd1234").contains("abcd1234"));
    }

    fn keyed_profile(id: &str, key: &str) -> Profile {
        let (model_catalog, default_model_route_id, role_bindings) =
            crate::model_catalog::new_profile_catalog("deepseek", "anthropic", None).unwrap();
        let model = model_catalog
            .iter()
            .find(|route| route.selector_id == default_model_route_id)
            .unwrap()
            .upstream_model
            .clone();
        Profile {
            id: id.into(),
            name: id.into(),
            template_id: "deepseek".into(),
            category: "cn_official".into(),
            api_format: "anthropic".into(),
            api_key: key.into(),
            model,
            model_catalog,
            default_model_route_id,
            role_bindings,
            model_policy: crate::provider_contracts::ModelPolicy::SavedCatalog,
            ..Default::default()
        }
    }

    fn codex_profile(id: &str) -> Profile {
        Profile {
            id: id.into(),
            name: id.into(),
            template_id: "codex".into(),
            category: "experimental".into(),
            api_format: "openai_responses".into(),
            credential_source: crate::provider_contracts::CredentialSource::CsswitchOauth,
            credential_ref: Some("csswitch:codex:default".into()),
            model_policy: crate::provider_contracts::ModelPolicy::DynamicCatalog,
            ..Default::default()
        }
    }

    #[test]
    fn decide_launch_defaults_to_showing_panel() {
        let active_with_key = Config {
            profiles: vec![keyed_profile("p1", "sk-present")],
            active_id: "p1".into(),
            ..Default::default()
        };
        assert_eq!(
            decide_launch_with_auto_boot(&active_with_key, false),
            LaunchPath::ShowPanel
        );
    }

    #[test]
    fn decide_launch_auto_boot_uses_current_mode_and_active_profile_key() {
        let official = Config {
            mode: "official".into(),
            ..Default::default()
        };
        assert_eq!(
            decide_launch_with_auto_boot(&official, true),
            LaunchPath::OpenOfficial
        );

        let no_active = Config {
            profiles: vec![keyed_profile("p1", "sk-present")],
            active_id: String::new(),
            ..Default::default()
        };
        assert_eq!(
            decide_launch_with_auto_boot(&no_active, true),
            LaunchPath::ShowPanel
        );

        let active_without_key = Config {
            profiles: vec![keyed_profile("p1", "")],
            active_id: "p1".into(),
            ..Default::default()
        };
        assert_eq!(
            decide_launch_with_auto_boot(&active_without_key, true),
            LaunchPath::ShowPanel
        );

        let active_with_key = Config {
            profiles: vec![keyed_profile("p1", "sk-present")],
            active_id: "p1".into(),
            ..Default::default()
        };
        assert_eq!(
            decide_launch_with_auto_boot(&active_with_key, true),
            LaunchPath::BootScience
        );

        let dangling_active = Config {
            profiles: vec![keyed_profile("p1", "sk-present")],
            active_id: "missing".into(),
            ..Default::default()
        };
        assert_eq!(
            decide_launch_with_auto_boot(&dangling_active, true),
            LaunchPath::ShowPanel
        );

        let codex_disabled = Config {
            profiles: vec![codex_profile("codex-1")],
            active_id: "codex-1".into(),
            ..Default::default()
        };
        assert_eq!(
            decide_launch_with_auto_boot(&codex_disabled, true),
            LaunchPath::ShowPanel
        );
        let codex_enabled = Config {
            experimental_codex_enabled: true,
            ..codex_disabled
        };
        assert_eq!(
            decide_launch_with_auto_boot(&codex_enabled, true),
            LaunchPath::ShowPanel
        );
    }

    #[test]
    fn should_begin_boot_only_from_idle_or_failed() {
        assert!(should_begin_boot(BootState::Idle));
        assert!(should_begin_boot(BootState::Failed));
        assert!(!should_begin_boot(BootState::Starting));
        assert!(!should_begin_boot(BootState::Ready));
    }

    #[test]
    fn r0_second_instance_callback_boot_state_matrix_reuses_one_click_login_cmd() {
        type MockBootScienceCommand = BootScienceCommand<tauri::test::MockRuntime>;
        let production: MockBootScienceCommand = production_boot_science_command();
        let expected: MockBootScienceCommand =
            crate::commands::runtime::one_click_login_cmd::<tauri::test::MockRuntime>;
        assert_eq!(
            production as usize, expected as usize,
            "the production BootScience binding must remain the manual one-click command"
        );

        let choices = RefCell::new(Vec::new());
        for initial in [
            BootState::Idle,
            BootState::Failed,
            BootState::Starting,
            BootState::Ready,
        ] {
            let mut authority = AppState::default();
            authority.boot = initial;
            authority.boot_error = Some(serde_json::json!({"sentinel": "error"}));
            authority.boot_attention = Some(serde_json::json!({"sentinel": "attention"}));
            let state: SharedAppState = Arc::new(Mutex::new(authority));
            let lifecycle: SharedLifecycle = Arc::new(crate::lifecycle::Lifecycle::new());
            let app = tauri::test::mock_builder()
                .manage(state.clone())
                .manage(lifecycle)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .unwrap();

            let execute_count = Cell::new(0);
            let load_count = Cell::new(0);
            let decision_count = Cell::new(0);
            let command_count = Cell::new(0);
            let show_count = Cell::new(0);
            run_second_instance_callback_with(
                app.handle(),
                |app| {
                    execute_count.set(execute_count.get() + 1);
                    assert_eq!(lock(&state).boot, BootState::Starting);
                    run_boot_decision_with(
                        app,
                        || {
                            load_count.set(load_count.get() + 1);
                            Ok(Config::default())
                        },
                        |_| {
                            decision_count.set(decision_count.get() + 1);
                            LaunchPath::BootScience
                        },
                        || -> Result<(), String> {
                            panic!("BootScience must not call the official launch action")
                        },
                        |_,
                         _,
                         _,
                         runtime_choice|
                         -> Result<
                            serde_json::Value,
                            crate::commands::codex::RuntimeCommandError,
                        > {
                            command_count.set(command_count.get() + 1);
                            choices.borrow_mut().push(runtime_choice);
                            Ok(serde_json::json!({"status": "ok"}))
                        },
                        |_| show_count.set(show_count.get() + 1),
                    );
                },
                |_| show_count.set(show_count.get() + 1),
            );

            let authority = lock(&state);
            if matches!(initial, BootState::Idle | BootState::Failed) {
                assert_eq!(execute_count.get(), 1, "initial state: {initial:?}");
                assert_eq!(load_count.get(), 1, "initial state: {initial:?}");
                assert_eq!(decision_count.get(), 1, "initial state: {initial:?}");
                assert_eq!(command_count.get(), 1, "initial state: {initial:?}");
                assert_eq!(show_count.get(), 0, "initial state: {initial:?}");
                assert_eq!(authority.boot, BootState::Ready);
                assert!(authority.boot_error.is_none());
                assert!(authority.boot_attention.is_none());
            } else {
                assert_eq!(execute_count.get(), 0, "initial state: {initial:?}");
                assert_eq!(load_count.get(), 0, "initial state: {initial:?}");
                assert_eq!(decision_count.get(), 0, "initial state: {initial:?}");
                assert_eq!(command_count.get(), 0, "initial state: {initial:?}");
                assert_eq!(show_count.get(), 1, "initial state: {initial:?}");
                assert_eq!(authority.boot, initial);
                assert_eq!(
                    authority.boot_error,
                    Some(serde_json::json!({"sentinel": "error"}))
                );
                assert_eq!(
                    authority.boot_attention,
                    Some(serde_json::json!({"sentinel": "attention"}))
                );
            }
        }
        assert_eq!(
            *choices.borrow(),
            vec![None, None],
            "Idle and Failed callbacks must both pass no runtime choice"
        );
    }
}
