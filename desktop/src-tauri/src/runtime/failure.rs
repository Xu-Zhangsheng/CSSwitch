//! Typed one-click / auto-boot failure projection.
//!
//! Failures carry a produce-site [`OneClickFailureKind`]; UI coarse `stage` and
//! recovery fields are projected from that kind (and explicit recovery flags),
//! never by scanning user-facing message text.

use serde_json::{json, Value};

/// CSSwitch-owned failure domain for internal diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailureDomain {
    Desktop,
    RuntimeTransaction,
    GatewayProvider,
    RuntimeAdapter,
    #[allow(dead_code)] // reserved for Skill/SSH/Codex bridge-local failures
    Bridge,
}

/// Fine-grained one-click failure kind. Maps 1:N to frozen UI coarse stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OneClickFailureKind {
    // prepare
    ConfigLoad,
    NoActiveProfile,
    LaunchPlan,
    AuthPreflight,
    PreflightSnapshot,
    Prepare,
    // science_stop
    ScienceStop,
    // gateway_start
    GatewayStart,
    ProxySpawn,
    ProxyHealth,
    // catalog_verify
    CatalogVerify,
    // science_start
    AuthoritySnapshot,
    SandboxLogin,
    SandboxLaunch,
    SandboxHealth,
    ScienceDbReverify,
    OpenSurface,
    ScienceStart,
}

impl OneClickFailureKind {
    pub(crate) fn domain(self) -> FailureDomain {
        match self {
            Self::ConfigLoad
            | Self::NoActiveProfile
            | Self::LaunchPlan
            | Self::AuthPreflight
            | Self::PreflightSnapshot
            | Self::Prepare => FailureDomain::Desktop,
            Self::ScienceStop | Self::AuthoritySnapshot => FailureDomain::RuntimeTransaction,
            Self::GatewayStart | Self::ProxySpawn | Self::ProxyHealth | Self::CatalogVerify => {
                FailureDomain::GatewayProvider
            }
            Self::SandboxLogin
            | Self::SandboxLaunch
            | Self::SandboxHealth
            | Self::ScienceDbReverify
            | Self::OpenSurface
            | Self::ScienceStart => FailureDomain::RuntimeAdapter,
        }
    }

    /// Frozen product-facing coarse stage strings.
    pub(crate) fn coarse_stage(self) -> &'static str {
        match self {
            Self::ScienceStop => "science_stop",
            Self::GatewayStart | Self::ProxySpawn | Self::ProxyHealth => "gateway_start",
            Self::CatalogVerify => "catalog_verify",
            Self::AuthoritySnapshot
            | Self::SandboxLogin
            | Self::SandboxLaunch
            | Self::SandboxHealth
            | Self::ScienceDbReverify
            | Self::OpenSurface
            | Self::ScienceStart => "science_start",
            Self::ConfigLoad
            | Self::NoActiveProfile
            | Self::LaunchPlan
            | Self::AuthPreflight
            | Self::PreflightSnapshot
            | Self::Prepare => "prepare",
        }
    }
}

/// Recovery / environment projection independent of message text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedRecovery {
    pub recovery_status: &'static str,
    pub environment_status: &'static str,
}

impl ProjectedRecovery {
    pub(crate) const NOT_NEEDED: Self = Self {
        recovery_status: "not_needed",
        environment_status: "not_exposed",
    };

    pub(crate) const DEGRADED: Self = Self {
        recovery_status: "degraded",
        environment_status: "not_exposed",
    };

    pub(crate) const ENVIRONMENT_UNCERTAIN: Self = Self {
        recovery_status: "environment_uncertain",
        environment_status: "uncertain",
    };

    pub(crate) const CLEANUP_REQUIRED: Self = Self {
        recovery_status: "cleanup_required",
        environment_status: "not_exposed",
    };

    pub(crate) const MANUAL_RECOVERY_REQUIRED: Self = Self {
        recovery_status: "manual_recovery_required",
        environment_status: "not_exposed",
    };

    pub(crate) fn environment_uncertain_manual() -> Self {
        Self {
            recovery_status: "manual_recovery_required",
            environment_status: "uncertain",
        }
    }

    pub(crate) fn cleanup_required_uncertain() -> Self {
        Self {
            recovery_status: "cleanup_required",
            environment_status: "uncertain",
        }
    }
}

