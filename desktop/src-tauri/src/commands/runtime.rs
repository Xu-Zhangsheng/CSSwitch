use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::State;

use crate::lifecycle::RuntimeMutationDomain;
use crate::runtime::capability_catalog::diagnostics_for_profile;
use crate::runtime::diagnostics::{
    build_status_response, proxy_status_last_error, science_diagnostics, status_lights,
    ScienceDiagnosticsInput, StatusProbeInput,
};
use crate::runtime::failure::{OneClickFailureKind, ProjectedRecovery, TypedOneClickFailure};
use crate::runtime::operation;
use crate::runtime::profile::profile_capabilities;
use crate::runtime::provider::{
    current_shim_mode_for_adapter, gateway_kind_for_adapter, resolve_launch_plan,
    status_upstream_endpoint,
};
use crate::runtime::science::{
    science_runtime_preflight as runtime_preflight, settings_change_needs_teardown,
    SandboxScienceState, ScienceHostAdapter, ScienceStopOwnershipReceipt, ScienceStopRequest,
    SCIENCE_DOWNLOAD_URL,
};
use crate::runtime::settings::{
    remove_managed_sandbox_ssh_stub, system_ssh_config_path, validate_runtime_ports,
};
use crate::runtime::ssh_bridge::{revoke_science_ssh_bridge, system_ssh_hosts};
use crate::runtime::system::open_in_browser;
use crate::{
    config, lock, proc, run_blocking, run_blocking_typed, AppState, SharedAppState, SharedLifecycle,
};

mod actions;
mod gateway;
mod lifecycle;
mod one_click;
mod status;

pub(crate) use gateway::FetchModelsReq;
pub(crate) use lifecycle::{stop_sandbox_state, UiSettings};
pub(crate) use one_click::one_click_login_cmd;

#[tauri::command]
pub(crate) async fn set_mode(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    mode: String,
) -> Result<(), String> {
    lifecycle::set_mode_command(app, state, lifecycle, mode).await
}

/// 官方模式：干净地打开用户【真实】的 Claude Science（不碰/复制真实凭证，抹掉 ANTHROPIC_*）。
#[tauri::command]
pub(crate) fn open_official() -> Result<(), String> {
    actions::open_official_inner()
}

#[tauri::command]
pub(crate) async fn set_settings(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    cfg: UiSettings,
) -> Result<(), String> {
    lifecycle::set_settings_command(app, state, lifecycle, cfg).await
}

#[tauri::command]
pub(crate) async fn fetch_models(
    app: tauri::AppHandle,
    lifecycle: State<'_, SharedLifecycle>,
    req: FetchModelsReq,
) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError> {
    gateway::fetch_models_command(app, lifecycle, req).await
}

#[tauri::command]
pub(crate) async fn stop_all(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
) -> Result<(), String> {
    lifecycle::stop_all_command(app, state, lifecycle).await
}

#[tauri::command]
pub(crate) async fn one_click_login<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    runtime_choice: Option<String>,
) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError> {
    one_click::one_click_login_command(app, state, lifecycle, runtime_choice).await
}

#[tauri::command]
pub(crate) async fn restore_history_choice<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
    reference: String,
) -> Result<serde_json::Value, String> {
    one_click::restore_history_choice_command(app, state, lifecycle, reference).await
}

#[tauri::command]
pub(crate) async fn science_runtime_preflight(
    state: State<'_, SharedAppState>,
) -> Result<Value, String> {
    status::science_runtime_preflight_command(state).await
}

#[tauri::command]
pub(crate) fn open_science_download_page() -> Result<(), String> {
    actions::open_science_download_page_inner()
}

#[tauri::command]
pub(crate) fn status(state: State<'_, SharedAppState>) -> serde_json::Value {
    status::status_inner(state)
}

#[tauri::command]
pub(crate) fn boot_error(state: State<'_, SharedAppState>) -> Option<serde_json::Value> {
    status::boot_error_inner(state)
}

#[tauri::command]
pub(crate) fn boot_attention(state: State<'_, SharedAppState>) -> Option<serde_json::Value> {
    status::boot_attention_inner(state)
}

#[tauri::command]
pub(crate) async fn open_url(
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
) -> Result<serde_json::Value, String> {
    actions::open_url_command(state, lifecycle).await
}

#[tauri::command]
pub(crate) async fn quit_app(
    app: tauri::AppHandle,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
) -> Result<(), String> {
    lifecycle::quit_app_command(app, state, lifecycle).await
}

#[cfg(test)]
use actions::{manual_open_result, open_url_inner};
#[cfg(test)]
use one_click::project_one_click_failure;
#[cfg(test)]
use status::{
    config_last_error_json, status_response_for_config_error, status_runtime_identity,
    status_upstream_applicable,
};

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
