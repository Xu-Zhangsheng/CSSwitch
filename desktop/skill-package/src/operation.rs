//! Skill operation deterministic single-Skill apply coordinator.
//!
//! This deliberately accepts only the exact staged GitHub archive handle made
//! by `resolver`: local archives, Plugin/MCP payloads, bundles and unknown
//! package shapes never reach this module.  The confirmation secret is an
//! opaque, single-use capability; only its SHA-256 is durable.  The ledger is
//! the source of every final response, including recovery projection.

#[cfg(test)]
use std::cell::Cell;
#[cfg(test)]
use std::cell::RefCell;
use std::ffi::CStr;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::archive::package_or_bundle_from_github_archive;
use crate::inspection::GithubInspectionSource;
use crate::install::{acquire_install_lock_at, commit_package_at, SourceDescriptor};
use crate::plan::{
    build_skill_removal_plan, validate_skill_plan, ConfirmablePlanRequestV1,
    ConfirmablePlanTargetV1, PlanEffectKindV1, PlanEligibility, PlanExpiryV1, PlanTargetV1,
    SkillPlanV1, SkillRemovalPlanRequestV1,
};
use crate::resolver::{
    exact_staging_directory_name, remove_exact_github_archive, remove_partial_exact_staging,
    stage_inspect_exact_github_archive, ExactStagedGithubArchive,
};
use crate::{
    active_org, find_bundle_for_skill, InstallError, SourceKind, ValidatedArchive, MAX_FILES,
    MAX_TOTAL_BYTES,
};

pub const SKILL_OPERATION_LEDGER_SCHEMA: &str = "csswitch.skill-operation-ledger.v1";
const LEDGER_SUFFIX: &str = ".skill-operation-ledger.json";
const RESERVATION_SUFFIX: &str = ".skill-operation-reservation.json";
const GC_TOMBSTONE_SUFFIX: &str = ".skill-operation-gc-tombstone.json";
const LIFECYCLE_LOCK_NAME: &str = ".skill-operation-lifecycle.lock";
// Buckets are permanent authority infrastructure, not one file per operation:
// a collision merely serializes independent plans and can never exhaust the
// bounded lifecycle directory.
const OPERATION_LOCK_BUCKETS: usize = 8;
const MAX_OPERATION_LIFECYCLE_ENTRIES: usize = 32;
const MAX_ACTIVE_INSTALL_PLANS: usize = 4;
const MAX_RESERVED_STAGED_BYTES: u64 =
    (MAX_ACTIVE_INSTALL_PLANS as u64) * (crate::MAX_ARCHIVE_BYTES as u64);
// This scanner reads an already-owned installed Skill during removal
// readback.  File and byte limits are inherited from archive validation; the
// directory and dirent ceilings additionally bound metadata-only trees (for
// example, a tree of empty directories) before they can turn readback into an
// unbounded filesystem walk.
const MAX_PAYLOAD_SCAN_DIRECTORIES: usize = 512;
const MAX_PAYLOAD_SCAN_DIRENTS: usize = crate::MAX_ARCHIVE_ENTRIES;
static CAPABILITY_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static TEST_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
thread_local! {
    // Per-test-thread fault seam.  It fires only at the post-effect successor
    // write, never at the pre-effect intent write.
    static FAIL_POST_EFFECT_PERSIST_ONCE: Cell<bool> = const { Cell::new(false) };
    // This is deliberately a separate seam for the error-recovery successor:
    // a failed post-rename parent sync has already happened when it fires.
    static FAIL_UNCERTAIN_PERSIST_ONCE: Cell<bool> = const { Cell::new(false) };
    static STOP_AFTER_PACKAGE_VERIFY_ONCE: Cell<bool> = const { Cell::new(false) };
    static ARM_POST_EFFECT_PERSIST_AFTER_QUARANTINE: Cell<bool> = const { Cell::new(false) };
    // The two parents participate in one cross-directory rename, but their
    // durability acknowledgements are independent.  Keep the seams separate
    // so neither a source-parent nor destination-parent failure can be
    // accidentally treated as a fully durable quarantine commit.
    static FAIL_QUARANTINE_SKILLS_ROOT_SYNC_ONCE: Cell<bool> = const { Cell::new(false) };
    static FAIL_QUARANTINE_ROOT_SYNC_ONCE: Cell<bool> = const { Cell::new(false) };
    static REPLACE_QUARANTINE_NAME_AFTER_INTENT: RefCell<Option<(PathBuf, PathBuf)>> = const { RefCell::new(None) };
    static REPLACE_INSTALL_NAME_AFTER_CONTINUATION_LOCK: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    static TEST_UNIX_SECONDS_OVERRIDE: Cell<Option<u64>> = const { Cell::new(None) };
}

#[cfg(test)]
fn fail_next_post_effect_persist() {
    FAIL_POST_EFFECT_PERSIST_ONCE.with(|value| value.set(true));
}

#[cfg(test)]
fn fail_next_uncertain_persist() {
    FAIL_UNCERTAIN_PERSIST_ONCE.with(|value| value.set(true));
}

#[cfg(test)]
fn stop_after_next_package_verify() {
    STOP_AFTER_PACKAGE_VERIFY_ONCE.with(|value| value.set(true));
}

#[cfg(test)]
fn fail_post_effect_persist_after_next_quarantine() {
    ARM_POST_EFFECT_PERSIST_AFTER_QUARANTINE.with(|value| value.set(true));
}

#[cfg(test)]
fn fail_next_quarantine_skills_root_sync() {
    FAIL_QUARANTINE_SKILLS_ROOT_SYNC_ONCE.with(|value| value.set(true));
}

#[cfg(test)]
fn fail_next_quarantine_root_sync() {
    FAIL_QUARANTINE_ROOT_SYNC_ONCE.with(|value| value.set(true));
}

#[cfg(test)]
fn replace_quarantine_name_after_next_intent(source: PathBuf, displaced: PathBuf) {
    REPLACE_QUARANTINE_NAME_AFTER_INTENT
        .with(|value| *value.borrow_mut() = Some((source, displaced)));
}

#[cfg(test)]
fn replace_install_name_after_next_continuation_lock(target: PathBuf) {
    REPLACE_INSTALL_NAME_AFTER_CONTINUATION_LOCK.with(|value| *value.borrow_mut() = Some(target));
}

fn persist_after_effect(
    root: &File,
    name: &str,
    ledger: &mut SkillOperationLedgerV1,
) -> Result<(), SkillOperationError> {
    #[cfg(test)]
    if FAIL_POST_EFFECT_PERSIST_ONCE.with(|value| value.replace(false)) {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_FAULT_INJECTED",
            "ledger",
        ));
    }
    persist(root, name, ledger)
}

fn persist_uncertain(
    root: &File,
    name: &str,
    ledger: &mut SkillOperationLedgerV1,
) -> Result<(), SkillOperationError> {
    #[cfg(test)]
    if FAIL_UNCERTAIN_PERSIST_ONCE.with(|value| value.replace(false)) {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_FAULT_INJECTED",
            "ledger",
        ));
    }
    persist(root, name, ledger)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillOperationPrepareRequestV1 {
    pub operation_id: String,
    /// Digest of the exact caller text accepted by the trusted resolver.  It
    /// lets apply reject a different URL spelling before it can reopen the
    /// durable snapshot.
    pub source_request_sha256: String,
    pub plan: ConfirmablePlanRequestV1,
    pub target: SkillOperationTargetBindingV1,
}

/// Durable pre-download admission.  A reservation counts one maximum-sized
/// archive, rather than trusting mutable remote metadata, so concurrent plans
/// cannot download themselves past the host-owned staging budget.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SkillOperationReservationV1 {
    schema: String,
    operation_id: String,
    expires_at_unix_seconds: u64,
    reserved_staged_bytes: u64,
    staging_directory_name: String,
    phase: SkillOperationReservationPhaseV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    staged: Option<SkillOperationReservationStagedV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SkillOperationReservationPhaseV1 {
    Reserved,
    Staged,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SkillOperationReservationStagedV1 {
    source: GithubInspectionSource,
    archive_sha256: String,
    staging_object_identity_sha256: String,
    staging_receipt_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SkillOperationGcTombstoneV1 {
    schema: String,
    operation_id: String,
    ledger_sha256: String,
}

/// The Gateway derives these from its verified ScienceHostContext and freshly
/// read active-org state; an Agent cannot choose them.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillOperationTargetBindingV1 {
    pub science_runtime_identity_sha256: String,
    pub data_dir_identity_sha256: String,
    pub active_org_identity_sha256: String,
    pub active_org: String,
    /// The verified current-org Skills root is part of the target, not a
    /// name to reopen after a confirmation capability was issued.
    pub skills_root_device: u64,
    pub skills_root_inode: u64,
}

/// The raw capability is intentionally not serializable and must not be put
/// in a progress mailbox or durable receipt. The Gateway may return it only in
/// its mode-0600 terminal response, then accepts it once from an explicit
/// confirmed apply request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillOperationConfirmationCapabilityV1 {
    operation_id: String,
    value: String,
}

impl SkillOperationConfirmationCapabilityV1 {
    /// Reconstructs a caller-presented opaque confirmation capability. Its
    /// authority is checked only against the durable hash in the ledger.
    pub fn from_raw(operation_id: String, value: String) -> Self {
        Self {
            operation_id,
            value,
        }
    }

    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    pub fn raw(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillOperationEffectStateV1 {
    NotStarted,
    Committed,
    Verified,
    Uncertain,
    Failed,
    Compensated,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillOperationEffectIntentV1 {
    None,
    InProgress,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillOperationEffectReceiptV1 {
    pub order: u32,
    pub kind: PlanEffectKindV1,
    pub state: SkillOperationEffectStateV1,
    pub intent: SkillOperationEffectIntentV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_started_at_unix_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SkillOperationLedgerV1 {
    pub schema: String,
    #[serde(default)]
    pub generation: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_ledger_sha256: Option<String>,
    #[serde(default)]
    pub operation_kind: SkillOperationKindV1,
    pub operation_id: String,
    pub plan_digest_sha256: String,
    pub confirmation_capability_sha256: String,
    /// Present only for an archive-backed installation.  A removal is an
    /// installed-directory subject; it must never smuggle its marker hash into
    /// an archive or staging field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_request_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_archive_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staging_object_identity_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staging_receipt_sha256: Option<String>,
    /// Exact GitHub operations bind the inspection-domain digest separately
    /// from the materialized package/marker digest below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_content_sha256: Option<String>,
    /// Materialized package canonical digest used by commit and filesystem
    /// readback.  It must not be compared to the inspection-domain digest.
    pub source_content_sha256: String,
    pub target: SkillOperationTargetBindingV1,
    pub expires_at_unix_seconds: u64,
    pub plan: SkillPlanV1,
    pub consumed: bool,
    pub effects: Vec<SkillOperationEffectReceiptV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removal: Option<SkillRemovalSnapshotV1>,
}

/// One ledger schema owns every consumable Skill effect set.  The variants are
/// protocol semantics, not release milestones.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillOperationKindV1 {
    #[default]
    Install,
    Removal,
}

/// Canonical ownership evidence sealed before a removal capability is shown.
/// `quarantine_destination` is an exact, caller-opened-private-root-relative
/// destination, persisted before the corresponding WAL intent.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRemovalSnapshotV1 {
    pub skill_name: String,
    pub marker_sha256: String,
    pub content_sha256: String,
    pub skill_leaf_device: u64,
    pub skill_leaf_inode: u64,
    pub skills_root_device: u64,
    pub skills_root_inode: u64,
    pub quarantine_root_device: u64,
    pub quarantine_root_inode: u64,
    pub quarantine_destination: String,
}

/// Caller-opened mutation roots.  The host owns opening these directories from
/// its verified sandbox parent; the core never creates a pathname root or
/// widens its permissions.  Both descriptors stay open through the operation
/// so the mutation and recovery boundary is identity-based, not name-based.
#[derive(Debug)]
pub struct SkillRemovalRoots {
    pub skills_root: File,
    pub quarantine_root: File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillRemovalRootIdentityV1 {
    pub skills_root_device: u64,
    pub skills_root_inode: u64,
    pub quarantine_root_device: u64,
    pub quarantine_root_inode: u64,
}

impl SkillRemovalRoots {
    pub fn identity(&self) -> Result<SkillRemovalRootIdentityV1, SkillOperationError> {
        let skills = directory_identity(&self.skills_root, "SKILL_OPERATION_SKILLS_ROOT_INVALID")?;
        let quarantine = directory_identity(
            &self.quarantine_root,
            "SKILL_OPERATION_QUARANTINE_ROOT_INVALID",
        )?;
        Ok(SkillRemovalRootIdentityV1 {
            skills_root_device: skills.0,
            skills_root_inode: skills.1,
            quarantine_root_device: quarantine.0,
            quarantine_root_inode: quarantine.1,
        })
    }
}

/// Computes a safe relative leaf before the quarantine intent is persisted.
/// It contains no user pathname and remains stable across a restart.
pub fn skill_removal_destination(
    operation_id: &str,
    skill_name: &str,
    marker_sha256: &str,
    content_sha256: &str,
) -> Result<String, SkillOperationError> {
    valid_operation_id(operation_id)?;
    if skill_name.is_empty()
        || skill_name.len() > 80
        || !skill_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || !valid_sha256(marker_sha256)
        || !valid_sha256(content_sha256)
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_DESTINATION_INVALID",
            "plan",
        ));
    }
    Ok(format!(
        "{}-{}",
        operation_id,
        digest(
            [
                skill_name.as_bytes(),
                marker_sha256.as_bytes(),
                content_sha256.as_bytes()
            ]
            .concat()
            .as_slice()
        )
    ))
}

/// Performs the already-intended quarantine rename exclusively between the
/// caller-opened roots.  It does not create roots, delete data, or retry a
/// prior rename.  Higher-level prepare/apply records the exact marker/content
/// snapshot before calling this primitive.
pub fn quarantine_owned_skill_at(
    roots: &SkillRemovalRoots,
    snapshot: &SkillRemovalSnapshotV1,
    held_source_leaf: &File,
) -> Result<(), SkillOperationError> {
    let identity = roots.identity()?;
    if identity.skills_root_device != snapshot.skills_root_device
        || identity.skills_root_inode != snapshot.skills_root_inode
        || identity.quarantine_root_device != snapshot.quarantine_root_device
        || identity.quarantine_root_inode != snapshot.quarantine_root_inode
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_ROOT_IDENTITY_DRIFT",
            "quarantine",
        ));
    }
    let from = std::ffi::CString::new(snapshot.skill_name.as_str()).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_REMOVAL_DESTINATION_INVALID", "quarantine")
    })?;
    let to = std::ffi::CString::new(snapshot.quarantine_destination.as_str()).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_REMOVAL_DESTINATION_INVALID", "quarantine")
    })?;
    #[cfg(test)]
    if let Some((source_path, displaced_path)) =
        REPLACE_QUARANTINE_NAME_AFTER_INTENT.with(|value| value.borrow_mut().take())
    {
        std::fs::rename(&source_path, &displaced_path).map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_TEST_SEAM_FAILED", "quarantine")
        })?;
        std::fs::create_dir(&source_path).map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_TEST_SEAM_FAILED", "quarantine")
        })?;
        std::fs::write(source_path.join("SKILL.md"), b"replacement").map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_TEST_SEAM_FAILED", "quarantine")
        })?;
    }
    let mut source = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe {
        libc::fstatat(
            roots.skills_root.as_raw_fd(),
            from.as_ptr(),
            source.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_TARGET_MISSING",
            "quarantine",
        ));
    }
    let source = unsafe { source.assume_init() };
    if source.st_mode & libc::S_IFMT != libc::S_IFDIR {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_TARGET_INVALID",
            "quarantine",
        ));
    }
    let mut destination = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe {
        libc::fstatat(
            roots.quarantine_root.as_raw_fd(),
            to.as_ptr(),
            destination.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_DESTINATION_EXISTS",
            "quarantine",
        ));
    }
    if std::io::Error::last_os_error().kind() != std::io::ErrorKind::NotFound {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_DESTINATION_CHECK_FAILED",
            "quarantine",
        ));
    }
    let (held_device, held_inode) =
        directory_object_identity(held_source_leaf, "SKILL_OPERATION_REMOVAL_SNAPSHOT_DRIFT")?;
    if held_device != snapshot.skill_leaf_device
        || held_inode != snapshot.skill_leaf_inode
        || source.st_dev as u64 != held_device
        || source.st_ino as u64 != held_inode
        || !removal_source_leaf_matches(held_source_leaf, snapshot)?
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_SNAPSHOT_DRIFT",
            "quarantine",
        ));
    }
    #[cfg(target_os = "macos")]
    let renamed = unsafe {
        libc::renameatx_np(
            roots.skills_root.as_raw_fd(),
            from.as_ptr(),
            roots.quarantine_root.as_raw_fd(),
            to.as_ptr(),
            libc::RENAME_EXCL,
        )
    } == 0;
    #[cfg(not(target_os = "macos"))]
    let renamed = false;
    if !renamed {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_RENAME_FAILED",
            "quarantine",
        ));
    }
    sync_removal_parent_roots(roots)?;
    #[cfg(test)]
    if ARM_POST_EFFECT_PERSIST_AFTER_QUARANTINE.with(|value| value.replace(false)) {
        fail_next_post_effect_persist();
    }
    Ok(())
}

/// A cross-directory rename is visible before either parent has acknowledged
/// it durably.  These are deliberately separate operations on the caller-held
/// descriptors: callers must never collapse a source-parent failure and a
/// destination-parent failure into a generic, safely retryable rename error.
fn sync_removal_parent_roots(roots: &SkillRemovalRoots) -> Result<(), SkillOperationError> {
    sync_quarantine_skills_root(&roots.skills_root).map_err(|_| {
        SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_POST_RENAME_SKILLS_ROOT_SYNC_FAILED",
            "quarantine",
        )
    })?;
    sync_quarantine_root(&roots.quarantine_root).map_err(|_| {
        SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_POST_RENAME_QUARANTINE_ROOT_SYNC_FAILED",
            "quarantine",
        )
    })
}

#[cfg(test)]
fn sync_quarantine_skills_root(root: &File) -> std::io::Result<()> {
    if FAIL_QUARANTINE_SKILLS_ROOT_SYNC_ONCE.with(|value| value.replace(false)) {
        return Err(std::io::Error::other(
            "test post-rename skills root sync failure",
        ));
    }
    root.sync_all()
}

#[cfg(not(test))]
fn sync_quarantine_skills_root(root: &File) -> std::io::Result<()> {
    root.sync_all()
}

#[cfg(test)]
fn sync_quarantine_root(root: &File) -> std::io::Result<()> {
    if FAIL_QUARANTINE_ROOT_SYNC_ONCE.with(|value| value.replace(false)) {
        return Err(std::io::Error::other(
            "test post-rename quarantine root sync failure",
        ));
    }
    root.sync_all()
}

#[cfg(not(test))]
fn sync_quarantine_root(root: &File) -> std::io::Result<()> {
    root.sync_all()
}

fn is_post_rename_parent_sync_failure(error: &SkillOperationError) -> bool {
    matches!(
        error.code.as_str(),
        "SKILL_OPERATION_REMOVAL_POST_RENAME_SKILLS_ROOT_SYNC_FAILED"
            | "SKILL_OPERATION_REMOVAL_POST_RENAME_QUARANTINE_ROOT_SYNC_FAILED"
    )
}

/// Readback used after a crash at quarantine intent.  It intentionally only
/// answers whether the exact name transition occurred; the coordinator must
/// additionally re-open and verify the sealed marker/content before marking
/// the durable effect verified.
pub fn quarantine_name_transitioned_at(
    roots: &SkillRemovalRoots,
    snapshot: &SkillRemovalSnapshotV1,
) -> Result<bool, SkillOperationError> {
    let identity = roots.identity()?;
    if identity.skills_root_device != snapshot.skills_root_device
        || identity.skills_root_inode != snapshot.skills_root_inode
        || identity.quarantine_root_device != snapshot.quarantine_root_device
        || identity.quarantine_root_inode != snapshot.quarantine_root_inode
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_ROOT_IDENTITY_DRIFT",
            "recovery",
        ));
    }
    let source = std::ffi::CString::new(snapshot.skill_name.as_str()).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_REMOVAL_DESTINATION_INVALID", "recovery")
    })?;
    let destination =
        std::ffi::CString::new(snapshot.quarantine_destination.as_str()).map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_REMOVAL_DESTINATION_INVALID", "recovery")
        })?;
    let mut from = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let source_missing = unsafe {
        libc::fstatat(
            roots.skills_root.as_raw_fd(),
            source.as_ptr(),
            from.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
        && std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound;
    let mut to = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let destination_present = unsafe {
        libc::fstatat(
            roots.quarantine_root.as_raw_fd(),
            destination.as_ptr(),
            to.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
        && unsafe { to.assume_init() }.st_mode & libc::S_IFMT == libc::S_IFDIR;
    Ok(source_missing && destination_present)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillOperationFinalResponseV1 {
    pub operation_id: String,
    pub status: String,
    /// `None` is an explicit unknown, never a convenient `false`: a native
    /// effect may have completed before its successor receipt became durable.
    pub directory_commit: Option<bool>,
    pub attach_verified: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detach_verified: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine_commit: Option<bool>,
    pub recovery_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_effect_observation: Option<SkillOperationPostEffectObservationV1>,
}

/// A trusted readback taken immediately after a post-effect persistence
/// failure.  This is intentionally response-only: it is not represented as a
/// durable successor until a later reconcile can write a verified receipt.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillOperationPostEffectObservationV1 {
    pub effect: PlanEffectKindV1,
    pub observed: SkillOperationObservedStateV1,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillOperationObservedStateV1 {
    ObservedTrue,
    ObservedFalse,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct SkillOperationApplyReceiptV1 {
    pub final_response: SkillOperationFinalResponseV1,
    pub ledger: SkillOperationLedgerV1,
}

#[derive(Debug)]
pub struct SkillOperationError {
    pub code: String,
    pub phase: String,
}

impl SkillOperationError {
    fn new(code: &str, phase: &str) -> Self {
        Self {
            code: code.into(),
            phase: phase.into(),
        }
    }
    fn install(error: InstallError) -> Self {
        Self::new(&error.code, &error.phase)
    }

    /// Errors returned before a durable intent is recorded are definitive: an
    /// operation-id-only reconcile has no authority state to recover.  The
    /// coordinator returns uncertain outcomes directly after an effect crosses
    /// its native boundary, so this conservative classifier must never turn a
    /// malformed request, missing ledger, expiry, or target drift into a
    /// spurious recovery obligation.
    pub fn recovery_required(&self) -> bool {
        !matches!(
            self.code.as_str(),
            "SKILL_OPERATION_LEDGER_NOT_FOUND"
                | "SKILL_OPERATION_CONFIRMATION_CAPABILITY_INVALID"
                | "SKILL_OPERATION_CONFIRMATION_EXPIRED"
                | "SKILL_OPERATION_TARGET_BINDING_DRIFT"
                | "SKILL_OPERATION_TARGET_ORG_DRIFT"
                | "SKILL_OPERATION_SOURCE_REQUEST_INVALID"
                | "SKILL_OPERATION_PLAN_MISMATCH"
                | "SKILL_OPERATION_OPERATION_BUSY"
                | "SKILL_OPERATION_RECONCILE_NOT_ADMITTED"
                | "SKILL_OPERATION_CONTINUE_NOT_ADMITTED"
                | "SKILL_OPERATION_NEW_PLAN_REQUIRED"
        ) && matches!(self.phase.as_str(), "recovery" | "ledger")
    }
}
impl std::fmt::Display for SkillOperationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} during {}", self.code, self.phase)
    }
}
impl std::error::Error for SkillOperationError {}

/// A prepared operation owns the opened exact staged archive snapshot until the one explicit
/// confirmation is consumed.  It is deliberately not Clone: callers cannot
/// fork an exact effect set into two applies.
pub struct SkillOperationPrepared {
    staged: ExactStagedGithubArchive,
    source: GithubInspectionSource,
    plan: SkillPlanV1,
    ledger_root: File,
    ledger_name: String,
    _operation_lock: File,
    ledger: SkillOperationLedgerV1,
    capability: SkillOperationConfirmationCapabilityV1,
}

/// Reserves bounded host-owned lifecycle capacity before the Gateway downloads
/// an archive.  The reservation is durable so a competing process observes it
/// even after the downloader crashes; expiry GC can reclaim only this
/// pre-effect, unconsumed state.
pub fn reserve_install_plan_capacity(
    staging_root: &File,
    ledger_root: &File,
    operation_id: &str,
    expires_at_unix_seconds: u64,
) -> Result<(), SkillOperationError> {
    valid_operation_id(operation_id)?;
    ensure_private_root(staging_root)?;
    ensure_private_root(ledger_root)?;
    let _lifecycle = acquire_lifecycle_lock(ledger_root)?;
    gc_expired_install_lifecycle(staging_root, ledger_root)?;
    let usage = lifecycle_usage(ledger_root)?;
    if usage.active_entries >= MAX_ACTIVE_INSTALL_PLANS
        || usage
            .reserved_bytes
            .saturating_add(crate::MAX_ARCHIVE_BYTES as u64)
            > MAX_RESERVED_STAGED_BYTES
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_PLAN_CAPACITY_EXHAUSTED",
            "admission",
        ));
    }
    let reservation = SkillOperationReservationV1 {
        schema: SKILL_OPERATION_LEDGER_SCHEMA.into(),
        operation_id: operation_id.into(),
        expires_at_unix_seconds,
        reserved_staged_bytes: crate::MAX_ARCHIVE_BYTES as u64,
        staging_directory_name: exact_staging_directory_name(operation_id)
            .map_err(|error| SkillOperationError::new(&error.code, &error.phase))?,
        phase: SkillOperationReservationPhaseV1::Reserved,
        staged: None,
    };
    let name = reservation_name(operation_id);
    let file = create_new_at(
        ledger_root.as_raw_fd(),
        &name,
        0o600,
        "SKILL_OPERATION_OPERATION_EXISTS",
    )?;
    write_json_owned(file, &reservation)?;
    sync_directory(ledger_root)
}

/// Releases only a reservation whose caller has not created a staging object.
/// Gateway uses this on resolver/download failure while still holding no
/// archive handle.  Staged failures intentionally remain durable for the
/// lifecycle GC path; deleting their reservation by name would make an
/// unverified staging child invisible and violate the capacity invariant.
pub fn release_unstaged_install_plan_reservation(
    ledger_root: &File,
    operation_id: &str,
) -> Result<(), SkillOperationError> {
    valid_operation_id(operation_id)?;
    ensure_private_root(ledger_root)?;
    let _lifecycle = acquire_lifecycle_lock(ledger_root)?;
    release_unstaged_install_plan_reservation_locked(ledger_root, operation_id)
}

fn release_unstaged_install_plan_reservation_locked(
    ledger_root: &File,
    operation_id: &str,
) -> Result<(), SkillOperationError> {
    let ledger_name = format!("{operation_id}{LEDGER_SUFFIX}");
    if name_exists(ledger_root.as_raw_fd(), &ledger_name)? {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_RESERVATION_RELEASE_NOT_ADMITTED",
            "admission",
        ));
    }
    let reservation_name = reservation_name(operation_id);
    let reservation: SkillOperationReservationV1 = read_json_owned(
        ledger_root,
        &reservation_name,
        "SKILL_OPERATION_RESERVATION_INVALID",
    )?;
    if !valid_reservation(&reservation, operation_id)
        || reservation.phase != SkillOperationReservationPhaseV1::Reserved
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_RESERVATION_INVALID",
            "admission",
        ));
    }
    remove_file_at(ledger_root.as_raw_fd(), &reservation_name)?;
    sync_directory(ledger_root)
}

impl SkillOperationPrepared {
    pub fn prepare(
        staging_root: &File,
        ledger_root: &File,
        source: GithubInspectionSource,
        archive_bytes: &[u8],
        request: SkillOperationPrepareRequestV1,
    ) -> Result<Self, SkillOperationError> {
        Self::prepare_inner(
            staging_root,
            ledger_root,
            source,
            archive_bytes,
            request,
            false,
        )
    }

    /// Production callers must create the durable conservative reservation
    /// before resolving bytes from the network.  The original `prepare`
    /// remains available to isolated core fixtures only.
    pub fn prepare_reserved(
        staging_root: &File,
        ledger_root: &File,
        source: GithubInspectionSource,
        archive_bytes: &[u8],
        request: SkillOperationPrepareRequestV1,
    ) -> Result<Self, SkillOperationError> {
        Self::prepare_inner(
            staging_root,
            ledger_root,
            source,
            archive_bytes,
            request,
            true,
        )
    }

