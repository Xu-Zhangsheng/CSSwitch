use std::time::Duration;

use csswitch_provider_contracts::{
    contract_by_id, contract_for_adapter, static_catalog_digest, ProviderContract,
};

pub(crate) use csswitch_provider_contracts::{AuthScheme, EndpointJoin};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexRuntimeContract {
    pub contract_id: String,
    pub catalog_digest: String,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub read_idle_timeout: Duration,
    pub normal_ttl_seconds: u64,
    pub stale_ttl_seconds: u64,
    pub model_catalog_client_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRuntimeContract {
    pub contract_id: String,
    pub catalog_digest: String,
    pub auth_mode: String,
    pub auth_scheme: AuthScheme,
    pub api_key_env: Option<String>,
    pub transport: String,
    pub endpoint_policy: String,
    pub endpoint_join: EndpointJoin,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub read_idle_timeout: Duration,
    pub normal_ttl_seconds: u64,
    pub stale_ttl_seconds: u64,
    pub upstream_client_version: Option<String>,
}

fn project_runtime_contract(contract: ProviderContract, digest: String) -> ProviderRuntimeContract {
    ProviderRuntimeContract {
        contract_id: contract.id,
        catalog_digest: digest,
        auth_mode: match contract.auth_mode {
            csswitch_provider_contracts::AuthMode::ApiKey => "api_key".into(),
            csswitch_provider_contracts::AuthMode::CsswitchOauth => "csswitch_oauth".into(),
            csswitch_provider_contracts::AuthMode::None => "none".into(),
        },
        auth_scheme: contract.auth_scheme,
        api_key_env: contract.api_key_env,
        transport: match contract.transport {
            csswitch_provider_contracts::Transport::AnthropicMessages => {
                "anthropic_messages".into()
            }
            csswitch_provider_contracts::Transport::OpenaiChat => "openai_chat".into(),
            csswitch_provider_contracts::Transport::OpenaiResponses => "openai_responses".into(),
            csswitch_provider_contracts::Transport::CodexResponsesSse => {
                "codex_responses_sse".into()
            }
        },
        endpoint_policy: match contract.endpoint_policy {
            csswitch_provider_contracts::EndpointPolicy::GatewayManagedOfficial => {
                "gateway_managed_official".into()
            }
            csswitch_provider_contracts::EndpointPolicy::ProfileRequired => {
                "profile_required".into()
            }
        },
        endpoint_join: contract.endpoint_join,
        connect_timeout: Duration::from_millis(contract.timeouts.connect_ms),
        request_timeout: Duration::from_millis(contract.timeouts.total_ms),
        read_idle_timeout: Duration::from_millis(contract.timeouts.read_idle_ms),
        normal_ttl_seconds: contract.cache.normal_ttl_seconds,
        stale_ttl_seconds: contract.cache.stale_ttl_seconds,
        upstream_client_version: contract.upstream_client_version,
    }
}

pub(crate) fn load_runtime_contract(
    provider: &str,
    expected_id: Option<&str>,
    expected_digest: Option<&str>,
) -> Result<ProviderRuntimeContract, String> {
    let digest = static_catalog_digest();
    let contract = match (expected_id, expected_digest) {
        (Some(id), Some(expected)) => {
            if expected != digest {
                return Err("managed provider contract identity mismatch".into());
            }
            contract_by_id(id).map_err(|_| "managed provider contract is unavailable")?
        }
        (None, None) => contract_for_adapter(provider)
            .map_err(|_| "provider contract identity is required for this adapter")?,
        _ => return Err("managed provider contract identity is incomplete".into()),
    };
    if contract.adapter != provider {
        return Err("managed provider contract adapter mismatch".into());
    }
    Ok(project_runtime_contract(contract, digest))
}

pub(crate) fn codex_contract_from_runtime(
    runtime: &ProviderRuntimeContract,
) -> Result<CodexRuntimeContract, String> {
    if runtime.contract_id != "codex-oauth"
        || runtime.auth_mode != "csswitch_oauth"
        || runtime.auth_scheme != AuthScheme::CsswitchOauth
        || runtime.transport != "codex_responses_sse"
        || runtime.endpoint_policy != "gateway_managed_official"
        || runtime.endpoint_join != EndpointJoin::ManagedOfficial
        || runtime.api_key_env.is_some()
        || runtime.upstream_client_version.as_deref() != Some("0.144.4")
    {
        return Err("Codex provider contract is invalid".into());
    }
    Ok(CodexRuntimeContract {
        contract_id: runtime.contract_id.clone(),
        catalog_digest: runtime.catalog_digest.clone(),
        connect_timeout: runtime.connect_timeout,
        request_timeout: runtime.request_timeout,
        read_idle_timeout: runtime.read_idle_timeout,
        normal_ttl_seconds: runtime.normal_ttl_seconds,
        stale_ttl_seconds: runtime.stale_ttl_seconds,
        model_catalog_client_version: runtime
            .upstream_client_version
            .clone()
            .expect("validated Codex client version"),
    })
}

#[cfg(test)]
pub(crate) fn load_codex_runtime_contract() -> Result<CodexRuntimeContract, String> {
    let digest = static_catalog_digest();
    codex_contract_from_runtime(&load_runtime_contract(
        "codex",
        Some("codex-oauth"),
        Some(&digest),
    )?)
}

#[cfg(test)]
pub(crate) fn validate_managed_identity(
    contract: &CodexRuntimeContract,
    expected_id: Option<&str>,
    expected_digest: Option<&str>,
) -> Result<(), String> {
    match (expected_id, expected_digest) {
        (None, None) => Ok(()),
        (Some(id), Some(digest))
            if id == contract.contract_id && digest == contract.catalog_digest =>
        {
            Ok(())
        }
        _ => Err("managed provider contract identity mismatch".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use csswitch_provider_contracts::{contract_for, load_provider_contracts};

    #[test]
    fn codex_projection_preserves_shared_runtime_values() {
        let contract = load_codex_runtime_contract().unwrap();
        assert_eq!(contract.contract_id, "codex-oauth");
        assert_eq!(contract.catalog_digest.len(), 64);
        assert_eq!(contract.connect_timeout, Duration::from_secs(10));
        assert_eq!(contract.request_timeout, Duration::from_secs(30));
        assert_eq!(contract.read_idle_timeout, Duration::from_secs(300));
        assert_eq!(contract.normal_ttl_seconds, 300);
        assert_eq!(contract.stale_ttl_seconds, 86_400);
        assert_eq!(contract.model_catalog_client_version, "0.144.4");
    }

    #[test]
    fn managed_identity_is_optional_for_standalone_but_fail_closed_when_present() {
        let contract = load_codex_runtime_contract().unwrap();
        assert!(validate_managed_identity(&contract, None, None).is_ok());
        assert!(validate_managed_identity(
            &contract,
            Some(&contract.contract_id),
            Some(&contract.catalog_digest)
        )
        .is_ok());
        assert!(validate_managed_identity(&contract, Some("wrong"), None).is_err());
        assert!(
            validate_managed_identity(&contract, Some(&contract.contract_id), Some("wrong"))
                .is_err()
        );
    }

    #[test]
    fn managed_contract_selects_the_exact_shared_id() {
        let digest = static_catalog_digest();
        let kimi =
            load_runtime_contract("relay", Some("kimi-anthropic-relay"), Some(&digest)).unwrap();
        assert_eq!(kimi.contract_id, "kimi-anthropic-relay");
        assert_eq!(kimi.endpoint_join, EndpointJoin::AnthropicV1);
        assert_eq!(kimi.transport, "anthropic_messages");
        assert_eq!(kimi.catalog_digest, digest);
    }

    #[test]
    fn standalone_adapter_selection_requires_one_shared_contract() {
        assert_eq!(
            load_runtime_contract("deepseek", None, None)
                .unwrap()
                .contract_id,
            "deepseek-native"
        );
        assert!(load_runtime_contract("relay", None, None).is_err());
    }

    #[test]
    fn managed_contract_rejects_cross_adapter_ids_and_incomplete_identity() {
        let digest = static_catalog_digest();
        assert!(load_runtime_contract(
            "openai-custom",
            Some("kimi-anthropic-relay"),
            Some(&digest),
        )
        .is_err());
        assert!(load_runtime_contract("relay", Some("kimi-anthropic-relay"), None).is_err());
        assert!(load_runtime_contract(
            "relay",
            Some("kimi-anthropic-relay"),
            Some(&"0".repeat(64)),
        )
        .is_err());
    }

    #[test]
    fn gateway_projection_matches_shared_catalog_selection() {
        let digest = static_catalog_digest();
        for contract in load_provider_contracts().unwrap().contracts {
            for template_id in &contract.template_ids {
                for api_format in &contract.api_formats {
                    let desktop_contract = contract_for(template_id, api_format).unwrap();
                    let gateway_contract = load_runtime_contract(
                        &desktop_contract.adapter,
                        Some(&desktop_contract.id),
                        Some(&digest),
                    )
                    .unwrap();
                    assert_eq!(gateway_contract.contract_id, desktop_contract.id);
                    assert_eq!(gateway_contract.catalog_digest, digest);
                    assert_eq!(gateway_contract.auth_scheme, desktop_contract.auth_scheme);
                    assert_eq!(
                        gateway_contract.endpoint_join,
                        desktop_contract.endpoint_join
                    );
                    assert_eq!(
                        gateway_contract.connect_timeout,
                        Duration::from_millis(desktop_contract.timeouts.connect_ms)
                    );
                    assert_eq!(
                        gateway_contract.request_timeout,
                        Duration::from_millis(desktop_contract.timeouts.total_ms)
                    );
                }
            }
        }
    }
}
