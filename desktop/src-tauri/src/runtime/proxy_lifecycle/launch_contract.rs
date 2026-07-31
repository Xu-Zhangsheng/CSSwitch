fn formal_proxy_env(launch: &FormalGatewayPlan) -> Result<Vec<(String, String)>, String> {
    let mut env = vec![
        ("CSSWITCH_GATEWAY_INTENT".into(), "formal".into()),
        (
            "CSSWITCH_PROVIDER_CONTRACT_ID".into(),
            launch.contract_id.clone(),
        ),
        (
            "CSSWITCH_PROVIDER_CONTRACT_DIGEST".into(),
            launch.contract_digest.clone(),
        ),
    ];
    if let FormalCredential::ApiKey { env: key, value } = &launch.credential {
        env.push((key.clone(), value.clone()));
    }
    if let Some(catalog) = &launch.static_model_catalog {
        env.push(("CSSWITCH_STATIC_MODEL_CATALOG_V1".into(), catalog.clone()));
    }
    if launch.endpoint_policy == crate::provider_contracts::EndpointPolicy::ProfileRequired {
        if is_openai_adapter(&launch.adapter) {
            env.push(("CSSWITCH_OPENAI_BASE_URL".into(), launch.endpoint.clone()));
        } else {
            env.push(("CSSWITCH_RELAY_BASE_URL".into(), launch.endpoint.clone()));
            if !launch.thinking_policy.is_empty() {
                env.push((
                    "CSSWITCH_RELAY_THINKING".into(),
                    launch.thinking_policy.to_string(),
                ));
            }
        }
    }
    if let Some(route) = &launch.codex_network_route {
        let encoded = csswitch_codex_network::encode_route(route)
            .map_err(|_| "无法编码 Codex 网络路由。".to_string())?;
        env.push((csswitch_codex_network::ROUTE_ENV.into(), encoded));
    }
    Ok(env)
}

pub(crate) fn configure_managed_proxy_command(
    cmd: &mut Command,
    provider: &str,
    shim_mode: &str,
    port: u16,
    secret: &str,
    launch_id: &str,
) -> Result<(), String> {
    let shim_mode = normalize_shim_mode(provider, Some(shim_mode));
    // Allowlist: drop ambient parent env entirely, then set only Gateway base vars.
    // Plan-specific secrets and contract keys are added by callers after this.
    // CSSWITCH_UPSTREAM_URL is intentionally not inherited for any adapter; set it
    // explicitly after this call when a test/diagnostic override is required.
    crate::runtime::launch_env::configure_gateway_base_command(cmd);
    cmd.arg("--provider")
        .arg(provider)
        .arg("--port")
        .arg(port.to_string())
        .env("CSSWITCH_AUTH_TOKEN", secret)
        .env("CSSWITCH_LAUNCH_ID", launch_id)
        .env("CSSWITCH_TOOLUSE_SHIM", shim_mode);
    Ok(())
}