    fn prepare_inner(
        staging_root: &File,
        ledger_root: &File,
        source: GithubInspectionSource,
        archive_bytes: &[u8],
        request: SkillOperationPrepareRequestV1,
        require_reservation: bool,
    ) -> Result<Self, SkillOperationError> {
        valid_operation_id(&request.operation_id)?;
        if !valid_sha256(&request.source_request_sha256) {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_SOURCE_REQUEST_INVALID",
                "source_resolution",
            ));
        }
        ensure_private_root(staging_root)?;
        ensure_private_root(ledger_root)?;
        let _lifecycle = acquire_lifecycle_lock(ledger_root)?;
        gc_expired_install_lifecycle(staging_root, ledger_root)?;
        let mut reservation = if require_reservation {
            Some(consume_exact_reservation(
                ledger_root,
                &request.operation_id,
                request.plan.expires_at_unix_seconds,
            )?)
        } else {
            None
        };
        let operation_lock = acquire_operation_lock(ledger_root, &request.operation_id)?;
        let staged = match stage_inspect_exact_github_archive(
            staging_root,
            &request.operation_id,
            &source,
            archive_bytes,
        ) {
            Ok(staged) => staged,
            Err(error) => {
                // This is still the pre-stage Reserved phase: no authoritative
                // staging identity exists, so release the exact reservation
                // immediately instead of converting a deterministic rejection
                // into a TTL denial-of-service.  A release failure is surfaced
                // as lifecycle truth, never hidden behind the resolver error.
                if require_reservation {
                    release_unstaged_install_plan_reservation_locked(
                        ledger_root,
                        &request.operation_id,
                    )?;
                }
                return Err(SkillOperationError::new(&error.code, &error.phase));
            }
        };
        if let Some(reservation) = reservation.as_mut() {
            reservation.phase = SkillOperationReservationPhaseV1::Staged;
            reservation.staged = Some(SkillOperationReservationStagedV1 {
                source: source.clone(),
                archive_sha256: staged.identity().archive_sha256.clone(),
                staging_object_identity_sha256: staged
                    .identity()
                    .staging_object_identity_sha256
                    .clone(),
                staging_receipt_sha256: staged.receipt_sha256().to_string(),
            });
            persist_reservation(ledger_root, reservation)?;
        }
        let plan = staged
            .build_confirmable_plan(&request.plan)
            .map_err(|error| SkillOperationError::new(&error.code, &error.phase))?;
        if plan.eligibility != PlanEligibility::Confirmable || !valid_single_skill_effects(&plan) {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_PLAN_NOT_SINGLE_SKILL",
                "plan",
            ));
        }
        let capability = SkillOperationConfirmationCapabilityV1 {
            operation_id: request.operation_id.clone(),
            value: capability_value(&request.operation_id)?,
        };
        bind_target(&plan, &request.target)?;
        let expires_at_unix_seconds = match plan.expiry {
            PlanExpiryV1::ExpiresAtUnixSeconds { unix_seconds } => unix_seconds,
            PlanExpiryV1::NotApplicable => {
                return Err(SkillOperationError::new(
                    "SKILL_OPERATION_PLAN_EXPIRY_INVALID",
                    "plan",
                ))
            }
        };
        let ledger = SkillOperationLedgerV1 {
            schema: SKILL_OPERATION_LEDGER_SCHEMA.into(),
            generation: 0,
            previous_ledger_sha256: None,
            operation_kind: SkillOperationKindV1::Install,
            operation_id: request.operation_id.clone(),
            plan_digest_sha256: plan.plan_digest_sha256.clone(),
            confirmation_capability_sha256: digest(capability.value.as_bytes()),
            source_request_sha256: Some(request.source_request_sha256),
            source_archive_sha256: Some(staged.identity().archive_sha256.clone()),
            staging_object_identity_sha256: Some(
                staged.identity().staging_object_identity_sha256.clone(),
            ),
            staging_receipt_sha256: Some(staged.receipt_sha256().to_string()),
            inspection_content_sha256: Some(plan.inspection_content_sha256()),
            source_content_sha256: plan.materialized_content_sha256()?,
            target: request.target,
            expires_at_unix_seconds,
            plan: plan.clone(),
            consumed: false,
            effects: plan
                .effects
                .iter()
                .map(|effect| SkillOperationEffectReceiptV1 {
                    order: effect.order,
                    kind: effect.kind.clone(),
                    state: SkillOperationEffectStateV1::NotStarted,
                    intent: SkillOperationEffectIntentV1::None,
                    attempt_id: None,
                    attempt_started_at_unix_seconds: None,
                    evidence_sha256: None,
                    error_code: None,
                })
                .collect(),
            removal: None,
        };
        let ledger_name = format!("{}{}", request.operation_id, LEDGER_SUFFIX);
        write_new_ledger(ledger_root, &ledger_name, &ledger)?;
        if require_reservation {
            remove_file_at(
                ledger_root.as_raw_fd(),
                &reservation_name(&request.operation_id),
            )?;
            sync_directory(ledger_root)?;
        }
        Ok(Self {
            staged,
            source,
            plan,
            ledger_root: ledger_root.try_clone().map_err(|_| {
                SkillOperationError::new("SKILL_OPERATION_LEDGER_ROOT_CLONE_FAILED", "ledger")
            })?,
            ledger_name,
            _operation_lock: operation_lock,
            ledger,
            capability,
        })
    }

    pub fn plan(&self) -> &SkillPlanV1 {
        &self.plan
    }
    pub fn confirmation_capability(&self) -> &SkillOperationConfirmationCapabilityV1 {
        &self.capability
    }
    pub fn ledger(&self) -> &SkillOperationLedgerV1 {
        &self.ledger
    }

    /// Recovery reconstructs the same sealed archive only from the durable
    /// ledger. It never downloads the source URL again and it never recreates
    /// a confirmation capability.
    pub fn resume(
        staging_root: &File,
        ledger_root: &File,
        operation_id: &str,
    ) -> Result<Self, SkillOperationError> {
        valid_operation_id(operation_id)?;
        ensure_private_root(ledger_root)?;
        let operation_lock = acquire_operation_lock(ledger_root, operation_id)?;
        let ledger_name = format!("{}{}", operation_id, LEDGER_SUFFIX);
        let ledger = load_ledger(ledger_root, &ledger_name)?;
        validate_ledger_kind(&ledger, "recovery")?;
        if ledger.schema != SKILL_OPERATION_LEDGER_SCHEMA || ledger.operation_id != operation_id {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT",
                "recovery",
            ));
        }
        if ledger.operation_kind != SkillOperationKindV1::Install
            || ledger.removal.is_some()
            || !valid_sha256(ledger.source_request_sha256.as_deref().unwrap_or(""))
            || !valid_sha256(ledger.source_archive_sha256.as_deref().unwrap_or(""))
            || !valid_sha256(
                ledger
                    .staging_object_identity_sha256
                    .as_deref()
                    .unwrap_or(""),
            )
            || !valid_sha256(ledger.staging_receipt_sha256.as_deref().unwrap_or(""))
            || !valid_sha256(ledger.inspection_content_sha256.as_deref().unwrap_or(""))
            || !valid_sha256(&ledger.source_content_sha256)
            || !valid_sha256(&ledger.confirmation_capability_sha256)
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT",
                "recovery",
            ));
        }
        validate_skill_plan(&ledger.plan)
            .map_err(|error| SkillOperationError::new(&error.code, "recovery"))?;
        if ledger.plan.plan_digest_sha256 != ledger.plan_digest_sha256
            || !valid_single_skill_effects(&ledger.plan)
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_PLAN_DRIFT",
                "recovery",
            ));
        }
        bind_target(&ledger.plan, &ledger.target)?;
        let source = source_from_plan(&ledger.plan)?;
        let source_archive_sha256 = ledger.source_archive_sha256.as_deref().ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_LEDGER_IDENTITY_DRIFT", "recovery")
        })?;
        let staging_object_identity_sha256 = ledger
            .staging_object_identity_sha256
            .as_deref()
            .ok_or_else(|| {
                SkillOperationError::new("SKILL_OPERATION_LEDGER_IDENTITY_DRIFT", "recovery")
            })?;
        let staging_receipt_sha256 = ledger.staging_receipt_sha256.as_deref().ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_LEDGER_IDENTITY_DRIFT", "recovery")
        })?;
        let staged = crate::resolver::reopen_exact_github_archive(
            staging_root,
            operation_id,
            &source,
            source_archive_sha256,
            staging_object_identity_sha256,
            staging_receipt_sha256,
        )
        .map_err(|error| SkillOperationError::new(&error.code, &error.phase))?;
        // Reopening validates the staged object itself.  Bind its recovered
        // source and inspection back to the durable plan as well, so a
        // syntactically matching owner/repo/commit/path cannot substitute
        // archive or package bytes before this handle becomes usable.
        if !same_staged_plan_source(&staged.identity().source, &ledger.plan.source)
            || staged.identity().archive_sha256 != source_archive_sha256
            || staged.inspection().package.content_sha256
                != ledger.inspection_content_sha256.as_deref().unwrap_or("")
            || staged.inspection().package.content_sha256 != ledger.plan.inspection_content_sha256()
            || exact_materialized_content_sha256(&staged, &source)? != ledger.source_content_sha256
            || exact_materialized_content_sha256(&staged, &source)?
                != ledger.plan.materialized_content_sha256()?
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_STAGED_SOURCE_DRIFT",
                "recovery",
            ));
        }
        Ok(Self {
            staged,
            source,
            plan: ledger.plan.clone(),
            ledger_root: ledger_root.try_clone().map_err(|_| {
                SkillOperationError::new("SKILL_OPERATION_LEDGER_ROOT_CLONE_FAILED", "recovery")
            })?,
            ledger_name,
            ledger,
            capability: SkillOperationConfirmationCapabilityV1 {
                operation_id: operation_id.into(),
                value: String::new(),
            },
            _operation_lock: operation_lock,
        })
    }

    /// If attach was interrupted, recovery is readback-only.  It intentionally
    /// cannot issue a second attach request; a missing membership remains
    /// uncertain and is surfaced to the deterministic host.
    pub fn reconcile_attach_readback(
        &mut self,
        fresh_target: &SkillOperationTargetBindingV1,
        operon: &mut dyn SkillOperationOperonAdapter,
    ) -> Result<SkillOperationFinalResponseV1, SkillOperationError> {
        reconcile_target(&self.ledger, fresh_target)?;
        let effect = self.ledger.effects.get(1).ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_LEDGER_EFFECTS_INVALID", "recovery")
        })?;
        if effect.intent != SkillOperationEffectIntentV1::InProgress
            || !matches!(
                effect.state,
                SkillOperationEffectStateV1::Uncertain | SkillOperationEffectStateV1::NotStarted
            )
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_RECONCILE_NOT_ADMITTED",
                "recovery",
            ));
        }
        let skill = selected_skill_name(&self.plan)?;
        match operon.readback(&skill, &self.ledger.target.active_org) {
            Ok(true) => {
                self.ledger.effects[1].state = SkillOperationEffectStateV1::Verified;
                self.ledger.effects[1].intent = SkillOperationEffectIntentV1::None;
                self.ledger.effects[1].evidence_sha256 = Some(digest(skill.as_bytes()));
                self.ledger.effects[1].error_code = None;
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
            }
            Ok(false) => self.reconcile_uncertain(1, None)?,
            Err(error) => self.reconcile_uncertain(1, Some(error.code))?,
        }
        Ok(project(&self.ledger))
    }

    /// Package crash recovery is likewise readback-only: a matching owned
    /// marker and canonical content hash can close the attempt, otherwise the
    /// operation stays uncertain and never calls commit a second time.
    pub fn reconcile_package_readback(
        &mut self,
        skills_root: &File,
        fresh_target: &SkillOperationTargetBindingV1,
    ) -> Result<SkillOperationFinalResponseV1, SkillOperationError> {
        reconcile_target(&self.ledger, fresh_target)?;
        validate_install_skills_root(skills_root, fresh_target)?;
        let effect = self.ledger.effects.first().ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_LEDGER_EFFECTS_INVALID", "recovery")
        })?;
        if effect.intent != SkillOperationEffectIntentV1::InProgress {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_RECONCILE_NOT_ADMITTED",
                "recovery",
            ));
        }
        let skill = selected_skill_name(&self.plan)?;
        match package_snapshot_matches_at(
            skills_root,
            &skill,
            &self.source,
            &self.ledger.source_content_sha256,
        ) {
            SkillOperationObservedStateV1::ObservedTrue => {
                self.ledger.effects[0].state = SkillOperationEffectStateV1::Verified;
                self.ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
                self.ledger.effects[0].evidence_sha256 =
                    Some(digest(self.ledger.source_content_sha256.as_bytes()));
                self.ledger.effects[0].error_code = None;
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
            }
            SkillOperationObservedStateV1::ObservedFalse => {
                self.ledger.effects[0].state = SkillOperationEffectStateV1::Failed;
                self.ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
                self.ledger.effects[0].error_code =
                    Some("SKILL_OPERATION_PACKAGE_EFFECT_NOT_OBSERVED".into());
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
            }
            SkillOperationObservedStateV1::Unknown => self.reconcile_uncertain(0, None)?,
        }
        Ok(project(&self.ledger))
    }

    /// Applies the only admitted effect set. The caller supplies a typed
    /// OPERON adapter so tests never need a real Science runtime; production
    /// wiring uses `update_agent_skills` and its native GET readback.
    pub fn apply(
        mut self,
        data_dir: &Path,
        skills_root: &File,
        capability: &SkillOperationConfirmationCapabilityV1,
        fresh_target: &SkillOperationTargetBindingV1,
        operon: &mut dyn SkillOperationOperonAdapter,
    ) -> Result<SkillOperationApplyReceiptV1, SkillOperationError> {
        validate_ledger_kind(&self.ledger, "apply")?;
        if capability.operation_id != self.ledger.operation_id
            || digest(capability.value.as_bytes()) != self.ledger.confirmation_capability_sha256
            || self.ledger.consumed
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_CONFIRMATION_CAPABILITY_INVALID",
                "confirmation",
            ));
        }
        if fresh_target != &self.ledger.target {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_TARGET_BINDING_DRIFT",
                "preflight",
            ));
        }
        validate_install_skills_root(skills_root, fresh_target)?;
        if unix_seconds() >= self.ledger.expires_at_unix_seconds {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_CONFIRMATION_EXPIRED",
                "confirmation",
            ));
        }
        if active_org(data_dir).map_err(SkillOperationError::install)?
            != self.ledger.target.active_org
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_TARGET_ORG_DRIFT",
                "preflight",
            ));
        }
        validate_skill_plan(&self.plan)
            .map_err(|error| SkillOperationError::new(&error.code, "plan"))?;
        // Re-read exact staged archive's retained object immediately before materialisation.
        let bytes = self
            .staged
            .archive_bytes()
            .map_err(|error| SkillOperationError::new(&error.code, &error.phase))?;
        let package = match package_or_bundle_from_github_archive(
            &bytes,
            &self.source.path,
            &self.source.repo,
        )
        .map_err(SkillOperationError::install)?
        {
            ValidatedArchive::Skill(value) => value,
            _ => {
                return Err(SkillOperationError::new(
                    "SKILL_OPERATION_BUNDLE_OR_PLUGIN_REJECTED",
                    "validation",
                ))
            }
        };
        if package.content_sha256 != self.ledger.source_content_sha256 {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_CONTENT_DRIFT",
                "readback",
            ));
        }
        let org = self.ledger.target.active_org.clone();
        let descriptor = SourceDescriptor {
            kind: SourceKind::Github,
            repo: format!("{}/{}", self.source.owner, self.source.repo),
            sha: self.source.commit_sha.clone(),
            path: self.source.path.clone(),
            archive_sha256: None,
        };
        let skill_name = package.skill_name.clone();
        // Hold the same durable per-name fence used by the legacy bridge from
        // the final sealed-root preflight through package publication, ledger
        // transitions, OPERON attach/readback and the returned projection.
        // The fd-relative open prevents a post-preflight pathname rebind from
        // silently selecting a different lock inode.
        let _skill_lock = acquire_install_lock_at(skills_root, &skill_name)
            .map_err(SkillOperationError::install)?;
        self.begin_effect(0)?;
        let commit = match commit_package_at(skills_root, package, descriptor, &org) {
            Ok(value) => value,
            Err(error) => {
                if error.directory_commit {
                    // `commit_package_at` reports this bit only after its
                    // no-clobber rename succeeded.  Treat it exactly like a
                    // successor-persist interruption: the pre-effect intent
                    // remains recoverable, response is readback-based, and
                    // operation-id reconcile is the only closure route.
                    let observed = package_snapshot_matches_at(
                        skills_root,
                        &skill_name,
                        &self.source,
                        &self.ledger.source_content_sha256,
                    );
                    let final_response = record_post_effect_persist_failure(
                        &self.ledger_root,
                        &self.ledger_name,
                        &mut self.ledger,
                        0,
                        SkillOperationError::install(error),
                        observed,
                    );
                    return Ok(SkillOperationApplyReceiptV1 {
                        final_response,
                        ledger: self.ledger,
                    });
                }
                self.ledger.effects[0].state = SkillOperationEffectStateV1::Failed;
                self.ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
                self.ledger.effects[0].error_code = Some(error.code);
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
                return Ok(SkillOperationApplyReceiptV1 {
                    final_response: project(&self.ledger),
                    ledger: self.ledger,
                });
            }
        };
        let skill = commit.skill_name;
        if let Err(error) = self.verified(0, digest(commit.content_sha256.as_bytes())) {
            let observed = package_snapshot_matches_at(
                skills_root,
                &skill,
                &self.source,
                &self.ledger.source_content_sha256,
            );
            let final_response = record_post_effect_persist_failure(
                &self.ledger_root,
                &self.ledger_name,
                &mut self.ledger,
                0,
                error,
                observed,
            );
            return Ok(SkillOperationApplyReceiptV1 {
                final_response,
                ledger: self.ledger,
            });
        }
        #[cfg(test)]
        if STOP_AFTER_PACKAGE_VERIFY_ONCE.with(|value| value.replace(false)) {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_TEST_PAUSE_AFTER_PACKAGE",
                "test",
            ));
        }
        self.begin_effect(1)?;
        match operon.attach_and_readback(&skill, &org) {
            Ok(()) => {
                if let Err(error) = self.verified(1, digest(skill.as_bytes())) {
                    let observed = operon_observation(operon, &skill, &org, true);
                    let final_response = record_post_effect_persist_failure(
                        &self.ledger_root,
                        &self.ledger_name,
                        &mut self.ledger,
                        1,
                        error,
                        observed,
                    );
                    return Ok(SkillOperationApplyReceiptV1 {
                        final_response,
                        ledger: self.ledger,
                    });
                }
            }
            Err(error) => {
                let state = if error.uncertain {
                    SkillOperationEffectStateV1::Uncertain
                } else {
                    SkillOperationEffectStateV1::Failed
                };
                self.ledger.effects[1].state = state;
                if self.ledger.effects[1].state == SkillOperationEffectStateV1::Failed {
                    self.ledger.effects[1].intent = SkillOperationEffectIntentV1::None;
                }
                self.ledger.effects[1].error_code = Some(error.code);
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
                return Ok(SkillOperationApplyReceiptV1 {
                    final_response: project(&self.ledger),
                    ledger: self.ledger,
                });
            }
        }
        Ok(SkillOperationApplyReceiptV1 {
            final_response: project(&self.ledger),
            ledger: self.ledger,
        })
    }
    /// Completes only the next untouched effect after a crash between two
    /// verified effects.  No capability is accepted here: the original
    /// capability was consumed by the durable first-effect intent, and this
    /// route is admitted only for its exact verified prefix.
    pub fn continue_apply(
        mut self,
        _data_dir: &Path,
        skills_root: &File,
        fresh_target: &SkillOperationTargetBindingV1,
        operon: &mut dyn SkillOperationOperonAdapter,
    ) -> Result<SkillOperationApplyReceiptV1, SkillOperationError> {
        let skill = selected_skill_name(&self.plan)?;
        // Take the same non-blocking fd-relative fence before *any*
        // mutable-package readback.  Continuation preserves the bridge's
        // immediate INSTALL_BUSY contract; a caller retries only after the
        // competing removal/replacement releases this exact lock inode.
        let _skill_lock =
            acquire_install_lock_at(skills_root, &skill).map_err(SkillOperationError::install)?;
        #[cfg(test)]
        if let Some(target) =
            REPLACE_INSTALL_NAME_AFTER_CONTINUATION_LOCK.with(|value| value.borrow_mut().take())
        {
            std::fs::remove_dir_all(&target).map_err(|_| {
                SkillOperationError::new("SKILL_OPERATION_TEST_REPLACE_FAILED", "test")
            })?;
            std::fs::create_dir_all(&target).map_err(|_| {
                SkillOperationError::new("SKILL_OPERATION_TEST_REPLACE_FAILED", "test")
            })?;
            std::fs::write(target.join("SKILL.md"), b"replacement\n").map_err(|_| {
                SkillOperationError::new("SKILL_OPERATION_TEST_REPLACE_FAILED", "test")
            })?;
        }
        self.require_continuation(fresh_target)?;
        validate_install_skills_root(skills_root, fresh_target)?;
        let org = self.ledger.target.active_org.clone();
        // A verified prefix is not authority to attach a replacement placed
        // under the same name after the crash.  Re-read the owned marker and
        // canonical payload against the sealed archive identity first.
        match package_snapshot_matches_at(
            skills_root,
            &skill,
            &self.source,
            &self.ledger.source_content_sha256,
        ) {
            SkillOperationObservedStateV1::ObservedTrue => {}
            _ => {
                return Err(SkillOperationError::new(
                    "SKILL_OPERATION_NEW_PLAN_REQUIRED",
                    "preflight",
                ))
            }
        }
        // Retain the already-held fence through native attach/readback and
        // final ledger projection.
        self.begin_effect(1)?;
        match operon.attach_and_readback(&skill, &org) {
            Ok(()) => {
                if let Err(error) = self.verified(1, digest(skill.as_bytes())) {
                    let observed = operon_observation(operon, &skill, &org, true);
                    let final_response = record_post_effect_persist_failure(
                        &self.ledger_root,
                        &self.ledger_name,
                        &mut self.ledger,
                        1,
                        error,
                        observed,
                    );
                    return Ok(SkillOperationApplyReceiptV1 {
                        final_response,
                        ledger: self.ledger,
                    });
                }
            }
            Err(error) => {
                self.ledger.effects[1].state = if error.uncertain {
                    SkillOperationEffectStateV1::Uncertain
                } else {
                    SkillOperationEffectStateV1::Failed
                };
                if self.ledger.effects[1].state == SkillOperationEffectStateV1::Failed {
                    self.ledger.effects[1].intent = SkillOperationEffectIntentV1::None;
                }
                self.ledger.effects[1].error_code = Some(error.code);
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
            }
        }
        Ok(SkillOperationApplyReceiptV1 {
            final_response: project(&self.ledger),
            ledger: self.ledger,
        })
    }

    fn verified(&mut self, index: usize, evidence: String) -> Result<(), SkillOperationError> {
        self.ledger.effects[index].state = SkillOperationEffectStateV1::Verified;
        self.ledger.effects[index].intent = SkillOperationEffectIntentV1::None;
        self.ledger.effects[index].evidence_sha256 = Some(evidence);
        self.ledger.effects[index].error_code = None;
        // There is exactly one successor write after a completed external
        // effect.  If it cannot become durable, the caller takes a trusted
        // immediate readback and returns a recovery projection instead of
        // leaving an untested Committed -> Verified second-write window.
        persist_after_effect(&self.ledger_root, &self.ledger_name, &mut self.ledger)
    }
    fn reconcile_uncertain(
        &mut self,
        index: usize,
        error_code: Option<String>,
    ) -> Result<(), SkillOperationError> {
        self.ledger.effects[index].state = SkillOperationEffectStateV1::Uncertain;
        self.ledger.effects[index].intent = SkillOperationEffectIntentV1::InProgress;
        self.ledger.effects[index].error_code = error_code;
        persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)
    }
    fn require_continuation(
        &self,
        fresh_target: &SkillOperationTargetBindingV1,
    ) -> Result<(), SkillOperationError> {
        validate_ledger_kind(&self.ledger, "continue")?;
        if fresh_target != &self.ledger.target
            || unix_seconds() >= self.ledger.expires_at_unix_seconds
            || !exact_verified_prefix_next_not_started(&self.ledger)
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_NEW_PLAN_REQUIRED",
                "confirmation",
            ));
        }
        Ok(())
    }
    fn begin_effect(&mut self, index: usize) -> Result<(), SkillOperationError> {
        if self.ledger.effects[index].state != SkillOperationEffectStateV1::NotStarted
            || self.ledger.effects[index].intent != SkillOperationEffectIntentV1::None
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_EFFECT_REENTRY_REQUIRES_RECONCILE",
                "recovery",
            ));
        }
        if index == 0 {
            // Capability consumption and the first destructive-effect intent
            // share the same durable write. A crash can therefore never leave
            // a consumed confirmation paired with a NotStarted package.
            self.ledger.consumed = true;
        }
        self.ledger.effects[index].intent = SkillOperationEffectIntentV1::InProgress;
        self.ledger.effects[index].attempt_id = Some(capability_value(&format!(
            "{}-{index}",
            self.ledger.operation_id
        ))?);
        self.ledger.effects[index].attempt_started_at_unix_seconds = Some(unix_seconds());
        persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)
    }
}

pub trait SkillOperationOperonAdapter {
    fn attach_and_readback(&mut self, skill: &str, org: &str) -> Result<(), crate::AttachError>;
    fn detach_and_readback(&mut self, skill: &str, org: &str) -> Result<(), crate::AttachError>;
    fn readback(&mut self, skill: &str, org: &str) -> Result<bool, crate::AttachError>;
}

/// Removal uses the same durable ledger, lock and projection vocabulary as
/// installation.  It is intentionally a separate prepared handle because no
/// archive/staging descriptor exists for an already-installed directory.
pub struct SkillOperationRemovalPrepared {
    ledger_root: File,
    ledger_name: String,
    _operation_lock: File,
    ledger: SkillOperationLedgerV1,
    capability: SkillOperationConfirmationCapabilityV1,
}

