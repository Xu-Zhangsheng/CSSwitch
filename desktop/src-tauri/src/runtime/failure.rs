//! Typed runtime failure and one-click / auto-boot projection.
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

/// Internal runtime phase. Product-facing stage strings are projected only at
/// the command boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimePhase {
    Prepare,
    ScienceStop,
    GatewayStart,
    CatalogVerify,
    ScienceStart,
}

impl RuntimePhase {
    pub(crate) fn coarse_stage(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::ScienceStop => "science_stop",
            Self::GatewayStart => "gateway_start",
            Self::CatalogVerify => "catalog_verify",
            Self::ScienceStart => "science_start",
        }
    }
}

/// Typed recovery disposition. Strings exist only in the frozen frontend DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryDisposition {
    NotNeeded,
    Degraded,
    EnvironmentUncertain,
    CleanupRequired,
    ManualRecoveryRequired,
}

impl RecoveryDisposition {
    pub(crate) fn dto_status(self) -> &'static str {
        match self {
            Self::NotNeeded => "not_needed",
            Self::Degraded => "degraded",
            Self::EnvironmentUncertain => "environment_uncertain",
            Self::CleanupRequired => "cleanup_required",
            Self::ManualRecoveryRequired => "manual_recovery_required",
        }
    }
}

/// Whether a failed operation may have exposed the candidate Science
/// environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EnvironmentExposure {
    NotExposed,
    Uncertain,
}

impl EnvironmentExposure {
    pub(crate) fn dto_status(self) -> &'static str {
        match self {
            Self::NotExposed => "not_exposed",
            Self::Uncertain => "uncertain",
        }
    }
}

/// Sanitized internal cause. Causes are intentionally excluded from frontend
/// projection; callers must never put credentials or private paths here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SafeCause {
    pub code: &'static str,
    pub safe_detail: String,
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

    pub(crate) fn phase(self) -> RuntimePhase {
        match self {
            Self::ScienceStop => RuntimePhase::ScienceStop,
            Self::GatewayStart | Self::ProxySpawn | Self::ProxyHealth => RuntimePhase::GatewayStart,
            Self::CatalogVerify => RuntimePhase::CatalogVerify,
            Self::AuthoritySnapshot
            | Self::SandboxLogin
            | Self::SandboxLaunch
            | Self::SandboxHealth
            | Self::ScienceDbReverify
            | Self::OpenSurface
            | Self::ScienceStart => RuntimePhase::ScienceStart,
            Self::ConfigLoad
            | Self::NoActiveProfile
            | Self::LaunchPlan
            | Self::AuthPreflight
            | Self::PreflightSnapshot
            | Self::Prepare => RuntimePhase::Prepare,
        }
    }
}

/// Recovery / environment projection independent of message text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedRecovery {
    pub recovery: RecoveryDisposition,
    pub environment: EnvironmentExposure,
}

impl ProjectedRecovery {
    pub(crate) const NOT_NEEDED: Self = Self {
        recovery: RecoveryDisposition::NotNeeded,
        environment: EnvironmentExposure::NotExposed,
    };

    pub(crate) const DEGRADED: Self = Self {
        recovery: RecoveryDisposition::Degraded,
        environment: EnvironmentExposure::NotExposed,
    };

    pub(crate) const ENVIRONMENT_UNCERTAIN: Self = Self {
        recovery: RecoveryDisposition::EnvironmentUncertain,
        environment: EnvironmentExposure::Uncertain,
    };

    pub(crate) const CLEANUP_REQUIRED: Self = Self {
        recovery: RecoveryDisposition::CleanupRequired,
        environment: EnvironmentExposure::NotExposed,
    };

    pub(crate) const MANUAL_RECOVERY_REQUIRED: Self = Self {
        recovery: RecoveryDisposition::ManualRecoveryRequired,
        environment: EnvironmentExposure::NotExposed,
    };

    pub(crate) fn environment_uncertain_manual() -> Self {
        Self {
            recovery: RecoveryDisposition::ManualRecoveryRequired,
            environment: EnvironmentExposure::Uncertain,
        }
    }

    pub(crate) fn cleanup_required_uncertain() -> Self {
        Self {
            recovery: RecoveryDisposition::CleanupRequired,
            environment: EnvironmentExposure::Uncertain,
        }
    }
}

