//! Pure, deterministic plan-contract projection for package inspection.
//!
//! inspect-only plan has no product caller, resolver, confirmation capability, or effect
//! owner. Its builder therefore emits only non-consumable inspect-only plans.
//! The schema deliberately reserves typed exact-source, target, effect, expiry,
//! and reentry fields for later slices without treating their presence as apply
//! authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::inspection::{
    CompatibilityStatus, ComponentKind, InspectionEffects, InspectionOutcome, InspectionReportV1,
    InspectionSeverity, CALLER_ASSERTED_UNVERIFIED, INSPECTION_REPORT_SCHEMA,
};

pub const SKILL_PLAN_SCHEMA: &str = "csswitch.skill-plan.v1";
pub const MAX_PLAN_COMPONENTS: usize = 4_096;
pub const MAX_PLAN_FINDINGS: usize = 1_024;
pub const MAX_PLAN_FINDINGS_PER_COMPONENT: usize = 1_024;
pub const MAX_PLAN_EFFECTS: usize = 4_096;
const SHA1_HEX_LENGTH: usize = 40;
const SHA256_HEX_LENGTH: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillPlanV1 {
    pub schema: String,
    pub identity: PlanIdentityV1,
    pub plan_digest_sha256: String,
    pub inspection_report_digest_sha256: String,
    pub component_graph_digest_sha256: String,
    pub inspection_outcome: InspectionOutcome,
    pub source: PlanSourceV1,
    pub target: PlanTargetV1,
    pub eligibility: PlanEligibility,
    pub confirmation: PlanConfirmationState,
    pub selection: PlanSelectionV1,
    pub expiry: PlanExpiryV1,
    pub reentry_policy: ReentryPolicyV1,
    pub components: Vec<PlanComponentV1>,
    pub effects: Vec<PlanEffectV1>,
    pub summary: PlanSummaryV1,
}