impl SkillOperationRemovalPrepared {
    pub fn prepare(
        operation_staging_root: &File,
        ledger_root: &File,
        data_dir: &Path,
        roots: &SkillRemovalRoots,
        operation_id: String,
        expires_at_unix_seconds: u64,
        target: SkillOperationTargetBindingV1,
        skill_name: String,
    ) -> Result<Self, SkillOperationError> {
        valid_operation_id(&operation_id)?;
        ensure_private_root(operation_staging_root)?;
        ensure_private_root(ledger_root)?;
        // Removal has no archive staging, but its terminal ledgers occupy the
        // same fixed authority namespace.  A bounded sequence of expired,
        // completed removals must therefore retire before admitting another
        // plan instead of accumulating until directory enumeration fails.
        let _lifecycle = acquire_lifecycle_lock(ledger_root)?;
        gc_expired_install_lifecycle(operation_staging_root, ledger_root)?;
        if lifecycle_usage(ledger_root)?.active_entries >= MAX_ACTIVE_INSTALL_PLANS {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_PLAN_CAPACITY_EXHAUSTED",
                "admission",
            ));
        }
        if active_org(data_dir).map_err(SkillOperationError::install)? != target.active_org {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_TARGET_ORG_DRIFT",
                "preflight",
            ));
        }
        if find_bundle_for_skill(data_dir, &skill_name)
            .map_err(SkillOperationError::install)?
            .is_some()
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_BUNDLE_UNSUPPORTED",
                "preflight",
            ));
        }
        let roots_identity = roots.identity()?;
        let leaf = open_removal_source_leaf(roots, &skill_name)?;
        let (skill_leaf_device, skill_leaf_inode) =
            directory_object_identity(&leaf, "SKILL_OPERATION_REMOVAL_SNAPSHOT_DRIFT")?;
        let marker = read_json_file_at(&leaf, crate::IMPORT_ORIGIN_FILE)?;
        let marker_sha256 = digest(&serde_json::to_vec(&marker).map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_MARKER_INVALID", "preflight")
        })?);
        let files = scan_payload_at(&leaf, MAX_FILES, MAX_TOTAL_BYTES)?;
        let content_sha256 = crate::archive::canonical_content_sha256(&files);
        if marker
            .get("content_sha256")
            .and_then(serde_json::Value::as_str)
            != Some(content_sha256.as_str())
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_CONTENT_DRIFT",
                "preflight",
            ));
        }
        let destination =
            skill_removal_destination(&operation_id, &skill_name, &marker_sha256, &content_sha256)?;
        let plan = build_skill_removal_plan(&SkillRemovalPlanRequestV1 {
            plan_id: operation_id.clone(),
            expires_at_unix_seconds,
            target: ConfirmablePlanTargetV1 {
                science_runtime_identity_sha256: target.science_runtime_identity_sha256.clone(),
                data_dir_identity_sha256: target.data_dir_identity_sha256.clone(),
                active_org_identity_sha256: target.active_org_identity_sha256.clone(),
                skills_root_device: target.skills_root_device,
                skills_root_inode: target.skills_root_inode,
            },
            skill_name: skill_name.clone(),
            marker_sha256: marker_sha256.clone(),
            content_sha256: content_sha256.clone(),
        })
        .map_err(|e| SkillOperationError::new(&e.code, "plan"))?;
        let snapshot = SkillRemovalSnapshotV1 {
            skill_name,
            marker_sha256: marker_sha256.clone(),
            content_sha256: content_sha256.clone(),
            skill_leaf_device,
            skill_leaf_inode,
            skills_root_device: roots_identity.skills_root_device,
            skills_root_inode: roots_identity.skills_root_inode,
            quarantine_root_device: roots_identity.quarantine_root_device,
            quarantine_root_inode: roots_identity.quarantine_root_inode,
            quarantine_destination: destination,
        };
        let lock = acquire_operation_lock(ledger_root, &operation_id)?;
        let capability = SkillOperationConfirmationCapabilityV1 {
            operation_id: operation_id.clone(),
            value: capability_value(&operation_id)?,
        };
        let ledger = SkillOperationLedgerV1 {
            schema: SKILL_OPERATION_LEDGER_SCHEMA.into(),
            generation: 0,
            previous_ledger_sha256: None,
            operation_kind: SkillOperationKindV1::Removal,
            operation_id: operation_id.clone(),
            plan_digest_sha256: plan.plan_digest_sha256.clone(),
            confirmation_capability_sha256: digest(capability.value.as_bytes()),
            source_request_sha256: None,
            source_archive_sha256: None,
            staging_object_identity_sha256: None,
            staging_receipt_sha256: None,
            inspection_content_sha256: None,
            source_content_sha256: content_sha256,
            target,
            expires_at_unix_seconds,
            plan: plan.clone(),
            consumed: false,
            effects: plan
                .effects
                .iter()
                .map(|effect| SkillOperationEffectReceiptV1 {
                    order: effect.order,
                    kind: effect.kind.clone(),
                    state: SkillOperationEffectStateV1::NotStarted,
                    intent: SkillOperationEffectIntentV1::None,
                    attempt_id: None,
                    attempt_started_at_unix_seconds: None,
                    evidence_sha256: None,
                    error_code: None,
                })
                .collect(),
            removal: Some(snapshot),
        };
        let ledger_name = format!("{operation_id}{LEDGER_SUFFIX}");
        write_new_ledger(ledger_root, &ledger_name, &ledger)?;
        Ok(Self {
            ledger_root: ledger_root.try_clone().map_err(|_| {
                SkillOperationError::new("SKILL_OPERATION_LEDGER_ROOT_CLONE_FAILED", "ledger")
            })?,
            ledger_name,
            _operation_lock: lock,
            ledger,
            capability,
        })
    }
    pub fn ledger(&self) -> &SkillOperationLedgerV1 {
        &self.ledger
    }
    pub fn confirmation_capability(&self) -> &SkillOperationConfirmationCapabilityV1 {
        &self.capability
    }
    /// Removal recovery never reconstructs an archive and never generates a
    /// capability.  It is deliberately readback-only: a stored intent cannot
    /// authorize another detach or another rename after a crash.
    pub fn resume(ledger_root: &File, operation_id: &str) -> Result<Self, SkillOperationError> {
        valid_operation_id(operation_id)?;
        ensure_private_root(ledger_root)?;
        let lock = acquire_operation_lock(ledger_root, operation_id)?;
        let ledger_name = format!("{operation_id}{LEDGER_SUFFIX}");
        let ledger = load_ledger(ledger_root, &ledger_name)?;
        validate_ledger_kind(&ledger, "recovery")?;
        if ledger.schema != SKILL_OPERATION_LEDGER_SCHEMA
            || ledger.operation_id != operation_id
            || ledger.operation_kind != SkillOperationKindV1::Removal
            || ledger.removal.is_none()
            || ledger.source_request_sha256.is_some()
            || ledger.source_archive_sha256.is_some()
            || ledger.staging_object_identity_sha256.is_some()
            || ledger.staging_receipt_sha256.is_some()
            || ledger.inspection_content_sha256.is_some()
            || !valid_sha256(&ledger.confirmation_capability_sha256)
            || !valid_sha256(&ledger.source_content_sha256)
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT",
                "recovery",
            ));
        }
        validate_skill_plan(&ledger.plan)
            .map_err(|error| SkillOperationError::new(&error.code, "recovery"))?;
        if ledger.plan.plan_digest_sha256 != ledger.plan_digest_sha256
            || !valid_removal_effects(&ledger.plan)
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_PLAN_DRIFT",
                "recovery",
            ));
        }
        bind_target(&ledger.plan, &ledger.target)?;
        Ok(Self {
            ledger_root: ledger_root.try_clone().map_err(|_| {
                SkillOperationError::new("SKILL_OPERATION_LEDGER_ROOT_CLONE_FAILED", "recovery")
            })?,
            ledger_name,
            _operation_lock: lock,
            ledger,
            capability: SkillOperationConfirmationCapabilityV1 {
                operation_id: operation_id.into(),
                value: String::new(),
            },
        })
    }

    pub fn reconcile_detach_readback(
        &mut self,
        fresh_target: &SkillOperationTargetBindingV1,
        operon: &mut dyn SkillOperationOperonAdapter,
    ) -> Result<SkillOperationFinalResponseV1, SkillOperationError> {
        reconcile_target(&self.ledger, fresh_target)?;
        let snapshot = self.ledger.removal.clone().ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_REMOVAL_SNAPSHOT_MISSING", "recovery")
        })?;
        let effect = self.ledger.effects.first().ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_LEDGER_EFFECTS_INVALID", "recovery")
        })?;
        if effect.intent != SkillOperationEffectIntentV1::InProgress {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_RECONCILE_NOT_ADMITTED",
                "recovery",
            ));
        }
        match operon.readback(&snapshot.skill_name, &self.ledger.target.active_org) {
            Ok(false) => self.verified(0, digest(snapshot.skill_name.as_bytes()))?,
            Ok(true) => {
                self.ledger.effects[0].state = SkillOperationEffectStateV1::Uncertain;
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
            }
            Err(error) => {
                self.ledger.effects[0].state = SkillOperationEffectStateV1::Uncertain;
                self.ledger.effects[0].error_code = Some(error.code);
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
            }
        }
        Ok(project(&self.ledger))
    }

    pub fn reconcile_quarantine_readback(
        &mut self,
        roots: &SkillRemovalRoots,
        fresh_target: &SkillOperationTargetBindingV1,
    ) -> Result<SkillOperationFinalResponseV1, SkillOperationError> {
        reconcile_target(&self.ledger, fresh_target)?;
        let snapshot = self.ledger.removal.clone().ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_REMOVAL_SNAPSHOT_MISSING", "recovery")
        })?;
        let effect = self.ledger.effects.get(1).ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_LEDGER_EFFECTS_INVALID", "recovery")
        })?;
        if effect.intent != SkillOperationEffectIntentV1::InProgress {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_RECONCILE_NOT_ADMITTED",
                "recovery",
            ));
        }
        // No rename is issued here.  The only admissible closure is exact
        // destination readback plus source absence under the sealed roots.
        match quarantine_snapshot_matches_at(roots, &snapshot) {
            // The name transition readback proves only that the old rename is
            // visible.  It becomes a durable receipt only after *both* held
            // parent descriptors acknowledge that exact transition.  This
            // reconcile route never calls rename again.
            Ok(true) => match sync_removal_parent_roots(roots) {
                Ok(()) => self.verified(1, digest(snapshot.quarantine_destination.as_bytes()))?,
                Err(error) => self.reconcile_uncertain(1, Some(error.code))?,
            },
            Ok(false) => self.reconcile_uncertain(1, None)?,
            Err(error) => self.reconcile_uncertain(1, Some(error.code))?,
        }
        Ok(project(&self.ledger))
    }
    pub fn apply(
        mut self,
        data_dir: &Path,
        roots: &SkillRemovalRoots,
        capability: &SkillOperationConfirmationCapabilityV1,
        fresh_target: &SkillOperationTargetBindingV1,
        operon: &mut dyn SkillOperationOperonAdapter,
    ) -> Result<SkillOperationApplyReceiptV1, SkillOperationError> {
        validate_ledger_kind(&self.ledger, "apply")?;
        if capability.operation_id != self.ledger.operation_id
            || self.ledger.consumed
            || digest(capability.value.as_bytes()) != self.ledger.confirmation_capability_sha256
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_CONFIRMATION_CAPABILITY_INVALID",
                "confirmation",
            ));
        }
        if fresh_target != &self.ledger.target {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_TARGET_BINDING_DRIFT",
                "preflight",
            ));
        }
        if unix_seconds() >= self.ledger.expires_at_unix_seconds {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_CONFIRMATION_EXPIRED",
                "confirmation",
            ));
        }
        if self.ledger.operation_kind != SkillOperationKindV1::Removal
            || self.ledger.source_request_sha256.is_some()
            || self.ledger.source_archive_sha256.is_some()
            || self.ledger.staging_object_identity_sha256.is_some()
            || self.ledger.staging_receipt_sha256.is_some()
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_SUBJECT_INVALID",
                "ledger",
            ));
        }
        let snapshot = self.ledger.removal.clone().ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_REMOVAL_SNAPSHOT_MISSING", "ledger")
        })?;
        // This is the exact legacy lock filename, opened below the caller-held
        // root descriptor.  It remains held through detach, quarantine and
        // every durable state transition in this method.
        let _skill_lock = acquire_install_lock_at(&roots.skills_root, &snapshot.skill_name)
            .map_err(SkillOperationError::install)?;
        // This must happen *after* taking the same per-name lock as install.
        // A plan is merely a sealed intent; it is not authority to detach a
        // replacement, a re-used name, or a target that moved under us.
        let held_source_leaf =
            verify_removal_snapshot_at(data_dir, roots, &self.ledger.target, &snapshot)?;
        self.ledger.consumed = true;
        self.begin_effect(0)?;
        match operon.detach_and_readback(&snapshot.skill_name, &self.ledger.target.active_org) {
            Ok(()) => {
                if let Err(error) = self.verified(0, digest(snapshot.skill_name.as_bytes())) {
                    let observed = operon_observation(
                        operon,
                        &snapshot.skill_name,
                        &self.ledger.target.active_org,
                        false,
                    );
                    let final_response = record_post_effect_persist_failure(
                        &self.ledger_root,
                        &self.ledger_name,
                        &mut self.ledger,
                        0,
                        error,
                        observed,
                    );
                    return Ok(SkillOperationApplyReceiptV1 {
                        final_response,
                        ledger: self.ledger,
                    });
                }
            }
            Err(error) => {
                self.ledger.effects[0].state = if error.uncertain {
                    SkillOperationEffectStateV1::Uncertain
                } else {
                    SkillOperationEffectStateV1::Failed
                };
                if self.ledger.effects[0].state == SkillOperationEffectStateV1::Failed {
                    self.ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
                }
                self.ledger.effects[0].error_code = Some(error.code);
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
                return Ok(SkillOperationApplyReceiptV1 {
                    final_response: project(&self.ledger),
                    ledger: self.ledger,
                });
            }
        }
        // OPERON is an external mutation boundary.  Its successful detach
        // does not prove the opened owned directory remained unchanged while
        // it ran; never quarantine a replacement under the same leaf.
        if !removal_source_leaf_matches(&held_source_leaf, &snapshot)? {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_NEW_PLAN_REQUIRED",
                "preflight",
            ));
        }
        self.begin_effect(1)?;
        if let Err(error) = quarantine_owned_skill_at(roots, &snapshot, &held_source_leaf) {
            // `renameatx_np` has already made the exact held-FD transition
            // visible.  A parent sync failure is never a safe retry and is
            // never a verified/failed effect; return the bounded readback as
            // an observation while retaining an InProgress/Uncertain ledger
            // for operation-id-only reconcile.
            if is_post_rename_parent_sync_failure(&error) {
                let observed = quarantine_observation(roots, &snapshot);
                let final_response = record_post_effect_persist_failure(
                    &self.ledger_root,
                    &self.ledger_name,
                    &mut self.ledger,
                    1,
                    error,
                    observed,
                );
                return Ok(SkillOperationApplyReceiptV1 {
                    final_response,
                    ledger: self.ledger,
                });
            }
            if quarantine_snapshot_matches_at(roots, &snapshot)? {
                if let Err(persist_error) =
                    self.verified(1, digest(snapshot.quarantine_destination.as_bytes()))
                {
                    let observed = quarantine_observation(roots, &snapshot);
                    let final_response = record_post_effect_persist_failure(
                        &self.ledger_root,
                        &self.ledger_name,
                        &mut self.ledger,
                        1,
                        persist_error,
                        observed,
                    );
                    return Ok(SkillOperationApplyReceiptV1 {
                        final_response,
                        ledger: self.ledger,
                    });
                }
            } else if removal_source_snapshot_matches_at(roots, &snapshot)? {
                self.ledger.effects[1].state = SkillOperationEffectStateV1::Failed;
                self.ledger.effects[1].intent = SkillOperationEffectIntentV1::None;
                self.ledger.effects[1].error_code = Some(error.code);
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
            } else {
                self.ledger.effects[1].state = SkillOperationEffectStateV1::Uncertain;
                self.ledger.effects[1].error_code = Some(error.code);
                persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)?;
            }
            return Ok(SkillOperationApplyReceiptV1 {
                final_response: project(&self.ledger),
                ledger: self.ledger,
            });
        }
        if let Err(error) = self.verified(1, digest(snapshot.quarantine_destination.as_bytes())) {
            let observed = quarantine_observation(roots, &snapshot);
            let final_response = record_post_effect_persist_failure(
                &self.ledger_root,
                &self.ledger_name,
                &mut self.ledger,
                1,
                error,
                observed,
            );
            return Ok(SkillOperationApplyReceiptV1 {
                final_response,
                ledger: self.ledger,
            });
        }
        Ok(SkillOperationApplyReceiptV1 {
            final_response: project(&self.ledger),
            ledger: self.ledger,
        })
    }
    /// Continues only the quarantine effect after a verified detach prefix.
    /// The original confirmation is already durably consumed; this route is
    /// admitted by the exact ledger shape and never replays OPERON detach.
    pub fn continue_apply(
        mut self,
        data_dir: &Path,
        roots: &SkillRemovalRoots,
        fresh_target: &SkillOperationTargetBindingV1,
    ) -> Result<SkillOperationApplyReceiptV1, SkillOperationError> {
        self.require_continuation(fresh_target)?;
        let snapshot = self.ledger.removal.clone().ok_or_else(|| {
            SkillOperationError::new("SKILL_OPERATION_REMOVAL_SNAPSHOT_MISSING", "ledger")
        })?;
        let _skill_lock = acquire_install_lock_at(&roots.skills_root, &snapshot.skill_name)
            .map_err(SkillOperationError::install)?;
        let held_source_leaf =
            verify_removal_snapshot_at(data_dir, roots, &self.ledger.target, &snapshot)?;
        if !removal_source_leaf_matches(&held_source_leaf, &snapshot)? {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_NEW_PLAN_REQUIRED",
                "preflight",
            ));
        }
        self.begin_effect(1)?;
        match quarantine_owned_skill_at(roots, &snapshot, &held_source_leaf) {
            Ok(()) => {
                if let Err(error) =
                    self.verified(1, digest(snapshot.quarantine_destination.as_bytes()))
                {
                    let observed = quarantine_observation(roots, &snapshot);
                    let final_response = record_post_effect_persist_failure(
                        &self.ledger_root,
                        &self.ledger_name,
                        &mut self.ledger,
                        1,
                        error,
                        observed,
                    );
                    return Ok(SkillOperationApplyReceiptV1 {
                        final_response,
                        ledger: self.ledger,
                    });
                }
            }
            Err(error) if is_post_rename_parent_sync_failure(&error) => {
                let observed = quarantine_observation(roots, &snapshot);
                let final_response = record_post_effect_persist_failure(
                    &self.ledger_root,
                    &self.ledger_name,
                    &mut self.ledger,
                    1,
                    error,
                    observed,
                );
                return Ok(SkillOperationApplyReceiptV1 {
                    final_response,
                    ledger: self.ledger,
                });
            }
            Err(error) => match quarantine_snapshot_matches_at(roots, &snapshot) {
                Ok(true) => {
                    if let Err(persist_error) =
                        self.verified(1, digest(snapshot.quarantine_destination.as_bytes()))
                    {
                        let observed = quarantine_observation(roots, &snapshot);
                        let final_response = record_post_effect_persist_failure(
                            &self.ledger_root,
                            &self.ledger_name,
                            &mut self.ledger,
                            1,
                            persist_error,
                            observed,
                        );
                        return Ok(SkillOperationApplyReceiptV1 {
                            final_response,
                            ledger: self.ledger,
                        });
                    }
                }
                Ok(false) => self.reconcile_uncertain(1, Some(error.code))?,
                Err(readback_error) => self.reconcile_uncertain(1, Some(readback_error.code))?,
            },
        }
        Ok(SkillOperationApplyReceiptV1 {
            final_response: project(&self.ledger),
            ledger: self.ledger,
        })
    }
    fn begin_effect(&mut self, index: usize) -> Result<(), SkillOperationError> {
        if self.ledger.effects.get(index).is_none_or(|effect| {
            effect.state != SkillOperationEffectStateV1::NotStarted
                || effect.intent != SkillOperationEffectIntentV1::None
        }) {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_EFFECT_REENTRY_REQUIRES_RECONCILE",
                "recovery",
            ));
        }
        self.ledger.effects[index].intent = SkillOperationEffectIntentV1::InProgress;
        self.ledger.effects[index].attempt_id = Some(capability_value(&format!(
            "{}-{index}",
            self.ledger.operation_id
        ))?);
        self.ledger.effects[index].attempt_started_at_unix_seconds = Some(unix_seconds());
        persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)
    }
    fn verified(&mut self, index: usize, evidence: String) -> Result<(), SkillOperationError> {
        self.ledger.effects[index].state = SkillOperationEffectStateV1::Verified;
        self.ledger.effects[index].intent = SkillOperationEffectIntentV1::None;
        self.ledger.effects[index].evidence_sha256 = Some(evidence);
        self.ledger.effects[index].error_code = None;
        persist_after_effect(&self.ledger_root, &self.ledger_name, &mut self.ledger)
    }
    fn reconcile_uncertain(
        &mut self,
        index: usize,
        error_code: Option<String>,
    ) -> Result<(), SkillOperationError> {
        self.ledger.effects[index].state = SkillOperationEffectStateV1::Uncertain;
        self.ledger.effects[index].intent = SkillOperationEffectIntentV1::InProgress;
        self.ledger.effects[index].error_code = error_code;
        persist(&self.ledger_root, &self.ledger_name, &mut self.ledger)
    }
    fn require_continuation(
        &self,
        fresh_target: &SkillOperationTargetBindingV1,
    ) -> Result<(), SkillOperationError> {
        validate_ledger_kind(&self.ledger, "continue")?;
        if fresh_target != &self.ledger.target
            || unix_seconds() >= self.ledger.expires_at_unix_seconds
            || !exact_verified_prefix_next_not_started(&self.ledger)
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_NEW_PLAN_REQUIRED",
                "confirmation",
            ));
        }
        Ok(())
    }
}

fn quarantine_snapshot_matches_at(
    roots: &SkillRemovalRoots,
    snapshot: &SkillRemovalSnapshotV1,
) -> Result<bool, SkillOperationError> {
    if !quarantine_name_transitioned_at(roots, snapshot)? {
        return Ok(false);
    }
    let name = std::ffi::CString::new(snapshot.quarantine_destination.as_str()).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_REMOVAL_DESTINATION_INVALID", "recovery")
    })?;
    let fd = unsafe {
        libc::openat(
            roots.quarantine_root.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Ok(false);
    }
    let directory = unsafe { File::from_raw_fd(fd) };
    // Every read below is rooted at the opened descriptor.  In particular,
    // recovery does not rebuild a mutable pathname from /dev/fd and cannot
    // follow a replacement component after the original rename.
    let marker = match read_json_file_at(&directory, crate::IMPORT_ORIGIN_FILE) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    let marker_sha256 = digest(
        &serde_json::to_vec(&marker)
            .map_err(|_| SkillOperationError::new("SKILL_OPERATION_MARKER_INVALID", "recovery"))?,
    );
    if marker_sha256 != snapshot.marker_sha256 {
        return Ok(false);
    }
    let files = match scan_payload_at(&directory, MAX_FILES, MAX_TOTAL_BYTES) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    let content = crate::archive::canonical_content_sha256(&files);
    Ok(content == snapshot.content_sha256
        && marker
            .get("content_sha256")
            .and_then(serde_json::Value::as_str)
            == Some(content.as_str()))
}

fn validate_install_skills_root(
    skills_root: &File,
    target: &SkillOperationTargetBindingV1,
) -> Result<(), SkillOperationError> {
    let (device, inode) =
        directory_object_identity(skills_root, "SKILL_OPERATION_SKILLS_ROOT_IDENTITY_DRIFT")?;
    if device != target.skills_root_device || inode != target.skills_root_inode {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_SKILLS_ROOT_IDENTITY_DRIFT",
            "preflight",
        ));
    }
    Ok(())
}

fn package_snapshot_matches_at(
    skills_root: &File,
    skill_name: &str,
    source: &GithubInspectionSource,
    expected_content_sha256: &str,
) -> SkillOperationObservedStateV1 {
    let name = match std::ffi::CString::new(skill_name) {
        Ok(value) => value,
        Err(_) => return SkillOperationObservedStateV1::Unknown,
    };
    let fd = unsafe {
        libc::openat(
            skills_root.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
            SkillOperationObservedStateV1::ObservedFalse
        } else {
            SkillOperationObservedStateV1::Unknown
        };
    }
    let directory = unsafe { File::from_raw_fd(fd) };
    let marker = match read_json_file_at(&directory, crate::IMPORT_ORIGIN_FILE) {
        Ok(value) => value,
        Err(_) => return SkillOperationObservedStateV1::ObservedFalse,
    };
    let expected_repo = format!("{}/{}", source.owner, source.repo);
    if marker.get("repo").and_then(serde_json::Value::as_str) != Some(expected_repo.as_str())
        || marker.get("sha").and_then(serde_json::Value::as_str) != Some(source.commit_sha.as_str())
        || marker.get("path").and_then(serde_json::Value::as_str) != Some(source.path.as_str())
        || marker
            .get("source_kind")
            .and_then(serde_json::Value::as_str)
            != Some(SourceKind::Github.as_str())
        || marker
            .get("content_sha256")
            .and_then(serde_json::Value::as_str)
            != Some(expected_content_sha256)
    {
        return SkillOperationObservedStateV1::ObservedFalse;
    }
    match scan_payload_at(&directory, MAX_FILES, MAX_TOTAL_BYTES) {
        Ok(files)
            if crate::archive::canonical_content_sha256(&files) == expected_content_sha256 =>
        {
            SkillOperationObservedStateV1::ObservedTrue
        }
        Ok(_) => SkillOperationObservedStateV1::ObservedFalse,
        Err(_) => SkillOperationObservedStateV1::Unknown,
    }
}

fn operon_observation(
    operon: &mut dyn SkillOperationOperonAdapter,
    skill_name: &str,
    org: &str,
    expected_attached: bool,
) -> SkillOperationObservedStateV1 {
    match operon.readback(skill_name, org) {
        Ok(attached) if attached == expected_attached => {
            SkillOperationObservedStateV1::ObservedTrue
        }
        Ok(_) => SkillOperationObservedStateV1::ObservedFalse,
        Err(_) => SkillOperationObservedStateV1::Unknown,
    }
}

fn quarantine_observation(
    roots: &SkillRemovalRoots,
    snapshot: &SkillRemovalSnapshotV1,
) -> SkillOperationObservedStateV1 {
    match quarantine_snapshot_matches_at(roots, snapshot) {
        Ok(true) => SkillOperationObservedStateV1::ObservedTrue,
        Ok(false) => SkillOperationObservedStateV1::ObservedFalse,
        Err(_) => SkillOperationObservedStateV1::Unknown,
    }
}

fn record_post_effect_persist_failure(
    root: &File,
    name: &str,
    ledger: &mut SkillOperationLedgerV1,
    index: usize,
    error: SkillOperationError,
    observed: SkillOperationObservedStateV1,
) -> SkillOperationFinalResponseV1 {
    // The intent write happened before the external call.  Retain that exact
    // durable recovery shape, then make one best-effort durable `Uncertain`
    // successor.  Even if it writes, a successful native effect is not
    // reported as durably verified until operation-id-only reconcile seals it.
    if let Some(effect) = ledger.effects.get_mut(index) {
        effect.state = SkillOperationEffectStateV1::Uncertain;
        effect.intent = SkillOperationEffectIntentV1::InProgress;
        effect.error_code = Some(error.code);
    }
    let _ = persist_uncertain(root, name, ledger);
    non_durable_post_effect_projection(ledger, index, observed)
}

fn read_json_file_at(parent: &File, name: &str) -> Result<serde_json::Value, SkillOperationError> {
    let mut file = open_regular_at(parent, name)?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_REMOVAL_READBACK_FAILED", "recovery")
        })?;
    if bytes.len() > 16 * 1024 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_READBACK_FAILED",
            "recovery",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_MARKER_INVALID", "recovery"))
}

fn open_regular_at(parent: &File, name: &str) -> Result<File, SkillOperationError> {
    let name = std::ffi::CString::new(name).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_REMOVAL_READBACK_FAILED", "recovery")
    })?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_READBACK_FAILED",
            "recovery",
        ));
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(file.as_raw_fd(), stat.as_mut_ptr()) } != 0
        || unsafe { stat.assume_init() }.st_mode & libc::S_IFMT != libc::S_IFREG
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_READBACK_FAILED",
            "recovery",
        ));
    }
    Ok(file)
}

fn scan_payload_at(
    root: &File,
    max_files: usize,
    max_total_bytes: usize,
) -> Result<Vec<crate::PackageFile>, SkillOperationError> {
    scan_payload_at_with_limits(
        root,
        PayloadScanLimits {
            max_files,
            max_total_bytes,
            max_directories: MAX_PAYLOAD_SCAN_DIRECTORIES,
            max_dirents: MAX_PAYLOAD_SCAN_DIRENTS,
        },
    )
}

#[derive(Clone, Copy)]
struct PayloadScanLimits {
    max_files: usize,
    max_total_bytes: usize,
    max_directories: usize,
    max_dirents: usize,
}

#[derive(Default)]
struct PayloadScanBudget {
    directories: usize,
    dirents: usize,
    total_bytes: usize,
}

impl PayloadScanBudget {
    fn consume_directory(&mut self, limits: PayloadScanLimits) -> Result<(), SkillOperationError> {
        self.directories = self
            .directories
            .checked_add(1)
            .ok_or_else(scan_readback_error)?;
        if self.directories > limits.max_directories {
            return Err(scan_readback_error());
        }
        Ok(())
    }

    fn consume_dirent(&mut self, limits: PayloadScanLimits) -> Result<(), SkillOperationError> {
        self.dirents = self
            .dirents
            .checked_add(1)
            .ok_or_else(scan_readback_error)?;
        if self.dirents > limits.max_dirents {
            return Err(scan_readback_error());
        }
        Ok(())
    }

    fn add_file_bytes(
        &mut self,
        bytes: usize,
        limits: PayloadScanLimits,
    ) -> Result<(), SkillOperationError> {
        self.total_bytes = self
            .total_bytes
            .checked_add(bytes)
            .ok_or_else(scan_readback_error)?;
        if self.total_bytes > limits.max_total_bytes {
            return Err(scan_readback_error());
        }
        Ok(())
    }
}

/// `fdopendir` assumes ownership of its descriptor.  Keeping that ownership
/// in one RAII value makes every `?` in the walker (non-UTF8 name, metadata,
/// open, recursive scan, and read failures) close exactly once.
struct OwnedDirStream {
    raw: *mut libc::DIR,
}

impl OwnedDirStream {
    fn from_directory(directory: &File) -> Result<Self, SkillOperationError> {
        let duplicate = unsafe { libc::dup(directory.as_raw_fd()) };
        if duplicate < 0 {
            return Err(scan_readback_error());
        }
        if unsafe { libc::fcntl(duplicate, libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
            unsafe {
                libc::close(duplicate);
            }
            return Err(scan_readback_error());
        }
        // A duplicated directory descriptor shares its cursor with the
        // caller-owned descriptor, so rewind before making the DIR stream.
        // This scanner is the sole directory reader for that descriptor.
        if unsafe { libc::lseek(duplicate, 0, libc::SEEK_SET) } < 0 {
            unsafe {
                libc::close(duplicate);
            }
            return Err(scan_readback_error());
        }
        let raw = unsafe { libc::fdopendir(duplicate) };
        if raw.is_null() {
            unsafe {
                libc::close(duplicate);
            }
            return Err(scan_readback_error());
        }
        Ok(Self { raw })
    }

    fn next_name(&mut self) -> Result<Option<Vec<u8>>, SkillOperationError> {
        clear_readdir_errno();
        let entry = unsafe { libc::readdir(self.raw) };
        if entry.is_null() {
            if std::io::Error::last_os_error().raw_os_error().unwrap_or(0) != 0 {
                return Err(scan_readback_error());
            }
            return Ok(None);
        }
        Ok(Some(
            unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_bytes()
                .to_vec(),
        ))
    }
}

impl Drop for OwnedDirStream {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                libc::closedir(self.raw);
            }
            self.raw = std::ptr::null_mut();
        }
    }
}

#[cfg(target_os = "macos")]
fn clear_readdir_errno() {
    unsafe {
        *libc::__error() = 0;
    }
}

