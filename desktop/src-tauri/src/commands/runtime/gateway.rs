use super::*;

pub(super) async fn start_proxy_command<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, SharedAppState>,
    lifecycle: State<'_, SharedLifecycle>,
) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError> {
    let state = state.inner().clone();
    let lifecycle = lifecycle.inner().clone();
    run_blocking_typed(move || start_proxy_inner_cmd(app, state, lifecycle)).await
}

pub(super) fn start_proxy_inner_cmd<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: SharedAppState,
    lifecycle: SharedLifecycle,
) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError> {
    let cfg = config::load_from(&config::default_dir()).map_err(|error| error.to_string())?;
    let active = cfg
        .active_profile()
        .ok_or("未配置生效 profile，请先在面板选择或新建一条配置。")?;
    let adapter = resolve_launch_plan(active)?.adapter;
    let prepared = crate::commands::codex::prepare_provider_auth(
        &app,
        &adapter,
        crate::commands::codex::CodexPreflightTarget::ActiveProfile,
    )?;
    // 经串行器：与切换/连接编辑/清 key/删/停等 ensure_proxy 竞争串行化，防陈旧读起旧配置代理
    // 又写回运行态（修 P1-a，比照 spec §8.1「ensure_proxy 都经一把 app 级 mutex」）。
    lifecycle.with_serialized(|| {
        if let Some(prepared) = prepared.as_ref() {
            prepared.verify_unchanged()?;
        }
        let trace = OperationTrace::start(OperationKind::StartProxy, "command=start_proxy");
        let (port, _secret, _action) = ensure_proxy(
            &app,
            &state,
            lifecycle.as_ref(),
            None,
            Some(&trace),
            prepared.as_ref().map(|prepared| prepared.proof()),
        )?;
        trace.finish(format!("ok port={port}"));
        Ok(json!({ "port": port }))
    })
}

#[derive(Deserialize)]
pub(crate) struct FetchModelsReq {
    /// 模板 id（决定 builtin / base_url 可编辑性 / 默认 base_url）。
    template_id: String,
    /// 编辑已存 profile 时的实际 api_format；为空则按模板默认值。
    #[serde(default)]
    api_format: Option<String>,
    /// 自定义模板时用户填的 base_url（不可编辑模板忽略）。
    #[serde(default)]
    base_url: String,
    /// 用户新填的 key；为空表示沿用 profile_id 已存的 key（后端不回传完整 key）。
    #[serde(default)]
    key: String,
    /// 编辑已存 profile 时传其 id（用于沿用已存 key）。
    #[serde(default)]
    profile_id: Option<String>,
}

/// 「获取可用模型」——纯 scratch 探测：只用临时代理探候选 base_url/key 的 /v1/models，
/// 绝不写 config、不改 AppState、不碰正在服务 Science 的正式代理。
pub(super) async fn fetch_models_command(
    app: tauri::AppHandle,
    lifecycle: State<'_, SharedLifecycle>,
    req: FetchModelsReq,
) -> Result<serde_json::Value, crate::commands::codex::RuntimeCommandError> {
    let lifecycle = lifecycle.inner().clone();
    run_blocking_typed(
        move || -> Result<_, crate::commands::codex::RuntimeCommandError> {
            let request = crate::runtime::model_discovery::ModelDiscoveryRequest {
                template_id: req.template_id,
                api_format: req.api_format,
                base_url: req.base_url,
                key: req.key,
                profile_id: req.profile_id,
            };
            let adapter = crate::runtime::model_discovery::request_adapter(&request)?;
            let target = request.profile_id.as_ref().map_or(
                crate::commands::codex::CodexPreflightTarget::NoProfile,
                |id| crate::commands::codex::CodexPreflightTarget::Profile(id.clone()),
            );
            let prepared = crate::commands::codex::prepare_provider_auth(&app, &adapter, target)?;
            lifecycle
                .with_serialized(|| -> Result<(), String> {
                    if let Some(prepared) = prepared.as_ref() {
                        prepared.verify_unchanged()?;
                    }
                    Ok(())
                })
                .map_err(crate::commands::codex::RuntimeCommandError::from)?;
            Ok(crate::runtime::model_discovery::fetch_models(
                app,
                request,
                prepared.as_ref().map(|prepared| prepared.proof()),
            )?)
        },
    )
    .await
}