/// Typed runtime failure envelope. `safe_detail` is for humans/logs only and
/// must not drive domain, phase, recovery, or environment classification.
#[derive(Clone, Debug)]
pub(crate) struct RuntimeError<K> {
    domain: FailureDomain,
    phase: RuntimePhase,
    kind: K,
    pub recovery: RecoveryDisposition,
    pub environment: EnvironmentExposure,
    pub cause_chain: Vec<SafeCause>,
    pub safe_detail: String,
}

pub(crate) type TypedOneClickFailure = RuntimeError<OneClickFailureKind>;

impl RuntimeError<OneClickFailureKind> {
    pub(crate) fn new(kind: OneClickFailureKind, message: impl Into<String>) -> Self {
        Self {
            domain: kind.domain(),
            phase: kind.phase(),
            kind,
            recovery: RecoveryDisposition::NotNeeded,
            environment: EnvironmentExposure::NotExposed,
            cause_chain: Vec::new(),
            safe_detail: message.into(),
        }
    }

    pub(crate) fn with_recovery(mut self, recovery: ProjectedRecovery) -> Self {
        self.recovery = recovery.recovery;
        self.environment = recovery.environment;
        self
    }

    #[allow(dead_code)] // R1-A envelope seam; R1-B/C populate production causes.
    pub(crate) fn with_safe_cause(
        mut self,
        code: &'static str,
        safe_detail: impl Into<String>,
    ) -> Self {
        self.cause_chain.push(SafeCause {
            code,
            safe_detail: safe_detail.into(),
        });
        self
    }

    pub(crate) fn domain(&self) -> FailureDomain {
        self.domain
    }

    pub(crate) fn kind(&self) -> OneClickFailureKind {
        self.kind
    }

    pub(crate) fn phase(&self) -> RuntimePhase {
        self.phase
    }

    pub(crate) fn coarse_stage(&self) -> &'static str {
        self.phase().coarse_stage()
    }

    pub(crate) fn projected_recovery(&self) -> ProjectedRecovery {
        ProjectedRecovery {
            recovery: self.recovery,
            environment: self.environment,
        }
    }

    /// Project to the frozen one-click failure DTO (existing keys only).
    pub(crate) fn project_dto(&self) -> Value {
        debug_assert_eq!(self.domain(), self.kind.domain());
        debug_assert!(self
            .cause_chain
            .iter()
            .all(|cause| !cause.code.is_empty() && !cause.safe_detail.is_empty()));
        json!({
            "action": "failed",
            "stage": self.coarse_stage(),
            "status": "error",
            "recovery_status": self.recovery.dto_status(),
            "environment_status": self.environment.dto_status(),
            "message": self.safe_detail,
            "fallback_url": null,
        })
    }

    /// When config still has an open runtime journal and recovery was not set,
    /// treat as degraded (matches prior command-layer journal presence check).
    pub(crate) fn apply_open_journal_degraded(mut self, journal_open: bool) -> Self {
        if journal_open
            && self.recovery == RecoveryDisposition::NotNeeded
            && self.environment == EnvironmentExposure::NotExposed
        {
            self.recovery = RecoveryDisposition::Degraded;
        }
        self
    }
}

impl std::fmt::Display for RuntimeError<OneClickFailureKind> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.safe_detail)
    }
}

impl std::ops::Deref for RuntimeError<OneClickFailureKind> {
    type Target = str;

    fn deref(&self) -> &str {
        &self.safe_detail
    }
}

impl AsRef<str> for RuntimeError<OneClickFailureKind> {
    fn as_ref(&self) -> &str {
        &self.safe_detail
    }
}

