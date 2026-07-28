//! Stable protocol primitives shared by CSSwitch UI and the external bridge.
//!
//! This crate deliberately has no Tauri, WebView, or app-path dependency. It
//! is the compatibility boundary that remains installed when CSSwitch.app is
//! updated.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROTOCOL_VERSION: &str = "csswitch-control/1";
pub const CONFIG_SCHEMA_MIN: u32 = 4;
pub const CONFIG_SCHEMA_MAX: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolInfo {
    pub protocol: String,
    pub config_schema_min: u32,
    pub config_schema_max: u32,
}

impl Default for ProtocolInfo {
    fn default() -> Self {
        Self {
            protocol: PROTOCOL_VERSION.to_string(),
            config_schema_min: CONFIG_SCHEMA_MIN,
            config_schema_max: CONFIG_SCHEMA_MAX,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySet {
    pub status: bool,
    pub config: bool,
    pub profiles: bool,
    pub model_catalog: bool,
    pub runtime: bool,
    pub provider_check: bool,
}

impl CapabilitySet {
    pub fn full() -> Self {
        Self { status: true, config: true, profiles: true, model_catalog: true, runtime: true, provider_check: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlHello {
    pub protocol: ProtocolInfo,
    pub capabilities: CapabilitySet,
    pub config_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MutationEnvelope {
    pub intent_id: String,
    pub expected_config_fingerprint: String,
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationResult {
    pub intent_id: String,
    pub committed: bool,
    pub state_uncertain: bool,
    pub config_fingerprint: String,
}

pub fn config_fingerprint<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    let digest = Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn schema_supported(schema: u32) -> bool {
    (CONFIG_SCHEMA_MIN..=CONFIG_SCHEMA_MAX).contains(&schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_is_app_path_independent() {
        assert_eq!(ProtocolInfo::default().protocol, "csswitch-control/1");
        assert!(schema_supported(4));
        assert!(!schema_supported(5));
    }

    #[test]
    fn fingerprint_is_stable() {
        assert_eq!(config_fingerprint(&serde_json::json!({"a": 1})).unwrap(), config_fingerprint(&serde_json::json!({"a": 1})).unwrap());
    }
}