/// Typed one-click failure. Message is for humans/logs only and must not drive stage.
#[derive(Clone, Debug)]
pub(crate) struct TypedOneClickFailure {
    pub kind: OneClickFailureKind,
    pub message: String,
    pub recovery: ProjectedRecovery,
}

impl TypedOneClickFailure {
    pub(crate) fn new(kind: OneClickFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            recovery: ProjectedRecovery::NOT_NEEDED,
        }
    }

    pub(crate) fn with_recovery(mut self, recovery: ProjectedRecovery) -> Self {
        self.recovery = recovery;
        self
    }

    pub(crate) fn domain(&self) -> FailureDomain {
        self.kind.domain()
    }

    pub(crate) fn coarse_stage(&self) -> &'static str {
        self.kind.coarse_stage()
    }

    /// Project to the frozen one-click failure DTO (existing keys only).
    pub(crate) fn project_dto(&self) -> Value {
        json!({
            "action": "failed",
            "stage": self.coarse_stage(),
            "status": "error",
            "recovery_status": self.recovery.recovery_status,
            "environment_status": self.recovery.environment_status,
            "message": self.message,
            "fallback_url": null,
        })
    }

    /// When config still has an open runtime journal and recovery was not set,
    /// treat as degraded (matches prior command-layer journal presence check).
    pub(crate) fn apply_open_journal_degraded(mut self, journal_open: bool) -> Self {
        if journal_open
            && self.recovery.recovery_status == ProjectedRecovery::NOT_NEEDED.recovery_status
            && self.recovery.environment_status == ProjectedRecovery::NOT_NEEDED.environment_status
        {
            self.recovery = ProjectedRecovery::DEGRADED;
        }
        self
    }
}

impl std::fmt::Display for TypedOneClickFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::ops::Deref for TypedOneClickFailure {
    type Target = str;

    fn deref(&self) -> &str {
        &self.message
    }
}

impl AsRef<str> for TypedOneClickFailure {
    fn as_ref(&self) -> &str {
        &self.message
    }
}

impl From<TypedOneClickFailure> for String {
    fn from(value: TypedOneClickFailure) -> Self {
        value.message
    }
}

