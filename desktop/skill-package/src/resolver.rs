//! Exact GitHub archive inspection and private staging boundary.
//!
//! The caller supplies bytes already obtained for an immutable 40-hex GitHub
//! commit. This module does not trust a mutable path or execute package
//! content: it creates a new private directory relative to an already-opened
//! trusted root, durably writes the archive, reads the same staged object back,
//! runs the inspect-only adapter, and binds the resulting content identity into
//! a confirmable plan. These bytes do not by themselves prove remote GitHub
//! provenance, so this module owns neither the production trusted caller nor
//! operation coordination: those live in the semantic operation ledger and
//! Gateway. Capability issuance and confirmation consumption are likewise
//! ledger-coordinator responsibilities, never resolver responsibilities.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::inspection::{inspect_github_skill_archive, GithubInspectionSource, InspectionReportV1};
use crate::plan::{
    build_confirmable_skill_plan, ConfirmablePlanRequestV1, PlanSourceV1, SkillPlanV1,
    SourceBindingV1,
};
use crate::MAX_ARCHIVE_BYTES;

pub const EXACT_STAGED_ARCHIVE_SCHEMA: &str = "csswitch.exact-staged-archive.v1";
const STAGING_RECEIPT_FILE: &str = "staging-receipt.v1.json";
const STAGED_ARCHIVE_FILE: &str = "archive.zip";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExactStagedArchiveIdentityV1 {
    pub schema: String,
    pub source: PlanSourceV1,
    pub archive_sha256: String,
    pub staging_object_identity_sha256: String,
}

#[derive(Debug)]
pub struct ExactStagedGithubArchive {
    identity: ExactStagedArchiveIdentityV1,
    inspection: InspectionReportV1,
    root: File,
    directory: File,
    archive: File,
    receipt: File,
    directory_name: String,
    root_object: ObjectIdentityV1,
    directory_object: ObjectIdentityV1,
    archive_object: ObjectIdentityV1,
    receipt_object: ObjectIdentityV1,
    receipt_sha256: String,
}

impl ExactStagedGithubArchive {
    pub fn identity(&self) -> &ExactStagedArchiveIdentityV1 {
        &self.identity
    }

    pub fn inspection(&self) -> &InspectionReportV1 {
        &self.inspection
    }

    pub(crate) fn receipt_sha256(&self) -> &str {
        &self.receipt_sha256
    }

    /// Re-reads the same opened archive after all retained-object identity and
    /// receipt checks.  This is crate-private so only the durable Skill operation
    /// coordinator can materialise the snapshot; callers never receive a
    /// mutable staging pathname.
    pub(crate) fn archive_bytes(&self) -> Result<Vec<u8>, ResolveError> {
        self.verify_staged_archive()?;
        read_exact_archive(&self.archive, self.archive_object.size as usize)
    }

    pub fn build_confirmable_plan(
        &self,
        request: &ConfirmablePlanRequestV1,
    ) -> Result<SkillPlanV1, ResolveError> {
        self.verify_staged_archive()?;
        let bytes = self.archive_bytes()?;
        let PlanSourceV1::GithubExact { path, repo, .. } = &self.identity.source else {
            return Err(ResolveError::new("EXACT_SOURCE_REPORT_MISMATCH", "plan"));
        };
        let package =
            match crate::archive::package_or_bundle_from_github_archive(&bytes, path, repo)
                .map_err(|error| ResolveError::new(&error.code, &error.phase))?
            {
                crate::ValidatedArchive::Skill(package) => package,
                _ => return Err(ResolveError::new("CONFIRMABLE_PROJECTION_REQUIRED", "plan")),
            };
        let plan = build_confirmable_skill_plan(
            &self.inspection,
            self.identity.source.clone(),
            request,
            package.content_sha256,
        )
        .map_err(|error| ResolveError::new(&error.code, "plan"))?;
        if !matches!(
            plan.effects.get(1).map(|effect| &effect.subject),
            Some(crate::PlanEffectSubjectV1::OperonSkill { skill_name, .. })
                if skill_name == &package.skill_name
        ) {
            return Err(ResolveError::new("CONFIRMABLE_SELECTION_MISMATCH", "plan"));
        }
        Ok(plan)
    }

