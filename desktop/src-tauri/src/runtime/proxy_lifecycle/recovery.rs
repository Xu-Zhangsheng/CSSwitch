fn static_gateway_catalog_fp(launch: &FormalGatewayPlan) -> Option<String> {
    launch
        .static_model_catalog
        .as_deref()
        .and_then(|payload| serde_json::from_str::<serde_json::Value>(payload).ok())
        .and_then(|value| {
            value
                .get("catalog_fp")
                .and_then(|fp| fp.as_str())
                .map(str::to_string)
        })
}

fn interrupted_health_matches(
    health: &proc::GatewayHealth,
    target_provider: &str,
    target_shim: &str,
    target_contract_id: &str,
    target_contract_digest: &str,
    target_catalog_fp: Option<&str>,
    previous: Option<&config::GatewayRuntimeJournalIdentity>,
) -> bool {
    let managed_identity = health.gateway == "rust"
        && health.intent == "formal"
        && (24..=128).contains(&health.launch_id.len())
        && health
            .launch_id
            .chars()
            .all(|value| value.is_ascii_hexdigit());
    let target_matches = health.provider == target_provider
        && health.shim == target_shim
        && !target_contract_id.is_empty()
        && !target_contract_digest.is_empty()
        && health.provider_contract_id == target_contract_id
        && health.provider_contract_digest == target_contract_digest
        && target_catalog_fp
            .map(|expected| health.catalog_fp == expected)
            .unwrap_or(health.catalog_fp.is_empty());
    let previous_matches = previous.is_some_and(|expected| {
        health.provider == expected.provider
            && health.shim == expected.shim
            && health.launch_id == expected.launch_id
            && !expected.provider_contract_id.is_empty()
            && !expected.provider_contract_digest.is_empty()
            && health.provider_contract_id == expected.provider_contract_id
            && health.provider_contract_digest == expected.provider_contract_digest
            && health.catalog_fp == expected.catalog_fp
    });
    managed_identity && (target_matches || previous_matches)
}

/// Consume an interrupted profile-switch journal after an app restart. An
/// orphan is never adopted. It is stopped only when the persisted path secret
/// authenticates a formal Rust Gateway, its public launch/catalog identity
/// matches either the committed target or the journaled previous Gateway, and
/// the listener executes this exact packaged sidecar. The normal one-click
/// path then starts the committed config from a newly tracked child.
pub(crate) fn recover_interrupted_gateway<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
) -> Result<(), String> {
    let dir = config::default_dir();
    recover_interrupted_gateway_from_dir(app, state, &dir)
}