impl From<RuntimeError<OneClickFailureKind>> for String {
    fn from(value: RuntimeError<OneClickFailureKind>) -> Self {
        value.safe_detail
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
        let cases = [
            (
                OneClickFailureKind::ConfigLoad,
                FailureDomain::Desktop,
                RuntimePhase::Prepare,
            ),
            (
                OneClickFailureKind::NoActiveProfile,
                FailureDomain::Desktop,
                RuntimePhase::Prepare,
            ),
            (
                OneClickFailureKind::LaunchPlan,
                FailureDomain::Desktop,
                RuntimePhase::Prepare,
            ),
            (
                OneClickFailureKind::AuthPreflight,
                FailureDomain::Desktop,
                RuntimePhase::Prepare,
            ),
            (
                OneClickFailureKind::PreflightSnapshot,
                FailureDomain::Desktop,
                RuntimePhase::Prepare,
            ),
            (
                OneClickFailureKind::Prepare,
                FailureDomain::Desktop,
                RuntimePhase::Prepare,
            ),
            (
                OneClickFailureKind::ScienceStop,
                FailureDomain::RuntimeTransaction,
                RuntimePhase::ScienceStop,
            ),
            (
                OneClickFailureKind::GatewayStart,
                FailureDomain::GatewayProvider,
                RuntimePhase::GatewayStart,
            ),
            (
                OneClickFailureKind::ProxySpawn,
                FailureDomain::GatewayProvider,
                RuntimePhase::GatewayStart,
            ),
            (
                OneClickFailureKind::ProxyHealth,
                FailureDomain::GatewayProvider,
                RuntimePhase::GatewayStart,
            ),
            (
                OneClickFailureKind::CatalogVerify,
                FailureDomain::GatewayProvider,
                RuntimePhase::CatalogVerify,
            ),
            (
                OneClickFailureKind::AuthoritySnapshot,
                FailureDomain::RuntimeTransaction,
                RuntimePhase::ScienceStart,
            ),
            (
                OneClickFailureKind::SandboxLogin,
                FailureDomain::RuntimeAdapter,
                RuntimePhase::ScienceStart,
            ),
            (
                OneClickFailureKind::SandboxLaunch,
                FailureDomain::RuntimeAdapter,
                RuntimePhase::ScienceStart,
            ),
            (
                OneClickFailureKind::SandboxHealth,
                FailureDomain::RuntimeAdapter,
                RuntimePhase::ScienceStart,
            ),
            (
                OneClickFailureKind::ScienceDbReverify,
                FailureDomain::RuntimeAdapter,
                RuntimePhase::ScienceStart,
            ),
            (
                OneClickFailureKind::OpenSurface,
                FailureDomain::RuntimeAdapter,
                RuntimePhase::ScienceStart,
            ),
            (
                OneClickFailureKind::ScienceStart,
                FailureDomain::RuntimeAdapter,
                RuntimePhase::ScienceStart,
            ),
        ];
        for (kind, domain, phase) in cases {
            let failure = TypedOneClickFailure::new(kind, "x");
            assert_eq!(failure.domain(), domain, "domain drift for {kind:?}");
            assert_eq!(failure.phase(), phase, "phase drift for {kind:?}");
            assert_eq!(failure.coarse_stage(), phase.coarse_stage());
        }
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
        assert_eq!(degraded.recovery, RecoveryDisposition::Degraded);
        assert_eq!(degraded.environment, EnvironmentExposure::NotExposed);

        let uncertain = TypedOneClickFailure::new(OneClickFailureKind::ScienceStart, "x")
            .with_recovery(ProjectedRecovery::ENVIRONMENT_UNCERTAIN)
            .apply_open_journal_degraded(true);
        assert_eq!(
            uncertain.recovery,
            RecoveryDisposition::EnvironmentUncertain
        );
        assert_eq!(uncertain.environment, EnvironmentExposure::Uncertain);
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
        assert_eq!(cleanup.recovery, RecoveryDisposition::CleanupRequired);
        assert_eq!(cleanup.environment, EnvironmentExposure::Uncertain);

        let manual = recovery_from_diagnostic_codes(
            "x；environment_uncertain；recovery_status=manual_recovery_required",
        )
        .unwrap();
        assert_eq!(manual.recovery, RecoveryDisposition::ManualRecoveryRequired);
        assert_eq!(manual.environment, EnvironmentExposure::Uncertain);

        let bare = recovery_from_diagnostic_codes("x；environment_uncertain").unwrap();
        assert_eq!(bare.recovery, RecoveryDisposition::EnvironmentUncertain);
        assert_eq!(bare.environment, EnvironmentExposure::Uncertain);

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
        assert_eq!(failure.recovery, RecoveryDisposition::CleanupRequired);
        assert_eq!(failure.project_dto()["stage"], "catalog_verify");
    }

    #[test]
    fn cause_chain_is_internal_and_safe_detail_is_the_frozen_message() {
        let failure =
            TypedOneClickFailure::new(OneClickFailureKind::GatewayStart, "现有用户可见文本")
                .with_safe_cause("gateway_spawn_failed", "已脱敏的内部原因");
        assert_eq!(failure.cause_chain.len(), 1);
        assert_eq!(failure.cause_chain[0].code, "gateway_spawn_failed");
        assert_eq!(failure.cause_chain[0].safe_detail, "已脱敏的内部原因");
        let dto = failure.project_dto();
        assert_eq!(dto["message"], "现有用户可见文本");
        assert!(!dto.to_string().contains("gateway_spawn_failed"));
        assert!(!dto.to_string().contains("已脱敏的内部原因"));
    }
}