#[cfg(target_os = "linux")]
fn clear_readdir_errno() {
    unsafe {
        *libc::__errno_location() = 0;
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn clear_readdir_errno() {}

fn scan_readback_error() -> SkillOperationError {
    SkillOperationError::new("SKILL_OPERATION_REMOVAL_READBACK_FAILED", "recovery")
}

fn decode_payload_entry_name(name: &[u8]) -> Result<&str, SkillOperationError> {
    std::str::from_utf8(name).map_err(|_| scan_readback_error())
}

fn scan_payload_at_with_limits(
    root: &File,
    limits: PayloadScanLimits,
) -> Result<Vec<crate::PackageFile>, SkillOperationError> {
    let mut files = Vec::new();
    let mut budget = PayloadScanBudget::default();
    scan_payload_directory_at(root, PathBuf::new(), limits, &mut budget, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn removal_source_snapshot_matches_at(
    roots: &SkillRemovalRoots,
    snapshot: &SkillRemovalSnapshotV1,
) -> Result<bool, SkillOperationError> {
    let directory = match open_removal_source_leaf(roots, &snapshot.skill_name) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    removal_source_leaf_matches(&directory, snapshot)
}

fn removal_source_leaf_matches(
    directory: &File,
    snapshot: &SkillRemovalSnapshotV1,
) -> Result<bool, SkillOperationError> {
    let (device, inode) =
        directory_object_identity(directory, "SKILL_OPERATION_REMOVAL_SNAPSHOT_DRIFT")?;
    if device != snapshot.skill_leaf_device || inode != snapshot.skill_leaf_inode {
        return Ok(false);
    }
    let marker = match read_json_file_at(directory, crate::IMPORT_ORIGIN_FILE) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    let marker_sha256 = digest(
        &serde_json::to_vec(&marker)
            .map_err(|_| SkillOperationError::new("SKILL_OPERATION_MARKER_INVALID", "recovery"))?,
    );
    if marker_sha256 != snapshot.marker_sha256 {
        return Ok(false);
    }
    let files = match scan_payload_at(directory, MAX_FILES, MAX_TOTAL_BYTES) {
        Ok(value) => value,
        Err(_) => return Ok(false),
    };
    Ok(crate::archive::canonical_content_sha256(&files) == snapshot.content_sha256)
}

fn open_removal_source_leaf(
    roots: &SkillRemovalRoots,
    skill_name: &str,
) -> Result<File, SkillOperationError> {
    let name = std::ffi::CString::new(skill_name).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_REMOVAL_DESTINATION_INVALID", "recovery")
    })?;
    let fd = unsafe {
        libc::openat(
            roots.skills_root.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_TARGET_MISSING",
            "recovery",
        ));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn scan_payload_directory_at(
    parent: &File,
    relative: PathBuf,
    limits: PayloadScanLimits,
    budget: &mut PayloadScanBudget,
    files: &mut Vec<crate::PackageFile>,
) -> Result<(), SkillOperationError> {
    budget.consume_directory(limits)?;
    let mut directory = OwnedDirStream::from_directory(parent)?;
    loop {
        let Some(name_bytes) = directory.next_name()? else {
            break;
        };
        if matches!(name_bytes.as_slice(), b"." | b"..") {
            continue;
        }
        // This counts every actual directory entry before classification;
        // .import-origin remains omitted from package content but cannot make
        // a marker-heavy tree evade the shared all-tree budget.
        budget.consume_dirent(limits)?;
        let name = decode_payload_entry_name(&name_bytes)?;
        let child_relative = relative.join(name);
        if child_relative == Path::new(crate::IMPORT_ORIGIN_FILE) {
            continue;
        }
        if child_relative.as_os_str().as_encoded_bytes().len() > crate::MAX_PATH_BYTES
            || child_relative.components().count() > crate::MAX_PATH_DEPTH
        {
            return Err(scan_readback_error());
        }
        let c_name = std::ffi::CString::new(&name_bytes[..]).map_err(|_| scan_readback_error())?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
        if unsafe {
            libc::fstatat(
                parent.as_raw_fd(),
                c_name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(scan_readback_error());
        }
        let stat = unsafe { stat.assume_init() };
        match stat.st_mode & libc::S_IFMT {
            libc::S_IFDIR => {
                let fd = unsafe {
                    libc::openat(
                        parent.as_raw_fd(),
                        c_name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(scan_readback_error());
                }
                let child = unsafe { File::from_raw_fd(fd) };
                scan_payload_directory_at(&child, child_relative, limits, budget, files)?;
            }
            libc::S_IFREG => {
                if stat.st_size < 0
                    || stat.st_size as usize > crate::MAX_FILE_BYTES
                    || files.len() >= limits.max_files
                {
                    return Err(scan_readback_error());
                }
                budget.add_file_bytes(stat.st_size as usize, limits)?;
                let mut file = open_regular_at(parent, name)?;
                let mut content = Vec::with_capacity(stat.st_size as usize);
                Read::by_ref(&mut file)
                    .take((crate::MAX_FILE_BYTES + 1) as u64)
                    .read_to_end(&mut content)
                    .map_err(|_| scan_readback_error())?;
                if content.len() != stat.st_size as usize {
                    return Err(scan_readback_error());
                }
                files.push(crate::PackageFile {
                    path: child_relative,
                    content,
                    executable: stat.st_mode & 0o111 != 0,
                });
            }
            _ => {
                return Err(scan_readback_error());
            }
        }
    }
    Ok(())
}

pub(crate) fn project(ledger: &SkillOperationLedgerV1) -> SkillOperationFinalResponseV1 {
    if ledger.operation_kind == SkillOperationKindV1::Removal {
        let detach = ledger.effects.first().map(|value| &value.state);
        let quarantine = ledger.effects.get(1).map(|value| &value.state);
        let recovery = ledger.effects.iter().any(|effect| {
            matches!(effect.state, SkillOperationEffectStateV1::Uncertain)
                || effect.intent == SkillOperationEffectIntentV1::InProgress
        });
        let detach_verified = observed_verified(detach);
        let quarantine_commit = observed_committed(quarantine);
        let status = if quarantine_commit == Some(true) && detach_verified == Some(true) {
            "REMOVED_DETACHED"
        } else if recovery {
            "REMOVAL_STATE_UNCERTAIN"
        } else if detach_verified == Some(true) {
            "DETACHED_QUARANTINE_REQUIRED"
        } else {
            "SKILL_OPERATION_REMOVAL_FAILED"
        };
        return SkillOperationFinalResponseV1 {
            operation_id: ledger.operation_id.clone(),
            status: status.into(),
            directory_commit: None,
            attach_verified: None,
            detach_verified,
            quarantine_commit,
            recovery_required: recovery,
            post_effect_observation: None,
        };
    }
    let package = ledger.effects.first().map(|value| &value.state);
    let attach = ledger.effects.get(1).map(|value| &value.state);
    let recovery = ledger.effects.iter().any(|effect| {
        matches!(effect.state, SkillOperationEffectStateV1::Uncertain)
            || effect.intent == SkillOperationEffectIntentV1::InProgress
    });
    let attach_verified = observed_verified(attach);
    let directory_commit = observed_committed(package);
    let status = if attach_verified == Some(true) {
        "INSTALLED_ATTACHED_VERIFY_REQUIRED"
    } else if recovery {
        "ATTACH_STATE_UNCERTAIN"
    } else if directory_commit == Some(true) {
        "FILES_COMMITTED_ATTACH_REQUIRED"
    } else {
        "SKILL_OPERATION_APPLY_FAILED"
    };
    SkillOperationFinalResponseV1 {
        operation_id: ledger.operation_id.clone(),
        status: status.into(),
        directory_commit,
        attach_verified,
        detach_verified: None,
        quarantine_commit: None,
        recovery_required: recovery,
        post_effect_observation: None,
    }
}

fn observed_verified(state: Option<&SkillOperationEffectStateV1>) -> Option<bool> {
    match state {
        Some(SkillOperationEffectStateV1::Verified) => Some(true),
        Some(SkillOperationEffectStateV1::Uncertain) => None,
        Some(_) | None => Some(false),
    }
}

fn observed_committed(state: Option<&SkillOperationEffectStateV1>) -> Option<bool> {
    match state {
        Some(SkillOperationEffectStateV1::Committed | SkillOperationEffectStateV1::Verified) => {
            Some(true)
        }
        Some(SkillOperationEffectStateV1::Uncertain) => None,
        Some(_) | None => Some(false),
    }
}

fn non_durable_post_effect_projection(
    ledger: &SkillOperationLedgerV1,
    index: usize,
    observed: SkillOperationObservedStateV1,
) -> SkillOperationFinalResponseV1 {
    let mut response = project(ledger);
    response.status = "SKILL_OPERATION_DURABILITY_UNCERTAIN".into();
    response.recovery_required = true;
    response.post_effect_observation =
        ledger
            .effects
            .get(index)
            .map(|effect| SkillOperationPostEffectObservationV1 {
                effect: effect.kind.clone(),
                observed: observed.clone(),
            });
    let value = match observed {
        SkillOperationObservedStateV1::ObservedTrue => Some(true),
        SkillOperationObservedStateV1::ObservedFalse => Some(false),
        SkillOperationObservedStateV1::Unknown => None,
    };
    match ledger.operation_kind {
        SkillOperationKindV1::Install => match index {
            0 => response.directory_commit = value,
            1 => response.attach_verified = value,
            _ => {}
        },
        SkillOperationKindV1::Removal => match index {
            0 => response.detach_verified = value,
            1 => response.quarantine_commit = value,
            _ => {}
        },
    }
    response
}

/// Repeat every ownership predicate while the per-skill install lock is held.
/// The mutation root is the caller-opened descriptor, not a pathname reopened
/// from data-dir.  A swapped `orgs/.../skills` name therefore cannot select a
/// new lock or a new quarantine subject after the exact plan was sealed.
fn verify_removal_snapshot_at(
    data_dir: &Path,
    roots: &SkillRemovalRoots,
    target: &SkillOperationTargetBindingV1,
    snapshot: &SkillRemovalSnapshotV1,
) -> Result<File, SkillOperationError> {
    if active_org(data_dir).map_err(SkillOperationError::install)? != target.active_org {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_TARGET_ORG_DRIFT",
            "preflight",
        ));
    }
    let identities = roots.identity()?;
    if identities.skills_root_device != snapshot.skills_root_device
        || identities.skills_root_inode != snapshot.skills_root_inode
        || identities.quarantine_root_device != snapshot.quarantine_root_device
        || identities.quarantine_root_inode != snapshot.quarantine_root_inode
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_ROOT_IDENTITY_DRIFT",
            "preflight",
        ));
    }
    if identities.skills_root_device != target.skills_root_device
        || identities.skills_root_inode != target.skills_root_inode
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_ROOT_IDENTITY_DRIFT",
            "preflight",
        ));
    }
    if find_bundle_for_skill(data_dir, &snapshot.skill_name)
        .map_err(SkillOperationError::install)?
        .is_some()
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_BUNDLE_UNSUPPORTED",
            "preflight",
        ));
    }
    // Keep this no-follow leaf descriptor open through detach, WAL intent and
    // rename.  A later same-name lookup is checked against this descriptor;
    // it can never turn a replacement into the quarantine subject.
    let leaf = open_removal_source_leaf(roots, &snapshot.skill_name)?;
    if !removal_source_leaf_matches(&leaf, snapshot)? {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_REMOVAL_SNAPSHOT_DRIFT",
            "preflight",
        ));
    }
    Ok(leaf)
}

trait SourceContent {
    fn inspection_content_sha256(&self) -> String;
    fn materialized_content_sha256(&self) -> Result<String, SkillOperationError>;
}
impl SourceContent for SkillPlanV1 {
    fn inspection_content_sha256(&self) -> String {
        match &self.source {
            crate::PlanSourceV1::GithubExact { content_sha256, .. }
            | crate::PlanSourceV1::LocalOpenedArchive { content_sha256, .. }
            | crate::PlanSourceV1::InstalledOwnedSkill { content_sha256, .. } => {
                content_sha256.clone()
            }
        }
    }
    fn materialized_content_sha256(&self) -> Result<String, SkillOperationError> {
        match &self.source {
            crate::PlanSourceV1::GithubExact {
                materialized_content_sha256,
                ..
            } if valid_sha256(materialized_content_sha256) => {
                Ok(materialized_content_sha256.clone())
            }
            crate::PlanSourceV1::InstalledOwnedSkill { content_sha256, .. }
                if valid_sha256(content_sha256) =>
            {
                Ok(content_sha256.clone())
            }
            _ => Err(SkillOperationError::new(
                "SKILL_OPERATION_MATERIALIZED_CONTENT_INVALID",
                "ledger",
            )),
        }
    }
}

fn exact_materialized_content_sha256(
    staged: &ExactStagedGithubArchive,
    source: &GithubInspectionSource,
) -> Result<String, SkillOperationError> {
    let bytes = staged
        .archive_bytes()
        .map_err(|error| SkillOperationError::new(&error.code, &error.phase))?;
    match package_or_bundle_from_github_archive(&bytes, &source.path, &source.repo)
        .map_err(SkillOperationError::install)?
    {
        ValidatedArchive::Skill(package) => Ok(package.content_sha256),
        _ => Err(SkillOperationError::new(
            "SKILL_OPERATION_BUNDLE_OR_PLUGIN_REJECTED",
            "recovery",
        )),
    }
}

fn same_staged_plan_source(staged: &crate::PlanSourceV1, plan: &crate::PlanSourceV1) -> bool {
    matches!(
        (staged, plan),
        (
            crate::PlanSourceV1::GithubExact {
                owner: staged_owner,
                repo: staged_repo,
                resolved_commit_sha: staged_commit,
                path: staged_path,
                archive_sha256: staged_archive,
                content_sha256: staged_inspection,
                binding: staged_binding,
                ..
            },
            crate::PlanSourceV1::GithubExact {
                owner: plan_owner,
                repo: plan_repo,
                resolved_commit_sha: plan_commit,
                path: plan_path,
                archive_sha256: plan_archive,
                content_sha256: plan_inspection,
                binding: plan_binding,
                ..
            }
        ) if staged_owner == plan_owner
            && staged_repo == plan_repo
            && staged_commit == plan_commit
            && staged_path == plan_path
            && staged_archive == plan_archive
            && staged_inspection == plan_inspection
            && staged_binding == plan_binding
    )
}
fn valid_operation_id(value: &str) -> Result<(), SkillOperationError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        Err(SkillOperationError::new(
            "SKILL_OPERATION_OPERATION_ID_INVALID",
            "operation",
        ))
    } else {
        Ok(())
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn unix_seconds() -> u64 {
    #[cfg(test)]
    if let Some(value) = TEST_UNIX_SECONDS_OVERRIDE.with(|value| value.get()) {
        return value;
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn bind_target(
    plan: &SkillPlanV1,
    actual: &SkillOperationTargetBindingV1,
) -> Result<(), SkillOperationError> {
    match &plan.target {
        PlanTargetV1::Bound {
            science_runtime_identity_sha256,
            data_dir_identity_sha256,
            active_org_identity_sha256,
            skills_root_device,
            skills_root_inode,
            operon,
        } if operon == "OPERON"
            && science_runtime_identity_sha256 == &actual.science_runtime_identity_sha256
            && data_dir_identity_sha256 == &actual.data_dir_identity_sha256
            && active_org_identity_sha256 == &actual.active_org_identity_sha256
            && *skills_root_device == actual.skills_root_device
            && *skills_root_inode == actual.skills_root_inode =>
        {
            Ok(())
        }
        _ => Err(SkillOperationError::new(
            "SKILL_OPERATION_TARGET_BINDING_MISMATCH",
            "plan",
        )),
    }
}

/// Reconciliation has no authority to read a durable intent against a target
/// which is no longer the exact runtime/data-dir/org binding that created it.
/// This check is deliberately before every native or filesystem readback and
/// never persists a drift observation into the old target's ledger.
fn reconcile_target(
    ledger: &SkillOperationLedgerV1,
    fresh_target: &SkillOperationTargetBindingV1,
) -> Result<(), SkillOperationError> {
    validate_ledger_kind(ledger, "recovery")?;
    if fresh_target != &ledger.target {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_TARGET_BINDING_DRIFT",
            "recovery",
        ));
    }
    bind_target(&ledger.plan, fresh_target)
}

fn exact_verified_prefix_next_not_started(ledger: &SkillOperationLedgerV1) -> bool {
    ledger.consumed
        && ledger.effects.len() == 2
        && ledger.effects[0].state == SkillOperationEffectStateV1::Verified
        && ledger.effects[0].intent == SkillOperationEffectIntentV1::None
        && ledger.effects[1].state == SkillOperationEffectStateV1::NotStarted
        && ledger.effects[1].intent == SkillOperationEffectIntentV1::None
}

/// A pending WAL record is recoverable only if it is the one state-machine
/// edge `persist` could have written.  In particular, a hostile pending file
/// cannot turn an untouched later effect into Verified or skip an effect.
fn legal_single_step_transition(
    current: &SkillOperationLedgerV1,
    next: &SkillOperationLedgerV1,
) -> bool {
    if current.schema != next.schema
        || current.operation_id != next.operation_id
        || current.operation_kind != next.operation_kind
        || current.plan_digest_sha256 != next.plan_digest_sha256
        || current.confirmation_capability_sha256 != next.confirmation_capability_sha256
        || current.source_request_sha256 != next.source_request_sha256
        || current.source_archive_sha256 != next.source_archive_sha256
        || current.staging_object_identity_sha256 != next.staging_object_identity_sha256
        || current.staging_receipt_sha256 != next.staging_receipt_sha256
        || current.inspection_content_sha256 != next.inspection_content_sha256
        || current.source_content_sha256 != next.source_content_sha256
        || current.target != next.target
        || current.expires_at_unix_seconds != next.expires_at_unix_seconds
        || current.plan != next.plan
        || current.removal != next.removal
        || current.effects.len() != next.effects.len()
    {
        return false;
    }
    let changed: Vec<usize> = current
        .effects
        .iter()
        .zip(&next.effects)
        .enumerate()
        .filter_map(|(index, (before, after))| (before != after).then_some(index))
        .collect();
    if changed.len() != 1 {
        return false;
    }
    let index = changed[0];
    let before = &current.effects[index];
    let after = &next.effects[index];
    let verified_prefix = current.effects[..index].iter().all(|effect| {
        effect.state == SkillOperationEffectStateV1::Verified
            && effect.intent == SkillOperationEffectIntentV1::None
    });
    let tuple = (&before.state, &before.intent, &after.state, &after.intent);
    let effect_step = (verified_prefix
        && matches!(
            tuple,
            (
                SkillOperationEffectStateV1::NotStarted,
                SkillOperationEffectIntentV1::None,
                SkillOperationEffectStateV1::NotStarted,
                SkillOperationEffectIntentV1::InProgress,
            )
        ))
        || matches!(
            tuple,
            (
                SkillOperationEffectStateV1::NotStarted,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Committed,
                SkillOperationEffectIntentV1::InProgress,
            ) | (
                SkillOperationEffectStateV1::NotStarted,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Uncertain,
                SkillOperationEffectIntentV1::InProgress,
            ) | (
                SkillOperationEffectStateV1::NotStarted,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Failed,
                SkillOperationEffectIntentV1::None,
            ) | (
                SkillOperationEffectStateV1::NotStarted,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Verified,
                SkillOperationEffectIntentV1::None,
            ) | (
                SkillOperationEffectStateV1::Committed,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Verified,
                SkillOperationEffectIntentV1::None,
            ) | (
                SkillOperationEffectStateV1::Uncertain,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Committed,
                SkillOperationEffectIntentV1::InProgress,
            ) | (
                SkillOperationEffectStateV1::Uncertain,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Failed,
                SkillOperationEffectIntentV1::None,
            ) | (
                SkillOperationEffectStateV1::Uncertain,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Verified,
                SkillOperationEffectIntentV1::None,
            ) | (
                SkillOperationEffectStateV1::Uncertain,
                SkillOperationEffectIntentV1::InProgress,
                SkillOperationEffectStateV1::Uncertain,
                SkillOperationEffectIntentV1::InProgress,
            )
        );
    if !effect_step {
        return false;
    }
    // Consumption changes once, together with the first effect's durable
    // intent; later begin transitions preserve it.
    let consumed_legal = if current.consumed == next.consumed {
        true
    } else {
        !current.consumed
            && next.consumed
            && index == 0
            && before.state == SkillOperationEffectStateV1::NotStarted
            && before.intent == SkillOperationEffectIntentV1::None
            && after.state == SkillOperationEffectStateV1::NotStarted
            && after.intent == SkillOperationEffectIntentV1::InProgress
    };
    consumed_legal
        && (after.state != SkillOperationEffectStateV1::Verified
            || (after.intent == SkillOperationEffectIntentV1::None
                && after.evidence_sha256.as_deref().is_some_and(valid_sha256)))
}

fn valid_effect_receipt(receipt: &SkillOperationEffectReceiptV1) -> bool {
    let valid_attempt = receipt.attempt_id.as_deref().is_some_and(valid_sha256)
        && receipt
            .attempt_started_at_unix_seconds
            .is_some_and(|value| value > 0);
    let no_attempt =
        receipt.attempt_id.is_none() && receipt.attempt_started_at_unix_seconds.is_none();
    let no_evidence = receipt.evidence_sha256.is_none();
    let no_error = receipt.error_code.is_none();
    match (&receipt.state, &receipt.intent) {
        (SkillOperationEffectStateV1::NotStarted, SkillOperationEffectIntentV1::None) => {
            no_attempt && no_evidence && no_error
        }
        (SkillOperationEffectStateV1::NotStarted, SkillOperationEffectIntentV1::InProgress) => {
            valid_attempt && no_evidence && no_error
        }
        (SkillOperationEffectStateV1::Committed, SkillOperationEffectIntentV1::InProgress) => {
            valid_attempt && no_evidence && no_error
        }
        (SkillOperationEffectStateV1::Verified, SkillOperationEffectIntentV1::None) => {
            valid_attempt
                && receipt.evidence_sha256.as_deref().is_some_and(valid_sha256)
                && no_error
        }
        (SkillOperationEffectStateV1::Uncertain, SkillOperationEffectIntentV1::InProgress) => {
            valid_attempt && no_evidence
        }
        (SkillOperationEffectStateV1::Failed, SkillOperationEffectIntentV1::None) => {
            valid_attempt
                && no_evidence
                && receipt
                    .error_code
                    .as_deref()
                    .is_some_and(|value| !value.is_empty())
        }
        _ => false,
    }
}

fn valid_single_skill_effects(plan: &SkillPlanV1) -> bool {
    if plan.effects.len() != 2 {
        return false;
    }
    matches!(
        (&plan.effects[0].kind, &plan.effects[1].kind),
        (
            PlanEffectKindV1::PackageCommit,
            PlanEffectKindV1::OperonAttach
        )
    ) && plan.effects[0].order == 1
        && plan.effects[1].order == 2
}

fn valid_removal_effects(plan: &SkillPlanV1) -> bool {
    if plan.effects.len() != 2 {
        return false;
    }
    matches!(
        (&plan.effects[0].kind, &plan.effects[1].kind),
        (
            PlanEffectKindV1::OperonDetach,
            PlanEffectKindV1::PackageQuarantine
        )
    ) && plan.effects[0].order == 1
        && plan.effects[1].order == 2
}

/// Validate the discriminated ledger shape at every durable re-entry.  This
/// prevents an installed-directory removal from being interpreted through the
/// archive/staging install path (or vice versa), even if a hostile or stale
/// ledger has individually well-formed hashes.
fn validate_ledger_kind(
    ledger: &SkillOperationLedgerV1,
    phase: &str,
) -> Result<(), SkillOperationError> {
    let receipt_tuple_matches = ledger.effects.len() == ledger.plan.effects.len()
        && ledger
            .effects
            .iter()
            .zip(&ledger.plan.effects)
            .all(|(receipt, effect)| receipt.order == effect.order && receipt.kind == effect.kind)
        && ledger.effects.iter().all(valid_effect_receipt);
    let invocation_binds_ledger = matches!(
        &ledger.plan.identity,
        crate::PlanIdentityV1::Invocation { plan_id } if plan_id == &ledger.operation_id
    );
    let expiry_binds_ledger = matches!(
        ledger.plan.expiry,
        PlanExpiryV1::ExpiresAtUnixSeconds { unix_seconds } if unix_seconds == ledger.expires_at_unix_seconds
    );
    let valid = match ledger.operation_kind {
        SkillOperationKindV1::Install => {
            ledger.removal.is_none()
                && valid_sha256(ledger.source_request_sha256.as_deref().unwrap_or(""))
                && valid_sha256(ledger.source_archive_sha256.as_deref().unwrap_or(""))
                && valid_sha256(
                    ledger
                        .staging_object_identity_sha256
                        .as_deref()
                        .unwrap_or(""),
                )
                && valid_sha256(ledger.staging_receipt_sha256.as_deref().unwrap_or(""))
                && valid_sha256(ledger.inspection_content_sha256.as_deref().unwrap_or(""))
                && valid_single_skill_effects(&ledger.plan)
                && matches!(
                    &ledger.plan.source,
                    crate::PlanSourceV1::GithubExact {
                        archive_sha256,
                        content_sha256,
                        materialized_content_sha256,
                        binding: crate::SourceBindingV1::CsswitchExactContentBound,
                        ..
                    } if ledger.source_archive_sha256.as_deref() == Some(archive_sha256)
                        && ledger.inspection_content_sha256.as_deref() == Some(content_sha256)
                        && ledger.source_content_sha256 == *materialized_content_sha256
                )
                && receipt_tuple_matches
        }
        SkillOperationKindV1::Removal => {
            ledger.removal.is_some()
                && ledger.source_request_sha256.is_none()
                && ledger.source_archive_sha256.is_none()
                && ledger.staging_object_identity_sha256.is_none()
                && ledger.staging_receipt_sha256.is_none()
                && ledger.inspection_content_sha256.is_none()
                && valid_removal_effects(&ledger.plan)
                && receipt_tuple_matches
                && removal_snapshot_binds_plan(ledger)
        }
    };
    if !receipt_tuple_matches {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_RECEIPT_INVALID",
            phase,
        ));
    }
    if valid
        && invocation_binds_ledger
        && expiry_binds_ledger
        && validate_skill_plan(&ledger.plan).is_ok()
    {
        Ok(())
    } else {
        Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_SUBJECT_INVALID",
            phase,
        ))
    }
}

fn removal_snapshot_binds_plan(ledger: &SkillOperationLedgerV1) -> bool {
    let Some(snapshot) = ledger.removal.as_ref() else {
        return false;
    };
    let crate::PlanSourceV1::InstalledOwnedSkill {
        skill_name,
        marker_sha256,
        content_sha256,
        target_package_identity_sha256,
        ..
    } = &ledger.plan.source
    else {
        return false;
    };
    if skill_name != &snapshot.skill_name
        || marker_sha256 != &snapshot.marker_sha256
        || content_sha256 != &snapshot.content_sha256
        || ledger.source_content_sha256 != snapshot.content_sha256
        || snapshot.skill_leaf_device == 0
        || snapshot.skill_leaf_inode == 0
    {
        return false;
    }
    let Ok(expected_destination) = skill_removal_destination(
        &ledger.operation_id,
        &snapshot.skill_name,
        &snapshot.marker_sha256,
        &snapshot.content_sha256,
    ) else {
        return false;
    };
    if snapshot.quarantine_destination != expected_destination {
        return false;
    }
    matches!(ledger.plan.effects.as_slice(), [detach, quarantine]
        if matches!(&detach.subject, crate::PlanEffectSubjectV1::OperonSkill { skill_name: detach_skill, .. } if detach_skill == &snapshot.skill_name)
        && matches!(&quarantine.subject, crate::PlanEffectSubjectV1::Package { target_package_identity_sha256: effect_identity, .. } if effect_identity == target_package_identity_sha256))
}

fn selected_skill_name(plan: &SkillPlanV1) -> Result<String, SkillOperationError> {
    match &plan.effects[1].subject {
        crate::PlanEffectSubjectV1::OperonSkill { skill_name, .. } => Ok(skill_name.clone()),
        _ => Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_EFFECTS_INVALID",
            "recovery",
        )),
    }
}

fn source_from_plan(plan: &SkillPlanV1) -> Result<GithubInspectionSource, SkillOperationError> {
    match &plan.source {
        crate::PlanSourceV1::GithubExact {
            owner,
            repo,
            resolved_commit_sha,
            path,
            ..
        } => Ok(GithubInspectionSource {
            owner: owner.clone(),
            repo: repo.clone(),
            commit_sha: resolved_commit_sha.clone(),
            path: path.clone(),
        }),
        _ => Err(SkillOperationError::new(
            "SKILL_OPERATION_SOURCE_KIND_UNSUPPORTED",
            "recovery",
        )),
    }
}
fn ensure_private_root(root: &File) -> Result<(), SkillOperationError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(root.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_ROOT_INVALID",
            "ledger",
        ));
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o7777 != 0o700
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_ROOT_INVALID",
            "ledger",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct LifecycleUsage {
    active_entries: usize,
    reserved_bytes: u64,
}

fn reservation_name(operation_id: &str) -> String {
    format!(".{operation_id}{RESERVATION_SUFFIX}")
}

fn valid_reservation(reservation: &SkillOperationReservationV1, operation_id: &str) -> bool {
    if reservation.schema != SKILL_OPERATION_LEDGER_SCHEMA
        || reservation.operation_id != operation_id
        || valid_operation_id(operation_id).is_err()
        || reservation.reserved_staged_bytes != crate::MAX_ARCHIVE_BYTES as u64
        || exact_staging_directory_name(operation_id).ok().as_deref()
            != Some(reservation.staging_directory_name.as_str())
    {
        return false;
    }
    match (&reservation.phase, &reservation.staged) {
        (SkillOperationReservationPhaseV1::Reserved, None) => true,
        (SkillOperationReservationPhaseV1::Staged, Some(staged)) => {
            valid_sha256(&staged.archive_sha256)
                && valid_sha256(&staged.staging_object_identity_sha256)
                && valid_sha256(&staged.staging_receipt_sha256)
                && !staged.source.owner.is_empty()
                && !staged.source.repo.is_empty()
                && !staged.source.commit_sha.is_empty()
        }
        _ => false,
    }
}

fn tombstone_name(operation_id: &str) -> String {
    format!(".{operation_id}{GC_TOMBSTONE_SUFFIX}")
}

fn acquire_lifecycle_lock(root: &File) -> Result<File, SkillOperationError> {
    let file = create_or_open_at(
        root.as_raw_fd(),
        LIFECYCLE_LOCK_NAME,
        0o600,
        "SKILL_OPERATION_LIFECYCLE_LOCK_FAILED",
    )?;
    validate_open_owned_file(&file, "SKILL_OPERATION_LIFECYCLE_LOCK_FAILED")?;
    validate_name_matches_open_file(
        root.as_raw_fd(),
        LIFECYCLE_LOCK_NAME,
        &file,
        "SKILL_OPERATION_LIFECYCLE_LOCK_FAILED",
    )?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LIFECYCLE_BUSY",
            "admission",
        ));
    }
    validate_open_owned_file(&file, "SKILL_OPERATION_LIFECYCLE_LOCK_FAILED")?;
    validate_name_matches_open_file(
        root.as_raw_fd(),
        LIFECYCLE_LOCK_NAME,
        &file,
        "SKILL_OPERATION_LIFECYCLE_LOCK_FAILED",
    )?;
    Ok(file)
}

fn lifecycle_entry_names(root: &File) -> Result<Vec<String>, SkillOperationError> {
    let mut stream = OwnedDirStream::from_directory(root)?;
    let mut names = Vec::new();
    while let Some(raw) = stream.next_name()? {
        if raw == b"." || raw == b".." {
            continue;
        }
        if names.len() >= MAX_OPERATION_LIFECYCLE_ENTRIES {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LIFECYCLE_ENUMERATION_LIMIT",
                "admission",
            ));
        }
        let name = std::str::from_utf8(&raw).map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_LIFECYCLE_ENTRY_INVALID", "admission")
        })?;
        names.push(name.to_string());
    }
    Ok(names)
}

fn read_json_owned<T: serde::de::DeserializeOwned>(
    root: &File,
    name: &str,
    code: &str,
) -> Result<T, SkillOperationError> {
    serde_json::from_slice(&read_owned_ledger_bytes(root, name)?)
        .map_err(|_| SkillOperationError::new(code, "admission"))
}

