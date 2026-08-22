//! Desktop compatibility projection over the pure provider-contract owner.

pub(crate) use csswitch_provider_contracts::{
    AuthMode, AuthScheme, CachePolicy, CredentialSource, EndpointJoin, EndpointPolicy,
    ModelDiscovery, ModelPolicy, ProviderContract, ProviderContractCatalog, ScratchPolicy,
    TimeoutPolicy, Transport,
};

pub(crate) fn static_catalog_digest() -> String {
    csswitch_provider_contracts::static_catalog_digest()
}

pub(crate) fn load_provider_contracts() -> Result<ProviderContractCatalog, String> {
    csswitch_provider_contracts::load_provider_contracts().map_err(|error| error.to_string())
}

pub(crate) fn contract_for(
    template_id: &str,
    api_format: &str,
) -> Result<ProviderContract, String> {
    csswitch_provider_contracts::contract_for(template_id, api_format)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_compatibility_projection_keeps_catalog_and_launch_selection() {
        let catalog = load_provider_contracts().unwrap();
        assert_eq!(catalog.schema_version, 1);
        assert_eq!(catalog.contracts.len(), 12);
        assert_eq!(static_catalog_digest().len(), 64);
        assert_eq!(
            contract_for("codex", "openai_responses").unwrap().adapter,
            "codex"
        );
    }

    #[test]
    fn desktop_compatibility_projection_selects_api_format_exactly() {
        assert_eq!(
            contract_for("custom", "openai_chat").unwrap().adapter,
            "openai-custom"
        );
        assert_eq!(
            contract_for("custom", "openai_responses").unwrap().adapter,
            "openai-responses"
        );
    }
}