    fn verify_staged_archive(&self) -> Result<(), ResolveError> {
        let root = object_identity(self.root.as_raw_fd(), ObjectKind::Directory)?;
        if !same_directory_identity(&root, &self.root_object) {
            return Err(ResolveError::new(
                "STAGED_OBJECT_IDENTITY_DRIFT",
                "readback",
            ));
        }
        let directory_path_object = object_identity_at(
            self.root.as_raw_fd(),
            &self.directory_name,
            ObjectKind::Directory,
        )?;
        let archive_path_object = object_identity_at(
            self.directory.as_raw_fd(),
            STAGED_ARCHIVE_FILE,
            ObjectKind::RegularFile,
        )?;
        let receipt_path_object = object_identity_at(
            self.directory.as_raw_fd(),
            STAGING_RECEIPT_FILE,
            ObjectKind::RegularFile,
        )?;
        if directory_path_object != self.directory_object
            || archive_path_object != self.archive_object
            || receipt_path_object != self.receipt_object
        {
            return Err(ResolveError::new(
                "STAGED_OBJECT_IDENTITY_DRIFT",
                "readback",
            ));
        }
        let directory = object_identity(self.directory.as_raw_fd(), ObjectKind::Directory)?;
        let archive_object = object_identity(self.archive.as_raw_fd(), ObjectKind::RegularFile)?;
        let receipt_object = object_identity(self.receipt.as_raw_fd(), ObjectKind::RegularFile)?;
        if directory != self.directory_object
            || archive_object != self.archive_object
            || receipt_object != self.receipt_object
        {
            return Err(ResolveError::new(
                "STAGED_OBJECT_IDENTITY_DRIFT",
                "readback",
            ));
        }
        let bytes = read_exact_archive(&self.archive, self.archive_object.size as usize)?;
        if sha256_hex(&bytes) != self.identity.archive_sha256 {
            return Err(ResolveError::new(
                "STAGED_ARCHIVE_CONTENT_DRIFT",
                "readback",
            ));
        }
        let receipt = read_exact_archive(&self.receipt, self.receipt_object.size as usize)?;
        if sha256_hex(&receipt) != self.receipt_sha256 {
            return Err(ResolveError::new(
                "STAGING_RECEIPT_CONTENT_DRIFT",
                "readback",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveError {
    pub code: String,
    pub phase: String,
}

impl ResolveError {
    fn new(code: &str, phase: &str) -> Self {
        Self {
            code: code.into(),
            phase: phase.into(),
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} during {}", self.code, self.phase)
    }
}

impl std::error::Error for ResolveError {}

/// Stages and inspects an archive for one immutable GitHub commit. `root` must
/// be an already-opened, current-user-owned mode-0700 directory. The returned
/// handle retains the opened staged objects. Dropping it only closes handles;
/// cleanup remains reserved for a later durable coordinator/ledger rather than
/// using a check-then-unlink pathname sequence.
#[allow(dead_code)] // Sealed until a trusted in-module GitHub resolver can construct this handle.
pub(crate) fn stage_inspect_exact_github_archive(
    root: &File,
    invocation_id: &str,
    source: &GithubInspectionSource,
    archive_bytes: &[u8],
) -> Result<ExactStagedGithubArchive, ResolveError> {
    validate_invocation_id(invocation_id)?;
    let root_object = object_identity(root.as_raw_fd(), ObjectKind::Directory)?;
    if root_object.uid != effective_uid() || root_object.mode & 0o7777 != 0o700 {
        return Err(ResolveError::new("UNSAFE_STAGING_ROOT", "staging"));
    }
    if archive_bytes.is_empty() || archive_bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(ResolveError::new("ARCHIVE_SIZE_INVALID", "staging"));
    }
    let archive_sha256 = sha256_hex(archive_bytes);
    let directory_name = exact_staging_directory_name(invocation_id)?;
    mkdir_at(root.as_raw_fd(), &directory_name, 0o700)?;
    // The staged child itself changes directory metadata such as size.  Bind
    // the post-create root object (not the pre-create observation) so restart
    // recovery recomputes the same sealed object identity.
    let root_object = object_identity(root.as_raw_fd(), ObjectKind::Directory)?;
    let directory = open_directory_at(root.as_raw_fd(), &directory_name)?;
    let initial_directory_object = object_identity(directory.as_raw_fd(), ObjectKind::Directory)?;
    if initial_directory_object.uid != effective_uid()
        || initial_directory_object.mode & 0o7777 != 0o700
    {
        return Err(ResolveError::new("UNSAFE_STAGING_DIRECTORY", "staging"));
    }
    let mut archive = create_regular_at(directory.as_raw_fd(), STAGED_ARCHIVE_FILE, 0o600)?;
    archive
        .write_all(archive_bytes)
        .map_err(|_| ResolveError::new("STAGED_ARCHIVE_WRITE_FAILED", "staging"))?;
    archive
        .sync_all()
        .map_err(|_| ResolveError::new("STAGED_ARCHIVE_SYNC_FAILED", "staging"))?;
    let archive_object = object_identity(archive.as_raw_fd(), ObjectKind::RegularFile)?;
    if archive_object.uid != effective_uid()
        || archive_object.mode & 0o7777 != 0o600
        || archive_object.size as usize != archive_bytes.len()
    {
        return Err(ResolveError::new("UNSAFE_STAGED_ARCHIVE", "staging"));
    }
    let staged_bytes = read_exact_archive(&archive, archive_bytes.len())?;
    if staged_bytes != archive_bytes || sha256_hex(&staged_bytes) != archive_sha256 {
        return Err(ResolveError::new(
            "STAGED_ARCHIVE_READBACK_MISMATCH",
            "readback",
        ));
    }
    let inspection = inspect_github_skill_archive(source, &staged_bytes)
        .map_err(|error| ResolveError::new(&error.code, "inspection"))?;
    let plan_source = PlanSourceV1::GithubExact {
        owner: inspection.source_claim.owner.clone(),
        repo: inspection.source_claim.repo.clone(),
        resolved_commit_sha: inspection.source_claim.commit_sha.clone(),
        path: inspection.source_claim.path.clone(),
        archive_sha256: archive_sha256.clone(),
        content_sha256: inspection.package.content_sha256.clone(),
        materialized_content_sha256: String::new(),
        binding: SourceBindingV1::CsswitchExactContentBound,
    };
    let mut receipt_file = create_regular_at(directory.as_raw_fd(), STAGING_RECEIPT_FILE, 0o600)?;
    let directory_object = object_identity(directory.as_raw_fd(), ObjectKind::Directory)?;
    // The shared staging root is intentionally not content-private to an
    // operation: preparing a second sibling changes its directory size/link
    // metadata. Bind only the stable authority identity here. The operation's
    // own child, archive and receipt remain fully identity-bound below.
    let root_identity = stable_staging_root_identity(&root_object);
    let staging_object_identity_sha256 = digest_json(&StagingObjectIdentityInputV1 {
        schema: EXACT_STAGED_ARCHIVE_SCHEMA,
        root: &root_identity,
        directory: &directory_object,
        archive: &archive_object,
        archive_sha256: &archive_sha256,
        content_sha256: &inspection.package.content_sha256,
    })?;
    let identity = ExactStagedArchiveIdentityV1 {
        schema: EXACT_STAGED_ARCHIVE_SCHEMA.into(),
        source: plan_source,
        archive_sha256,
        staging_object_identity_sha256,
    };
    let receipt = serde_json::to_vec(&identity)
        .map_err(|_| ResolveError::new("STAGING_RECEIPT_SERIALIZATION_FAILED", "staging"))?;
    let receipt_sha256 = sha256_hex(&receipt);
    receipt_file
        .write_all(&receipt)
        .map_err(|_| ResolveError::new("STAGING_RECEIPT_WRITE_FAILED", "staging"))?;
    receipt_file
        .sync_all()
        .map_err(|_| ResolveError::new("STAGING_RECEIPT_SYNC_FAILED", "staging"))?;
    let receipt_object = object_identity(receipt_file.as_raw_fd(), ObjectKind::RegularFile)?;
    if receipt_object.uid != effective_uid() || receipt_object.mode & 0o7777 != 0o600 {
        return Err(ResolveError::new("UNSAFE_STAGING_RECEIPT", "staging"));
    }
    directory
        .sync_all()
        .map_err(|_| ResolveError::new("STAGING_DIRECTORY_SYNC_FAILED", "staging"))?;
    root.sync_all()
        .map_err(|_| ResolveError::new("STAGING_ROOT_SYNC_FAILED", "staging"))?;

    Ok(ExactStagedGithubArchive {
        identity,
        inspection,
        root: root
            .try_clone()
            .map_err(|_| ResolveError::new("STAGING_ROOT_CLONE_FAILED", "staging"))?,
        directory,
        archive,
        receipt: receipt_file,
        directory_name,
        root_object,
        directory_object,
        archive_object,
        receipt_object,
        receipt_sha256,
    })
}

/// Reopens a sealed exact staged archive snapshot after a coordinator process restart.  The
/// caller supplies the same already-open trusted root and exact source tuple;
/// every pathname is reopened no-follow and the receipt, object identity,
/// archive bytes and inspection identity are reconstructed before returning a
/// usable handle.
pub(crate) fn reopen_exact_github_archive(
    root: &File,
    invocation_id: &str,
    source: &GithubInspectionSource,
    archive_sha256: &str,
    staging_object_identity_sha256: &str,
    receipt_sha256: &str,
) -> Result<ExactStagedGithubArchive, ResolveError> {
    validate_invocation_id(invocation_id)?;
    let root_object = object_identity(root.as_raw_fd(), ObjectKind::Directory)?;
    if root_object.uid != effective_uid() || root_object.mode & 0o7777 != 0o700 {
        return Err(ResolveError::new("UNSAFE_STAGING_ROOT", "recovery"));
    }
    if archive_sha256.len() != 64 || !archive_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ResolveError::new(
            "STAGING_RECOVERY_IDENTITY_INVALID",
            "recovery",
        ));
    }
    // The operation id is the pre-reserved host-owned staging name. Exact
    // receipt and archive identities still bind source content before reuse.
    let directory_name = exact_staging_directory_name(invocation_id)?;
    let directory = open_directory_at(root.as_raw_fd(), &directory_name)?;
    let receipt = open_regular_at(directory.as_raw_fd(), STAGING_RECEIPT_FILE)?;
    let receipt_object = object_identity(receipt.as_raw_fd(), ObjectKind::RegularFile)?;
    if receipt_object.size > 64 * 1024 {
        return Err(ResolveError::new("STAGING_RECEIPT_TOO_LARGE", "recovery"));
    }
    let receipt_bytes = read_exact_archive(&receipt, receipt_object.size as usize)?;
    if sha256_hex(&receipt_bytes) != receipt_sha256 {
        return Err(ResolveError::new(
            "STAGING_RECEIPT_CONTENT_DRIFT",
            "recovery",
        ));
    }
    let identity: ExactStagedArchiveIdentityV1 = serde_json::from_slice(&receipt_bytes)
        .map_err(|_| ResolveError::new("STAGING_RECEIPT_INVALID", "recovery"))?;
    let matches_source = matches!(&identity.source, PlanSourceV1::GithubExact { owner, repo, resolved_commit_sha, path, .. } if owner == &source.owner && repo == &source.repo && resolved_commit_sha == &source.commit_sha && path == &source.path);
    if !matches_source || identity.archive_sha256 != archive_sha256 {
        return Err(ResolveError::new(
            "STAGING_RECEIPT_IDENTITY_DRIFT",
            "recovery",
        ));
    }
    let archive = open_regular_at(directory.as_raw_fd(), STAGED_ARCHIVE_FILE)?;
    let archive_object = object_identity(archive.as_raw_fd(), ObjectKind::RegularFile)?;
    let directory_object = object_identity(directory.as_raw_fd(), ObjectKind::Directory)?;
    let archive_bytes = read_exact_archive(&archive, archive_object.size as usize)?;
    if sha256_hex(&archive_bytes) != identity.archive_sha256 {
        return Err(ResolveError::new(
            "STAGED_ARCHIVE_CONTENT_DRIFT",
            "recovery",
        ));
    }
    let inspection = inspect_github_skill_archive(source, &archive_bytes)
        .map_err(|error| ResolveError::new(&error.code, "recovery"))?;
    let root_identity = stable_staging_root_identity(&root_object);
    let computed = digest_json(&StagingObjectIdentityInputV1 {
        schema: EXACT_STAGED_ARCHIVE_SCHEMA,
        root: &root_identity,
        directory: &directory_object,
        archive: &archive_object,
        archive_sha256: &identity.archive_sha256,
        content_sha256: &inspection.package.content_sha256,
    })?;
    if computed != identity.staging_object_identity_sha256
        || computed != staging_object_identity_sha256
    {
        return Err(ResolveError::new(
            "STAGED_OBJECT_IDENTITY_DRIFT",
            "recovery",
        ));
    }
    Ok(ExactStagedGithubArchive {
        identity,
        inspection,
        root: root
            .try_clone()
            .map_err(|_| ResolveError::new("STAGING_ROOT_CLONE_FAILED", "recovery"))?,
        directory,
        archive,
        receipt,
        directory_name,
        root_object,
        directory_object,
        archive_object,
        receipt_object,
        receipt_sha256: sha256_hex(&receipt_bytes),
    })
}

/// Deletes one already-reopened exact staging object.  This is deliberately
/// crate-private and only the durable operation lifecycle may call it: the
/// deterministic child name alone is never deletion authority.  Every
/// identity and receipt is reopened first, then the two known regular files
/// and their private directory are removed relative to held descriptors.
pub(crate) fn remove_exact_github_archive(
    root: &File,
    invocation_id: &str,
    source: &GithubInspectionSource,
    archive_sha256: &str,
    staging_object_identity_sha256: &str,
    receipt_sha256: &str,
) -> Result<(), ResolveError> {
    let staged = reopen_exact_github_archive(
        root,
        invocation_id,
        source,
        archive_sha256,
        staging_object_identity_sha256,
        receipt_sha256,
    )?;
    staged.verify_staged_archive()?;
    for name in [STAGED_ARCHIVE_FILE, STAGING_RECEIPT_FILE] {
        let name = c_string(name)?;
        if unsafe { libc::unlinkat(staged.directory.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err(ResolveError::new(
                "STAGING_CLEANUP_UNLINK_FAILED",
                "cleanup",
            ));
        }
    }
    staged
        .directory
        .sync_all()
        .map_err(|_| ResolveError::new("STAGING_CLEANUP_DIRECTORY_SYNC_FAILED", "cleanup"))?;
    let directory_name = c_string(&staged.directory_name)?;
    if unsafe {
        libc::unlinkat(
            staged.root.as_raw_fd(),
            directory_name.as_ptr(),
            libc::AT_REMOVEDIR,
        )
    } != 0
    {
        return Err(ResolveError::new("STAGING_CLEANUP_RMDIR_FAILED", "cleanup"));
    }
    staged
        .root
        .sync_all()
        .map_err(|_| ResolveError::new("STAGING_CLEANUP_ROOT_SYNC_FAILED", "cleanup"))
}

/// Reclaims the one pre-reserved staging child when a process stopped before
/// it could durably seal its receipt. The caller supplies the exact operation
/// name from the authority lifecycle record; this never scans the root. It
/// validates the complete child entry set *before* deleting anything: only
/// the two protocol-created private regular files may be present, and either
/// may be absent while a write was in flight. An unknown, substituted, or
/// oversized entry leaves the entire child untouched.
pub(crate) fn remove_partial_exact_staging(
    root: &File,
    invocation_id: &str,
) -> Result<(), ResolveError> {
    let root_object = object_identity(root.as_raw_fd(), ObjectKind::Directory)?;
    if root_object.uid != effective_uid() || root_object.mode & 0o7777 != 0o700 {
        return Err(ResolveError::new("UNSAFE_STAGING_ROOT", "cleanup"));
    }
    let directory_name = exact_staging_directory_name(invocation_id)?;
    let directory = open_directory_at(root.as_raw_fd(), &directory_name)?;
    let directory_object = object_identity(directory.as_raw_fd(), ObjectKind::Directory)?;
    if directory_object.uid != effective_uid() || directory_object.mode & 0o7777 != 0o700 {
        return Err(ResolveError::new("UNSAFE_STAGING_DIRECTORY", "cleanup"));
    }
    let entries = partial_staging_entry_names(&directory)?;
    for name in &entries {
        if !matches!(name.as_str(), STAGED_ARCHIVE_FILE | STAGING_RECEIPT_FILE) {
            return Err(ResolveError::new(
                "STAGING_PARTIAL_UNEXPECTED_ENTRY",
                "cleanup",
            ));
        }
    }
    for (name, max_size) in [
        (STAGED_ARCHIVE_FILE, MAX_ARCHIVE_BYTES),
        (STAGING_RECEIPT_FILE, 64 * 1024),
    ] {
        if !entries.iter().any(|entry| entry == name) {
            continue;
        }
        let object = object_identity_at(directory.as_raw_fd(), name, ObjectKind::RegularFile)?;
        if object.uid != effective_uid()
            || object.mode & 0o7777 != 0o600
            || object.size > max_size as u64
        {
            return Err(ResolveError::new(
                "STAGING_PARTIAL_IDENTITY_INVALID",
                "cleanup",
            ));
        }
    }
    // Every named child is now a checked protocol file.  Verify the held
    // directory is still the deterministic root child before the irreversible
    // portion; all unlink operations stay relative to that held descriptor.
    if object_identity_at(root.as_raw_fd(), &directory_name, ObjectKind::Directory)?
        != directory_object
    {
        return Err(ResolveError::new(
            "STAGING_PARTIAL_IDENTITY_INVALID",
            "cleanup",
        ));
    }
    for name in [STAGED_ARCHIVE_FILE, STAGING_RECEIPT_FILE] {
        if !entries.iter().any(|entry| entry == name) {
            continue;
        }
        let name = c_string(name)?;
        if unsafe { libc::unlinkat(directory.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err(ResolveError::new(
                "STAGING_CLEANUP_UNLINK_FAILED",
                "cleanup",
            ));
        }
    }
    directory
        .sync_all()
        .map_err(|_| ResolveError::new("STAGING_CLEANUP_DIRECTORY_SYNC_FAILED", "cleanup"))?;
    let name = c_string(&directory_name)?;
    if unsafe { libc::unlinkat(root.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } != 0 {
        return Err(ResolveError::new("STAGING_CLEANUP_RMDIR_FAILED", "cleanup"));
    }
    root.sync_all()
        .map_err(|_| ResolveError::new("STAGING_CLEANUP_ROOT_SYNC_FAILED", "cleanup"))
}

/// `fdopendir` takes ownership of the duplicated descriptor.  The source
/// descriptor remains held by the lifecycle caller, so directory enumeration
/// cannot escape the opened no-follow child through a mutable pathname.
struct PartialStagingDirStream {
    raw: *mut libc::DIR,
}

impl PartialStagingDirStream {
    fn from_directory(directory: &File) -> Result<Self, ResolveError> {
        let duplicate = unsafe { libc::dup(directory.as_raw_fd()) };
        if duplicate < 0 {
            return Err(ResolveError::new(
                "STAGING_DIRECTORY_OPEN_FAILED",
                "cleanup",
            ));
        }
        if unsafe { libc::fcntl(duplicate, libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
            unsafe { libc::close(duplicate) };
            return Err(ResolveError::new(
                "STAGING_DIRECTORY_OPEN_FAILED",
                "cleanup",
            ));
        }
        if unsafe { libc::lseek(duplicate, 0, libc::SEEK_SET) } < 0 {
            unsafe { libc::close(duplicate) };
            return Err(ResolveError::new(
                "STAGING_DIRECTORY_OPEN_FAILED",
                "cleanup",
            ));
        }
        let raw = unsafe { libc::fdopendir(duplicate) };
        if raw.is_null() {
            unsafe { libc::close(duplicate) };
            return Err(ResolveError::new(
                "STAGING_DIRECTORY_OPEN_FAILED",
                "cleanup",
            ));
        }
        Ok(Self { raw })
    }

    fn next_name(&mut self) -> Result<Option<Vec<u8>>, ResolveError> {
        #[cfg(target_os = "macos")]
        unsafe {
            *libc::__error() = 0;
        }
        #[cfg(not(target_os = "macos"))]
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(self.raw) };
        if entry.is_null() {
            return if std::io::Error::last_os_error().raw_os_error().unwrap_or(0) == 0 {
                Ok(None)
            } else {
                Err(ResolveError::new(
                    "STAGING_DIRECTORY_READ_FAILED",
                    "cleanup",
                ))
            };
        }
        Ok(Some(
            unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_bytes()
                .to_vec(),
        ))
    }
}

impl Drop for PartialStagingDirStream {
    fn drop(&mut self) {
        unsafe { libc::closedir(self.raw) };
    }
}

fn partial_staging_entry_names(directory: &File) -> Result<Vec<String>, ResolveError> {
    let mut stream = PartialStagingDirStream::from_directory(directory)?;
    let mut names = Vec::new();
    while let Some(raw) = stream.next_name()? {
        if raw == b"." || raw == b".." {
            continue;
        }
        let name = std::str::from_utf8(&raw)
            .map_err(|_| ResolveError::new("STAGING_PARTIAL_UNEXPECTED_ENTRY", "cleanup"))?;
        // There can be at most the two protocol files.  Bound before storing
        // to keep even a hostile private directory enumeration finite.
        if names.len() >= 2 {
            return Err(ResolveError::new(
                "STAGING_PARTIAL_UNEXPECTED_ENTRY",
                "cleanup",
            ));
        }
        names.push(name.to_string());
    }
    Ok(names)
}

pub(crate) fn exact_staging_directory_name(invocation_id: &str) -> Result<String, ResolveError> {
    validate_invocation_id(invocation_id)?;
    Ok(format!("exact-{invocation_id}"))
}

#[derive(Serialize)]
struct StagingObjectIdentityInputV1<'a> {
    schema: &'static str,
    root: &'a StagingRootIdentityV1,
    directory: &'a ObjectIdentityV1,
    archive: &'a ObjectIdentityV1,
    archive_sha256: &'a str,
    content_sha256: &'a str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct StagingRootIdentityV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
}

fn stable_staging_root_identity(identity: &ObjectIdentityV1) -> StagingRootIdentityV1 {
    StagingRootIdentityV1 {
        device: identity.device,
        inode: identity.inode,
        mode: identity.mode,
        uid: identity.uid,
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct ObjectIdentityV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    size: u64,
    links: u64,
}

fn same_directory_identity(left: &ObjectIdentityV1, right: &ObjectIdentityV1) -> bool {
    left.device == right.device
        && left.inode == right.inode
        && left.mode == right.mode
        && left.uid == right.uid
}

#[derive(Clone, Copy)]
enum ObjectKind {
    Directory,
    RegularFile,
}

fn object_identity(fd: RawFd, expected: ObjectKind) -> Result<ObjectIdentityV1, ResolveError> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } != 0 {
        return Err(ResolveError::new("STAGING_FSTAT_FAILED", "staging"));
    }
    let stat = unsafe { stat.assume_init() };
    let kind = stat.st_mode & libc::S_IFMT;
    let expected_kind = match expected {
        ObjectKind::Directory => libc::S_IFDIR,
        ObjectKind::RegularFile => libc::S_IFREG,
    };
    if kind != expected_kind
        || stat.st_nlink == 0
        || (matches!(expected, ObjectKind::RegularFile) && stat.st_nlink != 1)
        || stat.st_size < 0
    {
        return Err(ResolveError::new("STAGING_OBJECT_TYPE_INVALID", "staging"));
    }
    Ok(ObjectIdentityV1 {
        device: stat.st_dev as u64,
        inode: stat.st_ino,
        mode: stat.st_mode as u32,
        uid: stat.st_uid,
        size: stat.st_size as u64,
        links: stat.st_nlink as u64,
    })
}

fn object_identity_at(
    parent: RawFd,
    name: &str,
    expected: ObjectKind,
) -> Result<ObjectIdentityV1, ResolveError> {
    let file = match expected {
        ObjectKind::Directory => open_directory_at(parent, name)?,
        ObjectKind::RegularFile => open_regular_at(parent, name)?,
    };
    object_identity(file.as_raw_fd(), expected)
}

fn effective_uid() -> u32 {
    unsafe { libc::geteuid() as u32 }
}

fn validate_invocation_id(value: &str) -> Result<(), ResolveError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(ResolveError::new("INVALID_INVOCATION_ID", "staging"));
    }
    Ok(())
}

fn mkdir_at(parent: RawFd, name: &str, mode: u32) -> Result<(), ResolveError> {
    let name = c_string(name)?;
    if unsafe { libc::mkdirat(parent, name.as_ptr(), mode as libc::mode_t) } != 0 {
        let code = if std::io::Error::last_os_error().kind() == std::io::ErrorKind::AlreadyExists {
            "STAGING_ALREADY_EXISTS"
        } else {
            "STAGING_DIRECTORY_CREATE_FAILED"
        };
        return Err(ResolveError::new(code, "staging"));
    }
    Ok(())
}

fn open_directory_at(parent: RawFd, name: &str) -> Result<File, ResolveError> {
    let name = c_string(name)?;
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    file_from_fd(fd, "STAGING_DIRECTORY_OPEN_FAILED")
}

fn create_regular_at(parent: RawFd, name: &str, mode: u32) -> Result<File, ResolveError> {
    let name = c_string(name)?;
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode as libc::c_uint,
        )
    };
    file_from_fd(fd, "STAGING_FILE_CREATE_FAILED")
}