fn write_json_owned<T: Serialize>(file: File, value: &T) -> Result<(), SkillOperationError> {
    let bytes = serde_json::to_vec(value).map_err(|_| {
        SkillOperationError::new(
            "SKILL_OPERATION_LIFECYCLE_SERIALIZATION_FAILED",
            "admission",
        )
    })?;
    let mut file = file;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_LIFECYCLE_WRITE_FAILED", "admission")
        })
}

fn remove_file_at(parent: i32, name: &str) -> Result<(), SkillOperationError> {
    let name = std::ffi::CString::new(name)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "ledger"))?;
    if unsafe { libc::unlinkat(parent, name.as_ptr(), 0) } != 0 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LIFECYCLE_DELETE_FAILED",
            "admission",
        ));
    }
    Ok(())
}

fn name_exists(parent: i32, name: &str) -> Result<bool, SkillOperationError> {
    let name = std::ffi::CString::new(name)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "ledger"))?;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        return Ok(true);
    }
    if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
        Ok(false)
    } else {
        Err(SkillOperationError::new(
            "SKILL_OPERATION_LIFECYCLE_ENTRY_CHECK_FAILED",
            "admission",
        ))
    }
}

/// Only a fully terminal effect set may ever be retired.  In particular, an
/// already-verified prefix followed by NotStarted is still continuation
/// authority, and InProgress/Uncertain are recovery authority.  Neither is
/// garbage merely because its confirmation TTL elapsed.
fn gc_eligible_terminal(ledger: &SkillOperationLedgerV1) -> bool {
    validate_ledger_kind(ledger, "admission").is_ok()
        && unix_seconds() >= ledger.expires_at_unix_seconds
        && ledger.effects.iter().all(|effect| {
            matches!(
                effect.state,
                SkillOperationEffectStateV1::Verified | SkillOperationEffectStateV1::Failed
            ) && effect.intent == SkillOperationEffectIntentV1::None
        })
}

fn staged_reservation_matches_ledger(
    reservation: &SkillOperationReservationV1,
    ledger: &SkillOperationLedgerV1,
) -> bool {
    let Some(staged) = reservation.staged.as_ref() else {
        return false;
    };
    let Ok(source) = source_from_plan(&ledger.plan) else {
        return false;
    };
    reservation.phase == SkillOperationReservationPhaseV1::Staged
        && reservation.operation_id == ledger.operation_id
        && staged.source == source
        && ledger.source_archive_sha256.as_deref() == Some(staged.archive_sha256.as_str())
        && ledger.staging_object_identity_sha256.as_deref()
            == Some(staged.staging_object_identity_sha256.as_str())
        && ledger.staging_receipt_sha256.as_deref() == Some(staged.staging_receipt_sha256.as_str())
}

/// A process can stop after publishing the ledger and before dropping its
/// Staged reservation.  It is not a second archive reservation: validate the
/// complete identity tuple, then remove only that redundant lifecycle record.
fn clear_staged_reservation_promoted_to_ledger(
    ledger_root: &File,
    ledger: &SkillOperationLedgerV1,
) -> Result<(), SkillOperationError> {
    let name = reservation_name(&ledger.operation_id);
    if !name_exists(ledger_root.as_raw_fd(), &name)? {
        return Ok(());
    }
    let reservation: SkillOperationReservationV1 =
        read_json_owned(ledger_root, &name, "SKILL_OPERATION_RESERVATION_INVALID")?;
    if !valid_reservation(&reservation, &ledger.operation_id)
        || ledger.operation_kind != SkillOperationKindV1::Install
        || !staged_reservation_matches_ledger(&reservation, ledger)
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_RESERVATION_LEDGER_IDENTITY_DRIFT",
            "admission",
        ));
    }
    remove_file_at(ledger_root.as_raw_fd(), &name)?;
    sync_directory(ledger_root)
}

fn delete_expired_install_after_tombstone(
    staging_root: &File,
    ledger_root: &File,
    ledger_name: &str,
    ledger: &SkillOperationLedgerV1,
    ledger_bytes: &[u8],
) -> Result<(), SkillOperationError> {
    clear_staged_reservation_promoted_to_ledger(ledger_root, ledger)?;
    let tomb_name = tombstone_name(&ledger.operation_id);
    let tomb = SkillOperationGcTombstoneV1 {
        schema: SKILL_OPERATION_LEDGER_SCHEMA.into(),
        operation_id: ledger.operation_id.clone(),
        ledger_sha256: digest(ledger_bytes),
    };
    match read_json_owned::<SkillOperationGcTombstoneV1>(
        ledger_root,
        &tomb_name,
        "SKILL_OPERATION_GC_TOMBSTONE_INVALID",
    ) {
        Ok(existing) if existing == tomb => {}
        Ok(_) => {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_GC_TOMBSTONE_INVALID",
                "admission",
            ))
        }
        Err(error)
            if error.code == "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT"
                || error.code == "SKILL_OPERATION_LEDGER_NOT_FOUND" =>
        {
            let file = create_new_at(
                ledger_root.as_raw_fd(),
                &tomb_name,
                0o600,
                "SKILL_OPERATION_GC_TOMBSTONE_EXISTS",
            )?;
            write_json_owned(file, &tomb)?;
            sync_directory(ledger_root)?;
        }
        Err(error) => return Err(error),
    }
    if ledger.operation_kind == SkillOperationKindV1::Install {
        let source = source_from_plan(&ledger.plan)?;
        let delete = remove_exact_github_archive(
            staging_root,
            &ledger.operation_id,
            &source,
            ledger.source_archive_sha256.as_deref().unwrap_or(""),
            ledger
                .staging_object_identity_sha256
                .as_deref()
                .unwrap_or(""),
            ledger.staging_receipt_sha256.as_deref().unwrap_or(""),
        );
        if let Err(error) = delete {
            // A matching durable tombstone is the sole proof that absence was
            // caused by an earlier GC attempt, not by an arbitrary missing stage.
            if error.code != "STAGING_DIRECTORY_OPEN_FAILED" {
                return Err(SkillOperationError::new(&error.code, &error.phase));
            }
        }
    }
    remove_file_at(ledger_root.as_raw_fd(), ledger_name)?;
    sync_directory(ledger_root)?;
    remove_file_at(ledger_root.as_raw_fd(), &tomb_name)?;
    sync_directory(ledger_root)
}

/// A Staged reservation is the durable owner of an archive before the ledger
/// exists.  The tombstone orders its exact identity before deletion, so a
/// restart can distinguish this operation's earlier cleanup from a missing or
/// substituted staging child.  Reserved records deliberately do *not* take
/// this path: without the staged identity they can only be released when the
/// deterministic child name is absent.
fn delete_expired_staged_reservation_after_tombstone(
    staging_root: &File,
    ledger_root: &File,
    reservation_name_value: &str,
    reservation: &SkillOperationReservationV1,
    reservation_bytes: &[u8],
) -> Result<(), SkillOperationError> {
    let staged = reservation.staged.as_ref().ok_or_else(|| {
        SkillOperationError::new("SKILL_OPERATION_RESERVATION_INVALID", "admission")
    })?;
    let tomb_name = tombstone_name(&reservation.operation_id);
    let tomb = SkillOperationGcTombstoneV1 {
        schema: SKILL_OPERATION_LEDGER_SCHEMA.into(),
        operation_id: reservation.operation_id.clone(),
        // This field is an immutable digest of the retiring durable record.
        // It is named for the original ledger-only form to retain wire
        // compatibility with already-written tombstones.
        ledger_sha256: digest(reservation_bytes),
    };
    match read_json_owned::<SkillOperationGcTombstoneV1>(
        ledger_root,
        &tomb_name,
        "SKILL_OPERATION_GC_TOMBSTONE_INVALID",
    ) {
        Ok(existing) if existing == tomb => {}
        Ok(_) => {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_GC_TOMBSTONE_INVALID",
                "admission",
            ))
        }
        Err(error)
            if error.code == "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT"
                || error.code == "SKILL_OPERATION_LEDGER_NOT_FOUND" =>
        {
            let file = create_new_at(
                ledger_root.as_raw_fd(),
                &tomb_name,
                0o600,
                "SKILL_OPERATION_GC_TOMBSTONE_EXISTS",
            )?;
            write_json_owned(file, &tomb)?;
            sync_directory(ledger_root)?;
        }
        Err(error) => return Err(error),
    }
    let delete = remove_exact_github_archive(
        staging_root,
        &reservation.operation_id,
        &staged.source,
        &staged.archive_sha256,
        &staged.staging_object_identity_sha256,
        &staged.staging_receipt_sha256,
    );
    if let Err(error) = delete {
        if error.code != "STAGING_DIRECTORY_OPEN_FAILED" {
            return Err(SkillOperationError::new(&error.code, &error.phase));
        }
    }
    remove_file_at(ledger_root.as_raw_fd(), reservation_name_value)?;
    sync_directory(ledger_root)?;
    remove_file_at(ledger_root.as_raw_fd(), &tomb_name)?;
    sync_directory(ledger_root)
}

fn reserved_staging_child_exists(
    staging_root: &File,
    reservation: &SkillOperationReservationV1,
) -> Result<bool, SkillOperationError> {
    name_exists(
        staging_root.as_raw_fd(),
        &reservation.staging_directory_name,
    )
}

fn gc_expired_install_lifecycle(
    staging_root: &File,
    ledger_root: &File,
) -> Result<(), SkillOperationError> {
    for name in lifecycle_entry_names(ledger_root)? {
        let Some(operation_id) = name.strip_suffix(LEDGER_SUFFIX) else {
            continue;
        };
        if operation_id.starts_with('.') || !valid_operation_id(operation_id).is_ok() {
            continue;
        }
        let lock = match acquire_operation_lock(ledger_root, operation_id) {
            Ok(lock) => lock,
            Err(error) if error.code == "SKILL_OPERATION_OPERATION_BUSY" => continue,
            Err(error) => return Err(error),
        };
        let bytes = match read_owned_ledger_bytes(ledger_root, &name) {
            Ok(bytes) => bytes,
            Err(_) => {
                drop(lock);
                continue;
            }
        };
        let ledger = match serde_json::from_slice::<SkillOperationLedgerV1>(&bytes) {
            Ok(ledger) => ledger,
            Err(_) => {
                drop(lock);
                continue;
            }
        };
        if gc_eligible_terminal(&ledger) {
            delete_expired_install_after_tombstone(
                staging_root,
                ledger_root,
                &name,
                &ledger,
                &bytes,
            )?;
        }
        drop(lock);
    }
    // If the process stopped after the staged child and ledger were both
    // durably removed, the tombstone is the bounded proof of that completed
    // cleanup transition.  A still-present reservation owns a different
    // pre-ledger cleanup, so leave its tombstone for that exact owner.
    for name in lifecycle_entry_names(ledger_root)? {
        let Some(operation_id) = name
            .strip_prefix('.')
            .and_then(|value| value.strip_suffix(GC_TOMBSTONE_SUFFIX))
        else {
            continue;
        };
        let tombstone: SkillOperationGcTombstoneV1 =
            read_json_owned(ledger_root, &name, "SKILL_OPERATION_GC_TOMBSTONE_INVALID")?;
        if tombstone.schema != SKILL_OPERATION_LEDGER_SCHEMA
            || tombstone.operation_id != operation_id
            || !valid_operation_id(operation_id).is_ok()
            || !valid_sha256(&tombstone.ledger_sha256)
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_GC_TOMBSTONE_INVALID",
                "admission",
            ));
        }
        let ledger_exists = name_exists(
            ledger_root.as_raw_fd(),
            &format!("{operation_id}{LEDGER_SUFFIX}"),
        )?;
        let reservation_exists =
            name_exists(ledger_root.as_raw_fd(), &reservation_name(operation_id))?;
        if !ledger_exists && !reservation_exists {
            remove_file_at(ledger_root.as_raw_fd(), &name)?;
            sync_directory(ledger_root)?;
        }
    }
    // Expired pre-download reservations have no effects or staged bytes and
    // can be removed directly under the same lifecycle lock.
    for name in lifecycle_entry_names(ledger_root)? {
        let Some(operation_id) = name
            .strip_prefix('.')
            .and_then(|value| value.strip_suffix(RESERVATION_SUFFIX))
        else {
            continue;
        };
        let reservation = match read_json_owned::<SkillOperationReservationV1>(
            ledger_root,
            &name,
            "SKILL_OPERATION_RESERVATION_INVALID",
        ) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        if !valid_reservation(&reservation, operation_id) {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_RESERVATION_INVALID",
                "admission",
            ));
        }
        let lock = match acquire_operation_lock(ledger_root, operation_id) {
            Ok(lock) => lock,
            Err(error) if error.code == "SKILL_OPERATION_OPERATION_BUSY" => continue,
            Err(error) => return Err(error),
        };
        let ledger_name = format!("{operation_id}{LEDGER_SUFFIX}");
        if name_exists(ledger_root.as_raw_fd(), &ledger_name)? {
            let ledger: SkillOperationLedgerV1 =
                read_json_owned(ledger_root, &ledger_name, "SKILL_OPERATION_LEDGER_INVALID")?;
            if ledger.operation_id != operation_id
                || !staged_reservation_matches_ledger(&reservation, &ledger)
            {
                return Err(SkillOperationError::new(
                    "SKILL_OPERATION_RESERVATION_LEDGER_IDENTITY_DRIFT",
                    "admission",
                ));
            }
            remove_file_at(ledger_root.as_raw_fd(), &name)?;
            sync_directory(ledger_root)?;
        } else if unix_seconds() >= reservation.expires_at_unix_seconds {
            match reservation.phase {
                SkillOperationReservationPhaseV1::Reserved => {
                    // A Reserved record has no durable staged receipt.  The
                    // sole authorized recovery is the exact deterministic
                    // pre-receipt child: resolver validates the opened
                    // no-follow root/child and its complete two-name
                    // whitelist before deleting anything. Unknown or altered
                    // objects remain active and block capacity reclamation.
                    if reserved_staging_child_exists(staging_root, &reservation)? {
                        remove_partial_exact_staging(staging_root, operation_id)
                            .map_err(|error| SkillOperationError::new(&error.code, &error.phase))?;
                    }
                    remove_file_at(ledger_root.as_raw_fd(), &name)?;
                    sync_directory(ledger_root)?;
                }
                SkillOperationReservationPhaseV1::Staged => {
                    let bytes = read_owned_ledger_bytes(ledger_root, &name)?;
                    delete_expired_staged_reservation_after_tombstone(
                        staging_root,
                        ledger_root,
                        &name,
                        &reservation,
                        &bytes,
                    )?;
                }
            }
        }
        drop(lock);
    }
    Ok(())
}

fn lifecycle_usage(ledger_root: &File) -> Result<LifecycleUsage, SkillOperationError> {
    let mut usage = LifecycleUsage::default();
    for name in lifecycle_entry_names(ledger_root)? {
        if let Some(operation_id) = name
            .strip_prefix('.')
            .and_then(|value| value.strip_suffix(RESERVATION_SUFFIX))
        {
            let reservation: SkillOperationReservationV1 =
                read_json_owned(ledger_root, &name, "SKILL_OPERATION_RESERVATION_INVALID")?;
            if !valid_reservation(&reservation, operation_id) {
                return Err(SkillOperationError::new(
                    "SKILL_OPERATION_RESERVATION_INVALID",
                    "admission",
                ));
            }
            usage.active_entries = usage.active_entries.saturating_add(1);
            usage.reserved_bytes = usage
                .reserved_bytes
                .saturating_add(reservation.reserved_staged_bytes);
        } else if let Some(operation_id) = name.strip_suffix(LEDGER_SUFFIX) {
            if operation_id.starts_with('.') {
                continue;
            }
            let ledger: SkillOperationLedgerV1 =
                read_json_owned(ledger_root, &name, "SKILL_OPERATION_LEDGER_INVALID")?;
            if ledger.operation_id != operation_id {
                return Err(SkillOperationError::new(
                    "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT",
                    "admission",
                ));
            }
            validate_ledger_kind(&ledger, "admission")?;
            usage.active_entries = usage.active_entries.saturating_add(1);
            if ledger.operation_kind == SkillOperationKindV1::Install {
                usage.reserved_bytes = usage
                    .reserved_bytes
                    .saturating_add(crate::MAX_ARCHIVE_BYTES as u64);
            }
        } else if name == LIFECYCLE_LOCK_NAME
            || operation_lock_bucket_from_name(&name).is_some()
            || (name.starts_with('.') && name.ends_with(".pending"))
        {
            // Locks and a pending successor are durable protocol entries, not
            // capacity reservations. Pending validation remains the owning
            // operation's responsibility; unknown names are never ignored.
            continue;
        } else {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LIFECYCLE_ENTRY_INVALID",
                "admission",
            ));
        }
    }
    Ok(usage)
}

fn consume_exact_reservation(
    root: &File,
    operation_id: &str,
    expires: u64,
) -> Result<SkillOperationReservationV1, SkillOperationError> {
    let reservation: SkillOperationReservationV1 = read_json_owned(
        root,
        &reservation_name(operation_id),
        "SKILL_OPERATION_RESERVATION_REQUIRED",
    )?;
    if reservation.schema != SKILL_OPERATION_LEDGER_SCHEMA
        || reservation.operation_id != operation_id
        || reservation.expires_at_unix_seconds != expires
        || reservation.reserved_staged_bytes != crate::MAX_ARCHIVE_BYTES as u64
        || reservation.staging_directory_name
            != exact_staging_directory_name(operation_id)
                .map_err(|error| SkillOperationError::new(&error.code, &error.phase))?
        || unix_seconds() >= reservation.expires_at_unix_seconds
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_RESERVATION_REQUIRED",
            "admission",
        ));
    }
    Ok(reservation)
}

fn persist_reservation(
    root: &File,
    reservation: &SkillOperationReservationV1,
) -> Result<(), SkillOperationError> {
    let name = reservation_name(&reservation.operation_id);
    let temporary = format!(".{name}.pending");
    let file = create_new_at(
        root.as_raw_fd(),
        &temporary,
        0o600,
        "SKILL_OPERATION_RESERVATION_PENDING_EXISTS",
    )?;
    write_json_owned(file, reservation)?;
    let from = std::ffi::CString::new(temporary).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "admission")
    })?;
    let to = std::ffi::CString::new(name).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "admission")
    })?;
    if unsafe {
        libc::renameat(
            root.as_raw_fd(),
            from.as_ptr(),
            root.as_raw_fd(),
            to.as_ptr(),
        )
    } != 0
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_RESERVATION_RENAME_FAILED",
            "admission",
        ));
    }
    sync_directory(root)
}

fn directory_identity(root: &File, code: &str) -> Result<(u64, u64), SkillOperationError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(root.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(SkillOperationError::new(code, "filesystem"));
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o7777 != 0o700
    {
        return Err(SkillOperationError::new(code, "filesystem"));
    }
    Ok((stat.st_dev as u64, stat.st_ino as u64))
}

fn directory_object_identity(root: &File, code: &str) -> Result<(u64, u64), SkillOperationError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(root.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(SkillOperationError::new(code, "filesystem"));
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFDIR || stat.st_uid != unsafe { libc::geteuid() } {
        return Err(SkillOperationError::new(code, "filesystem"));
    }
    Ok((stat.st_dev as u64, stat.st_ino as u64))
}
fn capability_value(operation: &str) -> Result<String, SkillOperationError> {
    // The raw value is one-time authority, not an identifier.  Never derive
    // it from clock/PID/counter values that a co-resident process can guess.
    let mut random = [0_u8; 32];
    getrandom::getrandom(&mut random).map_err(|_| {
        SkillOperationError::new(
            "SKILL_OPERATION_CAPABILITY_RANDOM_UNAVAILABLE",
            "confirmation",
        )
    })?;
    Ok(digest(
        [
            operation.as_bytes(),
            &random,
            &CAPABILITY_SEQUENCE
                .fetch_add(1, Ordering::Relaxed)
                .to_be_bytes(),
        ]
        .concat()
        .as_slice(),
    ))
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn acquire_operation_lock(root: &File, operation_id: &str) -> Result<File, SkillOperationError> {
    valid_operation_id(operation_id)?;
    let name = operation_lock_bucket_name(operation_id);
    let file = create_or_open_at(
        root.as_raw_fd(),
        &name,
        0o600,
        "SKILL_OPERATION_OPERATION_LOCK_FAILED",
    )?;
    validate_open_owned_file(&file, "SKILL_OPERATION_OPERATION_LOCK_FAILED")?;
    validate_name_matches_open_file(
        root.as_raw_fd(),
        &name,
        &file,
        "SKILL_OPERATION_OPERATION_LOCK_FAILED",
    )?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_OPERATION_BUSY",
            "ledger",
        ));
    }
    validate_open_owned_file(&file, "SKILL_OPERATION_OPERATION_LOCK_FAILED")?;
    validate_name_matches_open_file(
        root.as_raw_fd(),
        &name,
        &file,
        "SKILL_OPERATION_OPERATION_LOCK_FAILED",
    )?;
    Ok(file)
}

fn operation_lock_bucket_name(operation_id: &str) -> String {
    let digest = Sha256::digest(operation_id.as_bytes());
    let bucket = usize::from(digest[0]) % OPERATION_LOCK_BUCKETS;
    format!(".skill-operation.bucket-{bucket}.lock")
}

fn operation_lock_bucket_from_name(name: &str) -> Option<usize> {
    let value = name
        .strip_prefix(".skill-operation.bucket-")?
        .strip_suffix(".lock")?
        .parse::<usize>()
        .ok()?;
    (value < OPERATION_LOCK_BUCKETS).then_some(value)
}

fn write_new_ledger(
    root: &File,
    name: &str,
    ledger: &SkillOperationLedgerV1,
) -> Result<(), SkillOperationError> {
    let file = create_new_at(
        root.as_raw_fd(),
        name,
        0o600,
        "SKILL_OPERATION_OPERATION_EXISTS",
    )?;
    write_ledger(file, ledger)?;
    sync_directory(root)
}

fn persist(
    root: &File,
    name: &str,
    ledger: &mut SkillOperationLedgerV1,
) -> Result<(), SkillOperationError> {
    // The operation lock makes this replacement a single-owner transition.
    // The pending record is self-describing, so a restart may promote only a
    // complete successor of the still-named ledger, never an arbitrary stale
    // temp with a convenient name.
    let temporary = format!(".{name}.pending");
    let current_bytes = read_owned_ledger_bytes(root, name)?;
    let current: SkillOperationLedgerV1 = serde_json::from_slice(&current_bytes)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_INVALID", "ledger"))?;
    if current.schema != ledger.schema || current.operation_id != ledger.operation_id {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT",
            "ledger",
        ));
    }
    let mut next = ledger.clone();
    next.generation = current.generation.checked_add(1).ok_or_else(|| {
        SkillOperationError::new("SKILL_OPERATION_LEDGER_GENERATION_EXHAUSTED", "ledger")
    })?;
    next.previous_ledger_sha256 = Some(digest(&current_bytes));
    if !legal_single_step_transition(&current, &next) {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_TRANSITION_INVALID",
            "ledger",
        ));
    }
    validate_ledger_kind(&next, "ledger")?;
    let file = create_new_at(
        root.as_raw_fd(),
        &temporary,
        0o600,
        "SKILL_OPERATION_LEDGER_PENDING_EXISTS",
    )?;
    write_ledger(file, &next)?;
    let from = std::ffi::CString::new(temporary)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "ledger"))?;
    let to = std::ffi::CString::new(name)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "ledger"))?;
    if unsafe {
        libc::renameat(
            root.as_raw_fd(),
            from.as_ptr(),
            root.as_raw_fd(),
            to.as_ptr(),
        )
    } != 0
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_RENAME_FAILED",
            "ledger",
        ));
    }
    sync_directory(root)?;
    *ledger = next;
    Ok(())
}

fn read_owned_ledger_bytes(root: &File, name: &str) -> Result<Vec<u8>, SkillOperationError> {
    validate_owned_file_at(root.as_raw_fd(), name)?;
    let name = std::ffi::CString::new(name)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "recovery"))?;
    let fd = unsafe {
        libc::openat(
            root.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_NOT_FOUND",
            "recovery",
        ));
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    validate_open_owned_file(&file, "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT")?;
    let size = file
        .metadata()
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_READ_FAILED", "recovery"))?
        .len();
    if size == 0 || size > 1024 * 1024 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_CONTENT_DRIFT",
            "recovery",
        ));
    }
    let mut body = Vec::with_capacity(size as usize);
    file.read_to_end(&mut body)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_READ_FAILED", "recovery"))?;
    if body.len() as u64 != size {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_CONTENT_DRIFT",
            "recovery",
        ));
    }
    Ok(body)
}
fn write_ledger(
    mut file: File,
    ledger: &SkillOperationLedgerV1,
) -> Result<(), SkillOperationError> {
    let bytes = serde_json::to_vec(ledger).map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_LEDGER_SERIALIZATION_FAILED", "ledger")
    })?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_WRITE_FAILED", "ledger"))
}

fn load_ledger(root: &File, name: &str) -> Result<SkillOperationLedgerV1, SkillOperationError> {
    let pending_name = format!(".{name}.pending");
    let pending = std::ffi::CString::new(pending_name.as_str())
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "recovery"))?;
    let mut pending_stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let pending_result = unsafe {
        libc::fstatat(
            root.as_raw_fd(),
            pending.as_ptr(),
            pending_stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if pending_result == 0 {
        validate_owned_file_at(root.as_raw_fd(), &pending_name).map_err(|_| {
            SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_PENDING_RECONCILE_REQUIRED",
                "recovery",
            )
        })?;
        let current_bytes = read_owned_ledger_bytes(root, name)?;
        let current: SkillOperationLedgerV1 = serde_json::from_slice(&current_bytes)
            .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_INVALID", "recovery"))?;
        let pending_bytes = read_owned_ledger_bytes(root, &pending_name)?;
        let pending_ledger: SkillOperationLedgerV1 = serde_json::from_slice(&pending_bytes)
            .map_err(|_| {
                SkillOperationError::new(
                    "SKILL_OPERATION_LEDGER_PENDING_RECONCILE_REQUIRED",
                    "recovery",
                )
            })?;
        let successor = pending_ledger.schema == current.schema
            && pending_ledger.operation_id == current.operation_id
            && pending_ledger.operation_kind == current.operation_kind
            && pending_ledger.plan_digest_sha256 == current.plan_digest_sha256
            && pending_ledger.confirmation_capability_sha256
                == current.confirmation_capability_sha256
            && pending_ledger.source_request_sha256 == current.source_request_sha256
            && pending_ledger.source_archive_sha256 == current.source_archive_sha256
            && pending_ledger.staging_object_identity_sha256
                == current.staging_object_identity_sha256
            && pending_ledger.staging_receipt_sha256 == current.staging_receipt_sha256
            && pending_ledger.inspection_content_sha256 == current.inspection_content_sha256
            && pending_ledger.source_content_sha256 == current.source_content_sha256
            && pending_ledger.target == current.target
            && pending_ledger.expires_at_unix_seconds == current.expires_at_unix_seconds
            && pending_ledger.plan == current.plan
            && pending_ledger.removal == current.removal
            && pending_ledger.generation == current.generation.saturating_add(1)
            && pending_ledger.previous_ledger_sha256.as_deref()
                == Some(digest(&current_bytes).as_str())
            && legal_single_step_transition(&current, &pending_ledger)
            && validate_ledger_kind(&pending_ledger, "recovery").is_ok();
        if !successor {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_PENDING_RECONCILE_REQUIRED",
                "recovery",
            ));
        }
        let to = std::ffi::CString::new(name).map_err(|_| {
            SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "recovery")
        })?;
        if unsafe {
            libc::renameat(
                root.as_raw_fd(),
                pending.as_ptr(),
                root.as_raw_fd(),
                to.as_ptr(),
            )
        } != 0
        {
            return Err(SkillOperationError::new(
                "SKILL_OPERATION_LEDGER_RENAME_FAILED",
                "recovery",
            ));
        }
        sync_directory(root)?;
    } else if std::io::Error::last_os_error().kind() != std::io::ErrorKind::NotFound {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_PENDING_CHECK_FAILED",
            "recovery",
        ));
    }
    let name = std::ffi::CString::new(name)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "recovery"))?;
    let fd = unsafe {
        libc::openat(
            root.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_NOT_FOUND",
            "recovery",
        ));
    }
    let mut file = unsafe { File::from_raw_fd(fd) };
    validate_open_owned_file(&file, "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT")?;
    let size = file
        .metadata()
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_IDENTITY_DRIFT", "recovery"))?
        .len();
    if size == 0 || size > 1024 * 1024 {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_SIZE_INVALID",
            "recovery",
        ));
    }
    let mut body = Vec::with_capacity(size as usize);
    Read::by_ref(&mut file)
        .take(size + 1)
        .read_to_end(&mut body)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_READ_FAILED", "recovery"))?;
    if body.len() as u64 != size {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_CONTENT_DRIFT",
            "recovery",
        ));
    }
    serde_json::from_slice(&body)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_INVALID", "recovery"))
}

fn create_new_at(
    parent: i32,
    name: &str,
    mode: u32,
    code: &str,
) -> Result<File, SkillOperationError> {
    let name = std::ffi::CString::new(name)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "ledger"))?;
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode,
        )
    };
    if fd < 0 {
        return Err(SkillOperationError::new(code, "ledger"));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn create_or_open_at(
    parent: i32,
    name: &str,
    mode: u32,
    code: &str,
) -> Result<File, SkillOperationError> {
    let name = std::ffi::CString::new(name)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "ledger"))?;
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode,
        )
    };
    if fd < 0 {
        return Err(SkillOperationError::new(code, "ledger"));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn sync_directory(root: &File) -> Result<(), SkillOperationError> {
    root.sync_all().map_err(|_| {
        SkillOperationError::new("SKILL_OPERATION_LEDGER_DIRECTORY_SYNC_FAILED", "ledger")
    })
}

fn validate_owned_file_at(parent: i32, name: &str) -> Result<(), SkillOperationError> {
    let name = std::ffi::CString::new(name)
        .map_err(|_| SkillOperationError::new("SKILL_OPERATION_LEDGER_NAME_INVALID", "ledger"))?;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT",
            "ledger",
        ));
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o7777 != 0o600
        || stat.st_nlink != 1
    {
        return Err(SkillOperationError::new(
            "SKILL_OPERATION_LEDGER_IDENTITY_DRIFT",
            "ledger",
        ));
    }
    Ok(())
}