fn recover_interrupted_gateway_from_dir<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SharedAppState,
    dir: &Path,
) -> Result<(), String> {
    let cfg = config::load_from(dir).map_err(|error| error.to_string())?;
    let Some(journal) = cfg.runtime_transaction.as_ref() else {
        return Ok(());
    };
    if journal.target_profile_id != cfg.active_id {
        return Err(
            "未完成运行事务的 target profile 与当前 active profile 不一致；已保留 listener 和事务 journal，拒绝自动恢复，等待人工或后续恢复。"
                .into(),
        );
    }
    if crate::runtime::sandbox_session::runtime_transaction_requires_snapshot_preservation(
        &journal.stage,
    ) {
        return Err(
            "检测到中断的 Science authority/environment 事务；已保留 Gateway、事务 journal 与恢复快照，拒绝自动探测、停止或改写其身份；recovery_status=manual_recovery_required"
                .into(),
        );
    }
    {
        let st = lock(state);
        if st.proxy.is_some()
            || !st.launch_id.is_empty()
            || !st.provider.is_empty()
            || !st.gateway_kind.is_empty()
        {
            // Same-process profile switching owns its Child and recovery.
            return Ok(());
        }
    }
    if !proc::loopback_port_in_use(cfg.proxy_port, operation::LOCAL_HEALTH_TIMEOUT_MS) {
        return Ok(());
    }
    if cfg.secret.is_empty() {
        return Err(
            "检测到未完成的运行事务，但正式端口被占用且配置没有 path secret；已拒绝接管。".into(),
        );
    }
    let active = cfg
        .active_profile()
        .ok_or("未完成的运行事务指向不存在的 active profile")?;
    let formal = crate::runtime::provider::resolve_launch_plan(active)?.formal();
    let target_shim = current_shim_mode_for_adapter(&formal.adapter);
    let target_catalog_fp = static_gateway_catalog_fp(&formal);
    let initial = proc::http_gateway_health(
        cfg.proxy_port,
        Some(&cfg.secret),
        operation::LOCAL_HEALTH_TIMEOUT_MS,
    )
    .ok_or("检测到未完成的运行事务，但端口 listener 不接受已提交 path secret；已拒绝接管。")?;
    if !interrupted_health_matches(
        &initial,
        &formal.adapter,
        target_shim,
        &formal.contract_id,
        &formal.contract_digest,
        target_catalog_fp.as_deref(),
        journal.previous_gateway.as_ref(),
    ) || !proc::http_health_gateway(
        cfg.proxy_port,
        Some(&cfg.secret),
        operation::LOCAL_HEALTH_TIMEOUT_MS,
        proc::GatewayHealthExpectation {
            gateway: "rust",
            provider: Some(&initial.provider),
            shim: Some(&initial.shim),
            launch_id: Some(&initial.launch_id),
            provider_contract_id: Some(&initial.provider_contract_id),
            provider_contract_digest: Some(&initial.provider_contract_digest),
        },
    ) {
        return Err(
            "未完成事务的端口 listener 与已提交/上一受管 Gateway 身份不一致；已拒绝结束未知进程。"
                .into(),
        );
    }
    let binary = gateway_bin_path(app).ok_or("未找到本次应用打包的 Gateway，无法安全恢复事务")?;
    finish_interrupted_gateway_recovery(dir, journal, || {
        let initial_for_probe = initial.clone();
        stop_managed_gateway_on_port(cfg.proxy_port, &binary, || {
            let Some(current) = proc::http_gateway_health(
                cfg.proxy_port,
                Some(&cfg.secret),
                operation::LOCAL_HEALTH_TIMEOUT_MS,
            ) else {
                return false;
            };
            current == initial_for_probe
                && proc::http_health_gateway(
                    cfg.proxy_port,
                    Some(&cfg.secret),
                    operation::LOCAL_HEALTH_TIMEOUT_MS,
                    proc::GatewayHealthExpectation {
                        gateway: "rust",
                        provider: Some(&initial_for_probe.provider),
                        shim: Some(&initial_for_probe.shim),
                        launch_id: Some(&initial_for_probe.launch_id),
                        provider_contract_id: Some(&initial_for_probe.provider_contract_id),
                        provider_contract_digest: Some(&initial_for_probe.provider_contract_digest),
                    },
                )
        })
    })
}

fn finish_interrupted_gateway_recovery<F>(
    dir: &Path,
    journal: &config::RuntimeTransactionJournal,
    cleanup: F,
) -> Result<(), String>
where
    F: FnOnce() -> ManagedGatewayCleanup,
{
    config::update(dir, |current| {
        if let Some(current_journal) = current.runtime_transaction.as_mut() {
            if current_journal.transaction_id == journal.transaction_id
                && !crate::runtime::sandbox_session::runtime_transaction_requires_snapshot_preservation(
                    &current_journal.stage,
                )
            {
                current_journal.stage = "recover_interrupted_gateway".into();
            }
        }
    })
    .map_err(|error| error.to_string())?;
    match cleanup() {
        ManagedGatewayCleanup::Stopped(_) => Ok(()),
        ManagedGatewayCleanup::NotManaged => Err(
            "未完成事务的 listener 未通过精确 Gateway binary/uid/PID 复核；已拒绝结束进程。".into(),
        ),
        ManagedGatewayCleanup::StopFailed(_) => {
            Err("已确认未完成事务遗留的受管 Gateway，但安全停止失败。".into())
        }
    }
}