fn open_regular_at(parent: RawFd, name: &str) -> Result<File, ResolveError> {
    let name = c_string(name)?;
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    file_from_fd(fd, "STAGING_FILE_OPEN_FAILED")
}

fn file_from_fd(fd: RawFd, code: &str) -> Result<File, ResolveError> {
    if fd < 0 {
        Err(ResolveError::new(code, "staging"))
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn c_string(value: &str) -> Result<std::ffi::CString, ResolveError> {
    std::ffi::CString::new(value).map_err(|_| ResolveError::new("STAGING_NAME_INVALID", "staging"))
}

fn read_exact_archive(file: &File, expected_len: usize) -> Result<Vec<u8>, ResolveError> {
    if expected_len > MAX_ARCHIVE_BYTES {
        return Err(ResolveError::new("ARCHIVE_SIZE_INVALID", "readback"));
    }
    let mut reader = file
        .try_clone()
        .map_err(|_| ResolveError::new("STAGED_ARCHIVE_CLONE_FAILED", "readback"))?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| ResolveError::new("STAGED_ARCHIVE_SEEK_FAILED", "readback"))?;
    let mut bytes = Vec::with_capacity(expected_len);
    reader
        .take((MAX_ARCHIVE_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ResolveError::new("STAGED_ARCHIVE_READ_FAILED", "readback"))?;
    if bytes.len() != expected_len {
        return Err(ResolveError::new("STAGED_ARCHIVE_SIZE_DRIFT", "readback"));
    }
    Ok(bytes)
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, ResolveError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| ResolveError::new("RESOLVER_SERIALIZATION_FAILED", "identity"))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::write::SimpleFileOptions;

    use crate::plan::{
        ConfirmablePlanTargetV1, EffectApplyStateV1, PlanEligibility, PlanSelectionV1,
    };

    struct TempRoot(PathBuf);
    static TEMP_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    impl TempRoot {
        fn new(mode: u32) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "csswitch-exact-staging-{}-{nonce}-{}",
                std::process::id(),
                TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            Self(path)
        }

        fn open(&self) -> File {
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(&self.0)
                .unwrap()
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn source() -> GithubInspectionSource {
        GithubInspectionSource {
            owner: "owner".into(),
            repo: "repo".into(),
            commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
            path: "skills/demo".into(),
        }
    }

    fn archive(extra: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(cursor);
        let prefix = "repo-0123456789ab/skills/demo/";
        writer
            .start_file(
                format!("{prefix}SKILL.md"),
                SimpleFileOptions::default().unix_permissions(0o644),
            )
            .unwrap();
        writer
            .write_all(b"---\nname: demo\ndescription: Demo skill\n---\nBody\n")
            .unwrap();
        for (path, content, executable) in extra {
            writer
                .start_file(
                    format!("{prefix}{path}"),
                    SimpleFileOptions::default().unix_permissions(if *executable {
                        0o755
                    } else {
                        0o644
                    }),
                )
                .unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn request(plan_id: &str) -> ConfirmablePlanRequestV1 {
        ConfirmablePlanRequestV1 {
            plan_id: plan_id.into(),
            expires_at_unix_seconds: 2_000_000_000,
            target: ConfirmablePlanTargetV1 {
                science_runtime_identity_sha256: "a".repeat(64),
                data_dir_identity_sha256: "b".repeat(64),
                active_org_identity_sha256: "c".repeat(64),
                skills_root_device: 1,
                skills_root_inode: 2,
            },
        }
    }

    #[test]
    fn exact_stage_readback_and_confirmable_plan_are_content_bound_and_inert() {
        let root = TempRoot::new(0o700);
        let handle = stage_inspect_exact_github_archive(
            &root.open(),
            "exact-invocation",
            &source(),
            &archive(&[]),
        )
        .unwrap();
        assert_eq!(handle.identity.schema, EXACT_STAGED_ARCHIVE_SCHEMA);
        assert!(matches!(
            handle.identity.source,
            PlanSourceV1::GithubExact {
                binding: SourceBindingV1::CsswitchExactContentBound,
                ..
            }
        ));
        let plan = handle.build_confirmable_plan(&request("plan-one")).unwrap();
        assert_eq!(plan.eligibility, PlanEligibility::Confirmable);
        assert!(matches!(plan.selection, PlanSelectionV1::Degraded { .. }));
        assert_eq!(plan.effects.len(), 2);
        assert!(plan
            .effects
            .iter()
            .all(|effect| effect.apply == EffectApplyStateV1::NotRun));
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        drop(handle);
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
    }

    #[test]
    fn private_root_collision_and_staged_content_drift_fail_closed() {
        let root = TempRoot::new(0o700);
        let bytes = archive(&[]);
        let handle =
            stage_inspect_exact_github_archive(&root.open(), "same-invocation", &source(), &bytes)
                .unwrap();
        assert_eq!(
            stage_inspect_exact_github_archive(&root.open(), "same-invocation", &source(), &bytes,)
                .unwrap_err()
                .code,
            "STAGING_ALREADY_EXISTS"
        );
        let mut replacement = handle.archive.try_clone().unwrap();
        replacement.seek(SeekFrom::Start(0)).unwrap();
        replacement.write_all(b"drift").unwrap();
        replacement.sync_all().unwrap();
        assert_eq!(
            handle
                .build_confirmable_plan(&request("drifted-plan"))
                .unwrap_err()
                .code,
            "STAGED_ARCHIVE_CONTENT_DRIFT"
        );
    }

    #[test]
    fn shared_staging_root_allows_sibling_prepare_but_rejects_root_replacement() {
        let root = TempRoot::new(0o700);
        let first = stage_inspect_exact_github_archive(
            &root.open(),
            "shared-root-first",
            &source(),
            &archive(&[]),
        )
        .unwrap();
        let archive_sha256 = first.identity.archive_sha256.clone();
        let object_sha256 = first.identity.staging_object_identity_sha256.clone();
        let receipt_sha256 = first.receipt_sha256.clone();

        // A different operation is allowed to materialize a sibling below the
        // same private root. This changes root size/link metadata but does not
        // change root authority or either operation's private children.
        let _second = stage_inspect_exact_github_archive(
            &root.open(),
            "shared-root-second",
            &source(),
            &archive(&[("scripts/run.sh", b"#!/bin/sh\n", true)]),
        )
        .unwrap();
        reopen_exact_github_archive(
            &root.open(),
            "shared-root-first",
            &source(),
            &archive_sha256,
            &object_sha256,
            &receipt_sha256,
        )
        .unwrap();

        // Replacing the root object is still a hard failure even if its path
        // is reused with private permissions.
        let displaced = root.0.with_extension("displaced");
        fs::rename(&root.0, &displaced).unwrap();
        fs::create_dir(&root.0).unwrap();
        fs::set_permissions(&root.0, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(reopen_exact_github_archive(
            &root.open(),
            "shared-root-first",
            &source(),
            &archive_sha256,
            &object_sha256,
            &receipt_sha256,
        )
        .is_err());
        fs::remove_dir_all(displaced).unwrap();
    }

    #[test]
    fn cleanup_preserves_a_replaced_receipt_instead_of_deleting_by_name() {
        let root = TempRoot::new(0o700);
        let handle = stage_inspect_exact_github_archive(
            &root.open(),
            "cleanup-replacement",
            &source(),
            &archive(&[]),
        )
        .unwrap();
        let directory = root.0.join(&handle.directory_name);
        let receipt = directory.join(STAGING_RECEIPT_FILE);
        fs::remove_file(&receipt).unwrap();
        fs::write(&receipt, b"foreign-replacement").unwrap();
        fs::set_permissions(&receipt, fs::Permissions::from_mode(0o600)).unwrap();
        drop(handle);
        assert_eq!(fs::read(&receipt).unwrap(), b"foreign-replacement");

        let handle = stage_inspect_exact_github_archive(
            &root.open(),
            "cleanup-archive-replacement",
            &source(),
            &archive(&[]),
        )
        .unwrap();
        let archive_path = root
            .0
            .join(&handle.directory_name)
            .join(STAGED_ARCHIVE_FILE);
        fs::remove_file(&archive_path).unwrap();
        fs::write(&archive_path, b"foreign-archive").unwrap();
        fs::set_permissions(&archive_path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            handle
                .build_confirmable_plan(&request("replaced-archive"))
                .unwrap_err()
                .code,
            "STAGED_OBJECT_IDENTITY_DRIFT"
        );
        drop(handle);
        assert_eq!(fs::read(&archive_path).unwrap(), b"foreign-archive");

        let handle = stage_inspect_exact_github_archive(
            &root.open(),
            "cleanup-receipt-drift",
            &source(),
            &archive(&[]),
        )
        .unwrap();
        let receipt_path = root
            .0
            .join(&handle.directory_name)
            .join(STAGING_RECEIPT_FILE);
        let receipt_len = fs::metadata(&receipt_path).unwrap().len() as usize;
        OpenOptions::new()
            .write(true)
            .open(&receipt_path)
            .unwrap()
            .write_all(&vec![b'x'; receipt_len])
            .unwrap();
        assert_eq!(
            handle
                .build_confirmable_plan(&request("modified-receipt"))
                .unwrap_err()
                .code,
            "STAGING_RECEIPT_CONTENT_DRIFT"
        );
        drop(handle);
        assert_eq!(fs::read(&receipt_path).unwrap(), vec![b'x'; receipt_len]);
    }

    #[test]
    fn staging_rejects_non_private_root_invalid_invocation_and_oversize() {
        let root = TempRoot::new(0o755);
        assert_eq!(
            stage_inspect_exact_github_archive(&root.open(), "valid", &source(), &archive(&[]),)
                .unwrap_err()
                .code,
            "UNSAFE_STAGING_ROOT"
        );
        let private = TempRoot::new(0o700);
        assert_eq!(
            stage_inspect_exact_github_archive(
                &private.open(),
                "../invalid",
                &source(),
                &archive(&[]),
            )
            .unwrap_err()
            .code,
            "INVALID_INVOCATION_ID"
        );
        assert_eq!(
            stage_inspect_exact_github_archive(
                &private.open(),
                "oversize",
                &source(),
                &vec![0; MAX_ARCHIVE_BYTES + 1],
            )
            .unwrap_err()
            .code,
            "ARCHIVE_SIZE_INVALID"
        );
    }

    #[test]
    fn unsafe_components_are_explicitly_excluded_from_degraded_selection() {
        let root = TempRoot::new(0o700);
        let bytes = archive(&[
            ("assets/readme.txt", b"asset", false),
            ("scripts/run.sh", b"#!/bin/sh\n", true),
            ("plugin.json", b"{}", false),
        ]);
        let handle =
            stage_inspect_exact_github_archive(&root.open(), "degraded", &source(), &bytes)
                .unwrap();
        let error = handle
            .build_confirmable_plan(&request("plan-degraded"))
            .unwrap_err();
        assert_eq!(error.code, "CONFIRMABLE_PROJECTION_REQUIRED");
    }
}