/// A content-addressed inspection identity is an immutable display/cache key,
/// never a confirmation capability. Invocation IDs are reserved for the later
/// coordinator-owned confirmation slice.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanIdentityV1 {
    ContentAddressedInspection { digest_sha256: String },
    Invocation { plan_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanSourceV1 {
    GithubExact {
        owner: String,
        repo: String,
        resolved_commit_sha: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        path: String,
        /// The exact staged archive accepted by CSSwitch.  Inspect-only plans
        /// intentionally leave this empty because they have no staged object.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        archive_sha256: String,
        /// Inspection and materialisation intentionally use different
        /// canonical domains.  Exact plans bind both rather than pretending
        /// either digest can verify the other boundary.
        content_sha256: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        materialized_content_sha256: String,
        binding: SourceBindingV1,
    },
    LocalOpenedArchive {
        archive_sha256: String,
        opened_object_identity_sha256: String,
        content_sha256: String,
        binding: SourceBindingV1,
    },
    /// A removal target is an installed, CSSwitch-owned directory, not an
    /// archive.  Keeping it distinct prevents a marker hash from being
    /// misrepresented as either downloaded bytes or an opened archive object.
    InstalledOwnedSkill {
        skill_name: String,
        marker_sha256: String,
        content_sha256: String,
        target_package_identity_sha256: String,
        binding: SourceBindingV1,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceBindingV1 {
    CallerAssertedUnverified,
    CsswitchExactContentBound,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanTargetV1 {
    Unbound,
    Bound {
        science_runtime_identity_sha256: String,
        data_dir_identity_sha256: String,
        active_org_identity_sha256: String,
        skills_root_device: u64,
        skills_root_inode: u64,
        operon: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanEligibility {
    InspectOnly,
    Confirmable,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanConfirmationState {
    NotCapable,
    RequiresExactPlanCapability,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanSelectionV1 {
    NotApplicable,
    Full,
    Degraded {
        selected_component_ids: Vec<String>,
        excluded_component_ids: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanExpiryV1 {
    NotApplicable,
    ExpiresAtUnixSeconds { unix_seconds: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReentryPolicyV1 {
    InspectOnlyNonConsumable,
    ReadbackBeforeRetry,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanComponentV1 {
    pub id: String,
    pub kind: ComponentKind,
    pub source_path: String,
    pub local_name: String,
    pub compatibility_status: CompatibilityStatus,
    pub degradation: ComponentDegradationV1,
    pub confirmation_reasons: Vec<ConfirmationReasonV1>,
    pub executable: bool,
    pub declared_preapproved_tools: Vec<String>,
    pub findings: Vec<PlanFindingV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComponentDegradationV1 {
    None,
    InspectOnlyUnverifiedSource,
    ExcludedUnsupported,
    ExcludedExecutable,
    ExcludedNonAsciiPath,
    ExcludedUnknownHostComponent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationReasonV1 {
    CallerAssertedUnverified,
    InspectionPartial,
    UnsupportedComponent,
    UnknownHostComponent,
    ExecutablePayload,
    NonAsciiPath,
    ExactTargetBinding,
    ExactEffectSet,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct PlanFindingV1 {
    pub severity: InspectionSeverity,
    pub code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanEffectV1 {
    pub order: u32,
    pub kind: PlanEffectKindV1,
    pub subject: PlanEffectSubjectV1,
    pub authority: EffectAuthorityV1,
    pub verifier: EffectVerifierV1,
    pub expected: ExpectedEffectV1,
    pub rollback: EffectRollbackV1,
    pub apply: EffectApplyStateV1,
    pub confirmation_reasons: Vec<ConfirmationReasonV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlanEffectSubjectV1 {
    Package {
        component_id: String,
        target_package_identity_sha256: String,
    },
    OperonSkill {
        component_id: String,
        skill_name: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanEffectKindV1 {
    PackageCommit,
    PackageQuarantine,
    OperonAttach,
    OperonDetach,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectAuthorityV1 {
    NotApplicable,
    CsswitchHost,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectVerifierV1 {
    NotApplicable,
    PackageReadback,
    OperonReadback,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedEffectV1 {
    NotApplicable,
    PackageContentBound {
        content_sha256: String,
    },
    PackageAbsent {
        target_package_identity_sha256: String,
    },
    OperonMembershipPresent {
        skill_name: String,
    },
    OperonMembershipAbsent {
        skill_name: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectRollbackV1 {
    NoInverse,
    CompensateByQuarantine,
    CompensateByDetach,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffectApplyStateV1 {
    NotRun,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanSummaryV1 {
    pub component_count: usize,
    pub effect_count: usize,
    pub unsupported_component_count: usize,
    pub degraded_component_count: usize,
    pub not_run_effect_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanError {
    pub code: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmablePlanTargetV1 {
    pub science_runtime_identity_sha256: String,
    pub data_dir_identity_sha256: String,
    pub active_org_identity_sha256: String,
    pub skills_root_device: u64,
    pub skills_root_inode: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfirmablePlanRequestV1 {
    pub plan_id: String,
    pub expires_at_unix_seconds: u64,
    pub target: ConfirmablePlanTargetV1,
}

/// Sealed input for the single-Skill removal protocol.  The marker identity
/// is not treated as remote provenance; it is the exact ownership/content
/// snapshot that must be re-read before either removal effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRemovalPlanRequestV1 {
    pub plan_id: String,
    pub expires_at_unix_seconds: u64,
    pub target: ConfirmablePlanTargetV1,
    pub skill_name: String,
    pub marker_sha256: String,
    pub content_sha256: String,
}

impl PlanError {
    fn new(code: &str) -> Self {
        Self { code: code.into() }
    }
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.code)
    }
}

impl std::error::Error for PlanError {}

/// Builds the only inspect-only plan output: a non-consumable content-addressed inspection
/// plan. This function intentionally rejects any report provenance other than
/// the current caller-asserted inspection contract.
pub fn build_skill_plan(report: &InspectionReportV1) -> Result<SkillPlanV1, PlanError> {
    let report = canonical_report(report);
    validate_inspection_report(&report)?;

    let findings = findings_by_component(&report)?;
    let mut components = report
        .graph
        .nodes
        .iter()
        .map(|node| component_for_node(node, &findings, &report))
        .collect::<Vec<_>>();
    components.sort_by(|left, right| left.id.cmp(&right.id));
    let effects = Vec::new();
    let summary = summarize(&components, &effects);
    let source = PlanSourceV1::GithubExact {
        owner: report.source_claim.owner.clone(),
        repo: report.source_claim.repo.clone(),
        resolved_commit_sha: report.source_claim.commit_sha.clone(),
        path: report.source_claim.path.clone(),
        archive_sha256: String::new(),
        content_sha256: report.package.content_sha256.clone(),
        materialized_content_sha256: String::new(),
        binding: SourceBindingV1::CallerAssertedUnverified,
    };
    let inspection_report_digest_sha256 = digest_json(&report)?;
    let component_graph_digest_sha256 = report.graph.sha256.clone();
    let target = PlanTargetV1::Unbound;
    let eligibility = PlanEligibility::InspectOnly;
    let confirmation = PlanConfirmationState::NotCapable;
    let selection = PlanSelectionV1::NotApplicable;
    let expiry = PlanExpiryV1::NotApplicable;
    let reentry_policy = ReentryPolicyV1::InspectOnlyNonConsumable;
    let digest = digest_json(&PlanDigestInput {
        schema: SKILL_PLAN_SCHEMA,
        identity: PlanDigestIdentityV1::ContentAddressedInspection,
        inspection_report_digest_sha256: &inspection_report_digest_sha256,
        component_graph_digest_sha256: &component_graph_digest_sha256,
        inspection_outcome: &report.outcome,
        source: &source,
        target: &target,
        eligibility: &eligibility,
        confirmation: &confirmation,
        selection: &selection,
        expiry: &expiry,
        reentry_policy: &reentry_policy,
        components: &components,
        effects: &effects,
        summary: &summary,
    })?;
    let plan = SkillPlanV1 {
        schema: SKILL_PLAN_SCHEMA.into(),
        identity: PlanIdentityV1::ContentAddressedInspection {
            digest_sha256: digest.clone(),
        },
        plan_digest_sha256: digest,
        inspection_report_digest_sha256,
        component_graph_digest_sha256,
        inspection_outcome: report.outcome,
        source,
        target,
        eligibility,
        confirmation,
        selection,
        expiry,
        reentry_policy,
        components,
        effects,
        summary,
    };
    validate_skill_plan(&plan)?;
    Ok(plan)
}

/// Seals one exact, staged GitHub inspection into a confirmable plan. The
/// caller may retain and display this plan, but this crate still exposes no
/// capability that can consume it or perform any effect.
pub(crate) fn build_confirmable_skill_plan(
    report: &InspectionReportV1,
    source: PlanSourceV1,
    request: &ConfirmablePlanRequestV1,
    materialized_content_sha256: String,
) -> Result<SkillPlanV1, PlanError> {
    let mut plan = build_skill_plan(report)?;
    let source = match source {
        PlanSourceV1::GithubExact {
            owner,
            repo,
            resolved_commit_sha,
            path,
            archive_sha256,
            content_sha256,
            materialized_content_sha256: _,
            binding: SourceBindingV1::CsswitchExactContentBound,
        } if owner == report.source_claim.owner
            && repo == report.source_claim.repo
            && resolved_commit_sha == report.source_claim.commit_sha
            && path == report.source_claim.path
            && is_lower_sha256(&archive_sha256)
            && content_sha256 == report.package.content_sha256
            && is_lower_sha256(&materialized_content_sha256) =>
        {
            PlanSourceV1::GithubExact {
                owner,
                repo,
                resolved_commit_sha,
                path,
                archive_sha256,
                content_sha256,
                materialized_content_sha256: materialized_content_sha256.clone(),
                binding: SourceBindingV1::CsswitchExactContentBound,
            }
        }
        _ => return Err(PlanError::new("EXACT_SOURCE_REPORT_MISMATCH")),
    };
    let identity = PlanIdentityV1::Invocation {
        plan_id: request.plan_id.clone(),
    };
    validate_identity(&identity, &"0".repeat(SHA256_HEX_LENGTH))?;
    let target = PlanTargetV1::Bound {
        science_runtime_identity_sha256: request.target.science_runtime_identity_sha256.clone(),
        data_dir_identity_sha256: request.target.data_dir_identity_sha256.clone(),
        active_org_identity_sha256: request.target.active_org_identity_sha256.clone(),
        skills_root_device: request.target.skills_root_device,
        skills_root_inode: request.target.skills_root_inode,
        operon: "OPERON".into(),
    };
    validate_target(&target)?;
    if request.expires_at_unix_seconds == 0 {
        return Err(PlanError::new("INVALID_PLAN_EXPIRY"));
    }

    for component in &mut plan.components {
        component.degradation =
            expected_component_degradation(component, &SourceBindingV1::CsswitchExactContentBound);
        component.confirmation_reasons = expected_component_reasons(
            component,
            &SourceBindingV1::CsswitchExactContentBound,
            &plan.inspection_outcome,
        );
    }
    let selected_component_ids = plan
        .components
        .iter()
        .filter(|component| {
            component.kind == ComponentKind::Skill
                && component.compatibility_status == CompatibilityStatus::Adapted
                && !component_requires_degraded_selection(component)
        })
        .map(|component| component.id.clone())
        .collect::<Vec<_>>();
    if selected_component_ids.is_empty() {
        return Err(PlanError::new("NO_CONFIRMABLE_SKILL_COMPONENT"));
    }
    if plan.inspection_outcome != InspectionOutcome::Complete
        || selected_component_ids.len() != 1
        || plan.components.len() != 2
        || plan
            .components
            .iter()
            .filter(|component| component.kind == ComponentKind::Package)
            .count()
            != 1
        || plan.components.iter().any(|component| {
            !selected_component_ids.contains(&component.id)
                && (component.kind != ComponentKind::Package
                    || !component.source_path.is_empty()
                    || component.compatibility_status != CompatibilityStatus::Unsupported
                    || component.executable)
        })
    {
        return Err(PlanError::new("CONFIRMABLE_PROJECTION_REQUIRED"));
    }
    let selected = selected_component_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let excluded_component_ids = plan
        .components
        .iter()
        .filter(|component| !selected.contains(component.id.as_str()))
        .map(|component| component.id.clone())
        .collect::<Vec<_>>();
    let selection = PlanSelectionV1::Degraded {
        selected_component_ids: selected_component_ids.clone(),
        excluded_component_ids,
    };
    let mut effects = Vec::new();
    for component_id in selected_component_ids {
        let component = plan
            .components
            .iter()
            .find(|candidate| candidate.id == component_id)
            .ok_or_else(|| PlanError::new("INVALID_PLAN_SELECTION"))?;
        let target_package_identity_sha256 = digest_json(&TargetPackageIdentityInputV1 {
            schema: "csswitch.target-package-identity.v1",
            data_dir_identity_sha256: &request.target.data_dir_identity_sha256,
            active_org_identity_sha256: &request.target.active_org_identity_sha256,
            component_id: &component.id,
            local_name: &component.local_name,
        })?;
        let mut confirmation_reasons = vec![
            ConfirmationReasonV1::ExactTargetBinding,
            ConfirmationReasonV1::ExactEffectSet,
        ];
        confirmation_reasons.sort();
        effects.push(PlanEffectV1 {
            order: effects.len() as u32 + 1,
            kind: PlanEffectKindV1::PackageCommit,
            subject: PlanEffectSubjectV1::Package {
                component_id: component.id.clone(),
                target_package_identity_sha256,
            },
            authority: EffectAuthorityV1::CsswitchHost,
            verifier: EffectVerifierV1::PackageReadback,
            expected: ExpectedEffectV1::PackageContentBound {
                content_sha256: materialized_content_sha256.clone(),
            },
            rollback: EffectRollbackV1::CompensateByQuarantine,
            apply: EffectApplyStateV1::NotRun,
            confirmation_reasons: confirmation_reasons.clone(),
        });
        effects.push(PlanEffectV1 {
            order: effects.len() as u32 + 1,
            kind: PlanEffectKindV1::OperonAttach,
            subject: PlanEffectSubjectV1::OperonSkill {
                component_id: component.id.clone(),
                skill_name: component.local_name.clone(),
            },
            authority: EffectAuthorityV1::CsswitchHost,
            verifier: EffectVerifierV1::OperonReadback,
            expected: ExpectedEffectV1::OperonMembershipPresent {
                skill_name: component.local_name.clone(),
            },
            rollback: EffectRollbackV1::CompensateByDetach,
            apply: EffectApplyStateV1::NotRun,
            confirmation_reasons,
        });
    }
    if effects.len() > MAX_PLAN_EFFECTS {
        return Err(PlanError::new("PLAN_EFFECT_LIMIT"));
    }

    plan.identity = identity;
    plan.source = source;
    plan.target = target;
    plan.eligibility = PlanEligibility::Confirmable;
    plan.confirmation = PlanConfirmationState::RequiresExactPlanCapability;
    plan.selection = selection;
    plan.expiry = PlanExpiryV1::ExpiresAtUnixSeconds {
        unix_seconds: request.expires_at_unix_seconds,
    };
    plan.reentry_policy = ReentryPolicyV1::ReadbackBeforeRetry;
    plan.effects = effects;
    plan.summary = summarize(&plan.components, &plan.effects);
    plan.plan_digest_sha256 = compute_plan_digest(&plan)?;
    validate_skill_plan(&plan)?;
    Ok(plan)
}

/// Builds the exact two-effect removal plan.  It has no inspection report or
/// archive claim to reuse: removal is bound to the already-installed,
/// CSSwitch-owned directory marker and canonical payload hash.  The plan is
/// still represented by the same SkillPlanV1 digest contract as install.
pub fn build_skill_removal_plan(
    request: &SkillRemovalPlanRequestV1,
) -> Result<SkillPlanV1, PlanError> {
    if request.expires_at_unix_seconds == 0
        || !is_lower_sha256(&request.marker_sha256)
        || !is_lower_sha256(&request.content_sha256)
        || !safe_local_name(&request.skill_name)
    {
        return Err(PlanError::new("INVALID_REMOVAL_PLAN_INPUT"));
    }
    let identity = PlanIdentityV1::Invocation {
        plan_id: request.plan_id.clone(),
    };
    validate_identity(&identity, &"0".repeat(SHA256_HEX_LENGTH))?;
    let source = PlanSourceV1::InstalledOwnedSkill {
        skill_name: request.skill_name.clone(),
        marker_sha256: request.marker_sha256.clone(),
        content_sha256: request.content_sha256.clone(),
        target_package_identity_sha256: "0".repeat(SHA256_HEX_LENGTH),
        binding: SourceBindingV1::CsswitchExactContentBound,
    };
    let target = PlanTargetV1::Bound {
        science_runtime_identity_sha256: request.target.science_runtime_identity_sha256.clone(),
        data_dir_identity_sha256: request.target.data_dir_identity_sha256.clone(),
        active_org_identity_sha256: request.target.active_org_identity_sha256.clone(),
        skills_root_device: request.target.skills_root_device,
        skills_root_inode: request.target.skills_root_inode,
        operon: "OPERON".into(),
    };
    validate_target(&target)?;
    let component = PlanComponentV1 {
        id: "installed-skill".into(),
        kind: ComponentKind::Skill,
        source_path: "installed-skill/SKILL.md".into(),
        local_name: request.skill_name.clone(),
        compatibility_status: CompatibilityStatus::Adapted,
        degradation: ComponentDegradationV1::None,
        confirmation_reasons: Vec::new(),
        executable: false,
        declared_preapproved_tools: Vec::new(),
        findings: Vec::new(),
    };
    let package_identity = digest_json(&TargetPackageIdentityInputV1 {
        schema: "csswitch.target-package-identity.v1",
        data_dir_identity_sha256: &request.target.data_dir_identity_sha256,
        active_org_identity_sha256: &request.target.active_org_identity_sha256,
        component_id: &component.id,
        local_name: &component.local_name,
    })?;
    let source = match source {
        PlanSourceV1::InstalledOwnedSkill {
            skill_name,
            marker_sha256,
            content_sha256,
            binding,
            ..
        } => PlanSourceV1::InstalledOwnedSkill {
            skill_name,
            marker_sha256,
            content_sha256,
            target_package_identity_sha256: package_identity.clone(),
            binding,
        },
        _ => unreachable!("removal source is fixed above"),
    };
    let reasons = vec![
        ConfirmationReasonV1::ExactTargetBinding,
        ConfirmationReasonV1::ExactEffectSet,
    ];
    let effects = vec![
        PlanEffectV1 {
            order: 1,
            kind: PlanEffectKindV1::OperonDetach,
            subject: PlanEffectSubjectV1::OperonSkill {
                component_id: component.id.clone(),
                skill_name: component.local_name.clone(),
            },
            authority: EffectAuthorityV1::CsswitchHost,
            verifier: EffectVerifierV1::OperonReadback,
            expected: ExpectedEffectV1::OperonMembershipAbsent {
                skill_name: component.local_name.clone(),
            },
            rollback: EffectRollbackV1::NoInverse,
            apply: EffectApplyStateV1::NotRun,
            confirmation_reasons: reasons.clone(),
        },
        PlanEffectV1 {
            order: 2,
            kind: PlanEffectKindV1::PackageQuarantine,
            subject: PlanEffectSubjectV1::Package {
                component_id: component.id.clone(),
                target_package_identity_sha256: package_identity.clone(),
            },
            authority: EffectAuthorityV1::CsswitchHost,
            verifier: EffectVerifierV1::PackageReadback,
            expected: ExpectedEffectV1::PackageAbsent {
                target_package_identity_sha256: package_identity,
            },
            rollback: EffectRollbackV1::NoInverse,
            apply: EffectApplyStateV1::NotRun,
            confirmation_reasons: reasons,
        },
    ];
    let components = vec![component];
    let summary = summarize(&components, &effects);
    let mut plan = SkillPlanV1 {
        schema: SKILL_PLAN_SCHEMA.into(),
        identity,
        plan_digest_sha256: "0".repeat(SHA256_HEX_LENGTH),
        inspection_report_digest_sha256: request.marker_sha256.clone(),
        component_graph_digest_sha256: request.content_sha256.clone(),
        inspection_outcome: InspectionOutcome::Complete,
        source,
        target,
        eligibility: PlanEligibility::Confirmable,
        confirmation: PlanConfirmationState::RequiresExactPlanCapability,
        selection: PlanSelectionV1::Full,
        expiry: PlanExpiryV1::ExpiresAtUnixSeconds {
            unix_seconds: request.expires_at_unix_seconds,
        },
        reentry_policy: ReentryPolicyV1::ReadbackBeforeRetry,
        components,
        effects,
        summary,
    };
    plan.plan_digest_sha256 = compute_plan_digest(&plan)?;
    validate_skill_plan(&plan)?;
    Ok(plan)
}

#[derive(Serialize)]
struct TargetPackageIdentityInputV1<'a> {
    schema: &'static str,
    data_dir_identity_sha256: &'a str,
    active_org_identity_sha256: &'a str,
    component_id: &'a str,
    local_name: &'a str,
}

/// Validates a deserialized plan without opening files, resolving a source, or
/// consuming confirmation. A confirmable plan is schema validation only here;
/// inspect-only plan provides no caller that can create or consume one.
pub fn validate_skill_plan(plan: &SkillPlanV1) -> Result<(), PlanError> {
    if plan.schema != SKILL_PLAN_SCHEMA
        || !is_lower_sha256(&plan.plan_digest_sha256)
        || !is_lower_sha256(&plan.inspection_report_digest_sha256)
        || !is_lower_sha256(&plan.component_graph_digest_sha256)
    {
        return Err(PlanError::new("INVALID_PLAN_IDENTITY"));
    }
    validate_identity(&plan.identity, &plan.plan_digest_sha256)?;
    validate_source(&plan.source)?;
    validate_target(&plan.target)?;
    validate_components(plan)?;
    validate_effects(plan)?;
    validate_eligibility(plan)?;
    if plan.summary != summarize(&plan.components, &plan.effects) {
        return Err(PlanError::new("PLAN_SUMMARY_MISMATCH"));
    }
    let expected_digest = compute_plan_digest(plan)?;
    if expected_digest != plan.plan_digest_sha256 {
        return Err(PlanError::new("PLAN_DIGEST_MISMATCH"));
    }
    Ok(())
}

fn compute_plan_digest(plan: &SkillPlanV1) -> Result<String, PlanError> {
    digest_json(&PlanDigestInput {
        schema: SKILL_PLAN_SCHEMA,
        identity: digest_identity(&plan.identity),
        inspection_report_digest_sha256: &plan.inspection_report_digest_sha256,
        component_graph_digest_sha256: &plan.component_graph_digest_sha256,
        inspection_outcome: &plan.inspection_outcome,
        source: &plan.source,
        target: &plan.target,
        eligibility: &plan.eligibility,
        confirmation: &plan.confirmation,
        selection: &plan.selection,
        expiry: &plan.expiry,
        reentry_policy: &plan.reentry_policy,
        components: &plan.components,
        effects: &plan.effects,
        summary: &plan.summary,
    })
}

#[derive(Serialize)]
struct PlanDigestInput<'a> {
    schema: &'a str,
    identity: PlanDigestIdentityV1<'a>,
    inspection_report_digest_sha256: &'a str,
    component_graph_digest_sha256: &'a str,
    inspection_outcome: &'a InspectionOutcome,
    source: &'a PlanSourceV1,
    target: &'a PlanTargetV1,
    eligibility: &'a PlanEligibility,
    confirmation: &'a PlanConfirmationState,
    selection: &'a PlanSelectionV1,
    expiry: &'a PlanExpiryV1,
    reentry_policy: &'a ReentryPolicyV1,
    components: &'a [PlanComponentV1],
    effects: &'a [PlanEffectV1],
    summary: &'a PlanSummaryV1,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PlanDigestIdentityV1<'a> {
    ContentAddressedInspection,
    Invocation { plan_id: &'a str },
}

fn digest_identity(identity: &PlanIdentityV1) -> PlanDigestIdentityV1<'_> {
    match identity {
        PlanIdentityV1::ContentAddressedInspection { .. } => {
            PlanDigestIdentityV1::ContentAddressedInspection
        }
        PlanIdentityV1::Invocation { plan_id } => PlanDigestIdentityV1::Invocation { plan_id },
    }
}

fn canonical_report(report: &InspectionReportV1) -> InspectionReportV1 {
    let mut canonical = report.clone();
    canonical.graph.nodes.sort();
    canonical.graph.edges.sort();
    canonical.findings.sort();
    canonical
}

fn component_for_node(
    node: &crate::inspection::ComponentNode,
    findings: &BTreeMap<String, Vec<PlanFindingV1>>,
    report: &InspectionReportV1,
) -> PlanComponentV1 {
    let mut confirmation_reasons = component_reasons(node, report);
    confirmation_reasons.sort();
    confirmation_reasons.dedup();
    let mut declared_preapproved_tools = node.declared_preapproved_tools.clone();
    declared_preapproved_tools.sort();
    declared_preapproved_tools.dedup();
    PlanComponentV1 {
        id: node.id.clone(),
        kind: node.kind.clone(),
        source_path: node.source_path.clone(),
        local_name: node.local_name.clone(),
        compatibility_status: node.compatibility_status.clone(),
        degradation: component_degradation(node, report),
        confirmation_reasons,
        executable: node.executable,
        declared_preapproved_tools,
        findings: findings.get(&node.id).cloned().unwrap_or_default(),
    }
}

fn component_reasons(
    node: &crate::inspection::ComponentNode,
    report: &InspectionReportV1,
) -> Vec<ConfirmationReasonV1> {
    let mut reasons = Vec::new();
    if report.source_claim.verification == CALLER_ASSERTED_UNVERIFIED {
        reasons.push(ConfirmationReasonV1::CallerAssertedUnverified);
    }
    if report.outcome == InspectionOutcome::Partial {
        reasons.push(ConfirmationReasonV1::InspectionPartial);
    }
    if node.compatibility_status == CompatibilityStatus::Unsupported {
        reasons.push(ConfirmationReasonV1::UnsupportedComponent);
    }
    if node.kind == ComponentKind::UnknownHostComponent {
        reasons.push(ConfirmationReasonV1::UnknownHostComponent);
    }
    if node.executable || node.kind == ComponentKind::Executable {
        reasons.push(ConfirmationReasonV1::ExecutablePayload);
    }
    if !node.source_path.is_ascii() {
        reasons.push(ConfirmationReasonV1::NonAsciiPath);
    }
    reasons
}

fn component_degradation(
    node: &crate::inspection::ComponentNode,
    report: &InspectionReportV1,
) -> ComponentDegradationV1 {
    if node.kind == ComponentKind::UnknownHostComponent {
        ComponentDegradationV1::ExcludedUnknownHostComponent
    } else if node.executable || node.kind == ComponentKind::Executable {
        ComponentDegradationV1::ExcludedExecutable
    } else if !node.source_path.is_ascii() {
        ComponentDegradationV1::ExcludedNonAsciiPath
    } else if node.compatibility_status == CompatibilityStatus::Unsupported {
        ComponentDegradationV1::ExcludedUnsupported
    } else if report.source_claim.verification == CALLER_ASSERTED_UNVERIFIED {
        ComponentDegradationV1::InspectOnlyUnverifiedSource
    } else {
        ComponentDegradationV1::None
    }
}

fn findings_by_component(
    report: &InspectionReportV1,
) -> Result<BTreeMap<String, Vec<PlanFindingV1>>, PlanError> {
    let mut paths = BTreeMap::new();
    for node in &report.graph.nodes {
        paths
            .entry(node.source_path.as_str())
            .or_insert_with(Vec::new)
            .push(&node.id);
    }
    let package_id = report
        .graph
        .nodes
        .iter()
        .find(|node| node.kind == ComponentKind::Package)
        .map(|node| node.id.as_str());
    let mut projected = BTreeMap::<String, BTreeSet<PlanFindingV1>>::new();
    for finding in &report.findings {
        let mut targets = Vec::new();
        if let Some(component_id) = &finding.component_id {
            targets.push(component_id.as_str());
        }
        if let Some(path) = &finding.source_path {
            if let Some(component_ids) = paths.get(path.as_str()) {
                targets.extend(
                    component_ids
                        .iter()
                        .map(|component_id| component_id.as_str()),
                );
            }
        }
        if targets.is_empty() {
            if let Some(package_id) = package_id {
                targets.push(package_id);
            } else {
                return Err(PlanError::new("UNPROJECTABLE_FINDING"));
            }
        }
        for target in targets {
            projected
                .entry(target.into())
                .or_default()
                .insert(PlanFindingV1 {
                    severity: finding.severity.clone(),
                    code: finding.code.clone(),
                });
        }
    }
    Ok(projected
        .into_iter()
        .map(|(component_id, values)| (component_id, values.into_iter().collect()))
        .collect())
}

fn summarize(components: &[PlanComponentV1], effects: &[PlanEffectV1]) -> PlanSummaryV1 {
    PlanSummaryV1 {
        component_count: components.len(),
        effect_count: effects.len(),
        unsupported_component_count: components
            .iter()
            .filter(|component| component.compatibility_status == CompatibilityStatus::Unsupported)
            .count(),
        degraded_component_count: components
            .iter()
            .filter(|component| component.degradation != ComponentDegradationV1::None)
            .count(),
        not_run_effect_count: effects
            .iter()
            .filter(|effect| effect.apply == EffectApplyStateV1::NotRun)
            .count(),
    }
}

fn validate_inspection_report(report: &InspectionReportV1) -> Result<(), PlanError> {
    if report.schema != INSPECTION_REPORT_SCHEMA
        || report.graph.schema != "csswitch.component-graph.v1"
        || report.source_claim.verification != CALLER_ASSERTED_UNVERIFIED
        || report.effects != InspectionEffects::default()
        || !is_lower_sha256(&report.graph.sha256)
        || !is_lower_sha256(&report.package.content_sha256)
        || report.graph.nodes.len() > MAX_PLAN_COMPONENTS
        || report.graph.nodes.len() > report.limits.graph_nodes
        || report.graph.edges.len() > report.limits.graph_edges
        || report.findings.len() > MAX_PLAN_FINDINGS
        || report.findings.len() > report.limits.findings
    {
        return Err(PlanError::new("INVALID_INSPECTION_REPORT"));
    }
    validate_github_identity(
        &report.source_claim.owner,
        &report.source_claim.repo,
        &report.source_claim.commit_sha,
        &report.source_claim.path,
    )?;
    let mut ids = BTreeSet::new();
    let mut previous_id = None;
    for node in &report.graph.nodes {
        if node.id.is_empty()
            || node.id.len() > 80
            || previous_id.as_deref() >= Some(node.id.as_str())
            || !ids.insert(node.id.as_str())
            || !safe_archive_path(&node.source_path)
        {
            return Err(PlanError::new("INVALID_INSPECTION_COMPONENT"));
        }
        previous_id = Some(node.id.clone());
    }
    let mut previous_edge = None;
    for edge in &report.graph.edges {
        if previous_edge >= Some(edge)
            || edge.relation.is_empty()
            || edge.relation.len() > 64
            || !edge
                .relation
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || !ids.contains(edge.from.as_str())
            || !ids.contains(edge.to.as_str())
        {
            return Err(PlanError::new("INVALID_INSPECTION_GRAPH"));
        }
        previous_edge = Some(edge);
    }
    let graph_bytes = serde_json::to_vec(&(&report.graph.nodes, &report.graph.edges))
        .map_err(|_| PlanError::new("PLAN_SERIALIZATION_FAILED"))?;
    if format!("{:x}", Sha256::digest(graph_bytes)) != report.graph.sha256 {
        return Err(PlanError::new("COMPONENT_GRAPH_DIGEST_MISMATCH"));
    }
    for finding in &report.findings {
        if finding.code.is_empty()
            || finding.code.len() > 100
            || finding
                .source_path
                .as_deref()
                .is_some_and(|path| !safe_archive_path(path))
            || finding
                .component_id
                .as_deref()
                .is_some_and(|id| !ids.contains(id))
        {
            return Err(PlanError::new("INVALID_FINDING_REFERENCE"));
        }
    }
    let has_blocking = report
        .findings
        .iter()
        .any(|finding| finding.severity == InspectionSeverity::Blocking);
    if (report.outcome == InspectionOutcome::Partial) != has_blocking {
        return Err(PlanError::new("INSPECTION_OUTCOME_MISMATCH"));
    }
    Ok(())
}

fn validate_identity(identity: &PlanIdentityV1, digest: &str) -> Result<(), PlanError> {
    match identity {
        PlanIdentityV1::ContentAddressedInspection { digest_sha256 }
            if digest_sha256 == digest && is_lower_sha256(digest_sha256) =>
        {
            Ok(())
        }
        PlanIdentityV1::Invocation { plan_id }
            if !plan_id.is_empty()
                && plan_id.len() <= 128
                && plan_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')) =>
        {
            Ok(())
        }
        _ => Err(PlanError::new("INVALID_PLAN_IDENTITY")),
    }
}

fn validate_source(source: &PlanSourceV1) -> Result<(), PlanError> {
    match source {
        PlanSourceV1::GithubExact {
            owner,
            repo,
            resolved_commit_sha,
            path,
            archive_sha256,
            content_sha256,
            materialized_content_sha256,
            binding,
        } => {
            validate_github_identity(owner, repo, resolved_commit_sha, path)?;
            if !is_lower_sha256(content_sha256)
                || (!archive_sha256.is_empty() && !is_lower_sha256(archive_sha256))
                || (*binding == SourceBindingV1::CsswitchExactContentBound
                    && (!is_lower_sha256(archive_sha256)
                        || !is_lower_sha256(materialized_content_sha256)))
            {
                return Err(PlanError::new("INVALID_PLAN_SOURCE"));
            }
        }
        PlanSourceV1::LocalOpenedArchive {
            archive_sha256,
            opened_object_identity_sha256,
            content_sha256,
            ..
        } => {
            if !is_lower_sha256(archive_sha256)
                || !is_lower_sha256(opened_object_identity_sha256)
                || !is_lower_sha256(content_sha256)
            {
                return Err(PlanError::new("INVALID_PLAN_SOURCE"));
            }
        }
        PlanSourceV1::InstalledOwnedSkill {
            skill_name,
            marker_sha256,
            content_sha256,
            target_package_identity_sha256,
            ..
        } => {
            if !safe_local_name(skill_name)
                || !is_lower_sha256(marker_sha256)
                || !is_lower_sha256(content_sha256)
                || !is_lower_sha256(target_package_identity_sha256)
            {
                return Err(PlanError::new("INVALID_PLAN_SOURCE"));
            }
        }
    }
    Ok(())
}

fn validate_github_identity(
    owner: &str,
    repo: &str,
    commit: &str,
    path: &str,
) -> Result<(), PlanError> {
    let safe_name = |value: &str| {
        !value.is_empty()
            && value.len() <= 100
            && !matches!(value, "." | "..")
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
    };
    if !safe_name(owner) || !safe_name(repo) || !is_lower_sha1(commit) || !safe_archive_path(path) {
        return Err(PlanError::new("INVALID_PLAN_SOURCE"));
    }
    Ok(())
}

fn validate_target(target: &PlanTargetV1) -> Result<(), PlanError> {
    match target {
        PlanTargetV1::Unbound => Ok(()),
        PlanTargetV1::Bound {
            science_runtime_identity_sha256,
            data_dir_identity_sha256,
            active_org_identity_sha256,
            skills_root_device,
            skills_root_inode,
            operon,
        } if is_lower_sha256(science_runtime_identity_sha256)
            && is_lower_sha256(data_dir_identity_sha256)
            && is_lower_sha256(active_org_identity_sha256)
            && *skills_root_device > 0
            && *skills_root_inode > 0
            && operon == "OPERON" =>
        {
            Ok(())
        }
        _ => Err(PlanError::new("INVALID_PLAN_TARGET")),
    }
}

fn validate_components(plan: &SkillPlanV1) -> Result<(), PlanError> {
    let components = &plan.components;
    if components.len() > MAX_PLAN_COMPONENTS {
        return Err(PlanError::new("PLAN_COMPONENT_LIMIT"));
    }
    let mut previous_id = None;
    let mut total_findings = 0usize;
    for component in components {
        if component.id.is_empty()
            || component.id.len() > 80
            || previous_id.as_deref() >= Some(component.id.as_str())
            || !safe_archive_path(&component.source_path)
            || !safe_local_name(&component.local_name)
            || component.findings.len() > MAX_PLAN_FINDINGS_PER_COMPONENT
            || component.findings.windows(2).any(|pair| pair[0] >= pair[1])
            || component
                .findings
                .iter()
                .any(|finding| finding.code.is_empty() || finding.code.len() > 100)
            || component.declared_preapproved_tools.len() > 128
            || component.declared_preapproved_tools.iter().any(|tool| {
                tool.is_empty()
                    || tool.len() > 100
                    || !tool
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
            || component
                .declared_preapproved_tools
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || component
                .confirmation_reasons
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(PlanError::new("INVALID_PLAN_COMPONENT"));
        }
        total_findings = total_findings
            .checked_add(component.findings.len())
            .ok_or_else(|| PlanError::new("PLAN_FINDING_LIMIT"))?;
        if total_findings > MAX_PLAN_FINDINGS
            || component.degradation
                != expected_component_degradation(component, plan.source_binding())
            || component.confirmation_reasons
                != expected_component_reasons(
                    component,
                    plan.source_binding(),
                    &plan.inspection_outcome,
                )
        {
            return Err(PlanError::new("PLAN_COMPONENT_PROJECTION_MISMATCH"));
        }
        previous_id = Some(component.id.clone());
    }
    Ok(())
}

fn expected_component_degradation(
    component: &PlanComponentV1,
    binding: &SourceBindingV1,
) -> ComponentDegradationV1 {
    if component.kind == ComponentKind::UnknownHostComponent {
        ComponentDegradationV1::ExcludedUnknownHostComponent
    } else if component.executable || component.kind == ComponentKind::Executable {
        ComponentDegradationV1::ExcludedExecutable
    } else if !component.source_path.is_ascii() {
        ComponentDegradationV1::ExcludedNonAsciiPath
    } else if component.compatibility_status == CompatibilityStatus::Unsupported {
        ComponentDegradationV1::ExcludedUnsupported
    } else if *binding == SourceBindingV1::CallerAssertedUnverified {
        ComponentDegradationV1::InspectOnlyUnverifiedSource
    } else {
        ComponentDegradationV1::None
    }
}

fn expected_component_reasons(
    component: &PlanComponentV1,
    binding: &SourceBindingV1,
    outcome: &InspectionOutcome,
) -> Vec<ConfirmationReasonV1> {
    let mut reasons = Vec::new();
    if *binding == SourceBindingV1::CallerAssertedUnverified {
        reasons.push(ConfirmationReasonV1::CallerAssertedUnverified);
    }
    if *outcome == InspectionOutcome::Partial {
        reasons.push(ConfirmationReasonV1::InspectionPartial);
    }
    if component.compatibility_status == CompatibilityStatus::Unsupported {
        reasons.push(ConfirmationReasonV1::UnsupportedComponent);
    }
    if component.kind == ComponentKind::UnknownHostComponent {
        reasons.push(ConfirmationReasonV1::UnknownHostComponent);
    }
    if component.executable || component.kind == ComponentKind::Executable {
        reasons.push(ConfirmationReasonV1::ExecutablePayload);
    }
    if !component.source_path.is_ascii() {
        reasons.push(ConfirmationReasonV1::NonAsciiPath);
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

fn validate_effects(plan: &SkillPlanV1) -> Result<(), PlanError> {
    if plan.effects.len() > MAX_PLAN_EFFECTS {
        return Err(PlanError::new("PLAN_EFFECT_LIMIT"));
    }
    let components = plan
        .components
        .iter()
        .map(|component| (component.id.as_str(), component))
        .collect::<BTreeMap<_, _>>();
    let mut expected_order = 1u32;
    for effect in &plan.effects {
        if effect.order != expected_order
            || effect
                .confirmation_reasons
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(PlanError::new("INVALID_PLAN_EFFECT"));
        }
        let component = components
            .get(effect.component_id())
            .copied()
            .ok_or_else(|| PlanError::new("INVALID_PLAN_EFFECT"))?;
        if !valid_effect_subject(effect, component) {
            return Err(PlanError::new("INVALID_PLAN_EFFECT_SUBJECT"));
        }
        expected_order = expected_order
            .checked_add(1)
            .ok_or_else(|| PlanError::new("PLAN_EFFECT_LIMIT"))?;
        if plan.eligibility == PlanEligibility::InspectOnly
            && (effect.authority != EffectAuthorityV1::NotApplicable
                || effect.verifier != EffectVerifierV1::NotApplicable
                || effect.expected != ExpectedEffectV1::NotApplicable
                || effect.rollback != EffectRollbackV1::NoInverse
                || effect.apply != EffectApplyStateV1::NotRun)
        {
            return Err(PlanError::new("INSPECT_ONLY_EFFECT_FORBIDDEN"));
        }
        if plan.eligibility == PlanEligibility::Confirmable
            && (effect.authority != EffectAuthorityV1::CsswitchHost
                || effect.verifier == EffectVerifierV1::NotApplicable
                || effect.expected == ExpectedEffectV1::NotApplicable
                || effect.confirmation_reasons.is_empty()
                || effect.apply != EffectApplyStateV1::NotRun)
        {
            return Err(PlanError::new("CONFIRMABLE_EFFECT_INCOMPLETE"));
        }
        if plan.eligibility == PlanEligibility::Confirmable
            && (!effect
                .confirmation_reasons
                .contains(&ConfirmationReasonV1::ExactTargetBinding)
                || !effect
                    .confirmation_reasons
                    .contains(&ConfirmationReasonV1::ExactEffectSet)
                || !valid_confirmable_effect_tuple(effect, component))
        {
            return Err(PlanError::new("CONFIRMABLE_EFFECT_TUPLE_MISMATCH"));
        }
    }
    Ok(())
}

impl PlanEffectV1 {
    fn component_id(&self) -> &str {
        match &self.subject {
            PlanEffectSubjectV1::Package { component_id, .. }
            | PlanEffectSubjectV1::OperonSkill { component_id, .. } => component_id,
        }
    }
}

fn valid_effect_subject(effect: &PlanEffectV1, component: &PlanComponentV1) -> bool {
    match &effect.subject {
        PlanEffectSubjectV1::Package {
            target_package_identity_sha256,
            ..
        } => is_lower_sha256(target_package_identity_sha256),
        PlanEffectSubjectV1::OperonSkill { skill_name, .. } => {
            component.kind == ComponentKind::Skill
                && skill_name == &component.local_name
                && safe_local_name(skill_name)
        }
    }
}

fn valid_confirmable_effect_tuple(effect: &PlanEffectV1, component: &PlanComponentV1) -> bool {
    match (
        &effect.kind,
        &effect.subject,
        &effect.verifier,
        &effect.expected,
        &effect.rollback,
    ) {
        (
            PlanEffectKindV1::PackageCommit,
            PlanEffectSubjectV1::Package { .. },
            EffectVerifierV1::PackageReadback,
            ExpectedEffectV1::PackageContentBound { content_sha256 },
            EffectRollbackV1::CompensateByQuarantine,
        ) => is_lower_sha256(content_sha256),
        (
            PlanEffectKindV1::PackageQuarantine,
            PlanEffectSubjectV1::Package {
                target_package_identity_sha256,
                ..
            },
            EffectVerifierV1::PackageReadback,
            ExpectedEffectV1::PackageAbsent {
                target_package_identity_sha256: expected,
            },
            EffectRollbackV1::NoInverse,
        ) => target_package_identity_sha256 == expected,
        (
            PlanEffectKindV1::OperonAttach,
            PlanEffectSubjectV1::OperonSkill { skill_name, .. },
            EffectVerifierV1::OperonReadback,
            ExpectedEffectV1::OperonMembershipPresent {
                skill_name: expected,
            },
            EffectRollbackV1::CompensateByDetach,
        ) => component.kind == ComponentKind::Skill && skill_name == expected,
        (
            PlanEffectKindV1::OperonDetach,
            PlanEffectSubjectV1::OperonSkill { skill_name, .. },
            EffectVerifierV1::OperonReadback,
            ExpectedEffectV1::OperonMembershipAbsent {
                skill_name: expected,
            },
            EffectRollbackV1::NoInverse,
        ) => component.kind == ComponentKind::Skill && skill_name == expected,
        _ => false,
    }
}

fn validate_eligibility(plan: &SkillPlanV1) -> Result<(), PlanError> {
    match plan.eligibility {
        PlanEligibility::InspectOnly => {
            if !matches!(
                plan.identity,
                PlanIdentityV1::ContentAddressedInspection { .. }
            ) || !matches!(
                plan.source_binding(),
                SourceBindingV1::CallerAssertedUnverified
            ) || !matches!(plan.target, PlanTargetV1::Unbound)
                || plan.confirmation != PlanConfirmationState::NotCapable
                || plan.selection != PlanSelectionV1::NotApplicable
                || plan.expiry != PlanExpiryV1::NotApplicable
                || plan.reentry_policy != ReentryPolicyV1::InspectOnlyNonConsumable
                || !plan.effects.is_empty()
            {
                return Err(PlanError::new("INSPECT_ONLY_CONTRACT_MISMATCH"));
            }
        }
        PlanEligibility::Confirmable => {
            if !matches!(plan.identity, PlanIdentityV1::Invocation { .. })
                || !matches!(
                    plan.source_binding(),
                    SourceBindingV1::CsswitchExactContentBound
                )
                || !matches!(plan.target, PlanTargetV1::Bound { .. })
                || plan.confirmation != PlanConfirmationState::RequiresExactPlanCapability
                || !matches!(
                    plan.expiry,
                    PlanExpiryV1::ExpiresAtUnixSeconds { unix_seconds: 1.. }
                )
                || plan.reentry_policy != ReentryPolicyV1::ReadbackBeforeRetry
                || plan.effects.is_empty()
            {
                return Err(PlanError::new("CONFIRMABLE_CONTRACT_MISMATCH"));
            }
            validate_confirmable_selection(plan)?;
        }
    }
    Ok(())
}

impl SkillPlanV1 {
    fn source_binding(&self) -> &SourceBindingV1 {
        match &self.source {
            PlanSourceV1::GithubExact { binding, .. }
            | PlanSourceV1::LocalOpenedArchive { binding, .. }
            | PlanSourceV1::InstalledOwnedSkill { binding, .. } => binding,
        }
    }
}

fn validate_confirmable_selection(plan: &SkillPlanV1) -> Result<(), PlanError> {
    let unsafe_ids = plan
        .components
        .iter()
        .filter(|component| component_requires_degraded_selection(component))
        .map(|component| component.id.as_str())
        .collect::<BTreeSet<_>>();
    match &plan.selection {
        PlanSelectionV1::Full
            if unsafe_ids.is_empty() && plan.inspection_outcome == InspectionOutcome::Complete =>
        {
            Ok(())
        }
        PlanSelectionV1::Degraded {
            selected_component_ids,
            excluded_component_ids,
        } => {
            validate_sorted_ids(
                selected_component_ids,
                &plan.components,
                "INVALID_PLAN_SELECTION",
            )?;
            validate_sorted_ids(
                excluded_component_ids,
                &plan.components,
                "INVALID_PLAN_SELECTION",
            )?;
            let selected = selected_component_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let excluded = excluded_component_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            let all = plan
                .components
                .iter()
                .map(|component| component.id.as_str())
                .collect::<BTreeSet<_>>();
            let covered = selected.union(&excluded).copied().collect::<BTreeSet<_>>();
            if selected.is_empty()
                || !unsafe_ids.is_subset(&excluded)
                || !selected.is_disjoint(&excluded)
                || covered != all
                || !selected.is_disjoint(&unsafe_ids)
                || plan
                    .effects
                    .iter()
                    .any(|effect| !selected.contains(effect.component_id()))
            {
                return Err(PlanError::new("INVALID_DEGRADED_SELECTION"));
            }
            Ok(())
        }
        _ => Err(PlanError::new("CONFIRMABLE_SELECTION_REQUIRED")),
    }
}

fn component_requires_degraded_selection(component: &PlanComponentV1) -> bool {
    component.compatibility_status == CompatibilityStatus::Unsupported
        || component.executable
        || component.kind == ComponentKind::Executable
        || component.kind == ComponentKind::UnknownHostComponent
        || !component.source_path.is_ascii()
}

fn validate_sorted_ids(
    values: &[String],
    components: &[PlanComponentV1],
    code: &str,
) -> Result<(), PlanError> {
    let known = components
        .iter()
        .map(|component| component.id.as_str())
        .collect::<BTreeSet<_>>();
    if values.is_empty()
        || values.windows(2).any(|pair| pair[0] >= pair[1])
        || values.iter().any(|value| !known.contains(value.as_str()))
    {
        return Err(PlanError::new(code));
    }
    Ok(())
}

fn safe_archive_path(path: &str) -> bool {
    path.is_empty()
        || (path.len() <= 1_024
            && path.split('/').count() <= 32
            && path.split('/').all(|part| {
                !part.is_empty()
                    && !matches!(part, "." | "..")
                    && !part.contains('\\')
                    && !part.contains('\0')
            }))
}

fn safe_local_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && !matches!(value, "." | "..")
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains('\0')
        && !value.chars().any(char::is_control)
}

fn is_lower_sha1(value: &str) -> bool {
    value.len() == SHA1_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte <= b'f'))
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == SHA256_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte <= b'f'))
}

fn digest_json(value: &impl Serialize) -> Result<String, PlanError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| PlanError::new("PLAN_SERIALIZATION_FAILED"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspection::{
        ComponentEdge, ComponentGraph, ComponentNode, GithubSourceClaim, InspectionFinding,
        InspectionLimits, PackageSummary,
    };

    fn node(
        id: &str,
        kind: ComponentKind,
        path: &str,
        compatibility: CompatibilityStatus,
        executable: bool,
    ) -> ComponentNode {
        ComponentNode {
            id: id.into(),
            kind,
            source_path: path.into(),
            local_name: "component".into(),
            compatibility_status: compatibility,
            confirmation_required: executable,
            executable,
            declared_preapproved_tools: if id == "cmp_skill" {
                vec!["Read".into(), "Search".into()]
            } else {
                Vec::new()
            },
        }
    }

    fn report() -> InspectionReportV1 {
        let nodes = vec![
            node(
                "cmp_asset",
                ComponentKind::Asset,
                "assets/你好.txt",
                CompatibilityStatus::Unsupported,
                false,
            ),
            node(
                "cmp_exec",
                ComponentKind::Executable,
                "scripts/run.sh",
                CompatibilityStatus::Unsupported,
                true,
            ),
            node(
                "cmp_package",
                ComponentKind::Package,
                "",
                CompatibilityStatus::Unsupported,
                false,
            ),
            node(
                "cmp_skill",
                ComponentKind::Skill,
                "SKILL.md",
                CompatibilityStatus::Adapted,
                false,
            ),
            node(
                "cmp_unknown",
                ComponentKind::UnknownHostComponent,
                "plugin.json",
                CompatibilityStatus::Unsupported,
                false,
            ),
        ];
        let edges = Vec::<ComponentEdge>::new();
        let sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(&nodes, &edges)).unwrap())
        );
        InspectionReportV1 {
            schema: INSPECTION_REPORT_SCHEMA.into(),
            source_claim: GithubSourceClaim {
                owner: "owner".into(),
                repo: "repo".into(),
                commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
                path: "skills/demo".into(),
                verification: CALLER_ASSERTED_UNVERIFIED.into(),
            },
            outcome: InspectionOutcome::Partial,
            disposition: "quarantined_inspect_only".into(),
            package: PackageSummary {
                kind: "github_skill_package".into(),
                content_sha256: "a".repeat(64),
                file_count: 5,
                total_bytes: 24,
                executable_file_count: 1,
            },
            graph: ComponentGraph {
                schema: "csswitch.component-graph.v1".into(),
                sha256,
                nodes,
                edges,
            },
            findings: vec![
                InspectionFinding {
                    severity: InspectionSeverity::Blocking,
                    code: "PATH_IDENTITY_UNSUPPORTED".into(),
                    source_path: Some("assets/你好.txt".into()),
                    component_id: None,
                },
                InspectionFinding {
                    severity: InspectionSeverity::Warning,
                    code: "UNKNOWN_HOST_MANIFEST".into(),
                    source_path: Some("plugin.json".into()),
                    component_id: None,
                },
            ],
            limits: InspectionLimits::default(),
            effects: InspectionEffects::default(),
        }
    }

    fn future_confirmable_shape() -> SkillPlanV1 {
        let mut plan = build_skill_plan(&report()).unwrap();
        plan.eligibility = PlanEligibility::Confirmable;
        plan.identity = PlanIdentityV1::Invocation {
            plan_id: "future-operation-owner".into(),
        };
        plan.confirmation = PlanConfirmationState::RequiresExactPlanCapability;
        plan.source = PlanSourceV1::GithubExact {
            owner: "owner".into(),
            repo: "repo".into(),
            resolved_commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
            path: "skills/demo".into(),
            archive_sha256: "b".repeat(64),
            content_sha256: "a".repeat(64),
            materialized_content_sha256: "c".repeat(64),
            binding: SourceBindingV1::CsswitchExactContentBound,
        };
        plan.target = PlanTargetV1::Bound {
            science_runtime_identity_sha256: "b".repeat(64),
            data_dir_identity_sha256: "c".repeat(64),
            active_org_identity_sha256: "d".repeat(64),
            skills_root_device: 1,
            skills_root_inode: 2,
            operon: "OPERON".into(),
        };
        plan.expiry = PlanExpiryV1::ExpiresAtUnixSeconds { unix_seconds: 1 };
        plan.reentry_policy = ReentryPolicyV1::ReadbackBeforeRetry;
        plan.selection = PlanSelectionV1::Degraded {
            selected_component_ids: vec!["cmp_skill".into()],
            excluded_component_ids: vec![
                "cmp_asset".into(),
                "cmp_exec".into(),
                "cmp_package".into(),
                "cmp_unknown".into(),
            ],
        };
        plan.effects = vec![PlanEffectV1 {
            order: 1,
            kind: PlanEffectKindV1::PackageCommit,
            subject: PlanEffectSubjectV1::Package {
                component_id: "cmp_skill".into(),
                target_package_identity_sha256: "e".repeat(64),
            },
            authority: EffectAuthorityV1::CsswitchHost,
            verifier: EffectVerifierV1::PackageReadback,
            expected: ExpectedEffectV1::PackageContentBound {
                content_sha256: "a".repeat(64),
            },
            rollback: EffectRollbackV1::CompensateByQuarantine,
            apply: EffectApplyStateV1::NotRun,
            confirmation_reasons: vec![
                ConfirmationReasonV1::ExactTargetBinding,
                ConfirmationReasonV1::ExactEffectSet,
            ],
        }];
        for component in &mut plan.components {
            component.degradation = expected_component_degradation(
                component,
                &SourceBindingV1::CsswitchExactContentBound,
            );
            component.confirmation_reasons = expected_component_reasons(
                component,
                &SourceBindingV1::CsswitchExactContentBound,
                &plan.inspection_outcome,
            );
        }
        plan.summary = summarize(&plan.components, &plan.effects);
        plan.plan_digest_sha256 = compute_plan_digest(&plan).unwrap();
        plan
    }

    #[test]
    fn caller_asserted_partial_report_only_builds_nonconsumable_inspect_only_plan() {
        let plan = build_skill_plan(&report()).unwrap();
        assert_eq!(plan.eligibility, PlanEligibility::InspectOnly);
        assert_eq!(plan.confirmation, PlanConfirmationState::NotCapable);
        assert_eq!(plan.target, PlanTargetV1::Unbound);
        assert!(plan.effects.is_empty());
        assert!(matches!(
            plan.identity,
            PlanIdentityV1::ContentAddressedInspection { .. }
        ));
        let skill = plan
            .components
            .iter()
            .find(|component| component.id == "cmp_skill")
            .unwrap();
        assert_eq!(skill.local_name, "component");
        assert_eq!(skill.declared_preapproved_tools, ["Read", "Search"]);
        assert!(skill
            .confirmation_reasons
            .contains(&ConfirmationReasonV1::CallerAssertedUnverified));
        assert!(plan
            .components
            .iter()
            .find(|component| component.id == "cmp_asset")
            .unwrap()
            .findings
            .contains(&PlanFindingV1 {
                severity: InspectionSeverity::Blocking,
                code: "PATH_IDENTITY_UNSUPPORTED".into()
            }));
    }

    #[test]
    fn local_opened_archive_source_round_trips_and_unbound_state_cannot_confirm() {
        let mut plan = build_skill_plan(&report()).unwrap();
        plan.source = PlanSourceV1::LocalOpenedArchive {
            archive_sha256: "b".repeat(64),
            opened_object_identity_sha256: "c".repeat(64),
            content_sha256: "a".repeat(64),
            binding: SourceBindingV1::CallerAssertedUnverified,
        };
        assert_eq!(
            validate_skill_plan(&plan).unwrap_err().code,
            "PLAN_DIGEST_MISMATCH"
        );
        let serialized = serde_json::to_vec(&plan.source).unwrap();
        let restored: PlanSourceV1 = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(plan.source, restored);
    }

    #[test]
    fn validator_rejects_unknown_report_effect_and_identity_tampering() {
        let mut tampered_report = report();
        tampered_report.effects.apply = "claimed".into();
        assert_eq!(
            build_skill_plan(&tampered_report).unwrap_err().code,
            "INVALID_INSPECTION_REPORT"
        );
        let plan = build_skill_plan(&report()).unwrap();
        let mut value = serde_json::to_value(&plan).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), serde_json::Value::Null);
        assert!(serde_json::from_value::<SkillPlanV1>(value).is_err());
        let mut invalid = plan.clone();
        invalid.confirmation = PlanConfirmationState::RequiresExactPlanCapability;
        assert_eq!(
            validate_skill_plan(&invalid).unwrap_err().code,
            "INSPECT_ONLY_CONTRACT_MISMATCH"
        );

        let mut upgraded_claim = report();
        upgraded_claim.source_claim.verification = "csswitch_content_bound_v1".into();
        assert_eq!(
            build_skill_plan(&upgraded_claim).unwrap_err().code,
            "INVALID_INSPECTION_REPORT"
        );

        let mut invalid_graph = report();
        invalid_graph.graph.edges.push(ComponentEdge {
            from: "cmp_skill".into(),
            relation: "contains".into(),
            to: "missing".into(),
        });
        assert_eq!(
            build_skill_plan(&invalid_graph).unwrap_err().code,
            "INVALID_INSPECTION_GRAPH"
        );

        let mut invalid_outcome = report();
        invalid_outcome.outcome = InspectionOutcome::Complete;
        assert_eq!(
            build_skill_plan(&invalid_outcome).unwrap_err().code,
            "INSPECTION_OUTCOME_MISMATCH"
        );
    }

    #[test]
    fn content_addressed_inspection_identity_cannot_be_confirmation_capability() {
        let mut plan = future_confirmable_shape();
        plan.identity = PlanIdentityV1::ContentAddressedInspection {
            digest_sha256: String::new(),
        };
        plan.plan_digest_sha256 = compute_plan_digest(&plan).unwrap();
        plan.identity = PlanIdentityV1::ContentAddressedInspection {
            digest_sha256: plan.plan_digest_sha256.clone(),
        };
        assert_eq!(
            validate_skill_plan(&plan).unwrap_err().code,
            "CONFIRMABLE_CONTRACT_MISMATCH"
        );
    }

    #[test]
    fn sealed_confirmable_round_trip_validates_and_binds_invocation_identity() {
        let plan = future_confirmable_shape();
        validate_skill_plan(&plan).unwrap();
        assert!(matches!(
            (&plan.effects[0].subject, &plan.effects[0].expected),
            (
                PlanEffectSubjectV1::Package {
                    component_id,
                    target_package_identity_sha256,
                },
                ExpectedEffectV1::PackageContentBound { content_sha256 },
            ) if component_id == "cmp_skill"
                && target_package_identity_sha256 == &"e".repeat(64)
                && content_sha256 == &"a".repeat(64)
        ));
        let restored: SkillPlanV1 =
            serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
        validate_skill_plan(&restored).unwrap();
        let mut changed_invocation = restored;
        changed_invocation.identity = PlanIdentityV1::Invocation {
            plan_id: "replacement-invocation".into(),
        };
        assert_eq!(
            validate_skill_plan(&changed_invocation).unwrap_err().code,
            "PLAN_DIGEST_MISMATCH"
        );
    }

    #[test]
    fn sealed_local_opened_archive_confirmable_round_trip_validates() {
        let mut plan = future_confirmable_shape();
        plan.source = PlanSourceV1::LocalOpenedArchive {
            archive_sha256: "f".repeat(64),
            opened_object_identity_sha256: "e".repeat(64),
            content_sha256: "a".repeat(64),
            binding: SourceBindingV1::CsswitchExactContentBound,
        };
        plan.plan_digest_sha256 = compute_plan_digest(&plan).unwrap();
        validate_skill_plan(&plan).unwrap();
        let restored: SkillPlanV1 =
            serde_json::from_slice(&serde_json::to_vec(&plan).unwrap()).unwrap();
        validate_skill_plan(&restored).unwrap();
    }

    #[test]
    fn validator_requires_complete_confirmable_target_effect_expiry_and_reentry_contract() {
        let mut incomplete = build_skill_plan(&report()).unwrap();
        incomplete.eligibility = PlanEligibility::Confirmable;
        assert_eq!(
            validate_skill_plan(&incomplete).unwrap_err().code,
            "CONFIRMABLE_CONTRACT_MISMATCH"
        );
    }

    #[test]
    fn confirmable_skills_root_identity_is_nonzero_and_digest_sealed() {
        let plan = future_confirmable_shape();
        validate_skill_plan(&plan).unwrap();

        let mut zero_root = plan.clone();
        let PlanTargetV1::Bound {
            skills_root_device, ..
        } = &mut zero_root.target
        else {
            panic!("future confirmable plans must bind a skills root");
        };
        *skills_root_device = 0;
        zero_root.plan_digest_sha256 = compute_plan_digest(&zero_root).unwrap();
        assert_eq!(
            validate_skill_plan(&zero_root).unwrap_err().code,
            "INVALID_PLAN_TARGET"
        );

        let mut zero_inode = plan.clone();
        let PlanTargetV1::Bound {
            skills_root_inode, ..
        } = &mut zero_inode.target
        else {
            panic!("future confirmable plans must bind a skills root");
        };
        *skills_root_inode = 0;
        zero_inode.plan_digest_sha256 = compute_plan_digest(&zero_inode).unwrap();
        assert_eq!(
            validate_skill_plan(&zero_inode).unwrap_err().code,
            "INVALID_PLAN_TARGET"
        );

        let mut altered_root = plan.clone();
        let PlanTargetV1::Bound {
            skills_root_inode, ..
        } = &mut altered_root.target
        else {
            panic!("future confirmable plans must bind a skills root");
        };
        *skills_root_inode += 1;
        assert_ne!(
            compute_plan_digest(&altered_root).unwrap(),
            plan.plan_digest_sha256
        );
        assert_eq!(
            validate_skill_plan(&altered_root).unwrap_err().code,
            "PLAN_DIGEST_MISMATCH"
        );
    }

    #[test]
    fn missing_contract_fields_and_incomplete_effects_fail_closed() {
        let plan = future_confirmable_shape();
        for field in ["target", "expiry", "reentry_policy"] {
            let mut value = serde_json::to_value(&plan).unwrap();
            value.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<SkillPlanV1>(value).is_err());
        }
        for field in ["order", "authority", "rollback"] {
            let mut value = serde_json::to_value(&plan).unwrap();
            value["effects"][0].as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<SkillPlanV1>(value).is_err());
        }
        let mut missing_authority = future_confirmable_shape();
        missing_authority.identity = PlanIdentityV1::Invocation {
            plan_id: "later-owner".into(),
        };
        missing_authority.effects[0].authority = EffectAuthorityV1::NotApplicable;
        assert_eq!(
            validate_skill_plan(&missing_authority).unwrap_err().code,
            "CONFIRMABLE_EFFECT_INCOMPLETE"
        );
        let mut unordered = future_confirmable_shape();
        unordered.identity = PlanIdentityV1::Invocation {
            plan_id: "later-owner".into(),
        };
        unordered.effects[0].order = 2;
        assert_eq!(
            validate_skill_plan(&unordered).unwrap_err().code,
            "INVALID_PLAN_EFFECT"
        );
    }

    #[test]
    fn removal_plan_is_exactly_detach_then_quarantine() {
        let plan = build_skill_removal_plan(&SkillRemovalPlanRequestV1 {
            plan_id: "remove-one".into(),
            expires_at_unix_seconds: 123,
            target: ConfirmablePlanTargetV1 {
                science_runtime_identity_sha256: "1".repeat(64),
                data_dir_identity_sha256: "2".repeat(64),
                active_org_identity_sha256: "3".repeat(64),
                skills_root_device: 1,
                skills_root_inode: 2,
            },
            skill_name: "demo".into(),
            marker_sha256: "4".repeat(64),
            content_sha256: "5".repeat(64),
        })
        .unwrap();
        assert_eq!(plan.effects.len(), 2);
        assert!(matches!(
            plan.source,
            PlanSourceV1::InstalledOwnedSkill { .. }
        ));
        assert_eq!(plan.effects[0].kind, PlanEffectKindV1::OperonDetach);
        assert_eq!(plan.effects[1].kind, PlanEffectKindV1::PackageQuarantine);
        validate_skill_plan(&plan).unwrap();
    }

    #[test]
    fn confirmable_effect_kind_tuple_and_degraded_selection_are_closed() {
        for (kind, subject, verifier, expected, rollback) in [
            (
                PlanEffectKindV1::PackageCommit,
                PlanEffectSubjectV1::Package {
                    component_id: "cmp_skill".into(),
                    target_package_identity_sha256: "e".repeat(64),
                },
                EffectVerifierV1::PackageReadback,
                ExpectedEffectV1::PackageContentBound {
                    content_sha256: "a".repeat(64),
                },
                EffectRollbackV1::CompensateByQuarantine,
            ),
            (
                PlanEffectKindV1::PackageQuarantine,
                PlanEffectSubjectV1::Package {
                    component_id: "cmp_skill".into(),
                    target_package_identity_sha256: "e".repeat(64),
                },
                EffectVerifierV1::PackageReadback,
                ExpectedEffectV1::PackageAbsent {
                    target_package_identity_sha256: "e".repeat(64),
                },
                EffectRollbackV1::NoInverse,
            ),
            (
                PlanEffectKindV1::OperonAttach,
                PlanEffectSubjectV1::OperonSkill {
                    component_id: "cmp_skill".into(),
                    skill_name: "component".into(),
                },
                EffectVerifierV1::OperonReadback,
                ExpectedEffectV1::OperonMembershipPresent {
                    skill_name: "component".into(),
                },
                EffectRollbackV1::CompensateByDetach,
            ),
            (
                PlanEffectKindV1::OperonDetach,
                PlanEffectSubjectV1::OperonSkill {
                    component_id: "cmp_skill".into(),
                    skill_name: "component".into(),
                },
                EffectVerifierV1::OperonReadback,
                ExpectedEffectV1::OperonMembershipAbsent {
                    skill_name: "component".into(),
                },
                EffectRollbackV1::NoInverse,
            ),
        ] {
            let mut plan = future_confirmable_shape();
            plan.effects[0].kind = kind;
            plan.effects[0].subject = subject;
            plan.effects[0].verifier = verifier;
            plan.effects[0].expected = expected;
            plan.effects[0].rollback = rollback;
            plan.summary = summarize(&plan.components, &plan.effects);
            plan.plan_digest_sha256 = compute_plan_digest(&plan).unwrap();
            validate_skill_plan(&plan).unwrap();
        }

        let mut invalid_tuple = future_confirmable_shape();
        invalid_tuple.effects[0].verifier = EffectVerifierV1::OperonReadback;
        invalid_tuple.summary = summarize(&invalid_tuple.components, &invalid_tuple.effects);
        invalid_tuple.plan_digest_sha256 = compute_plan_digest(&invalid_tuple).unwrap();
        assert_eq!(
            validate_skill_plan(&invalid_tuple).unwrap_err().code,
            "CONFIRMABLE_EFFECT_TUPLE_MISMATCH"
        );

        let mut missing_component = future_confirmable_shape();
        if let PlanSelectionV1::Degraded {
            excluded_component_ids,
            ..
        } = &mut missing_component.selection
        {
            excluded_component_ids.pop();
        }
        missing_component.plan_digest_sha256 = compute_plan_digest(&missing_component).unwrap();
        assert_eq!(
            validate_skill_plan(&missing_component).unwrap_err().code,
            "INVALID_DEGRADED_SELECTION"
        );
    }

    #[test]
    fn stable_round_trip_and_bounds_preserve_findings_reasons_and_not_run_state() {
        let plan = build_skill_plan(&report()).unwrap();
        let serialized = serde_json::to_vec(&plan).unwrap();
        let restored: SkillPlanV1 = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(plan, restored);
        validate_skill_plan(&restored).unwrap();
        assert_eq!(
            serialized,
            serde_json::to_vec(&build_skill_plan(&report()).unwrap()).unwrap()
        );
        let mut over_limit = plan;
        over_limit.components = (0..(MAX_PLAN_COMPONENTS + 1))
            .map(|index| PlanComponentV1 {
                id: format!("cmp_{index:064x}"),
                kind: ComponentKind::Asset,
                source_path: format!("asset-{index}"),
                local_name: format!("asset-{index}"),
                compatibility_status: CompatibilityStatus::Unsupported,
                degradation: ComponentDegradationV1::ExcludedUnsupported,
                confirmation_reasons: vec![ConfirmationReasonV1::UnsupportedComponent],
                executable: false,
                declared_preapproved_tools: Vec::new(),
                findings: Vec::new(),
            })
            .collect();
        assert_eq!(
            validate_skill_plan(&over_limit).unwrap_err().code,
            "PLAN_COMPONENT_LIMIT"
        );
    }

    #[test]
    fn plan_never_serializes_credential_or_effect_authority_values() {
        let serialized =
            String::from_utf8(serde_json::to_vec(&build_skill_plan(&report()).unwrap()).unwrap())
                .unwrap();
        for forbidden in [
            "credential",
            "token",
            "authorization",
            "filesystem_mutation",
            "science_attach",
            "operation_id",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }
}