fn validate_open_owned_file(file: &File, code: &str) -> Result<(), SkillOperationError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(file.as_raw_fd(), stat.as_mut_ptr()) } != 0 {
        return Err(SkillOperationError::new(code, "ledger"));
    }
    let stat = unsafe { stat.assume_init() };
    if stat.st_mode & libc::S_IFMT != libc::S_IFREG
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o7777 != 0o600
        || stat.st_nlink != 1
    {
        return Err(SkillOperationError::new(code, "ledger"));
    }
    Ok(())
}

fn validate_name_matches_open_file(
    parent: i32,
    name: &str,
    file: &File,
    code: &str,
) -> Result<(), SkillOperationError> {
    let name =
        std::ffi::CString::new(name).map_err(|_| SkillOperationError::new(code, "ledger"))?;
    let mut named = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let mut opened = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            named.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
        || unsafe { libc::fstat(file.as_raw_fd(), opened.as_mut_ptr()) } != 0
    {
        return Err(SkillOperationError::new(code, "ledger"));
    }
    let named = unsafe { named.assume_init() };
    let opened = unsafe { opened.assume_init() };
    if named.st_dev != opened.st_dev || named.st_ino != opened.st_ino {
        return Err(SkillOperationError::new(code, "ledger"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::{
        acquire_install_lock_at, fail_next_post_rename_root_sync,
        scan_installed_payload_with_limits,
    };
    use std::os::unix::fs::MetadataExt;
    use std::sync::mpsc;
    use std::time::Duration;

    #[derive(Default)]
    struct FakeOperon {
        attached: bool,
        attach_calls: usize,
        detach_calls: usize,
        readback_calls: usize,
        fail_persist_after_attach: bool,
        fail_persist_after_detach: bool,
        replace_on_detach: Option<PathBuf>,
    }

    impl SkillOperationOperonAdapter for FakeOperon {
        fn attach_and_readback(&mut self, _: &str, _: &str) -> Result<(), crate::AttachError> {
            self.attach_calls += 1;
            self.attached = true;
            if self.fail_persist_after_attach {
                fail_next_post_effect_persist();
            }
            Ok(())
        }
        fn detach_and_readback(&mut self, _: &str, _: &str) -> Result<(), crate::AttachError> {
            self.detach_calls += 1;
            self.attached = false;
            if self.fail_persist_after_detach {
                fail_next_post_effect_persist();
            }
            if let Some(path) = self.replace_on_detach.take() {
                std::fs::remove_dir_all(&path).unwrap();
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(path.join("SKILL.md"), b"replacement").unwrap();
            }
            Ok(())
        }
        fn readback(&mut self, _: &str, _: &str) -> Result<bool, crate::AttachError> {
            self.readback_calls += 1;
            Ok(self.attached)
        }
    }

    struct PausedAttachOperon {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }

    impl SkillOperationOperonAdapter for PausedAttachOperon {
        fn attach_and_readback(&mut self, _: &str, _: &str) -> Result<(), crate::AttachError> {
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            Ok(())
        }

        fn detach_and_readback(&mut self, _: &str, _: &str) -> Result<(), crate::AttachError> {
            unreachable!("install pause fixture must never detach")
        }

        fn readback(&mut self, _: &str, _: &str) -> Result<bool, crate::AttachError> {
            Ok(true)
        }
    }

    struct PausedDetachOperon {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }

    impl SkillOperationOperonAdapter for PausedDetachOperon {
        fn attach_and_readback(&mut self, _: &str, _: &str) -> Result<(), crate::AttachError> {
            unreachable!("removal pause fixture must never attach")
        }

        fn detach_and_readback(&mut self, _: &str, _: &str) -> Result<(), crate::AttachError> {
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            Ok(())
        }

        fn readback(&mut self, _: &str, _: &str) -> Result<bool, crate::AttachError> {
            Ok(false)
        }
    }

    struct InstallFixture {
        root: PathBuf,
        data: PathBuf,
        staging: File,
        ledger: File,
        target: SkillOperationTargetBindingV1,
        source: GithubInspectionSource,
        archive: Vec<u8>,
    }

    impl InstallFixture {
        fn new(label: &str) -> Self {
            use std::fs;
            use std::io::Cursor;
            use std::os::unix::fs::PermissionsExt;
            use zip::write::SimpleFileOptions;

            let root = Path::new("/private/tmp").join(format!(
                "csswitch-operation-install-{label}-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                TEST_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let data = root.join("data");
            let staging_path = root.join("staging");
            let ledger_path = root.join("ledger");
            fs::create_dir_all(data.join("orgs/org/skills")).unwrap();
            fs::create_dir_all(&staging_path).unwrap();
            fs::create_dir_all(&ledger_path).unwrap();
            for path in [&root, &data, &staging_path, &ledger_path] {
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            fs::write(data.join("active-org.json"), br#"{"org_uuid":"org"}"#).unwrap();
            let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
            writer
                .start_file(
                    "repo-0123456789ab/skills/demo/SKILL.md",
                    SimpleFileOptions::default().unix_permissions(0o644),
                )
                .unwrap();
            writer
                .write_all(b"---\nname: demo\ndescription: Demo skill\n---\nBody\n")
                .unwrap();
            let archive = writer.finish().unwrap().into_inner();
            let source = GithubInspectionSource {
                owner: "owner".into(),
                repo: "repo".into(),
                commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
                path: "skills/demo".into(),
            };
            let skills_metadata = fs::metadata(data.join("orgs/org/skills")).unwrap();
            let target = SkillOperationTargetBindingV1 {
                science_runtime_identity_sha256: "1".repeat(64),
                data_dir_identity_sha256: "2".repeat(64),
                active_org_identity_sha256: "3".repeat(64),
                active_org: "org".into(),
                skills_root_device: skills_metadata.dev(),
                skills_root_inode: skills_metadata.ino(),
            };
            Self {
                root,
                data,
                staging: File::open(staging_path).unwrap(),
                ledger: File::open(ledger_path).unwrap(),
                target,
                source,
                archive,
            }
        }

        fn prepare(&self, operation_id: &str) -> SkillOperationPrepared {
            self.prepare_with_expiry(operation_id, u64::MAX)
        }

        fn prepare_with_expiry(&self, operation_id: &str, expiry: u64) -> SkillOperationPrepared {
            SkillOperationPrepared::prepare(
                &self.staging,
                &self.ledger,
                self.source.clone(),
                &self.archive,
                SkillOperationPrepareRequestV1 {
                    operation_id: operation_id.into(),
                    source_request_sha256: "a".repeat(64),
                    plan: ConfirmablePlanRequestV1 {
                        plan_id: operation_id.into(),
                        expires_at_unix_seconds: expiry,
                        target: ConfirmablePlanTargetV1 {
                            science_runtime_identity_sha256: self
                                .target
                                .science_runtime_identity_sha256
                                .clone(),
                            data_dir_identity_sha256: self.target.data_dir_identity_sha256.clone(),
                            active_org_identity_sha256: self
                                .target
                                .active_org_identity_sha256
                                .clone(),
                            skills_root_device: self.target.skills_root_device,
                            skills_root_inode: self.target.skills_root_inode,
                        },
                    },
                    target: self.target.clone(),
                },
            )
            .unwrap()
        }
    }

    impl Drop for InstallFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    struct RemovalFixture {
        root: PathBuf,
        data: PathBuf,
        staging: File,
        roots: SkillRemovalRoots,
        ledger: File,
        ledger_path: PathBuf,
        target: SkillOperationTargetBindingV1,
    }

    impl RemovalFixture {
        fn new(label: &str) -> Self {
            use std::fs;
            use std::os::unix::fs::PermissionsExt;
            let root = Path::new("/private/tmp").join(format!(
                "csswitch-removal-{label}-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                TEST_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let data = root.join("data");
            let skills = data.join("orgs/org/skills");
            let staging = root.join("staging");
            let quarantine = root.join("quarantine");
            let ledger_path = root.join("ledger");
            fs::create_dir_all(skills.join("demo")).unwrap();
            fs::create_dir_all(&staging).unwrap();
            fs::create_dir_all(&quarantine).unwrap();
            fs::create_dir_all(&ledger_path).unwrap();
            for path in [&root, &data, &skills, &staging, &quarantine, &ledger_path] {
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            fs::write(data.join("active-org.json"), br#"{"org_uuid":"org"}"#).unwrap();
            fs::write(
                skills.join("demo/SKILL.md"),
                b"---\nname: demo\ndescription: demo\n---\n",
            )
            .unwrap();
            let files = scan_installed_payload_with_limits(
                &skills.join("demo"),
                MAX_FILES,
                MAX_TOTAL_BYTES,
                false,
            )
            .unwrap();
            let content = crate::archive::canonical_content_sha256(&files);
            let marker = serde_json::json!({"version":1,"repo":"owner/repo","sha":"0123456789abcdef0123456789abcdef01234567","plugin":"demo","marketplace":"csswitch-local-bridge","path":"demo","importedAt":"2026-08-20T00:00:00Z","license":"MIT","content_sha256":content,"source_kind":"github"});
            fs::write(
                skills.join("demo/.import-origin"),
                serde_json::to_vec(&marker).unwrap(),
            )
            .unwrap();
            let skills_metadata = fs::metadata(&skills).unwrap();
            let target = SkillOperationTargetBindingV1 {
                science_runtime_identity_sha256: "1".repeat(64),
                data_dir_identity_sha256: "2".repeat(64),
                active_org_identity_sha256: "3".repeat(64),
                active_org: "org".into(),
                skills_root_device: skills_metadata.dev(),
                skills_root_inode: skills_metadata.ino(),
            };
            Self {
                root,
                data,
                staging: File::open(staging).unwrap(),
                roots: SkillRemovalRoots {
                    skills_root: File::open(skills).unwrap(),
                    quarantine_root: File::open(quarantine).unwrap(),
                },
                ledger: File::open(&ledger_path).unwrap(),
                ledger_path,
                target,
            }
        }
        fn prepare(&self, operation_id: &str, expiry: u64) -> SkillOperationRemovalPrepared {
            SkillOperationRemovalPrepared::prepare(
                &self.staging,
                &self.ledger,
                &self.data,
                &self.roots,
                operation_id.into(),
                expiry,
                self.target.clone(),
                "demo".into(),
            )
            .unwrap()
        }
    }

    impl Drop for RemovalFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn receipt(
        order: u32,
        kind: PlanEffectKindV1,
        state: SkillOperationEffectStateV1,
    ) -> SkillOperationEffectReceiptV1 {
        SkillOperationEffectReceiptV1 {
            order,
            kind,
            state,
            intent: SkillOperationEffectIntentV1::None,
            attempt_id: None,
            attempt_started_at_unix_seconds: None,
            evidence_sha256: None,
            error_code: None,
        }
    }

    fn mark_in_progress(ledger: &mut SkillOperationLedgerV1, index: usize) {
        let receipt = &mut ledger.effects[index];
        receipt.intent = SkillOperationEffectIntentV1::InProgress;
        receipt.attempt_id = Some(capability_value(&format!("test-{index}")).unwrap());
        receipt.attempt_started_at_unix_seconds = Some(unix_seconds());
    }

    fn set_test_unix_seconds(value: Option<u64>) {
        TEST_UNIX_SECONDS_OVERRIDE.with(|override_value| override_value.set(value));
    }

    fn persist_verified_detach_prefix(prepared: &mut SkillOperationRemovalPrepared) {
        prepared.ledger.consumed = true;
        mark_in_progress(&mut prepared.ledger, 0);
        persist(
            &prepared.ledger_root,
            &prepared.ledger_name,
            &mut prepared.ledger,
        )
        .unwrap();
        prepared.ledger.effects[0].state = SkillOperationEffectStateV1::Verified;
        prepared.ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
        prepared.ledger.effects[0].evidence_sha256 = Some(digest(
            prepared
                .ledger
                .removal
                .as_ref()
                .unwrap()
                .skill_name
                .as_bytes(),
        ));
        persist(
            &prepared.ledger_root,
            &prepared.ledger_name,
            &mut prepared.ledger,
        )
        .unwrap();
    }

    fn complete_install(
        fixture: &InstallFixture,
        prepared: SkillOperationPrepared,
    ) -> SkillOperationApplyReceiptV1 {
        let capability = prepared.confirmation_capability().clone();
        let skills = File::open(fixture.data.join("orgs/org/skills")).unwrap();
        prepared
            .apply(
                &fixture.data,
                &skills,
                &capability,
                &fixture.target,
                &mut FakeOperon::default(),
            )
            .unwrap()
    }

    fn write_pending_successor(
        root: &File,
        root_path: &Path,
        ledger_name: &str,
        ledger: &SkillOperationLedgerV1,
        mutate: impl FnOnce(&mut SkillOperationLedgerV1),
    ) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let current = read_owned_ledger_bytes(root, ledger_name).unwrap();
        let mut successor = ledger.clone();
        successor.generation = ledger.generation + 1;
        successor.previous_ledger_sha256 = Some(digest(&current));
        mutate(&mut successor);
        let pending = root_path.join(format!(".{ledger_name}.pending"));
        std::fs::write(&pending, serde_json::to_vec(&successor).unwrap()).unwrap();
        std::fs::set_permissions(&pending, std::fs::Permissions::from_mode(0o600)).unwrap();
        pending
    }

    fn ledger(
        states: (SkillOperationEffectStateV1, SkillOperationEffectStateV1),
    ) -> SkillOperationLedgerV1 {
        SkillOperationLedgerV1 {
            schema: SKILL_OPERATION_LEDGER_SCHEMA.into(),
            generation: 0,
            previous_ledger_sha256: None,
            operation_kind: SkillOperationKindV1::Install,
            operation_id: "op-one".into(),
            plan_digest_sha256: "a".repeat(64),
            confirmation_capability_sha256: "b".repeat(64),
            source_request_sha256: Some("c".repeat(64)),
            source_archive_sha256: Some("c".repeat(64)),
            staging_object_identity_sha256: Some("d".repeat(64)),
            staging_receipt_sha256: Some("e".repeat(64)),
            inspection_content_sha256: Some("f".repeat(64)),
            source_content_sha256: "f".repeat(64),
            target: SkillOperationTargetBindingV1 {
                science_runtime_identity_sha256: "1".repeat(64),
                data_dir_identity_sha256: "2".repeat(64),
                active_org_identity_sha256: "3".repeat(64),
                active_org: "org".into(),
                skills_root_device: 1,
                skills_root_inode: 2,
            },
            expires_at_unix_seconds: u64::MAX,
            plan: impossible_plan(),
            consumed: false,
            effects: vec![
                receipt(1, PlanEffectKindV1::PackageCommit, states.0),
                receipt(2, PlanEffectKindV1::OperonAttach, states.1),
            ],
            removal: None,
        }
    }

    // The projection test constructs a placeholder only; `project` must never
    // inspect plan text or infer state outside durable effect receipts.
    fn impossible_plan() -> SkillPlanV1 {
        serde_json::from_value(serde_json::json!({
        "schema":"csswitch.skill-plan.v1", "identity":{"kind":"invocation","plan_id":"p"}, "plan_digest_sha256":"0".repeat(64),
        "inspection_report_digest_sha256":"0".repeat(64), "component_graph_digest_sha256":"0".repeat(64), "inspection_outcome":"complete",
        "source":{"kind":"github_exact","owner":"o","repo":"r","resolved_commit_sha":"0123456789abcdef0123456789abcdef01234567","content_sha256":"0".repeat(64),"binding":"csswitch_exact_content_bound"},
        "target":{"state":"unbound"}, "eligibility":"inspect_only", "confirmation":"not_capable", "selection":{"state":"not_applicable"}, "expiry":{"state":"not_applicable"}, "reentry_policy":"inspect_only_non_consumable", "components":[], "effects":[],
        "summary":{"component_count":0,"effect_count":0,"unsupported_component_count":0,"degraded_component_count":0,"not_run_effect_count":0}
    })).unwrap()
    }

    #[test]
    fn final_response_is_a_ledger_projection_and_uncertainty_never_becomes_success() {
        let verified = project(&ledger((
            SkillOperationEffectStateV1::Verified,
            SkillOperationEffectStateV1::Verified,
        )));
        assert_eq!(verified.status, "INSTALLED_ATTACHED_VERIFY_REQUIRED");
        let uncertain = project(&ledger((
            SkillOperationEffectStateV1::Verified,
            SkillOperationEffectStateV1::Uncertain,
        )));
        assert_eq!(uncertain.status, "ATTACH_STATE_UNCERTAIN");
        assert!(uncertain.recovery_required);
    }

    #[test]
    fn confirmation_values_are_not_predictable_counters() {
        let first = capability_value("op").unwrap();
        let second = capability_value("op").unwrap();
        assert_ne!(first, second);
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    #[test]
    fn install_pre_effect_bundle_rejection_leaves_ledger_not_started() {
        use std::fs;
        use std::io::Cursor;
        use std::os::unix::fs::PermissionsExt;
        use zip::write::SimpleFileOptions;

        let root = Path::new("/private/tmp").join(format!(
            "csswitch-operation-pre-effect-{}-{}",
            std::process::id(),
            unix_seconds()
        ));
        let staging_path = root.join("staging");
        let ledger_path = root.join("ledger");
        let data = root.join("data");
        fs::create_dir_all(data.join("orgs/org/skills")).unwrap();
        fs::create_dir_all(&staging_path).unwrap();
        fs::create_dir_all(&ledger_path).unwrap();
        for path in [&root, &staging_path, &ledger_path, &data] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(data.join("active-org.json"), br#"{"org_uuid":"org"}"#).unwrap();

        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in [
            "repo-0123456789ab/skills/demo/SKILL.md",
            "repo-0123456789ab/skills/other/SKILL.md",
        ] {
            writer
                .start_file(name, SimpleFileOptions::default().unix_permissions(0o644))
                .unwrap();
            let manifest = if name.contains("/other/") {
                b"---\nname: other\ndescription: Other skill\n---\nBody\n".as_slice()
            } else {
                b"---\nname: demo\ndescription: Demo skill\n---\nBody\n".as_slice()
            };
            writer.write_all(manifest).unwrap();
        }
        let archive = writer.finish().unwrap().into_inner();
        let source = GithubInspectionSource {
            owner: "owner".into(),
            repo: "repo".into(),
            commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
            path: "skills/demo".into(),
        };
        let skills_metadata = std::fs::metadata(data.join("orgs/org/skills")).unwrap();
        let target = SkillOperationTargetBindingV1 {
            science_runtime_identity_sha256: "1".repeat(64),
            data_dir_identity_sha256: "2".repeat(64),
            active_org_identity_sha256: "3".repeat(64),
            active_org: "org".into(),
            skills_root_device: skills_metadata.dev(),
            skills_root_inode: skills_metadata.ino(),
        };
        let staging = File::open(&staging_path).unwrap();
        let ledger_root = File::open(&ledger_path).unwrap();
        let mut prepared = SkillOperationPrepared::prepare(
            &staging,
            &ledger_root,
            source,
            &archive,
            SkillOperationPrepareRequestV1 {
                operation_id: "pre-effect-bundle".into(),
                source_request_sha256: "a".repeat(64),
                plan: ConfirmablePlanRequestV1 {
                    plan_id: "pre-effect-bundle".into(),
                    expires_at_unix_seconds: u64::MAX,
                    target: ConfirmablePlanTargetV1 {
                        science_runtime_identity_sha256: target
                            .science_runtime_identity_sha256
                            .clone(),
                        data_dir_identity_sha256: target.data_dir_identity_sha256.clone(),
                        active_org_identity_sha256: target.active_org_identity_sha256.clone(),
                        skills_root_device: target.skills_root_device,
                        skills_root_inode: target.skills_root_inode,
                    },
                },
                target: target.clone(),
            },
        )
        .unwrap();
        let before = prepared.ledger.clone();
        let ledger_name = prepared.ledger_name.clone();
        let capability = prepared.confirmation_capability().clone();
        // The immutable plan was made for one selected Skill. This simulates
        // a post-plan parser subject mismatch before any effect intent starts.
        prepared.source.path = "skills".into();
        let skills_root = File::open(data.join("orgs/org/skills")).unwrap();
        let error = prepared
            .apply(
                &data,
                &skills_root,
                &capability,
                &target,
                &mut FakeOperon::default(),
            )
            .unwrap_err();
        assert_eq!(error.code, "SKILL_OPERATION_BUNDLE_OR_PLUGIN_REJECTED");
        let durable = load_ledger(&ledger_root, &ledger_name).unwrap();
        assert_eq!(durable, before);
        assert!(durable.effects.iter().all(|effect| {
            effect.state == SkillOperationEffectStateV1::NotStarted
                && effect.intent == SkillOperationEffectIntentV1::None
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removal_destination_is_stable_relative_and_content_bound() {
        let first =
            skill_removal_destination("remove-one", "demo", &"a".repeat(64), &"b".repeat(64))
                .unwrap();
        let repeat =
            skill_removal_destination("remove-one", "demo", &"a".repeat(64), &"b".repeat(64))
                .unwrap();
        let changed =
            skill_removal_destination("remove-one", "demo", &"a".repeat(64), &"c".repeat(64))
                .unwrap();
        assert_eq!(first, repeat);
        assert_ne!(first, changed);
        assert!(!first.contains('/'));
    }

    #[test]
    fn confirmed_removal_under_held_shared_lock_does_not_detach_or_quarantine() {
        let fixture = RemovalFixture::new("shared-lock-removal");
        let prepared = fixture.prepare("shared-lock-removal", u64::MAX);
        let capability = prepared.confirmation_capability().clone();
        // Model an exact install that has already committed its package and is
        // paused immediately before native attach.  The confirmed removal
        // must contend on the very same fd-relative lock inode and fail before
        // it can issue a detach or move any directory.
        let _install_lock = acquire_install_lock_at(&fixture.roots.skills_root, "demo").unwrap();
        let mut operon = FakeOperon::default();
        let error = prepared
            .apply(
                &fixture.data,
                &fixture.roots,
                &capability,
                &fixture.target,
                &mut operon,
            )
            .err()
            .unwrap();
        assert_eq!(error.code, "INSTALL_BUSY");
        assert_eq!(operon.detach_calls, 0);
        assert!(fixture
            .data
            .join("orgs/org/skills/demo/.import-origin")
            .is_file());
        assert!(std::fs::read_dir(fixture.root.join("quarantine"))
            .unwrap()
            .next()
            .is_none());
    }

    #[test]
    fn paused_exact_install_blocks_confirmed_removal_until_attach_is_verified() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = InstallFixture::new("paused-install-removal");
        let prepared = fixture.prepare("paused-install-removal");
        let capability = prepared.confirmation_capability().clone();
        let data = fixture.data.clone();
        let target = fixture.target.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let install = std::thread::spawn(move || {
            let skills = File::open(data.join("orgs/org/skills")).unwrap();
            let mut operon = PausedAttachOperon {
                entered: entered_tx,
                release: release_rx,
            };
            prepared.apply(&data, &skills, &capability, &target, &mut operon)
        });
        // The signal is issued by `attach_and_readback`, after exact package
        // publication but before attach returns.  The operation-wide lock is
        // therefore still live at the interleaving point under test.
        entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();

        let quarantine = fixture.root.join("paused-install-quarantine");
        let removal_ledger = fixture.root.join("paused-install-removal-ledger");
        std::fs::create_dir_all(&quarantine).unwrap();
        std::fs::create_dir_all(&removal_ledger).unwrap();
        std::fs::set_permissions(
            fixture.data.join("orgs/org/skills"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        std::fs::set_permissions(&quarantine, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&removal_ledger, std::fs::Permissions::from_mode(0o700)).unwrap();
        let roots = SkillRemovalRoots {
            skills_root: File::open(fixture.data.join("orgs/org/skills")).unwrap(),
            quarantine_root: File::open(&quarantine).unwrap(),
        };
        let removal_ledger_fd = File::open(&removal_ledger).unwrap();
        let removal = SkillOperationRemovalPrepared::prepare(
            &fixture.staging,
            &removal_ledger_fd,
            &fixture.data,
            &roots,
            "paused-install-removal-confirmed".into(),
            u64::MAX,
            fixture.target.clone(),
            "demo".into(),
        )
        .unwrap();
        let removal_capability = removal.confirmation_capability().clone();
        let mut removal_operon = FakeOperon::default();
        let error = removal
            .apply(
                &fixture.data,
                &roots,
                &removal_capability,
                &fixture.target,
                &mut removal_operon,
            )
            .err()
            .unwrap();
        assert_eq!(error.code, "INSTALL_BUSY");
        assert_eq!(removal_operon.detach_calls, 0);
        assert!(std::fs::read_dir(&quarantine).unwrap().next().is_none());

        release_tx.send(()).unwrap();
        let receipt = install.join().unwrap().unwrap();
        assert_eq!(receipt.final_response.directory_commit, Some(true));
        assert_eq!(receipt.final_response.attach_verified, Some(true));
        assert!(fixture
            .data
            .join("orgs/org/skills/demo/.import-origin")
            .is_file());
    }

    #[test]
    fn paused_confirmed_removal_blocks_exact_install_continue() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = InstallFixture::new("paused-removal-install-continue");
        let initial = fixture.prepare("paused-removal-initial");
        let initial_capability = initial.confirmation_capability().clone();
        let skills = File::open(fixture.data.join("orgs/org/skills")).unwrap();
        stop_after_next_package_verify();
        let interrupted = initial
            .apply(
                &fixture.data,
                &skills,
                &initial_capability,
                &fixture.target,
                &mut FakeOperon::default(),
            )
            .err()
            .unwrap();
        assert_eq!(interrupted.code, "SKILL_OPERATION_TEST_PAUSE_AFTER_PACKAGE");
        std::fs::set_permissions(
            fixture.data.join("orgs/org/skills"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();

        // Resume a real durable verified-package / not-started-attach prefix;
        // this is operation-id-only continuation, not a forged ledger state.
        let continuation = SkillOperationPrepared::resume(
            &fixture.staging,
            &fixture.ledger,
            "paused-removal-initial",
        )
        .unwrap();

        let quarantine = fixture.root.join("paused-removal-quarantine");
        let removal_ledger = fixture.root.join("paused-removal-ledger");
        std::fs::create_dir_all(&quarantine).unwrap();
        std::fs::create_dir_all(&removal_ledger).unwrap();
        std::fs::set_permissions(&quarantine, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(&removal_ledger, std::fs::Permissions::from_mode(0o700)).unwrap();
        let roots = SkillRemovalRoots {
            skills_root: File::open(fixture.data.join("orgs/org/skills")).unwrap(),
            quarantine_root: File::open(&quarantine).unwrap(),
        };
        let removal = SkillOperationRemovalPrepared::prepare(
            &fixture.staging,
            &File::open(&removal_ledger).unwrap(),
            &fixture.data,
            &roots,
            "paused-removal-confirmed".into(),
            u64::MAX,
            fixture.target.clone(),
            "demo".into(),
        )
        .unwrap();
        let removal_capability = removal.confirmation_capability().clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let data = fixture.data.clone();
        let target = fixture.target.clone();
        let removal_thread = std::thread::spawn(move || {
            let mut operon = PausedDetachOperon {
                entered: entered_tx,
                release: release_rx,
            };
            removal.apply(&data, &roots, &removal_capability, &target, &mut operon)
        });
        entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let continue_skills = File::open(fixture.data.join("orgs/org/skills")).unwrap();
        let error = continuation
            .continue_apply(
                &fixture.data,
                &continue_skills,
                &fixture.target,
                &mut FakeOperon::default(),
            )
            .err()
            .unwrap();
        assert_eq!(error.code, "INSTALL_BUSY");
        release_tx.send(()).unwrap();
        let removal_receipt = removal_thread.join().unwrap().unwrap();
        assert_eq!(removal_receipt.final_response.detach_verified, Some(true));
        assert_eq!(removal_receipt.final_response.quarantine_commit, Some(true));
    }

    #[test]
    fn continue_rechecks_after_shared_lock_before_native_attach() {
        let fixture = InstallFixture::new("continue-lock-replacement");
        let initial = fixture.prepare("continue-lock-replacement");
        let capability = initial.confirmation_capability().clone();
        let skills_path = fixture.data.join("orgs/org/skills");
        stop_after_next_package_verify();
        let interrupted = initial
            .apply(
                &fixture.data,
                &File::open(&skills_path).unwrap(),
                &capability,
                &fixture.target,
                &mut FakeOperon::default(),
            )
            .unwrap_err();
        assert_eq!(interrupted.code, "SKILL_OPERATION_TEST_PAUSE_AFTER_PACKAGE");
        let continuation = SkillOperationPrepared::resume(
            &fixture.staging,
            &fixture.ledger,
            "continue-lock-replacement",
        )
        .unwrap();

        // This seam runs immediately after the continuation acquires the
        // actual shared fd-relative lock. It models the released competing
        // operation changing the name before continuation gets its sealed
        // readback; production remains fail-fast when the lock is held.
        let target_dir = skills_path.join("demo");
        replace_install_name_after_next_continuation_lock(target_dir);
        let mut operon = FakeOperon::default();
        let result = continuation.continue_apply(
            &fixture.data,
            &File::open(&skills_path).unwrap(),
            &fixture.target,
            &mut operon,
        );
        assert_eq!(
            result.unwrap_err().code,
            "SKILL_OPERATION_NEW_PLAN_REQUIRED"
        );
        assert_eq!(operon.attach_calls, 0);
    }

    #[test]
    fn post_effect_package_persist_failure_is_read_back_and_reconcile_admitted() {
        let fixture = InstallFixture::new("post-package");
        let prepared = fixture.prepare("post-package");
        let capability = prepared.confirmation_capability().clone();
        let skills = File::open(fixture.data.join("orgs/org/skills")).unwrap();
        fail_next_post_effect_persist();
        let receipt = prepared
            .apply(
                &fixture.data,
                &skills,
                &capability,
                &fixture.target,
                &mut FakeOperon::default(),
            )
            .unwrap();
        assert_eq!(
            receipt.final_response.status,
            "SKILL_OPERATION_DURABILITY_UNCERTAIN"
        );
        assert_eq!(receipt.final_response.directory_commit, Some(true));
        assert!(receipt.final_response.recovery_required);
        assert_eq!(
            receipt
                .final_response
                .post_effect_observation
                .unwrap()
                .observed,
            SkillOperationObservedStateV1::ObservedTrue
        );
        let mut resumed =
            SkillOperationPrepared::resume(&fixture.staging, &fixture.ledger, "post-package")
                .unwrap();
        let reconciled = resumed
            .reconcile_package_readback(
                &File::open(fixture.data.join("orgs/org/skills")).unwrap(),
                &fixture.target,
            )
            .unwrap();
        assert_eq!(reconciled.directory_commit, Some(true));
        assert!(!reconciled.recovery_required);
    }

    #[test]
    fn post_rename_skills_root_sync_failure_is_uncertain_and_reconcile_never_recommits() {
        let fixture = InstallFixture::new("post-rename-root-sync");
        let prepared = fixture.prepare("post-rename-root-sync");
        let ledger_name = prepared.ledger_name.clone();
        let capability = prepared.confirmation_capability().clone();
        let skills_path = fixture.data.join("orgs/org/skills");
        let skills = File::open(&skills_path).unwrap();
        fail_next_post_rename_root_sync();
        let receipt = prepared
            .apply(
                &fixture.data,
                &skills,
                &capability,
                &fixture.target,
                &mut FakeOperon::default(),
            )
            .unwrap();

        // Publication is visible despite the failed durability acknowledgement;
        // the externally returned value is an observation, never a durable
        // success claim.
        assert_eq!(receipt.final_response.directory_commit, Some(true));
        assert!(receipt.final_response.recovery_required);
        assert_eq!(
            receipt
                .final_response
                .post_effect_observation
                .as_ref()
                .unwrap()
                .observed,
            SkillOperationObservedStateV1::ObservedTrue
        );
        assert!(skills_path.join("demo/SKILL.md").is_file());
        let durable = load_ledger(&fixture.ledger, &ledger_name).unwrap();
        assert_eq!(
            durable.effects[0].state,
            SkillOperationEffectStateV1::Uncertain
        );
        assert_eq!(
            durable.effects[0].intent,
            SkillOperationEffectIntentV1::InProgress
        );

        // Resume is readback-only: the existing target is sealed against the
        // source snapshot and no second package commit is attempted.
        let mut resumed = SkillOperationPrepared::resume(
            &fixture.staging,
            &fixture.ledger,
            "post-rename-root-sync",
        )
        .unwrap();
        let reconciled = resumed
            .reconcile_package_readback(&File::open(&skills_path).unwrap(), &fixture.target)
            .unwrap();
        assert_eq!(reconciled.directory_commit, Some(true));
        assert!(!reconciled.recovery_required);
        assert!(skills_path.join("demo/SKILL.md").is_file());
    }

    #[test]
    fn post_rename_root_sync_with_uncertain_persist_failure_stays_typed() {
        let fixture = InstallFixture::new("post-rename-root-sync-persist");
        let prepared = fixture.prepare("post-rename-root-sync-persist");
        let capability = prepared.confirmation_capability().clone();
        let skills_path = fixture.data.join("orgs/org/skills");
        fail_next_post_rename_root_sync();
        fail_next_uncertain_persist();
        let receipt = prepared
            .apply(
                &fixture.data,
                &File::open(&skills_path).unwrap(),
                &capability,
                &fixture.target,
                &mut FakeOperon::default(),
            )
            .unwrap();
        assert_eq!(
            receipt.final_response.status,
            "SKILL_OPERATION_DURABILITY_UNCERTAIN"
        );
        assert_eq!(receipt.final_response.directory_commit, Some(true));
        assert!(receipt.final_response.recovery_required);
        assert_eq!(
            receipt
                .final_response
                .post_effect_observation
                .as_ref()
                .unwrap()
                .observed,
            SkillOperationObservedStateV1::ObservedTrue
        );
        assert!(skills_path.join("demo/SKILL.md").is_file());
    }

    #[test]
    fn post_effect_attach_persist_failure_reads_native_state_once_and_reconciles() {
        let fixture = InstallFixture::new("post-attach");
        let prepared = fixture.prepare("post-attach");
        let capability = prepared.confirmation_capability().clone();
        let skills = File::open(fixture.data.join("orgs/org/skills")).unwrap();
        let mut operon = FakeOperon {
            fail_persist_after_attach: true,
            ..Default::default()
        };
        let receipt = prepared
            .apply(
                &fixture.data,
                &skills,
                &capability,
                &fixture.target,
                &mut operon,
            )
            .unwrap();
        assert_eq!(operon.attach_calls, 1);
        assert_eq!(operon.readback_calls, 1);
        assert_eq!(receipt.final_response.attach_verified, Some(true));
        assert!(receipt.final_response.recovery_required);
        let mut resumed =
            SkillOperationPrepared::resume(&fixture.staging, &fixture.ledger, "post-attach")
                .unwrap();
        let mut readback = FakeOperon {
            attached: true,
            ..Default::default()
        };
        let reconciled = resumed
            .reconcile_attach_readback(&fixture.target, &mut readback)
            .unwrap();
        assert_eq!(readback.readback_calls, 1);
        assert_eq!(reconciled.attach_verified, Some(true));
    }

    #[test]
    fn post_effect_detach_persist_failure_reads_native_state_once_and_reconciles() {
        let fixture = RemovalFixture::new("post-detach");
        let prepared = fixture.prepare("post-detach", u64::MAX);
        let capability = prepared.confirmation_capability().clone();
        let mut operon = FakeOperon {
            attached: true,
            fail_persist_after_detach: true,
            ..Default::default()
        };
        let receipt = prepared
            .apply(
                &fixture.data,
                &fixture.roots,
                &capability,
                &fixture.target,
                &mut operon,
            )
            .unwrap();
        assert_eq!(operon.detach_calls, 1);
        assert_eq!(operon.readback_calls, 1);
        assert_eq!(receipt.final_response.detach_verified, Some(true));
        assert!(receipt.final_response.recovery_required);
        let mut resumed =
            SkillOperationRemovalPrepared::resume(&fixture.ledger, "post-detach").unwrap();
        let mut readback = FakeOperon {
            attached: false,
            ..Default::default()
        };
        let reconciled = resumed
            .reconcile_detach_readback(&fixture.target, &mut readback)
            .unwrap();
        assert_eq!(readback.readback_calls, 1);
        assert_eq!(reconciled.detach_verified, Some(true));
    }

    #[test]
    fn post_effect_quarantine_persist_failure_is_fd_read_back_and_reconciles() {
        let fixture = RemovalFixture::new("post-quarantine");
        let prepared = fixture.prepare("post-quarantine", u64::MAX);
        let capability = prepared.confirmation_capability().clone();
        fail_post_effect_persist_after_next_quarantine();
        let mut operon = FakeOperon {
            attached: true,
            ..Default::default()
        };
        let receipt = prepared
            .apply(
                &fixture.data,
                &fixture.roots,
                &capability,
                &fixture.target,
                &mut operon,
            )
            .unwrap();
        assert_eq!(operon.detach_calls, 1);
        assert!(!fixture.data.join("orgs/org/skills/demo").exists());
        assert_eq!(receipt.final_response.quarantine_commit, Some(true));
        assert!(receipt.final_response.recovery_required);
        let mut resumed =
            SkillOperationRemovalPrepared::resume(&fixture.ledger, "post-quarantine").unwrap();
        let reconciled = resumed
            .reconcile_quarantine_readback(&fixture.roots, &fixture.target)
            .unwrap();
        assert_eq!(reconciled.quarantine_commit, Some(true));
        assert!(!reconciled.recovery_required);
    }

    #[test]
    fn quarantine_apply_parent_sync_failure_is_visible_uncertain_until_a_restart_reconcile_syncs_both_roots(
    ) {
        let fixture = RemovalFixture::new("quarantine-apply-parent-sync");
        let prepared = fixture.prepare("quarantine-apply-parent-sync", u64::MAX);
        let ledger_name = prepared.ledger_name.clone();
        let capability = prepared.confirmation_capability().clone();
        // The source-parent sync is independent from destination-parent sync.
        // Rename succeeds first, so this must become a typed observation, not
        // a verified effect or a retryable failure.
        fail_next_quarantine_skills_root_sync();
        let receipt = prepared
            .apply(
                &fixture.data,
                &fixture.roots,
                &capability,
                &fixture.target,
                &mut FakeOperon {
                    attached: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            receipt.final_response.status,
            "SKILL_OPERATION_DURABILITY_UNCERTAIN"
        );
        assert_eq!(receipt.final_response.detach_verified, Some(true));
        assert_eq!(receipt.final_response.quarantine_commit, Some(true));
        assert!(receipt.final_response.recovery_required);
        assert_eq!(
            receipt
                .final_response
                .post_effect_observation
                .as_ref()
                .unwrap()
                .observed,
            SkillOperationObservedStateV1::ObservedTrue
        );
        assert!(!fixture.data.join("orgs/org/skills/demo").exists());
        assert!(quarantine_snapshot_matches_at(
            &fixture.roots,
            receipt.ledger.removal.as_ref().unwrap()
        )
        .unwrap());
        let durable = load_ledger(&fixture.ledger, &ledger_name).unwrap();
        assert_eq!(
            durable.effects[1].state,
            SkillOperationEffectStateV1::Uncertain
        );
        assert_eq!(
            durable.effects[1].intent,
            SkillOperationEffectIntentV1::InProgress
        );

        // A restart reconcile does not rename. A failed destination-parent
        // sync leaves the same sealed transition uncertain, even though its
        // held-FD readback remains true.
        let mut resumed =
            SkillOperationRemovalPrepared::resume(&fixture.ledger, "quarantine-apply-parent-sync")
                .unwrap();
        fail_next_quarantine_root_sync();
        let uncertain = resumed
            .reconcile_quarantine_readback(&fixture.roots, &fixture.target)
            .unwrap();
        assert_eq!(uncertain.status, "REMOVAL_STATE_UNCERTAIN");
        assert_eq!(uncertain.quarantine_commit, None);
        assert!(uncertain.recovery_required);
        assert!(!fixture.data.join("orgs/org/skills/demo").exists());
        drop(resumed);

        // Once both currently-held parent descriptors sync successfully, the
        // exact already-visible transition can be sealed. No source leaf is
        // present to rename a second time.
        let mut resumed =
            SkillOperationRemovalPrepared::resume(&fixture.ledger, "quarantine-apply-parent-sync")
                .unwrap();
        let reconciled = resumed
            .reconcile_quarantine_readback(&fixture.roots, &fixture.target)
            .unwrap();
        assert_eq!(reconciled.quarantine_commit, Some(true));
        assert!(!reconciled.recovery_required);
        assert!(!fixture.data.join("orgs/org/skills/demo").exists());
    }

    #[test]
    fn quarantine_continue_parent_sync_failure_is_visible_uncertain_not_verified_or_failed() {
        let fixture = RemovalFixture::new("quarantine-continue-parent-sync");
        let mut prepared = fixture.prepare("quarantine-continue-parent-sync", u64::MAX);
        persist_verified_detach_prefix(&mut prepared);
        drop(prepared);

        let continued = SkillOperationRemovalPrepared::resume(
            &fixture.ledger,
            "quarantine-continue-parent-sync",
        )
        .unwrap();
        // Exercise the other parent fault seam through continuation. The
        // external detach prefix is already sealed and must not be replayed.
        fail_next_quarantine_root_sync();
        let receipt = continued
            .continue_apply(&fixture.data, &fixture.roots, &fixture.target)
            .unwrap();
        assert_eq!(
            receipt.final_response.status,
            "SKILL_OPERATION_DURABILITY_UNCERTAIN"
        );
        assert_eq!(receipt.final_response.detach_verified, Some(true));
        assert_eq!(receipt.final_response.quarantine_commit, Some(true));
        assert!(receipt.final_response.recovery_required);
        assert_eq!(
            receipt
                .final_response
                .post_effect_observation
                .as_ref()
                .unwrap()
                .observed,
            SkillOperationObservedStateV1::ObservedTrue
        );
        assert_eq!(
            receipt.ledger.effects[1].state,
            SkillOperationEffectStateV1::Uncertain
        );
        assert_eq!(
            receipt.ledger.effects[1].intent,
            SkillOperationEffectIntentV1::InProgress
        );
        assert!(!fixture.data.join("orgs/org/skills/demo").exists());
    }

    #[test]
    fn install_skills_root_swap_rejects_apply_continue_and_reconcile_before_native_call() {
        let fixture = InstallFixture::new("skills-root-swap");
        let skills_path = fixture.data.join("orgs/org/skills");
        let displaced = fixture.root.join("skills-displaced");
        std::fs::rename(&skills_path, &displaced).unwrap();
        std::fs::create_dir(&skills_path).unwrap();
        let swapped_root = File::open(&skills_path).unwrap();

        let apply = fixture.prepare("skills-root-swap-apply");
        let capability = apply.confirmation_capability().clone();
        let mut operon = FakeOperon::default();
        let error = apply
            .apply(
                &fixture.data,
                &swapped_root,
                &capability,
                &fixture.target,
                &mut operon,
            )
            .unwrap_err();
        assert_eq!(error.code, "SKILL_OPERATION_SKILLS_ROOT_IDENTITY_DRIFT");
        assert_eq!(operon.attach_calls, 0);

        let mut continuation = fixture.prepare("skills-root-swap-continue");
        continuation.ledger.consumed = true;
        continuation.ledger.effects[0].state = SkillOperationEffectStateV1::Verified;
        continuation.ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
        continuation.ledger.effects[0].attempt_id = Some("a".repeat(64));
        continuation.ledger.effects[0].attempt_started_at_unix_seconds = Some(1);
        continuation.ledger.effects[0].evidence_sha256 = Some("b".repeat(64));
        let mut continued_operon = FakeOperon::default();
        let error = continuation
            .continue_apply(
                &fixture.data,
                &swapped_root,
                &fixture.target,
                &mut continued_operon,
            )
            .unwrap_err();
        assert_eq!(error.code, "SKILL_OPERATION_SKILLS_ROOT_IDENTITY_DRIFT");
        assert_eq!(continued_operon.attach_calls, 0);

        let mut reconcile = fixture.prepare("skills-root-swap-reconcile");
        reconcile.ledger.consumed = true;
        reconcile.ledger.effects[0].state = SkillOperationEffectStateV1::Verified;
        reconcile.ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
        reconcile.ledger.effects[0].attempt_id = Some("a".repeat(64));
        reconcile.ledger.effects[0].attempt_started_at_unix_seconds = Some(1);
        reconcile.ledger.effects[0].evidence_sha256 = Some("b".repeat(64));
        reconcile.ledger.effects[1].intent = SkillOperationEffectIntentV1::InProgress;
        reconcile.ledger.effects[1].attempt_id = Some("c".repeat(64));
        reconcile.ledger.effects[1].attempt_started_at_unix_seconds = Some(1);
        let mut swapped_target = fixture.target.clone();
        let swapped_metadata = swapped_root.metadata().unwrap();
        swapped_target.skills_root_device = swapped_metadata.dev();
        swapped_target.skills_root_inode = swapped_metadata.ino();
        let mut reconciled_operon = FakeOperon::default();
        let error = reconcile
            .reconcile_attach_readback(&swapped_target, &mut reconciled_operon)
            .unwrap_err();
        assert_eq!(error.code, "SKILL_OPERATION_TARGET_BINDING_DRIFT");
        assert_eq!(reconciled_operon.readback_calls, 0);
    }

    #[test]
    fn payload_scan_counts_empty_directories_and_marker_dirents() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "csswitch-operation-scan-budget-{}-{}",
            std::process::id(),
            unix_seconds()
        ));
        fs::create_dir_all(root.join("empty")).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.join(crate::IMPORT_ORIGIN_FILE), b"marker").unwrap();
        let directory = File::open(&root).unwrap();

        // The marker is not a package file but it is a real dirent: a marker
        // cannot be used to evade a tiny entry cap.
        assert!(scan_payload_at_with_limits(
            &directory,
            PayloadScanLimits {
                max_files: MAX_FILES,
                max_total_bytes: MAX_TOTAL_BYTES,
                max_directories: 4,
                max_dirents: 0,
            },
        )
        .is_err());

        // The root and its otherwise empty child both consume the shared
        // directory budget; no payload files are needed to exercise it.
        assert!(scan_payload_at_with_limits(
            &directory,
            PayloadScanLimits {
                max_files: MAX_FILES,
                max_total_bytes: MAX_TOTAL_BYTES,
                max_directories: 1,
                max_dirents: 8,
            },
        )
        .is_err());
        assert!(decode_payload_entry_name(&[0xff]).is_err());
        drop(directory);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_budget_rejection_prevents_removal_native_call_and_mutation() {
        let fixture = RemovalFixture::new("scan-budget-no-native");
        let prepared = fixture.prepare("scan-budget-no-native", u64::MAX);
        let destination = prepared
            .ledger
            .removal
            .as_ref()
            .unwrap()
            .quarantine_destination
            .clone();
        let capability = prepared.confirmation_capability().clone();
        for index in 0..MAX_PAYLOAD_SCAN_DIRECTORIES {
            std::fs::create_dir(
                fixture
                    .data
                    .join(format!("orgs/org/skills/demo/empty-{index}")),
            )
            .unwrap();
        }
        let mut operon = FakeOperon {
            attached: true,
            ..Default::default()
        };
        let error = prepared
            .apply(
                &fixture.data,
                &fixture.roots,
                &capability,
                &fixture.target,
                &mut operon,
            )
            .unwrap_err();
        assert_eq!(error.code, "SKILL_OPERATION_REMOVAL_SNAPSHOT_DRIFT");
        assert_eq!(operon.detach_calls, 0);
        assert!(fixture.data.join("orgs/org/skills/demo").is_dir());
        assert!(!fixture.root.join("quarantine").join(destination).exists());
    }

    #[test]
    fn scan_recursion_error_is_raii_closed_under_a_three_fd_rlimit_margin() {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "csswitch-operation-scan-raii-{}-{}",
            std::process::id(),
            unix_seconds()
        ));
        std::fs::create_dir_all(root.join("child")).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink("missing", root.join("child/link")).unwrap();

        let child = unsafe { libc::fork() };
        assert!(child >= 0, "fork failed");
        if child == 0 {
            let directory = File::open(&root).unwrap();
            let baseline = std::fs::read_dir("/dev/fd").unwrap().count();
            let mut original = std::mem::MaybeUninit::<libc::rlimit>::zeroed();
            if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, original.as_mut_ptr()) } != 0 {
                unsafe { libc::_exit(2) };
            }
            let original = unsafe { original.assume_init() };
            // The recursive walk needs a root DIR duplicate, an open child
            // directory and a child DIR duplicate. A leaked outer DIR from
            // the first error leaves no room for the second traversal.
            let target = (baseline + 3) as libc::rlim_t;
            if original.rlim_cur < target
                || unsafe {
                    libc::setrlimit(
                        libc::RLIMIT_NOFILE,
                        &libc::rlimit {
                            rlim_cur: target,
                            rlim_max: original.rlim_max,
                        },
                    )
                } != 0
            {
                unsafe { libc::_exit(3) };
            }
            // The second call has no headroom for a leaked outer DIR stream.
            // It therefore proves recursive early return closed every stream;
            // the postcondition separately verifies the FD count.
            let first = scan_payload_at(&directory, MAX_FILES, MAX_TOTAL_BYTES).is_err();
            let second = scan_payload_at(&directory, MAX_FILES, MAX_TOTAL_BYTES).is_err();
            let final_count = std::fs::read_dir("/dev/fd").unwrap().count();
            unsafe {
                libc::_exit(if first && second && final_count == baseline {
                    0
                } else {
                    1
                });
            }
        }
        let mut status = 0;
        assert_eq!(unsafe { libc::waitpid(child, &mut status, 0) }, child);
        assert!(libc::WIFEXITED(status));
        assert_eq!(libc::WEXITSTATUS(status), 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn quarantine_uses_no_replace_and_preserves_a_destination_race_winner() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "csswitch-operation-no-replace-{}-{}",
            std::process::id(),
            unix_seconds()
        ));
        let skills = root.join("skills");
        let quarantine = root.join("quarantine");
        fs::create_dir_all(skills.join("demo")).unwrap();
        fs::create_dir_all(quarantine.join("race-winner")).unwrap();
        fs::set_permissions(&skills, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&quarantine, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(skills.join("demo").join("payload"), b"source").unwrap();
        fs::write(quarantine.join("race-winner").join("payload"), b"winner").unwrap();
        let roots = SkillRemovalRoots {
            skills_root: File::open(&skills).unwrap(),
            quarantine_root: File::open(&quarantine).unwrap(),
        };
        let identity = roots.identity().unwrap();
        let leaf = File::open(skills.join("demo")).unwrap();
        let (skill_leaf_device, skill_leaf_inode) =
            directory_object_identity(&leaf, "test").unwrap();
        let snapshot = SkillRemovalSnapshotV1 {
            skill_name: "demo".into(),
            marker_sha256: "a".repeat(64),
            content_sha256: "b".repeat(64),
            skill_leaf_device,
            skill_leaf_inode,
            skills_root_device: identity.skills_root_device,
            skills_root_inode: identity.skills_root_inode,
            quarantine_root_device: identity.quarantine_root_device,
            quarantine_root_inode: identity.quarantine_root_inode,
            quarantine_destination: "race-winner".into(),
        };
        let held = open_removal_source_leaf(&roots, "demo").unwrap();
        let error = quarantine_owned_skill_at(&roots, &snapshot, &held).unwrap_err();
        assert_eq!(error.code, "SKILL_OPERATION_REMOVAL_DESTINATION_EXISTS");
        assert!(skills.join("demo").is_dir());
        assert_eq!(
            fs::read(quarantine.join("race-winner").join("payload")).unwrap(),
            b"winner"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removal_apply_is_exact_and_expiry_or_target_drift_never_detaches() {
        let fixture = RemovalFixture::new("apply");
        assert!(removal_source_snapshot_matches_at(
            &fixture.roots,
            fixture
                .prepare("remove-apply-snapshot", u64::MAX)
                .ledger()
                .removal
                .as_ref()
                .unwrap(),
        )
        .unwrap());
        let prepared = fixture.prepare("remove-apply", u64::MAX);
        validate_ledger_kind(prepared.ledger(), "test").unwrap();
        let mut began = prepared.ledger().clone();
        began.consumed = true;
        mark_in_progress(&mut began, 0);
        assert!(
            valid_effect_receipt(&began.effects[0]),
            "{:?}",
            began.effects[0]
        );
        assert!(
            valid_effect_receipt(&began.effects[1]),
            "{:?}",
            began.effects[1]
        );
        validate_ledger_kind(&began, "test").unwrap();
        let capability = prepared.confirmation_capability().clone();
        let mut operon = FakeOperon {
            attached: true,
            ..Default::default()
        };
        let receipt = prepared
            .apply(
                &fixture.data,
                &fixture.roots,
                &capability,
                &fixture.target,
                &mut operon,
            )
            .unwrap();
        assert_eq!(receipt.final_response.status, "REMOVED_DETACHED");
        assert_eq!(receipt.final_response.detach_verified, Some(true));
        assert_eq!(receipt.final_response.quarantine_commit, Some(true));
        assert_eq!(operon.detach_calls, 1);
        assert!(!fixture.data.join("orgs/org/skills/demo").exists());

        let expired = RemovalFixture::new("expired");
        let prepared = expired.prepare("remove-expired", unix_seconds().saturating_add(1));
        std::thread::sleep(std::time::Duration::from_secs(2));
        let capability = prepared.confirmation_capability().clone();
        let mut operon = FakeOperon {
            attached: true,
            ..Default::default()
        };
        assert_eq!(
            prepared
                .apply(
                    &expired.data,
                    &expired.roots,
                    &capability,
                    &expired.target,
                    &mut operon
                )
                .unwrap_err()
                .code,
            "SKILL_OPERATION_CONFIRMATION_EXPIRED"
        );
        assert_eq!(operon.detach_calls, 0);
        assert!(expired.data.join("orgs/org/skills/demo").is_dir());

        let drift = RemovalFixture::new("drift");
        let prepared = drift.prepare("remove-drift", u64::MAX);
        let capability = prepared.confirmation_capability().clone();
        let mut changed = drift.target.clone();
        changed.active_org_identity_sha256 = "f".repeat(64);
        let mut operon = FakeOperon {
            attached: true,
            ..Default::default()
        };
        assert_eq!(
            prepared
                .apply(
                    &drift.data,
                    &drift.roots,
                    &capability,
                    &changed,
                    &mut operon
                )
                .unwrap_err()
                .code,
            "SKILL_OPERATION_TARGET_BINDING_DRIFT"
        );
        assert_eq!(operon.detach_calls, 0);
        assert!(drift.data.join("orgs/org/skills/demo").is_dir());
    }

    #[test]
    fn replacement_after_detach_never_reaches_quarantine() {
        let fixture = RemovalFixture::new("replacement-after-detach");
        let prepared = fixture.prepare("remove-replacement", u64::MAX);
        let capability = prepared.confirmation_capability().clone();
        let source = fixture.data.join("orgs/org/skills/demo");
        let quarantine = fixture.data.join("orgs/org/skills/.csswitch-quarantine");
        let mut operon = FakeOperon {
            attached: true,
            replace_on_detach: Some(source.clone()),
            ..Default::default()
        };
        assert_eq!(
            prepared
                .apply(
                    &fixture.data,
                    &fixture.roots,
                    &capability,
                    &fixture.target,
                    &mut operon,
                )
                .unwrap_err()
                .code,
            "SKILL_OPERATION_NEW_PLAN_REQUIRED"
        );
        assert_eq!(operon.detach_calls, 1);
        assert!(source.is_dir());
        assert!(!quarantine.exists() || std::fs::read_dir(quarantine).unwrap().next().is_none());
    }

    #[test]
    fn quarantine_wal_to_rename_name_replacement_never_moves_the_replacement() {
        let fixture = RemovalFixture::new("quarantine-wal-name-replacement");
        let prepared = fixture.prepare("remove-wal-name-replacement", u64::MAX);
        let capability = prepared.confirmation_capability().clone();
        let source = fixture.data.join("orgs/org/skills/demo");
        let displaced = fixture.root.join("owned-before-quarantine");
        replace_quarantine_name_after_next_intent(source.clone(), displaced.clone());
        let mut operon = FakeOperon {
            attached: true,
            ..Default::default()
        };
        let receipt = prepared
            .apply(
                &fixture.data,
                &fixture.roots,
                &capability,
                &fixture.target,
                &mut operon,
            )
            .unwrap();
        assert_eq!(operon.detach_calls, 1);
        assert!(receipt.final_response.recovery_required);
        assert!(
            source.join("SKILL.md").is_file(),
            "replacement remains named"
        );
        assert!(
            displaced.join("SKILL.md").is_file(),
            "owned leaf was not lost"
        );
        let destination = fixture.roots.quarantine_root.try_clone().unwrap();
        let snapshot = receipt.ledger.removal.as_ref().unwrap();
        let destination_name =
            std::ffi::CString::new(snapshot.quarantine_destination.as_str()).unwrap();
        let fd = unsafe {
            libc::openat(
                destination.as_raw_fd(),
                destination_name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        assert!(fd < 0, "no replacement reached the quarantine destination");
    }

    #[test]
    fn pending_readback_edges_promote_only_the_actual_successor() {
        let cases: Vec<(&str, Box<dyn Fn(&mut SkillOperationLedgerV1)>)> = vec![
            (
                "install-readback-success",
                Box::new(|ledger| {
                    ledger.effects[0].state = SkillOperationEffectStateV1::Verified;
                    ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
                    ledger.effects[0].evidence_sha256 = Some("a".repeat(64));
                }),
            ),
            (
                "package-readback-failed",
                Box::new(|ledger| {
                    ledger.effects[0].state = SkillOperationEffectStateV1::Failed;
                    ledger.effects[0].intent = SkillOperationEffectIntentV1::None;
                    ledger.effects[0].error_code = Some("NOT_OBSERVED".into());
                }),
            ),
            (
                "removal-readback-committed",
                Box::new(|ledger| {
                    ledger.effects[0].state = SkillOperationEffectStateV1::Committed;
                    ledger.effects[0].error_code = None;
                }),
            ),
        ];
        for (label, mutate) in cases {
            let fixture = RemovalFixture::new(label);
            let mut prepared = fixture.prepare(&format!("remove-{label}"), u64::MAX);
            prepared.ledger.consumed = true;
            mark_in_progress(&mut prepared.ledger, 0);
            persist(
                &prepared.ledger_root,
                &prepared.ledger_name,
                &mut prepared.ledger,
            )
            .unwrap();
            let pending = write_pending_successor(
                &fixture.ledger,
                &fixture.ledger_path,
                &prepared.ledger_name,
                &prepared.ledger,
                mutate,
            );
            drop(prepared);
            let resumed =
                SkillOperationRemovalPrepared::resume(&fixture.ledger, &format!("remove-{label}"))
                    .unwrap();
            assert!(!pending.exists());
            assert_eq!(resumed.ledger.generation, 2);
        }
    }

    #[test]
    fn persist_rejects_skip_and_base_receipt_tampering() {
        let fixture = RemovalFixture::new("persist-reject");
        let mut prepared = fixture.prepare("remove-persist-reject", u64::MAX);
        prepared.ledger.effects[1].state = SkillOperationEffectStateV1::Verified;
        prepared.ledger.effects[1].evidence_sha256 = Some("a".repeat(64));
        assert_eq!(
            persist(
                &prepared.ledger_root,
                &prepared.ledger_name,
                &mut prepared.ledger,
            )
            .unwrap_err()
            .code,
            "SKILL_OPERATION_LEDGER_TRANSITION_INVALID"
        );

        let mut tampered = prepared.ledger.clone();
        tampered.effects[0].intent = SkillOperationEffectIntentV1::InProgress;
        tampered.effects[0].state = SkillOperationEffectStateV1::Verified;
        tampered.effects[0].evidence_sha256 = Some("a".repeat(64));
        std::fs::write(
            fixture.ledger_path.join(&prepared.ledger_name),
            serde_json::to_vec(&tampered).unwrap(),
        )
        .unwrap();
        drop(prepared);
        let error =
            match SkillOperationRemovalPrepared::resume(&fixture.ledger, "remove-persist-reject") {
                Ok(_) => panic!("tampered receipt must not resume"),
                Err(error) => error,
            };
        assert_eq!(error.code, "SKILL_OPERATION_LEDGER_RECEIPT_INVALID");
    }

    #[test]
    fn removal_plan_accepts_a_owned_local_zip_skill() {
        let fixture = RemovalFixture::new("local-owned");
        let marker_path = fixture.data.join("orgs/org/skills/demo/.import-origin");
        let mut marker: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&marker_path).unwrap()).unwrap();
        marker["source_kind"] = serde_json::Value::String("local_zip".into());
        std::fs::write(&marker_path, serde_json::to_vec(&marker).unwrap()).unwrap();
        let prepared = fixture.prepare("remove-local-owned", u64::MAX);
        assert!(matches!(
            prepared.ledger.plan.source,
            crate::PlanSourceV1::InstalledOwnedSkill { .. }
        ));
        assert_eq!(
            prepared.ledger.operation_kind,
            SkillOperationKindV1::Removal
        );
    }

    #[test]
    fn removal_restart_reconcile_reads_back_without_replaying_detach() {
        let fixture = RemovalFixture::new("restart");
        let mut prepared = fixture.prepare("remove-restart", u64::MAX);
        prepared.ledger.consumed = true;
        mark_in_progress(&mut prepared.ledger, 0);
        persist(
            &prepared.ledger_root,
            &prepared.ledger_name,
            &mut prepared.ledger,
        )
        .unwrap();
        drop(prepared);
        let mut resumed =
            SkillOperationRemovalPrepared::resume(&fixture.ledger, "remove-restart").unwrap();
        let mut operon = FakeOperon {
            attached: false,
            ..Default::default()
        };
        let response = resumed
            .reconcile_detach_readback(&fixture.target, &mut operon)
            .unwrap();
        assert!(response.detach_verified.unwrap());
        assert_eq!(operon.detach_calls, 0);
        assert_eq!(operon.readback_calls, 1);
        assert!(fixture.data.join("orgs/org/skills/demo").is_dir());
    }

    #[test]
    fn reconcile_target_drift_does_not_issue_native_readback() {
        let fixture = RemovalFixture::new("reconcile-target-drift");
        let mut prepared = fixture.prepare("remove-reconcile-target-drift", u64::MAX);
        prepared.ledger.consumed = true;
        mark_in_progress(&mut prepared.ledger, 0);
        persist(
            &prepared.ledger_root,
            &prepared.ledger_name,
            &mut prepared.ledger,
        )
        .unwrap();
        let mut changed = fixture.target.clone();
        changed.data_dir_identity_sha256 = "e".repeat(64);
        let mut operon = FakeOperon::default();
        assert_eq!(
            prepared
                .reconcile_detach_readback(&changed, &mut operon)
                .unwrap_err()
                .code,
            "SKILL_OPERATION_TARGET_BINDING_DRIFT"
        );
        assert_eq!(operon.readback_calls, 0);
    }

    #[test]
    fn tampered_quarantine_destination_never_detaches() {
        let fixture = RemovalFixture::new("tampered-destination");
        let mut prepared = fixture.prepare("remove-tampered-destination", u64::MAX);
        prepared
            .ledger
            .removal
            .as_mut()
            .unwrap()
            .quarantine_destination = "../foreign".into();
        let capability = prepared.confirmation_capability().clone();
        let mut operon = FakeOperon {
            attached: true,
            ..Default::default()
        };
        assert_eq!(
            prepared
                .apply(
                    &fixture.data,
                    &fixture.roots,
                    &capability,
                    &fixture.target,
                    &mut operon,
                )
                .unwrap_err()
                .code,
            "SKILL_OPERATION_LEDGER_SUBJECT_INVALID"
        );
        assert_eq!(operon.detach_calls, 0);
        assert!(fixture.data.join("orgs/org/skills/demo").is_dir());
    }

    #[test]
    fn complete_pending_successor_is_promoted_but_stale_pending_is_rejected() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let fixture = RemovalFixture::new("pending-promote");
        let prepared = fixture.prepare("remove-pending", u64::MAX);
        let ledger_name = prepared.ledger_name.clone();
        let mut successor = prepared.ledger.clone();
        successor.generation = 1;
        successor.previous_ledger_sha256 = Some(digest(
            &read_owned_ledger_bytes(&fixture.ledger, &ledger_name).unwrap(),
        ));
        successor.consumed = true;
        mark_in_progress(&mut successor, 0);
        let pending = fixture.ledger_path.join(format!(".{ledger_name}.pending"));
        fs::write(&pending, serde_json::to_vec(&successor).unwrap()).unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        drop(prepared);
        let resumed =
            SkillOperationRemovalPrepared::resume(&fixture.ledger, "remove-pending").unwrap();
        assert_eq!(resumed.ledger.generation, 1);
        assert!(!pending.exists());
        drop(resumed);

        let fixture = RemovalFixture::new("pending-reject");
        let prepared = fixture.prepare("remove-pending-bad", u64::MAX);
        let ledger_name = prepared.ledger_name.clone();
        let mut stale = prepared.ledger.clone();
        stale.generation = 9;
        stale.previous_ledger_sha256 = Some("f".repeat(64));
        let pending = fixture.ledger_path.join(format!(".{ledger_name}.pending"));
        fs::write(&pending, serde_json::to_vec(&stale).unwrap()).unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        drop(prepared);
        let error =
            match SkillOperationRemovalPrepared::resume(&fixture.ledger, "remove-pending-bad") {
                Ok(_) => panic!("stale pending must not be promoted"),
                Err(error) => error,
            };
        assert_eq!(
            error.code,
            "SKILL_OPERATION_LEDGER_PENDING_RECONCILE_REQUIRED"
        );
        assert!(pending.exists());
    }

    #[test]
    fn pending_ledger_cannot_skip_to_verified_effect() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let fixture = RemovalFixture::new("pending-skip");
        let prepared = fixture.prepare("remove-pending-skip", u64::MAX);
        let ledger_name = prepared.ledger_name.clone();
        let mut forged = prepared.ledger.clone();
        forged.generation = 1;
        forged.previous_ledger_sha256 = Some(digest(
            &read_owned_ledger_bytes(&fixture.ledger, &ledger_name).unwrap(),
        ));
        forged.consumed = true;
        forged.effects[1].state = SkillOperationEffectStateV1::Verified;
        forged.effects[1].evidence_sha256 = Some("a".repeat(64));
        let pending = fixture.ledger_path.join(format!(".{ledger_name}.pending"));
        fs::write(&pending, serde_json::to_vec(&forged).unwrap()).unwrap();
        fs::set_permissions(&pending, fs::Permissions::from_mode(0o600)).unwrap();
        drop(prepared);
        let error =
            match SkillOperationRemovalPrepared::resume(&fixture.ledger, "remove-pending-skip") {
                Ok(_) => panic!("skipped pending effect must not be promoted"),
                Err(error) => error,
            };
        assert_eq!(
            error.code,
            "SKILL_OPERATION_LEDGER_PENDING_RECONCILE_REQUIRED"
        );
        assert!(pending.exists());
    }

    #[test]
    fn lifecycle_reservation_is_pre_download_bounded_and_expiry_reclaims_it() {
        let fixture = InstallFixture::new("lifecycle-reservation");
        for index in 0..MAX_ACTIVE_INSTALL_PLANS {
            reserve_install_plan_capacity(
                &fixture.staging,
                &fixture.ledger,
                &format!("reserve-{index}"),
                u64::MAX,
            )
            .unwrap();
        }
        assert_eq!(
            reserve_install_plan_capacity(
                &fixture.staging,
                &fixture.ledger,
                "reserve-over-capacity",
                u64::MAX,
            )
            .unwrap_err()
            .code,
            "SKILL_OPERATION_PLAN_CAPACITY_EXHAUSTED"
        );

        let expired = InstallFixture::new("lifecycle-expiry");
        reserve_install_plan_capacity(&expired.staging, &expired.ledger, "expired-reservation", 0)
            .unwrap();
        reserve_install_plan_capacity(
            &expired.staging,
            &expired.ledger,
            "replacement-reservation",
            u64::MAX,
        )
        .unwrap();
        assert!(!expired
            .root
            .join("ledger")
            .join(reservation_name("expired-reservation"))
            .exists());

        let unstaged = InstallFixture::new("lifecycle-unstaged-release");
        reserve_install_plan_capacity(
            &unstaged.staging,
            &unstaged.ledger,
            "resolver-reject",
            u64::MAX,
        )
        .unwrap();
        release_unstaged_install_plan_reservation(&unstaged.ledger, "resolver-reject").unwrap();
        assert!(!unstaged
            .root
            .join("ledger")
            .join(reservation_name("resolver-reject"))
            .exists());
    }

    #[test]
    fn expired_reserved_cleanup_reclaims_only_absent_or_known_partial_staging() {
        use std::os::unix::fs::PermissionsExt;

        // The downloader can stop before mkdirat.  This retains the existing
        // direct reservation retirement path and proves it still frees one of
        // the four bounded admission slots.
        let before_mkdir = InstallFixture::new("reserved-before-mkdir");
        set_test_unix_seconds(Some(30_000));
        reserve_install_plan_capacity(
            &before_mkdir.staging,
            &before_mkdir.ledger,
            "before-mkdir",
            30_001,
        )
        .unwrap();
        set_test_unix_seconds(Some(30_002));
        reserve_install_plan_capacity(
            &before_mkdir.staging,
            &before_mkdir.ledger,
            "before-mkdir-trigger",
            u64::MAX,
        )
        .unwrap();
        assert!(!before_mkdir
            .root
            .join("ledger")
            .join(reservation_name("before-mkdir"))
            .exists());

        // The process can also stop after mkdirat and a partial archive write
        // but before the receipt.  These are the only pre-receipt entries the
        // resolver may remove.
        let mid_write = InstallFixture::new("reserved-mid-write");
        set_test_unix_seconds(Some(31_000));
        reserve_install_plan_capacity(&mid_write.staging, &mid_write.ledger, "mid-write", 31_001)
            .unwrap();
        let child = mid_write.root.join("staging").join("exact-mid-write");
        std::fs::create_dir(&child).unwrap();
        std::fs::set_permissions(&child, std::fs::Permissions::from_mode(0o700)).unwrap();
        let archive = child.join("archive.zip");
        std::fs::write(&archive, b"partial archive").unwrap();
        std::fs::set_permissions(&archive, std::fs::Permissions::from_mode(0o600)).unwrap();
        set_test_unix_seconds(Some(31_002));
        reserve_install_plan_capacity(
            &mid_write.staging,
            &mid_write.ledger,
            "mid-write-trigger",
            u64::MAX,
        )
        .unwrap();
        assert!(!child.exists());
        assert!(!mid_write
            .root
            .join("ledger")
            .join(reservation_name("mid-write"))
            .exists());
        set_test_unix_seconds(None);
    }

    #[test]
    fn expired_reserved_cleanup_preserves_a_hostile_extra_entry_without_partial_deletion() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = InstallFixture::new("reserved-hostile-extra");
        set_test_unix_seconds(Some(32_000));
        reserve_install_plan_capacity(&fixture.staging, &fixture.ledger, "hostile-extra", 32_001)
            .unwrap();
        let child = fixture.root.join("staging").join("exact-hostile-extra");
        std::fs::create_dir(&child).unwrap();
        std::fs::set_permissions(&child, std::fs::Permissions::from_mode(0o700)).unwrap();
        let archive = child.join("archive.zip");
        let foreign = child.join("foreign");
        std::fs::write(&archive, b"known partial").unwrap();
        std::fs::write(&foreign, b"must remain").unwrap();
        for path in [&archive, &foreign] {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        set_test_unix_seconds(Some(32_002));
        assert_eq!(
            reserve_install_plan_capacity(
                &fixture.staging,
                &fixture.ledger,
                "hostile-extra-trigger",
                u64::MAX,
            )
            .unwrap_err()
            .code,
            "STAGING_PARTIAL_UNEXPECTED_ENTRY"
        );
        assert_eq!(std::fs::read(&archive).unwrap(), b"known partial");
        assert_eq!(std::fs::read(&foreign).unwrap(), b"must remain");
        assert!(fixture
            .root
            .join("ledger")
            .join(reservation_name("hostile-extra"))
            .exists());
        set_test_unix_seconds(None);
    }

    #[test]
    fn source_owned_recovery_classification_keeps_pre_effect_rejections_final() {
        for code in [
            "SKILL_OPERATION_LEDGER_NOT_FOUND",
            "SKILL_OPERATION_CONFIRMATION_CAPABILITY_INVALID",
            "SKILL_OPERATION_CONFIRMATION_EXPIRED",
            "SKILL_OPERATION_TARGET_BINDING_DRIFT",
        ] {
            assert!(!SkillOperationError::new(code, "recovery").recovery_required());
        }
        assert!(
            SkillOperationError::new("SKILL_OPERATION_LEDGER_INVALID", "recovery")
                .recovery_required()
        );
        assert!(
            !SkillOperationError::new("SKILL_OPERATION_TARGET_ORG_DRIFT", "preflight")
                .recovery_required()
        );
    }

    #[test]
    fn expired_exact_install_gc_tombstones_then_reclaims_staging_and_ledger() {
        let fixture = InstallFixture::new("lifecycle-gc");
        let prepared =
            fixture.prepare_with_expiry("expired-exact-plan", unix_seconds().saturating_add(1));
        let ledger_name = complete_install(&fixture, prepared).ledger.operation_id + LEDGER_SUFFIX;
        std::thread::sleep(Duration::from_secs(2));

        reserve_install_plan_capacity(&fixture.staging, &fixture.ledger, "gc-trigger", u64::MAX)
            .unwrap();
        assert!(!fixture.root.join("ledger").join(ledger_name).exists());
        assert_eq!(
            std::fs::read_dir(fixture.root.join("staging"))
                .unwrap()
                .count(),
            0,
            "only the exact staged child may be removed"
        );
    }

    #[test]
    fn expired_terminal_removal_gc_retires_only_the_ledger() {
        let fixture = RemovalFixture::new("terminal-removal-gc");
        let prepared =
            fixture.prepare("expired-terminal-removal", unix_seconds().saturating_add(1));
        let ledger_name = prepared.ledger_name.clone();
        let capability = prepared.confirmation_capability().clone();
        let receipt = prepared
            .apply(
                &fixture.data,
                &fixture.roots,
                &capability,
                &fixture.target,
                &mut FakeOperon::default(),
            )
            .unwrap();
        assert_eq!(receipt.final_response.status, "REMOVED_DETACHED");
        std::thread::sleep(Duration::from_secs(2));

        // The shared collector receives a private placeholder staging root,
        // but a removal has no archive identity and therefore cannot remove
        // anything below it.
        reserve_install_plan_capacity(
            &fixture.roots.quarantine_root,
            &fixture.ledger,
            "terminal-removal-gc-trigger",
            u64::MAX,
        )
        .unwrap();
        assert!(!fixture.ledger_path.join(ledger_name).exists());
        assert_eq!(
            std::fs::read_dir(fixture.root.join("quarantine"))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn removal_prepare_obeys_the_shared_mixed_lifecycle_entry_cap() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = InstallFixture::new("mixed-removal-capacity");
        let install = fixture.prepare_with_expiry("mixed-terminal-install", u64::MAX);
        complete_install(&fixture, install);

        let quarantine = fixture.root.join("mixed-removal-quarantine");
        std::fs::create_dir(&quarantine).unwrap();
        std::fs::set_permissions(
            fixture.data.join("orgs/org/skills"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        std::fs::set_permissions(&quarantine, std::fs::Permissions::from_mode(0o700)).unwrap();
        let roots = SkillRemovalRoots {
            skills_root: File::open(fixture.data.join("orgs/org/skills")).unwrap(),
            quarantine_root: File::open(&quarantine).unwrap(),
        };
        for index in 0..(MAX_ACTIVE_INSTALL_PLANS - 1) {
            let prepared = SkillOperationRemovalPrepared::prepare(
                &fixture.staging,
                &fixture.ledger,
                &fixture.data,
                &roots,
                format!("mixed-removal-{index}"),
                u64::MAX,
                fixture.target.clone(),
                "demo".into(),
            )
            .unwrap();
            drop(prepared);
        }
        let error = match SkillOperationRemovalPrepared::prepare(
            &fixture.staging,
            &fixture.ledger,
            &fixture.data,
            &roots,
            "mixed-removal-over-capacity".into(),
            u64::MAX,
            fixture.target.clone(),
            "demo".into(),
        ) {
            Ok(_) => panic!("fifth mixed lifecycle entry must not be admitted"),
            Err(error) => error,
        };
        assert_eq!(error.code, "SKILL_OPERATION_PLAN_CAPACITY_EXHAUSTED");
    }

    #[test]
    fn removal_prepare_gc_uses_the_actual_install_staging_root() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = InstallFixture::new("removal-triggered-install-gc");
        set_test_unix_seconds(Some(41_000));
        let install = fixture.prepare_with_expiry("expired-install-for-removal", 41_001);
        let receipt = complete_install(&fixture, install);
        let install_ledger = format!("{}{}", receipt.ledger.operation_id, LEDGER_SUFFIX);
        assert_eq!(
            std::fs::read_dir(fixture.root.join("staging"))
                .unwrap()
                .count(),
            1
        );

        let quarantine = fixture.root.join("removal-triggered-quarantine");
        std::fs::create_dir(&quarantine).unwrap();
        std::fs::set_permissions(
            fixture.data.join("orgs/org/skills"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        std::fs::set_permissions(&quarantine, std::fs::Permissions::from_mode(0o700)).unwrap();
        let roots = SkillRemovalRoots {
            skills_root: File::open(fixture.data.join("orgs/org/skills")).unwrap(),
            quarantine_root: File::open(&quarantine).unwrap(),
        };
        set_test_unix_seconds(Some(41_002));
        let removal = SkillOperationRemovalPrepared::prepare(
            &fixture.staging,
            &fixture.ledger,
            &fixture.data,
            &roots,
            "removal-gc-trigger".into(),
            u64::MAX,
            fixture.target.clone(),
            "demo".into(),
        )
        .unwrap();
        assert!(!fixture.root.join("ledger").join(install_ledger).exists());
        assert_eq!(
            std::fs::read_dir(fixture.root.join("staging"))
                .unwrap()
                .count(),
            0
        );
        drop(removal);
        set_test_unix_seconds(None);
    }

    #[test]
    fn sequential_expired_terminal_operations_do_not_accumulate_past_fixed_entries() {
        let fixture = InstallFixture::new("sequential-terminal-gc");
        for index in 0..40 {
            let now = 1_000_u64.saturating_add(index * 2);
            set_test_unix_seconds(Some(now));
            let mut prepared =
                fixture.prepare_with_expiry(&format!("terminal-{index}"), now.saturating_add(1));
            prepared.begin_effect(0).unwrap();
            prepared
                .verified(0, digest(format!("package-{index}").as_bytes()))
                .unwrap();
            prepared.begin_effect(1).unwrap();
            prepared.ledger.effects[1].state = SkillOperationEffectStateV1::Failed;
            prepared.ledger.effects[1].intent = SkillOperationEffectIntentV1::None;
            prepared.ledger.effects[1].error_code = Some("TEST_TERMINAL_FAILURE".into());
            persist(
                &prepared.ledger_root,
                &prepared.ledger_name,
                &mut prepared.ledger,
            )
            .unwrap();
            drop(prepared);
        }
        set_test_unix_seconds(Some(2_000));
        reserve_install_plan_capacity(
            &fixture.staging,
            &fixture.ledger,
            "terminal-after-40",
            u64::MAX,
        )
        .unwrap();
        assert!(
            lifecycle_entry_names(&fixture.ledger).unwrap().len() <= OPERATION_LOCK_BUCKETS + 2
        );
        set_test_unix_seconds(None);
    }

    #[test]
    fn gc_tombstone_restarts_after_each_durable_cleanup_edge() {
        fn tombstone_for(fixture: &InstallFixture, ledger: &SkillOperationLedgerV1) {
            let name = format!("{}{}", ledger.operation_id, LEDGER_SUFFIX);
            let bytes = read_owned_ledger_bytes(&fixture.ledger, &name).unwrap();
            let tombstone = SkillOperationGcTombstoneV1 {
                schema: SKILL_OPERATION_LEDGER_SCHEMA.into(),
                operation_id: ledger.operation_id.clone(),
                ledger_sha256: digest(&bytes),
            };
            write_json_owned(
                create_new_at(
                    fixture.ledger.as_raw_fd(),
                    &tombstone_name(&ledger.operation_id),
                    0o600,
                    "test",
                )
                .unwrap(),
                &tombstone,
            )
            .unwrap();
            sync_directory(&fixture.ledger).unwrap();
        }
        fn remove_stage(fixture: &InstallFixture, ledger: &SkillOperationLedgerV1) {
            remove_exact_github_archive(
                &fixture.staging,
                &ledger.operation_id,
                &source_from_plan(&ledger.plan).unwrap(),
                ledger.source_archive_sha256.as_deref().unwrap(),
                ledger.staging_object_identity_sha256.as_deref().unwrap(),
                ledger.staging_receipt_sha256.as_deref().unwrap(),
            )
            .unwrap();
        }

        set_test_unix_seconds(Some(10_000));
        let expiry = 10_001;
        let first = InstallFixture::new("gc-crash-tombstone");
        let first_prepared = first.prepare_with_expiry("gc-crash-tombstone", expiry);
        let first_receipt = complete_install(&first, first_prepared);
        let first_name = format!("{}{}", first_receipt.ledger.operation_id, LEDGER_SUFFIX);
        let first_ledger = first_receipt.ledger;
        tombstone_for(&first, &first_ledger);

        let second = InstallFixture::new("gc-crash-stage");
        let second_prepared = second.prepare_with_expiry("gc-crash-stage", expiry);
        let second_receipt = complete_install(&second, second_prepared);
        let second_name = format!("{}{}", second_receipt.ledger.operation_id, LEDGER_SUFFIX);
        let second_ledger = second_receipt.ledger;
        tombstone_for(&second, &second_ledger);
        remove_stage(&second, &second_ledger);

        let third = InstallFixture::new("gc-crash-ledger");
        let third_prepared = third.prepare_with_expiry("gc-crash-ledger", expiry);
        let third_receipt = complete_install(&third, third_prepared);
        let third_name = format!("{}{}", third_receipt.ledger.operation_id, LEDGER_SUFFIX);
        let third_ledger = third_receipt.ledger;
        tombstone_for(&third, &third_ledger);
        remove_stage(&third, &third_ledger);
        remove_file_at(third.ledger.as_raw_fd(), &third_name).unwrap();
        sync_directory(&third.ledger).unwrap();

        set_test_unix_seconds(Some(10_002));
        for (fixture, trigger) in [
            (&first, "gc-edge-first"),
            (&second, "gc-edge-second"),
            (&third, "gc-edge-third"),
        ] {
            reserve_install_plan_capacity(&fixture.staging, &fixture.ledger, trigger, u64::MAX)
                .unwrap();
        }
        assert!(!first.root.join("ledger").join(first_name).exists());
        assert!(!second.root.join("ledger").join(second_name).exists());
        assert!(!third
            .root
            .join("ledger")
            .join(tombstone_name("gc-crash-ledger"))
            .exists());
        set_test_unix_seconds(None);
    }

    #[test]
    fn lifecycle_hostile_entries_and_enumeration_limit_fail_closed_without_deletion() {
        let hostile = InstallFixture::new("lifecycle-hostile");
        let hostile_path = hostile.root.join("ledger/foreign-authority");
        std::fs::write(&hostile_path, b"foreign").unwrap();
        assert_eq!(
            reserve_install_plan_capacity(
                &hostile.staging,
                &hostile.ledger,
                "hostile-reject",
                u64::MAX,
            )
            .unwrap_err()
            .code,
            "SKILL_OPERATION_LIFECYCLE_ENTRY_INVALID"
        );
        assert!(hostile_path.exists());

        let crowded = InstallFixture::new("lifecycle-enumeration");
        for index in 0..=MAX_OPERATION_LIFECYCLE_ENTRIES {
            std::fs::write(
                crowded.root.join("ledger").join(format!("foreign-{index}")),
                b"x",
            )
            .unwrap();
        }
        assert_eq!(
            reserve_install_plan_capacity(
                &crowded.staging,
                &crowded.ledger,
                "crowded-reject",
                u64::MAX,
            )
            .unwrap_err()
            .code,
            "SKILL_OPERATION_LIFECYCLE_ENUMERATION_LIMIT"
        );
        assert!(crowded.root.join("ledger/foreign-0").exists());
    }

    #[test]
    fn lifecycle_skips_an_expired_plan_while_its_operation_lock_is_held() {
        let fixture = InstallFixture::new("lifecycle-active-lock");
        let prepared =
            fixture.prepare_with_expiry("active-expired-plan", unix_seconds().saturating_add(1));
        let ledger_name = prepared.ledger_name.clone();
        std::thread::sleep(Duration::from_secs(2));
        reserve_install_plan_capacity(
            &fixture.staging,
            &fixture.ledger,
            "active-trigger",
            u64::MAX,
        )
        .unwrap();
        assert!(fixture.root.join("ledger").join(ledger_name).exists());
        drop(prepared);
    }

    #[test]
    fn operation_locks_use_a_fixed_bucket_and_collision_serializes() {
        let fixture = InstallFixture::new("lifecycle-buckets");
        let first = "bucket-first";
        let bucket = operation_lock_bucket_name(first);
        let second = (0..4096)
            .map(|index| format!("bucket-collision-{index}"))
            .find(|candidate| candidate != first && operation_lock_bucket_name(candidate) == bucket)
            .expect("SHA-256 bucket collision in bounded fixture search");
        let held = acquire_operation_lock(&fixture.ledger, first).unwrap();
        assert_eq!(
            acquire_operation_lock(&fixture.ledger, &second)
                .unwrap_err()
                .code,
            "SKILL_OPERATION_OPERATION_BUSY"
        );
        drop(held);
        let reacquired = acquire_operation_lock(&fixture.ledger, &second).unwrap();
        let entries: Vec<_> = std::fs::read_dir(fixture.root.join("ledger"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.contains(".skill-operation.bucket-"))
            .collect();
        assert_eq!(entries, vec![bucket]);
        drop(reacquired);
    }

    #[test]
    fn lifecycle_never_collects_consumed_intent_uncertain_removal_or_identity_drift() {
        let expiry = unix_seconds().saturating_add(1);

        let intent = InstallFixture::new("lifecycle-intent");
        let mut intent_prepared = intent.prepare_with_expiry("intent-plan", expiry);
        let intent_name = intent_prepared.ledger_name.clone();
        intent_prepared.begin_effect(0).unwrap();
        drop(intent_prepared);

        let uncertain = InstallFixture::new("lifecycle-uncertain");
        let mut uncertain_prepared = uncertain.prepare_with_expiry("uncertain-plan", expiry);
        let uncertain_name = uncertain_prepared.ledger_name.clone();
        uncertain_prepared.begin_effect(0).unwrap();
        uncertain_prepared.reconcile_uncertain(0, None).unwrap();
        drop(uncertain_prepared);

        let drift = InstallFixture::new("lifecycle-drift");
        let drift_prepared = drift.prepare_with_expiry("drift-plan", expiry);
        let drift_name = drift_prepared.ledger_name.clone();
        let mut forged = drift_prepared.ledger.clone();
        forged.source_archive_sha256 = Some("f".repeat(64));
        drop(drift_prepared);
        std::fs::write(
            drift.root.join("ledger").join(&drift_name),
            serde_json::to_vec(&forged).unwrap(),
        )
        .unwrap();

        let removal = RemovalFixture::new("lifecycle-removal");
        let removal_prepared = removal.prepare("removal-plan", expiry);
        let removal_name = removal_prepared.ledger_name.clone();
        drop(removal_prepared);

        std::thread::sleep(Duration::from_secs(2));
        for (staging, ledger_root, operation_id) in [
            (&intent.staging, &intent.ledger, "intent-trigger"),
            (&uncertain.staging, &uncertain.ledger, "uncertain-trigger"),
        ] {
            reserve_install_plan_capacity(staging, ledger_root, operation_id, u64::MAX).unwrap();
        }
        assert_eq!(
            reserve_install_plan_capacity(
                &drift.staging,
                &drift.ledger,
                "drift-trigger",
                u64::MAX,
            )
            .unwrap_err()
            .code,
            "SKILL_OPERATION_LEDGER_SUBJECT_INVALID"
        );
        // A removal ledger has no staging archive, but it must remain opaque
        // to install lifecycle GC even after expiry.
        reserve_install_plan_capacity(
            &removal.roots.quarantine_root,
            &removal.ledger,
            "removal-trigger",
            u64::MAX,
        )
        .unwrap();
        assert!(intent.root.join("ledger").join(intent_name).exists());
        assert!(uncertain.root.join("ledger").join(uncertain_name).exists());
        assert!(drift.root.join("ledger").join(drift_name).exists());
        assert!(removal.ledger_path.join(removal_name).exists());
    }
}
