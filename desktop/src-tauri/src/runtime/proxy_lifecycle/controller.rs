/// Exact in-memory recipe for the managed Gateway represented by a receipt.
///
/// The profile can contain credentials. This type is process-local only and
/// deliberately does not implement `Debug` or serialization.
#[derive(Clone, PartialEq)]
pub(crate) struct GatewayLaunchRecipe {
    pub(crate) profile: config::Profile,
    pub(crate) science_runtime: Option<crate::runtime::science::ScienceRuntimeIdentity>,
}

/// Identity that passed the managed Gateway health checks for this receipt.
pub(crate) struct GatewayHealthReceipt {
    pub(crate) gateway_kind: String,
    pub(crate) provider: String,
    pub(crate) shim_mode: String,
    pub(crate) launch_id: String,
    pub(crate) provider_contract_id: String,
    pub(crate) provider_contract_digest: String,
    pub(crate) intent: String,
}

/// Expected and accepted catalog identity from the managed health acceptance.
/// `None` is the existing dynamic-catalog contract and requires an empty
/// accepted Gateway health fingerprint.
pub(crate) struct GatewayCatalogReceipt {
    pub(crate) expected_fingerprint: Option<String>,
    pub(crate) accepted_fingerprint: String,
}

/// Typed proof returned only after a managed Gateway was reused or started,
/// passed identity/health/catalog checks, and had an exact launch recipe.
///
/// The route secret and profile recipe are sensitive. Keep this receipt inside
/// the Rust control plane; it intentionally has no `Debug`, `Clone`, serde, or
/// Tauri DTO projection.
pub(crate) struct GatewayReceipt {
    pub(crate) port: u16,
    pub(crate) route_secret: String,
    pub(crate) action: ProxyAction,
    pub(crate) health: GatewayHealthReceipt,
    pub(crate) catalog: GatewayCatalogReceipt,
    pub(crate) recipe: GatewayLaunchRecipe,
}

fn accepted_gateway_health(
    actual: &proc::GatewayHealth,
    expected: proc::GatewayHealthExpectation<'_>,
    expected_catalog_fingerprint: Option<&str>,
) -> bool {
    proc::gateway_health_matches(actual, expected)
        && actual.intent == "formal"
        && expected_catalog_fingerprint
            .map(|expected| actual.catalog_fp == expected)
            .unwrap_or(actual.catalog_fp.is_empty())
}

impl GatewayReceipt {
    fn verified(
        port: u16,
        route_secret: String,
        action: ProxyAction,
        accepted_health: &proc::GatewayHealth,
        expected_catalog_fingerprint: Option<String>,
        recipe: GatewayLaunchRecipe,
    ) -> Self {
        let receipt = Self {
            port,
            route_secret,
            action,
            health: GatewayHealthReceipt {
                gateway_kind: accepted_health.gateway.clone(),
                provider: accepted_health.provider.clone(),
                shim_mode: accepted_health.shim.clone(),
                launch_id: accepted_health.launch_id.clone(),
                provider_contract_id: accepted_health.provider_contract_id.clone(),
                provider_contract_digest: accepted_health.provider_contract_digest.clone(),
                intent: accepted_health.intent.clone(),
            },
            catalog: GatewayCatalogReceipt {
                expected_fingerprint: expected_catalog_fingerprint,
                accepted_fingerprint: accepted_health.catalog_fp.clone(),
            },
            recipe,
        };
        debug_assert!(receipt.is_internally_complete());
        receipt
    }

    fn is_internally_complete(&self) -> bool {
        self.port != 0
            && !self.route_secret.is_empty()
            && !self.health.gateway_kind.is_empty()
            && !self.health.provider.is_empty()
            && !self.health.shim_mode.is_empty()
            && !self.health.launch_id.is_empty()
            && !self.health.provider_contract_id.is_empty()
            && !self.health.provider_contract_digest.is_empty()
            && self.health.intent == "formal"
            && self
                .catalog
                .expected_fingerprint
                .as_deref()
                .map_or(self.catalog.accepted_fingerprint.is_empty(), |expected| {
                    !expected.is_empty() && self.catalog.accepted_fingerprint == expected
                })
            && !self.recipe.profile.id.is_empty()
    }
}

/// Process-local facade for the existing formal Gateway start/reuse core.
/// Callers retain their current mutation lease, ordering, checkpoints,
/// compensation, and DTO ownership.
pub(crate) struct GatewayController;

impl GatewayController {
    pub(crate) fn ensure_active<R: Runtime>(
        app: &tauri::AppHandle<R>,
        state: &SharedAppState,
        lifecycle: &lifecycle::Lifecycle,
        science_runtime: Option<&crate::runtime::science::ScienceRuntimeIdentity>,
        trace: Option<&OperationTrace>,
        auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    ) -> Result<GatewayReceipt, String> {
        let cfg = config::load_from(&config::default_dir()).map_err(|e| e.to_string())?;
        let profile = cfg
            .active_profile()
            .cloned()
            .ok_or("未配置生效 profile，请先在面板选择或新建一条配置。")?;
        start_proxy_for_inner(
            app,
            state,
            lifecycle,
            &profile,
            science_runtime,
            trace,
            auth_proof,
            None,
        )
    }

    /// Start or reuse a Gateway for an explicit profile without reading the
    /// active profile. The caller owns the mutation-lease boundary.
    pub(crate) fn start_for<R: Runtime>(
        app: &tauri::AppHandle<R>,
        state: &SharedAppState,
        lifecycle: &lifecycle::Lifecycle,
        profile: &config::Profile,
        science_runtime: Option<&crate::runtime::science::ScienceRuntimeIdentity>,
        trace: Option<&OperationTrace>,
        auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
    ) -> Result<GatewayReceipt, String> {
        start_proxy_for_inner(
            app,
            state,
            lifecycle,
            profile,
            science_runtime,
            trace,
            auth_proof,
            None,
        )
    }

    /// Restore an exact prior managed Gateway with a durable launch id.  The
    /// caller owns the operation receipt and auth/lifecycle leases; this facade
    /// keeps the normal full-owner reservation and health acceptance path.
    pub(crate) fn restore_for<R: Runtime>(
        app: &tauri::AppHandle<R>,
        state: &SharedAppState,
        lifecycle: &lifecycle::Lifecycle,
        profile: &config::Profile,
        science_runtime: Option<&crate::runtime::science::ScienceRuntimeIdentity>,
        auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
        durable_launch_id: &str,
    ) -> Result<GatewayReceipt, String> {
        start_proxy_for_inner(
            app,
            state,
            lifecycle,
            profile,
            science_runtime,
            None,
            auth_proof,
            Some(durable_launch_id),
        )
    }
}
