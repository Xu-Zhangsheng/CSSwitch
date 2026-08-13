use crate::runtime::operation::{OperationKind, OperationTrace};
use crate::runtime::profile::{nonactive_probe_verdict, probe_kind_for};
use crate::runtime::provider::proxy_args_for;
use crate::{config, scratch};

/// Validate a non-active candidate without touching config, AppState, or the active proxy.
pub(crate) fn scratch_validate_candidate(
    app: &tauri::AppHandle,
    candidate: &config::Profile,
    auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
) -> Result<bool, String> {
    let launch = proxy_args_for(candidate)?;
    let scratch_plan = launch.scratch();
    crate::commands::codex::require_provider_auth_proof(&scratch_plan.provider, auth_proof)?;
    let trace = OperationTrace::start(
        OperationKind::ValidateConnection,
        format!(
            "profile_id={} template_id={} adapter={}",
            candidate.id, candidate.template_id, launch.adapter
        ),
    );
    if !scratch_plan.should_probe() {
        trace.finish("skipped reason=missing_key_or_base");
        return Ok(false);
    }
    let (key_env, key) = scratch_plan.credential_parts();
    let backend = scratch::backend_for_app(app, &scratch_plan.provider)?;
    let res = scratch::scratch_probe(
        &backend,
        &scratch::ScratchTarget {
            provider: &scratch_plan.provider,
            contract_id: &scratch_plan.contract_id,
            contract_digest: &scratch_plan.contract_digest,
            key_env,
            base_url: &scratch_plan.endpoint,
            key,
            model: Some(&scratch_plan.model),
            static_model_catalog: scratch_plan.static_model_catalog.as_deref(),
            relay_thinking: &scratch_plan.thinking_policy,
        },
        probe_kind_for(&scratch_plan.provider, &scratch_plan.model),
        Some(&trace),
        auth_proof.map(|proof| proof.exit_cancel_flag()),
    );
    let outcome = scratch::classify(res.status);
    trace.finish(format!("outcome={outcome:?}"));
    nonactive_probe_verdict(&outcome)
}