/// Transitional: recovery flags still appear as operator diagnostic codes in
/// `message` after compensation. Prefer setting [`ProjectedRecovery`] at the
/// produce/compensate site. **Stage must never use this helper.**
pub(crate) fn recovery_from_diagnostic_codes(message: &str) -> Option<ProjectedRecovery> {
    if message.contains("recovery_status=cleanup_required") {
        if message.contains("environment_uncertain") {
            return Some(ProjectedRecovery::cleanup_required_uncertain());
        }
        return Some(ProjectedRecovery::CLEANUP_REQUIRED);
    }
    if message.contains("recovery_status=manual_recovery_required") {
        if message.contains("environment_uncertain") {
            return Some(ProjectedRecovery::environment_uncertain_manual());
        }
        return Some(ProjectedRecovery::MANUAL_RECOVERY_REQUIRED);
    }
    if message.contains("environment_uncertain") {
        return Some(ProjectedRecovery::ENVIRONMENT_UNCERTAIN);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_stage_table_is_stable() {
        assert_eq!(
            OneClickFailureKind::ScienceStop.coarse_stage(),
            "science_stop"
        );
        assert_eq!(
            OneClickFailureKind::ProxyHealth.coarse_stage(),
            "gateway_start"
        );
        assert_eq!(
            OneClickFailureKind::ProxySpawn.coarse_stage(),
            "gateway_start"
        );
        assert_eq!(
            OneClickFailureKind::GatewayStart.coarse_stage(),
            "gateway_start"
        );
        assert_eq!(
            OneClickFailureKind::CatalogVerify.coarse_stage(),
            "catalog_verify"
        );
        assert_eq!(
            OneClickFailureKind::SandboxLaunch.coarse_stage(),
            "science_start"
        );
        assert_eq!(
            OneClickFailureKind::SandboxHealth.coarse_stage(),
            "science_start"
        );
        assert_eq!(
            OneClickFailureKind::ScienceDbReverify.coarse_stage(),
            "science_start"
        );
        assert_eq!(OneClickFailureKind::ConfigLoad.coarse_stage(), "prepare");
        assert_eq!(OneClickFailureKind::AuthPreflight.coarse_stage(), "prepare");
        assert_eq!(
            OneClickFailureKind::CatalogVerify.domain(),
            FailureDomain::GatewayProvider
        );
        assert_eq!(
            TypedOneClickFailure::new(OneClickFailureKind::ScienceStop, "x").domain(),
            FailureDomain::RuntimeTransaction
        );
    }

    #[test]
    fn message_text_does_not_change_stage() {
        let catalog = TypedOneClickFailure::new(
            OneClickFailureKind::CatalogVerify,
            "gateway 模型目录探活无响应",
        );
        assert_eq!(catalog.coarse_stage(), "catalog_verify");
        assert_eq!(catalog.project_dto()["stage"], "catalog_verify");

        let gateway = TypedOneClickFailure::new(
            OneClickFailureKind::GatewayStart,
            "模型目录字样不应影响 gateway kind",
        );
        assert_eq!(gateway.coarse_stage(), "gateway_start");

        let prepare = TypedOneClickFailure::new(
            OneClickFailureKind::Prepare,
            "停止旧进程 代理 gateway 沙箱 Science science_api_ 模型目录",
        );
        assert_eq!(prepare.coarse_stage(), "prepare");
    }

    #[test]
    fn project_dto_keys_match_frozen_contract() {
        let failure = TypedOneClickFailure::new(OneClickFailureKind::SandboxHealth, "沙箱起后超时")
            .with_recovery(ProjectedRecovery::ENVIRONMENT_UNCERTAIN);
        let value = failure.project_dto();
        assert_eq!(value["action"], "failed");
        assert_eq!(value["stage"], "science_start");
        assert_eq!(value["status"], "error");
        assert_eq!(value["recovery_status"], "environment_uncertain");
        assert_eq!(value["environment_status"], "uncertain");
        assert_eq!(value["message"], "沙箱起后超时");
        assert!(value["fallback_url"].is_null());
    }

    #[test]
    fn open_journal_only_upgrades_not_needed() {
        let base = TypedOneClickFailure::new(OneClickFailureKind::Prepare, "配置不可用");
        let degraded = base.clone().apply_open_journal_degraded(true);
        assert_eq!(degraded.recovery.recovery_status, "degraded");

        let uncertain = TypedOneClickFailure::new(OneClickFailureKind::ScienceStart, "x")
            .with_recovery(ProjectedRecovery::ENVIRONMENT_UNCERTAIN)
            .apply_open_journal_degraded(true);
        assert_eq!(uncertain.recovery.recovery_status, "environment_uncertain");
    }

    #[test]
    fn deref_supports_legacy_error_contains_checks() {
        let failure = TypedOneClickFailure::new(
            OneClickFailureKind::ScienceStart,
            "science_db_reverify_timeout",
        );
        assert!(failure.contains("science_db_reverify"));
        assert_eq!(failure.as_ref(), "science_db_reverify_timeout");
        assert_eq!(failure.to_string(), "science_db_reverify_timeout");
    }

    #[test]
    fn recovery_diagnostic_codes_prefer_stronger_recovery_status() {
        let cleanup = recovery_from_diagnostic_codes(
            "x；recovery_status=cleanup_required；environment_uncertain",
        )
        .unwrap();
        assert_eq!(cleanup.recovery_status, "cleanup_required");
        assert_eq!(cleanup.environment_status, "uncertain");

        let manual = recovery_from_diagnostic_codes(
            "x；environment_uncertain；recovery_status=manual_recovery_required",
        )
        .unwrap();
        assert_eq!(manual.recovery_status, "manual_recovery_required");
        assert_eq!(manual.environment_status, "uncertain");

        let bare = recovery_from_diagnostic_codes("x；environment_uncertain").unwrap();
        assert_eq!(bare.recovery_status, "environment_uncertain");
        assert_eq!(bare.environment_status, "uncertain");

        assert!(recovery_from_diagnostic_codes("plain failure").is_none());
    }

    #[test]
    fn compensation_keeps_original_kind_when_message_gains_codes() {
        let failure = TypedOneClickFailure::new(
            OneClickFailureKind::CatalogVerify,
            "gateway 模型目录探活无响应；environment_uncertain；recovery_status=cleanup_required",
        )
        .with_recovery(ProjectedRecovery::cleanup_required_uncertain());
        assert_eq!(failure.coarse_stage(), "catalog_verify");
        assert_eq!(failure.recovery.recovery_status, "cleanup_required");
        assert_eq!(failure.project_dto()["stage"], "catalog_verify");
    }
}
