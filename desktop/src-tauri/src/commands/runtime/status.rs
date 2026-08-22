use super::*;

pub(super) fn config_last_error_json(error: &dyn std::fmt::Display) -> serde_json::Value {
    json!({
        "type": "config_error",
        "message": error.to_string(),
    })
}

pub(super) fn status_response_for_config_error(error: &dyn std::fmt::Display) -> serde_json::Value {
    build_status_response(
        status_lights(StatusProbeInput {
            proxy_ok: false,
            sandbox_ok: false,
            upstream_ok: false,
            upstream_applicable: true,
        }),
        serde_json::Value::Null,
        "",
        "off",
        diagnostics_for_profile(None, "off"),
        science_diagnostics(ScienceDiagnosticsInput {
            sandbox_port: 0,
            sandbox_ok: false,
        }),
        Some(config_last_error_json(error)),
    )
}

pub(super) fn status_runtime_identity(
    adapter: &str,
    secret: &str,
    launched_gateway_kind: String,
    launched_shim_mode: String,
) -> (String, String, &'static str) {
    let current_shim_mode = current_shim_mode_for_adapter(adapter);
    let gateway_kind = if !launched_gateway_kind.is_empty() {
        launched_gateway_kind
    } else if !secret.is_empty() {
        String::new()
    } else {
        gateway_kind_for_adapter(adapter).to_string()
    };
    let runtime_shim_mode = if !launched_shim_mode.is_empty() {
        launched_shim_mode
    } else if !secret.is_empty() {
        String::new()
    } else {
        current_shim_mode.to_string()
    };
    (gateway_kind, runtime_shim_mode, current_shim_mode)
}

pub(super) fn status_upstream_applicable(adapter: &str) -> bool {
    !adapter.is_empty() && adapter != "codex"
}

pub(super) async fn science_runtime_preflight_command(
    state: State<'_, SharedAppState>,
) -> Result<Value, String> {
    let (version_cache, confirmed_stopped) = {
        let st = lock(state.inner());
        (
            st.science_version_cache.clone(),
            st.science_confirmed_stopped.clone(),
        )
    };
    run_blocking(move || runtime_preflight(&version_cache, confirmed_stopped.as_ref())).await
}

pub(super) fn status_inner(state: State<'_, SharedAppState>) -> serde_json::Value {
    // 只在锁内取值，锁外做短超时探活。这里是高频 UI 状态灯，
    // 不能反复调用外部 `claude-science status`，否则前端轮询会卡住主线程。
    // 沙箱强身份确认保留在 one_click_login 的启动/复用边界。
    let (
        pport,
        secret,
        sport,
        adapter,
        base_url,
        active_profile,
        catalog_profile,
        tracked_proxy_child_alive,
        launched_provider,
        launched_gateway_kind,
        launched_shim_mode,
        launched_launch_id,
        active_contract_id,
        active_contract_digest,
        science_runtime,
    ) = {
        let mut st = lock(state.inner());
        let cfg = match config::load_from(&config::default_dir()) {
            Ok(cfg) => cfg,
            Err(e) => return status_response_for_config_error(&e),
        };
        let pport = if st.proxy_port != 0 {
            st.proxy_port
        } else {
            cfg.proxy_port
        };
        let sport = if st.sandbox_port != 0 {
            st.sandbox_port
        } else {
            cfg.sandbox_port
        };
        let tracked_proxy_child_alive = proc::tracked_child_is_running(&mut st.proxy);
        // 上游灯读生效 profile 的 adapter/base_url；无生效配置 → 空（灯显黄，不误探）。
        let (
            adapter,
            base_url,
            active_contract_id,
            active_contract_digest,
            active_profile,
            catalog_profile,
        ) = match cfg.active_profile() {
            Some(p) => {
                let (adapter, endpoint, contract_id, contract_digest) = resolve_launch_plan(p)
                    .map(|plan| {
                        (
                            plan.adapter,
                            plan.endpoint,
                            plan.contract_id,
                            plan.contract_digest,
                        )
                    })
                    .unwrap_or_else(|_| {
                        (
                            "unsupported".to_string(),
                            String::new(),
                            String::new(),
                            String::new(),
                        )
                    });
                (
                    adapter,
                    endpoint,
                    contract_id,
                    contract_digest,
                    json!({
                        "id": p.id,
                        "name": p.name,
                        "template_id": p.template_id,
                        "api_format": p.api_format,
                        "model": p.model,
                        "capabilities": profile_capabilities(p),
                    }),
                    Some(p.clone()),
                )
            }
            None => (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                serde_json::Value::Null,
                None,
            ),
        };
        (
            pport,
            st.secret.clone(),
            sport,
            adapter,
            base_url,
            active_profile,
            catalog_profile,
            tracked_proxy_child_alive,
            st.provider.clone(),
            st.gateway_kind.clone(),
            st.shim_mode.clone(),
            st.launch_id.clone(),
            active_contract_id,
            active_contract_digest,
            st.science_runtime.clone(),
        )
    };
    let diagnostic_override = std::env::var_os("CSSWITCH_UPSTREAM_URL");
    let upstream = status_upstream_endpoint(&adapter, &base_url, diagnostic_override.as_deref());
    let proxy_ok = tracked_proxy_child_alive
        && !secret.is_empty()
        && !launched_gateway_kind.is_empty()
        && !launched_provider.is_empty()
        && proc::http_health_gateway(
            pport,
            Some(&secret),
            operation::STATUS_HEALTH_TIMEOUT_MS,
            proc::GatewayHealthExpectation {
                gateway: &launched_gateway_kind,
                provider: Some(&launched_provider),
                shim: Some(launched_shim_mode.as_str()),
                launch_id: Some(launched_launch_id.as_str()),
                provider_contract_id: Some(active_contract_id.as_str()),
                provider_contract_digest: Some(active_contract_digest.as_str()),
            },
        );
    let last_error = proxy_status_last_error(!secret.is_empty(), proxy_ok, pport);
    let sandbox_ok = proc::http_health(sport, None, operation::STATUS_HEALTH_TIMEOUT_MS);
    let upstream_ok = upstream
        .as_ref()
        .map(|e| proc::tcp_reachable(&e.host, e.port, operation::STATUS_UPSTREAM_TIMEOUT_MS))
        .unwrap_or(false);
    let lights = status_lights(StatusProbeInput {
        proxy_ok,
        sandbox_ok,
        upstream_ok,
        upstream_applicable: status_upstream_applicable(&adapter),
    });
    let (gateway_kind, shim_mode, catalog_shim_mode) =
        status_runtime_identity(&adapter, &secret, launched_gateway_kind, launched_shim_mode);
    let mut science = science_diagnostics(ScienceDiagnosticsInput {
        sandbox_port: sport,
        sandbox_ok,
    });
    if let (Some(object), Some(runtime)) = (science.as_object_mut(), science_runtime) {
        object.insert(
            "runtime".into(),
            json!({
                "source": runtime.source.code(),
                "version": runtime.version,
            }),
        );
    }
    build_status_response(
        lights,
        active_profile,
        &gateway_kind,
        &shim_mode,
        diagnostics_for_profile(catalog_profile.as_ref(), catalog_shim_mode),
        science,
        last_error,
    )
}

pub(super) fn boot_snapshot_inner(state: State<'_, SharedAppState>) -> serde_json::Value {
    lock(state.inner()).boot.snapshot()
}
