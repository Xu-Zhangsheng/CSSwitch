//! Recovery projection orchestration: app/config snapshots and one-click authority capture/restore.
//! Coordinates `authority_snapshot` primitives with `pending_cleanup` registration.

use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Runtime;

use crate::config;
#[cfg(test)]
use crate::proc;
#[cfg(test)]
use crate::runtime::operation;
use crate::runtime::proxy::ProxyAction;
use crate::runtime::proxy_lifecycle::GatewayController;
use crate::runtime::science::ScienceRuntimeIdentity;
use crate::{lifecycle, lock, HistoryRecoverySession, SharedAppState};

#[cfg(test)]
use super::authority_snapshot::SANDBOX_SESSION_TEST_SEAMS;
use super::authority_snapshot::{
    inode_u64, AuthorityCopyBudget, AuthoritySnapshotScope, AuthorityTreeSnapshot,
    SCIENCE_OWNED_OPAQUE_ROOTS, SCIENCE_PROTECTED_AUTHORITY_ENTRIES,
};
use super::pending_cleanup::{
    finalize_failed_authority_snapshot, finalize_registered_authority_cleanup,
    prepare_registered_authority_cleanup, register_authority_cleanup,
    registered_authority_snapshot_for_ticket, AuthorityCleanupContext, AuthorityCleanupFailure,
    AuthorityCleanupOutcome, AuthorityCleanupPhase, PendingCleanupDisposition,
    RegisteredAuthorityCleanup,
};

pub(super) const DURABLE_AUTHORITY_REPLAY_MANIFEST: &str = "authority-replay.v1.json";
const MAX_DURABLE_PRIVATE_MANIFEST_BYTES: u64 = config::MAX_CONFIG_FILE_BYTES + 1024 * 1024;

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DurableAuthorityTree {
    scope: AuthoritySnapshotScope,
    source: PathBuf,
    backup_relative: PathBuf,
    existed: bool,
    source_parent_identity: Option<(u64, u64)>,
    backup_identity: Option<(u64, u64, libc::mode_t)>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DurableAuthorityReplayManifest {
    schema_version: u32,
    managed_id: String,
    config: config::Config,
    trees: Vec<DurableAuthorityTree>,
    science_root_path: PathBuf,
    science_root_identity: Option<(u64, u64)>,
    science_opaque_bindings: [Option<(u64, u64)>; SCIENCE_OWNED_OPAQUE_ROOTS.len()],
}

pub(super) fn read_registered_private_manifest(
    state: &SharedAppState,
    ticket: &config::RuntimeSnapshotTicket,
    name: &str,
) -> Result<Vec<u8>, String> {
    let (pending, _, _, root) =
        registered_authority_snapshot_for_ticket(state, ticket).map_err(String::from)?;
    if !matches!(
        pending.disposition,
        Some(PendingCleanupDisposition::ActiveRecovery | PendingCleanupDisposition::CleanupOnly)
    ) {
        return Err("private replay manifest requires a registered snapshot".into());
    }
    let name =
        std::ffi::CString::new(name).map_err(|_| "private replay manifest name is invalid")?;
    let mut file =
        AuthorityTreeSnapshot::open_destination_at(root.as_raw_fd(), &name, libc::O_RDONLY, 0)
            .map_err(|error| format!("private replay manifest open failed: {error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("private replay manifest metadata failed: {error}"))?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.len() == 0
        || metadata.len() > MAX_DURABLE_PRIVATE_MANIFEST_BYTES
    {
        return Err("private replay manifest identity is unsafe".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    (&mut file)
        .take(MAX_DURABLE_PRIVATE_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("private replay manifest read failed: {error}"))?;
    let after = file
        .metadata()
        .map_err(|error| format!("private replay manifest recheck failed: {error}"))?;
    if after.dev() != metadata.dev()
        || after.ino() != metadata.ino()
        || after.len() != metadata.len()
        || bytes.len() as u64 != metadata.len()
    {
        return Err("private replay manifest changed while reading".into());
    }
    Ok(bytes)
}

#[allow(clippy::large_enum_variant)]
pub(super) enum RuntimeTransactionRestoreExpectation {
    Exact(Option<config::RuntimeTransactionRecord>),
    ExactPreservingCompensation {
        runtime_transaction: Option<config::RuntimeTransactionRecord>,
        compensation: config::RuntimeCompensationJournal,
    },
    ExactConfig(Box<config::Config>),
}

pub(super) struct AppAuthoritySnapshot {
    pub(super) proxy_present: bool,
    pub(super) proxy_port: u16,
    pub(super) secret: String,
    pub(super) provider: String,
    pub(super) gateway_kind: String,
    pub(super) shim_mode: String,
    pub(super) launch_id: String,
    pub(super) key_fp: u64,
    pub(super) gateway_launch_context: Option<crate::runtime::proxy_lifecycle::GatewayLaunchRecipe>,
    pub(super) sandbox_present: bool,
    pub(super) sandbox_port: u16,
    pub(super) sandbox_url: Option<String>,
    pub(super) science_runtime: Option<ScienceRuntimeIdentity>,
    pub(super) science_confirmed_stopped: Option<ScienceRuntimeIdentity>,
    pub(super) history_recovery: Option<HistoryRecoverySession>,
    pub(super) pending_authority_cleanup: Vec<PathBuf>,
}

impl AppAuthoritySnapshot {
    pub(super) fn capture(state: &SharedAppState) -> Self {
        let state = lock(state);
        Self {
            proxy_present: state.proxy.is_some(),
            proxy_port: state.proxy_port,
            secret: state.secret.clone(),
            provider: state.provider.clone(),
            gateway_kind: state.gateway_kind.clone(),
            shim_mode: state.shim_mode.clone(),
            launch_id: state.launch_id.clone(),
            key_fp: state.key_fp,
            gateway_launch_context: state.gateway_launch_context.clone(),
            sandbox_present: state.sandbox.is_some(),
            sandbox_port: state.sandbox_port,
            sandbox_url: state.sandbox_url.clone(),
            science_runtime: state.science_runtime.clone(),
            science_confirmed_stopped: state.science_confirmed_stopped.clone(),
            history_recovery: state.history_recovery.clone(),
            pending_authority_cleanup: state.pending_authority_cleanup.clone(),
        }
    }

    #[cfg(test)]
    pub(super) fn restore(
        &self,
        state: &SharedAppState,
        proxy_action: ProxyAction,
    ) -> Result<(), String> {
        let mut current = lock(state);
        if proxy_action == ProxyAction::Restarted {
            current.stop_proxy();
        }
        if current.sandbox.is_some() && !self.sandbox_present {
            return Err("late-failure 补偿发现未预期的 Science child，拒绝伪造恢复状态".into());
        }
        if self.proxy_present != current.proxy.is_some() {
            return Err("late-failure 补偿无法恢复先前 Gateway child 所有权".into());
        }
        current.proxy_port = self.proxy_port;
        current.secret = self.secret.clone();
        current.provider = self.provider.clone();
        current.gateway_kind = self.gateway_kind.clone();
        current.shim_mode = self.shim_mode.clone();
        current.launch_id = self.launch_id.clone();
        current.key_fp = self.key_fp;
        current.gateway_launch_context = self.gateway_launch_context.clone();
        current.sandbox_port = self.sandbox_port;
        current.sandbox_url = self.sandbox_url.clone();
        current.science_runtime = self.science_runtime.clone();
        current.science_confirmed_stopped = self.science_confirmed_stopped.clone();
        current.history_recovery = self.history_recovery.clone();
        current.pending_authority_cleanup = self.pending_authority_cleanup.clone();
        Ok(())
    }

    pub(super) fn restore_with_gateway<R: Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        state: &SharedAppState,
        lifecycle: &lifecycle::Lifecycle,
        auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
        proxy_action: ProxyAction,
    ) -> Result<(), String> {
        if proxy_action == ProxyAction::Restarted {
            lock(state).stop_proxy();
        }
        if self.proxy_present {
            let context = self
                .gateway_launch_context
                .as_ref()
                .ok_or("late-failure 补偿缺少先前 Gateway 内存启动上下文")?;
            GatewayController::start_for(
                app,
                state,
                lifecycle,
                &context.profile,
                context.science_runtime.as_ref(),
                None,
                auth_proof,
            )
            .map_err(|error| format!("late-failure 补偿无法重启先前 Gateway：{error}"))?;
            if lock(state).proxy.is_none() {
                return Err("late-failure 补偿未恢复先前 Gateway child 所有权".into());
            }
        } else {
            let mut current = lock(state);
            if current.proxy.is_some() {
                return Err("late-failure 补偿发现未预期的 Gateway child".into());
            }
            current.proxy_port = self.proxy_port;
            current.secret = self.secret.clone();
            current.provider = self.provider.clone();
            current.gateway_kind = self.gateway_kind.clone();
            current.shim_mode = self.shim_mode.clone();
            current.launch_id = self.launch_id.clone();
            current.key_fp = self.key_fp;
            current.gateway_launch_context = self.gateway_launch_context.clone();
        }
        let mut current = lock(state);
        if current.sandbox.is_some() && !self.sandbox_present {
            return Err("late-failure 补偿发现未预期的 Science child，拒绝伪造恢复状态".into());
        }
        current.sandbox_port = self.sandbox_port;
        current.sandbox_url = self.sandbox_url.clone();
        current.science_runtime = self.science_runtime.clone();
        current.science_confirmed_stopped = self.science_confirmed_stopped.clone();
        current.history_recovery = self.history_recovery.clone();
        current.pending_authority_cleanup = self.pending_authority_cleanup.clone();
        Ok(())
    }
}

pub(super) struct OneClickAuthoritySnapshot {
    pub(super) backup_root: PathBuf,
    pub(super) cleanup_context: AuthorityCleanupContext,
    pub(super) cleanup_ticket: Option<RegisteredAuthorityCleanup>,
    pub(super) trees: Vec<AuthorityTreeSnapshot>,
    pub(super) science_root_path: PathBuf,
    pub(super) science_root: Option<std::fs::File>,
    pub(super) science_opaque_bindings: [Option<(u64, u64)>; SCIENCE_OWNED_OPAQUE_ROOTS.len()],
    pub(super) config: config::Config,
    pub(super) app: AppAuthoritySnapshot,
    pub(super) preserve_recovery: bool,
    pub(super) cleanup_prepared: bool,
}

impl OneClickAuthoritySnapshot {
    pub(super) fn load_durable(
        state: &SharedAppState,
        ticket: &config::RuntimeSnapshotTicket,
    ) -> Result<Self, String> {
        let (pending, cleanup_context, cleanup_ticket, root) =
            registered_authority_snapshot_for_ticket(state, ticket).map_err(String::from)?;
        if pending.disposition != Some(PendingCleanupDisposition::ActiveRecovery) {
            return Err("durable authority replay requires an ActiveRecovery snapshot".into());
        }
        let name = std::ffi::CString::new(DURABLE_AUTHORITY_REPLAY_MANIFEST).unwrap();
        let mut file =
            AuthorityTreeSnapshot::open_destination_at(root.as_raw_fd(), &name, libc::O_RDONLY, 0)
                .map_err(|error| {
                    format!("durable authority replay manifest open failed: {error}")
                })?;
        let metadata = file.metadata().map_err(|error| {
            format!("durable authority replay manifest metadata failed: {error}")
        })?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o777 != 0o600
            || metadata.nlink() != 1
            || metadata.len() == 0
            || metadata.len() > MAX_DURABLE_PRIVATE_MANIFEST_BYTES
        {
            return Err("durable authority replay manifest identity is unsafe".into());
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        (&mut file)
            .take(MAX_DURABLE_PRIVATE_MANIFEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("durable authority replay manifest read failed: {error}"))?;
        let after = file.metadata().map_err(|error| {
            format!("durable authority replay manifest recheck failed: {error}")
        })?;
        if after.dev() != metadata.dev()
            || after.ino() != metadata.ino()
            || after.len() != metadata.len()
            || bytes.len() as u64 != metadata.len()
        {
            return Err("durable authority replay manifest changed while reading".into());
        }
        let manifest: DurableAuthorityReplayManifest = serde_json::from_slice(&bytes)
            .map_err(|_| "durable authority replay manifest format is invalid")?;
        if manifest.schema_version != 1
            || manifest.managed_id != ticket.managed_id
            || manifest.managed_id != cleanup_context.managed_id
        {
            return Err("durable authority replay manifest identity mismatch".into());
        }
        let config_dir = config::default_dir();
        let sandbox_home = crate::runtime::science::sandbox_home();
        let expected_science_root = sandbox_home.join(".claude-science");
        if manifest.science_root_path != expected_science_root {
            return Err("durable authority replay Science root retargeted".into());
        }
        let mut expected_trees = SCIENCE_PROTECTED_AUTHORITY_ENTRIES
            .into_iter()
            .map(|entry| {
                (
                    AuthoritySnapshotScope::ScienceData,
                    expected_science_root.join(entry),
                    PathBuf::from("0").join(entry),
                )
            })
            .collect::<Vec<_>>();
        expected_trees.extend([
            (
                AuthoritySnapshotScope::SandboxState,
                sandbox_home
                    .parent()
                    .ok_or("sandbox HOME has no parent")?
                    .join("state"),
                PathBuf::from("1"),
            ),
            (
                AuthoritySnapshotScope::CsswitchRuntime,
                config_dir.join("runtime"),
                PathBuf::from("2"),
            ),
            (
                AuthoritySnapshotScope::ManagedReceipt,
                config_dir.join("science-managed-launch.v1.json"),
                PathBuf::from("3"),
            ),
        ]);
        if manifest.trees.len() != expected_trees.len() {
            return Err("durable authority replay tree plan length mismatch".into());
        }
        let mut trees = Vec::with_capacity(manifest.trees.len());
        for (tree, (scope, source, backup_relative)) in
            manifest.trees.into_iter().zip(expected_trees)
        {
            if tree.scope != scope
                || tree.source != source
                || tree.backup_relative != backup_relative
                || tree.backup_relative.is_absolute()
                || tree
                    .backup_relative
                    .components()
                    .any(|component| !matches!(component, std::path::Component::Normal(_)))
                || tree.existed != tree.backup_identity.is_some()
            {
                return Err("durable authority replay tree plan retargeted".into());
            }
            let source_parent = match tree.source_parent_identity {
                Some(expected) => {
                    let parent_path = tree
                        .source
                        .parent()
                        .ok_or("durable authority replay source parent missing")?;
                    let parent = AuthorityTreeSnapshot::open_absolute_directory(parent_path)
                        .map_err(|error| {
                            format!("durable authority replay source parent open failed: {error}")
                        })?;
                    let metadata = parent.metadata().map_err(|error| {
                        format!("durable authority replay source parent metadata failed: {error}")
                    })?;
                    if (metadata.dev(), metadata.ino()) != expected {
                        return Err(
                            "durable authority replay source parent identity changed".into()
                        );
                    }
                    Some(parent)
                }
                None => None,
            };
            let source_name = source_parent
                .as_ref()
                .map(|_| AuthorityTreeSnapshot::destination_name(&tree.source))
                .transpose()?;
            let backup = cleanup_context.root.join(&tree.backup_relative);
            let (backup_parent, backup_name) = if tree.existed {
                let parent_path = backup
                    .parent()
                    .ok_or("durable authority replay backup parent missing")?;
                let parent = AuthorityTreeSnapshot::open_absolute_directory(parent_path).map_err(
                    |error| format!("durable authority replay backup parent open failed: {error}"),
                )?;
                let name = AuthorityTreeSnapshot::destination_name(&backup)?;
                (Some(parent), Some(name))
            } else {
                (None, None)
            };
            trees.push(AuthorityTreeSnapshot {
                scope: tree.scope,
                source: tree.source,
                backup,
                existed: tree.existed,
                source_parent,
                source_name,
                backup_identity: tree.backup_identity,
                backup_parent,
                backup_name,
            });
        }
        let science_root = match manifest.science_root_identity {
            Some(expected) => {
                let root = AuthorityTreeSnapshot::open_absolute_directory(&expected_science_root)
                    .map_err(|error| {
                    format!("durable authority replay Science root open failed: {error}")
                })?;
                let metadata = root.metadata().map_err(|error| {
                    format!("durable authority replay Science root metadata failed: {error}")
                })?;
                if (metadata.dev(), metadata.ino()) != expected {
                    return Err("durable authority replay Science root identity changed".into());
                }
                Some(root)
            }
            None => match AuthorityTreeSnapshot::open_absolute_directory(&expected_science_root) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Ok(_) => return Err("durable authority replay Science root appeared".into()),
                Err(error) => {
                    return Err(format!(
                        "durable authority replay Science root check failed: {error}"
                    ))
                }
            },
        };
        if Self::science_opaque_root_bindings(science_root.as_ref())?
            != manifest.science_opaque_bindings
        {
            return Err("durable authority replay Science opaque binding changed".into());
        }
        Ok(Self {
            backup_root: cleanup_context.root.clone(),
            cleanup_context,
            cleanup_ticket: Some(cleanup_ticket),
            trees,
            science_root_path: expected_science_root,
            science_root,
            science_opaque_bindings: manifest.science_opaque_bindings,
            config: manifest.config,
            app: AppAuthoritySnapshot::capture(state),
            preserve_recovery: true,
            cleanup_prepared: false,
        })
    }

    fn durable_replay_manifest_bytes(&self) -> Result<Vec<u8>, String> {
        let trees = self
            .trees
            .iter()
            .map(|tree| {
                let backup_relative = tree
                    .backup
                    .strip_prefix(&self.backup_root)
                    .map_err(|_| "durable authority backup escaped the registered snapshot")?
                    .to_path_buf();
                if backup_relative.as_os_str().is_empty()
                    || backup_relative.is_absolute()
                    || backup_relative
                        .components()
                        .any(|component| !matches!(component, std::path::Component::Normal(_)))
                {
                    return Err("durable authority backup relative path is invalid".into());
                }
                let source_parent_identity = tree
                    .source_parent
                    .as_ref()
                    .map(|parent| {
                        let metadata = parent.metadata().map_err(|error| {
                            format!("durable authority source parent metadata failed: {error}")
                        })?;
                        Ok::<_, String>((metadata.dev(), metadata.ino()))
                    })
                    .transpose()?;
                Ok(DurableAuthorityTree {
                    scope: tree.scope,
                    source: tree.source.clone(),
                    backup_relative,
                    existed: tree.existed,
                    source_parent_identity,
                    backup_identity: tree.backup_identity,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let science_root_identity = self
            .science_root
            .as_ref()
            .map(|root| {
                let metadata = root
                    .metadata()
                    .map_err(|error| format!("durable Science root metadata failed: {error}"))?;
                Ok::<_, String>((metadata.dev(), metadata.ino()))
            })
            .transpose()?;
        let manifest = DurableAuthorityReplayManifest {
            schema_version: 1,
            managed_id: self.cleanup_context.managed_id.clone(),
            config: self.config.clone(),
            trees,
            science_root_path: self.science_root_path.clone(),
            science_root_identity,
            science_opaque_bindings: self.science_opaque_bindings,
        };
        let bytes = serde_json::to_vec(&manifest)
            .map_err(|error| format!("durable authority replay manifest encode failed: {error}"))?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_DURABLE_PRIVATE_MANIFEST_BYTES {
            return Err("durable authority replay manifest size is invalid".into());
        }
        Ok(bytes)
    }

    pub(super) fn registered_snapshot_ticket(
        &self,
    ) -> Result<config::RuntimeSnapshotTicket, String> {
        let ticket = self
            .cleanup_ticket
            .as_ref()
            .ok_or("registered authority snapshot ticket is missing")?;
        if ticket.entry.managed_id != self.cleanup_context.managed_id
            || ticket.entry.path != self.backup_root
            || ticket.entry.marker != ticket.entry.managed_id
        {
            return Err("registered authority snapshot ticket identity changed".into());
        }
        config::RuntimeSnapshotTicket::verified(ticket.entry.managed_id.clone())
    }

    pub(super) fn persist_private_manifest(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() as u64 > MAX_DURABLE_PRIVATE_MANIFEST_BYTES {
            return Err("private recovery manifest size is invalid".into());
        }
        let parent = AuthorityTreeSnapshot::open_absolute_directory(
            &self.cleanup_context.expected_snapshot_parent,
        )
        .map_err(|error| format!("private recovery parent open failed: {error}"))?;
        let root_name = AuthorityTreeSnapshot::destination_name(&self.backup_root)?;
        let root = AuthorityTreeSnapshot::open_directory_at(parent.as_raw_fd(), &root_name)
            .map_err(|error| format!("private recovery root open failed: {error}"))?;
        let metadata = root
            .metadata()
            .map_err(|error| format!("private recovery root metadata failed: {error}"))?;
        let expected = self
            .cleanup_context
            .expected_root_identity
            .ok_or("private recovery root identity is missing")?;
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o777 != 0o700
            || (metadata.dev(), metadata.ino()) != expected
        {
            return Err("private recovery root identity changed".into());
        }
        let manifest_name = std::ffi::CString::new(name)
            .map_err(|_| "private recovery manifest name is invalid")?;
        let mut manifest = AuthorityTreeSnapshot::open_destination_at(
            root.as_raw_fd(),
            &manifest_name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )
        .map_err(|error| format!("private recovery manifest create failed: {error}"))?;
        std::io::Write::write_all(&mut manifest, bytes)
            .and_then(|_| manifest.set_permissions(std::fs::Permissions::from_mode(0o600)))
            .and_then(|_| manifest.sync_all())
            .map_err(|error| format!("private recovery manifest sync failed: {error}"))?;
        let manifest_metadata = manifest
            .metadata()
            .map_err(|error| format!("private recovery manifest metadata failed: {error}"))?;
        if !manifest_metadata.is_file()
            || manifest_metadata.uid() != unsafe { libc::geteuid() }
            || manifest_metadata.permissions().mode() & 0o777 != 0o600
            || manifest_metadata.nlink() != 1
            || manifest_metadata.len() != bytes.len() as u64
        {
            return Err("private recovery manifest identity is unsafe".into());
        }
        root.sync_all()
            .and_then(|_| parent.sync_all())
            .map_err(|error| format!("private recovery directory sync failed: {error}"))
    }

    pub(super) fn science_opaque_root_bindings(
        root: Option<&std::fs::File>,
    ) -> Result<[Option<(u64, u64)>; SCIENCE_OWNED_OPAQUE_ROOTS.len()], String> {
        let mut bindings = [None; SCIENCE_OWNED_OPAQUE_ROOTS.len()];
        let Some(root) = root else {
            return Ok(bindings);
        };
        for (index, entry) in SCIENCE_OWNED_OPAQUE_ROOTS.iter().enumerate() {
            let name = std::ffi::CString::new(*entry)
                .map_err(|_| "code=science_environment_root_name_invalid")?;
            match AuthorityTreeSnapshot::stat_destination_at(root, &name) {
                Ok(identity)
                    if identity.st_mode & libc::S_IFMT == libc::S_IFDIR
                        && identity.st_uid == unsafe { libc::geteuid() }
                        && identity.st_mode & 0o022 == 0 =>
                {
                    let device = u64::try_from(identity.st_dev)
                        .map_err(|_| "code=science_environment_root_device_invalid")?;
                    let inode = inode_u64(identity.st_ino)
                        .ok_or("code=science_environment_root_inode_invalid")?;
                    bindings[index] = Some((device, inode));
                }
                Ok(_) => {
                    return Err(
                        "code=science_environment_root_identity_failed category=science_runtime"
                            .into(),
                    )
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "code=science_environment_root_validate_failed category=science_runtime os_error={}",
                        AuthorityTreeSnapshot::os_error_code(&error)
                    ))
                }
            }
        }
        Ok(bindings)
    }

    pub(super) fn pin_science_root_and_validate_opaque_entries(
        auth_dir: &Path,
    ) -> Result<Option<std::fs::File>, String> {
        let root = match AuthorityTreeSnapshot::open_absolute_directory(auth_dir) {
            Ok(root) => root,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "code=science_authority_root_open_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                ))
            }
        };
        let metadata = root.metadata().map_err(|error| {
            format!(
                "code=science_authority_root_validate_failed os_error={}",
                AuthorityTreeSnapshot::os_error_code(&error)
            )
        })?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err("code=science_authority_root_identity_failed".into());
        }
        Self::science_opaque_root_bindings(Some(&root))?;
        Ok(Some(root))
    }

    pub(super) fn revalidate_science_root_binding(
        auth_dir: &Path,
        pinned: &Option<std::fs::File>,
    ) -> Result<(), String> {
        let Some(pinned) = pinned.as_ref() else {
            return match AuthorityTreeSnapshot::open_absolute_directory(auth_dir) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Ok(_) => Err("code=science_authority_root_created_during_capture".into()),
                Err(error) => Err(format!(
                    "code=science_authority_root_revalidate_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                )),
            };
        };
        let matches = AuthorityTreeSnapshot::absolute_directory_binding_matches(auth_dir, pinned)
            .map_err(|error| {
            format!(
                "code=science_authority_root_revalidate_failed os_error={}",
                AuthorityTreeSnapshot::os_error_code(&error)
            )
        })?;
        if matches {
            Ok(())
        } else {
            Err("code=science_authority_root_rebound".into())
        }
    }

    pub(super) fn validate_science_restore_root(&self) -> Result<(), String> {
        let current = Self::pin_science_root_and_validate_opaque_entries(&self.science_root_path)?;
        if Self::science_opaque_root_bindings(current.as_ref())? != self.science_opaque_bindings {
            return Err("code=science_environment_root_rebound category=science_runtime".into());
        }
        match (self.science_root.as_ref(), current.as_ref()) {
            (Some(pinned), Some(_)) => {
                let matches = AuthorityTreeSnapshot::absolute_directory_binding_matches(
                    &self.science_root_path,
                    pinned,
                )
                .map_err(|error| {
                    format!(
                        "code=science_authority_restore_root_revalidate_failed os_error={}",
                        AuthorityTreeSnapshot::os_error_code(&error)
                    )
                })?;
                if matches {
                    Ok(())
                } else {
                    Err("code=science_authority_restore_root_rebound".into())
                }
            }
            (Some(_), None) => Err("code=science_authority_restore_root_missing".into()),
            (None, _) => Ok(()),
        }
    }

    pub(super) fn science_opaque_bindings_env(&self) -> String {
        SCIENCE_OWNED_OPAQUE_ROOTS
            .iter()
            .zip(self.science_opaque_bindings)
            .map(|(name, binding)| match binding {
                Some((device, inode)) => format!("{name}={device}:{inode}"),
                None => format!("{name}=absent"),
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    pub(super) fn capture(
        config_dir: &Path,
        sandbox_home: &Path,
        auth_dir: &Path,
        config: &config::Config,
        state: &SharedAppState,
    ) -> Result<Self, String> {
        #[cfg(test)]
        {
            let capture_seam = SANDBOX_SESSION_TEST_SEAMS
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .one_click_capture
                .as_ref()
                .filter(|(target_dir, _, _, _, _)| target_dir == config_dir)
                .cloned();
            if let Some((_, observation, _, expected_prior_pid, expected_receipt)) =
                capture_seam.as_ref()
            {
                let listener_state = if proc::loopback_port_in_use(
                    config.sandbox_port,
                    operation::LOCAL_HEALTH_TIMEOUT_MS,
                ) {
                    "running"
                } else {
                    "stopped"
                };
                let prior_process = if crate::runtime::science::test_process_start_identity_for_pid(
                    *expected_prior_pid,
                )
                .is_some()
                {
                    "alive"
                } else {
                    "absent"
                };
                let prior_receipt = if expected_receipt.exists() {
                    "present"
                } else {
                    "absent"
                };
                std::fs::write(
                    observation,
                    format!(
                        "expected_prior_pid={expected_prior_pid}\nexpected_receipt={}\nlistener={listener_state}\nprior_process={prior_process}\nprior_receipt={prior_receipt}\n",
                        expected_receipt.display()
                    ),
                )
                .map_err(|error| {
                        format!("test-only authority snapshot observation failed: {error}")
                    })?;
            }
            if capture_seam.is_some_and(|(_, _, fail, _, _)| fail) {
                return Err("test-only one-click authority snapshot capture failure".into());
            }
        }
        let sandbox_dir = sandbox_home
            .parent()
            .ok_or("沙箱 HOME 无父目录，无法建立事务快照")?;
        let mut cleanup_context = AuthorityCleanupContext::new(config_dir, sandbox_home, state)?;
        let backup_root = cleanup_context.root.clone();
        let snapshot_parent = AuthorityTreeSnapshot::open_or_create_authority_snapshot_parent(
            config_dir,
            sandbox_home,
        )?;
        let backup_root_name = AuthorityTreeSnapshot::destination_name(&backup_root)?;
        AuthorityTreeSnapshot::mkdir_destination_at(
            snapshot_parent.as_raw_fd(),
            &backup_root_name,
            0o700,
        )
        .map_err(|error| {
            format!(
                "code=authority_snapshot_root_create_failed os_error={}",
                AuthorityTreeSnapshot::os_error_code(&error)
            )
        })?;
        let created_root_entry =
            AuthorityTreeSnapshot::stat_destination_at(&snapshot_parent, &backup_root_name)
                .map_err(|error| {
                    finalize_failed_authority_snapshot(
                        &cleanup_context,
                        format!(
                            "code=authority_snapshot_root_entry_validate_failed os_error={}",
                            AuthorityTreeSnapshot::os_error_code(&error)
                        ),
                    )
                })?;
        cleanup_context.bind_root_identity(&created_root_entry)?;
        let backup_root_file = match (|| -> Result<std::fs::File, String> {
            let root = AuthorityTreeSnapshot::open_directory_at(
                snapshot_parent.as_raw_fd(),
                &backup_root_name,
            )
            .map_err(|error| {
                format!(
                    "code=authority_snapshot_root_open_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                )
            })?;
            root.set_permissions(std::fs::Permissions::from_mode(0o700))
                .map_err(|error| {
                    format!(
                        "code=authority_snapshot_root_chmod_failed os_error={}",
                        AuthorityTreeSnapshot::os_error_code(&error)
                    )
                })?;
            let metadata = root.metadata().map_err(|error| {
                format!(
                    "code=authority_snapshot_root_validate_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                )
            })?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.permissions().mode() & 0o777 != 0o700
                || !AuthorityTreeSnapshot::destination_entry_matches_file(
                    &created_root_entry,
                    &metadata,
                    libc::S_IFDIR,
                )
            {
                return Err("code=authority_snapshot_root_identity_failed".into());
            }
            root.sync_all().map_err(|error| {
                format!(
                    "code=authority_snapshot_root_sync_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                )
            })?;
            snapshot_parent.sync_all().map_err(|error| {
                format!(
                    "code=authority_snapshot_root_parent_sync_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                )
            })?;
            Ok(root)
        })() {
            Ok(root) => root,
            Err(error) => {
                return Err(finalize_failed_authority_snapshot(
                    &cleanup_context,
                    error.to_string(),
                ))
            }
        };
        let cleanup_ticket = match register_authority_cleanup(&cleanup_context) {
            Ok(ticket) => ticket,
            Err(error) => {
                return Err(finalize_failed_authority_snapshot(
                    &cleanup_context,
                    error.to_string(),
                ))
            }
        };
        let science_root = match Self::pin_science_root_and_validate_opaque_entries(auth_dir) {
            Ok(root) => root,
            Err(error) => return Err(finalize_failed_authority_snapshot(&cleanup_context, error)),
        };
        let science_opaque_bindings =
            match Self::science_opaque_root_bindings(science_root.as_ref()) {
                Ok(bindings) => bindings,
                Err(error) => {
                    return Err(finalize_failed_authority_snapshot(&cleanup_context, error))
                }
            };
        let science_backup = backup_root.join("0");
        let science_backup_name = AuthorityTreeSnapshot::destination_name(&science_backup)?;
        AuthorityTreeSnapshot::mkdir_destination_at(
            backup_root_file.as_raw_fd(),
            &science_backup_name,
            0o700,
        )
        .map_err(|error| {
            finalize_failed_authority_snapshot(
                &cleanup_context,
                format!(
                    "code=science_authority_projection_create_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                ),
            )
        })?;
        let science_backup_file = AuthorityTreeSnapshot::open_directory_at(
            backup_root_file.as_raw_fd(),
            &science_backup_name,
        )
        .map_err(|error| {
            finalize_failed_authority_snapshot(
                &cleanup_context,
                format!(
                    "code=science_authority_projection_open_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                ),
            )
        })?;
        let mut trees = Vec::with_capacity(SCIENCE_PROTECTED_AUTHORITY_ENTRIES.len() + 3);
        let mut science_budget = AuthorityCopyBudget::default();
        for entry in SCIENCE_PROTECTED_AUTHORITY_ENTRIES {
            let source = auth_dir.join(entry);
            let backup = science_backup.join(entry);
            let source_name = AuthorityTreeSnapshot::destination_name(&source)?;
            let backup_name = AuthorityTreeSnapshot::destination_name(&backup)?;
            let capture = match science_root.as_ref() {
                Some(science_root) => {
                    AuthorityTreeSnapshot::capture_scoped_from_parent_with_budget(
                        AuthoritySnapshotScope::ScienceData,
                        source,
                        backup,
                        science_root,
                        &source_name,
                        &science_backup_file,
                        &backup_name,
                        &mut science_budget,
                    )
                }
                None => AuthorityTreeSnapshot::capture_scoped_at_with_budget(
                    AuthoritySnapshotScope::ScienceData,
                    source,
                    backup,
                    &science_backup_file,
                    &backup_name,
                    &mut science_budget,
                ),
            };
            match capture {
                Ok(snapshot) => trees.push(snapshot),
                Err(error) => {
                    let durability = science_backup_file
                        .sync_all()
                        .and_then(|_| backup_root_file.sync_all())
                        .and_then(|_| snapshot_parent.sync_all());
                    let primary = match durability {
                        Ok(()) => error,
                        Err(sync_error) => format!(
                            "{error}; code=authority_snapshot_failure_sync_failed os_error={}",
                            AuthorityTreeSnapshot::os_error_code(&sync_error)
                        ),
                    };
                    return Err(finalize_failed_authority_snapshot(
                        &cleanup_context,
                        primary,
                    ));
                }
            }
        }
        if let Err(error) = Self::revalidate_science_root_binding(auth_dir, &science_root) {
            return Err(finalize_failed_authority_snapshot(&cleanup_context, error));
        }
        science_backup_file
            .sync_all()
            .and_then(|_| backup_root_file.sync_all())
            .map_err(|error| {
                finalize_failed_authority_snapshot(
                    &cleanup_context,
                    format!(
                        "code=science_authority_projection_sync_failed os_error={}",
                        AuthorityTreeSnapshot::os_error_code(&error)
                    ),
                )
            })?;
        let sources = [
            (
                AuthoritySnapshotScope::SandboxState,
                sandbox_dir.join("state"),
            ),
            (
                AuthoritySnapshotScope::CsswitchRuntime,
                config_dir.join("runtime"),
            ),
            (
                AuthoritySnapshotScope::ManagedReceipt,
                config_dir.join("science-managed-launch.v1.json"),
            ),
        ];
        for (index, (scope, source)) in sources.into_iter().enumerate() {
            let index = index + 1;
            let backup = backup_root.join(index.to_string());
            let backup_name = AuthorityTreeSnapshot::destination_name(&backup)?;
            match AuthorityTreeSnapshot::capture_scoped_at(
                scope,
                source,
                backup,
                &backup_root_file,
                &backup_name,
            ) {
                Ok(snapshot) => trees.push(snapshot),
                Err(error) => {
                    let durability = backup_root_file
                        .sync_all()
                        .and_then(|_| snapshot_parent.sync_all());
                    let primary = match durability {
                        Ok(()) => error,
                        Err(sync_error) => format!(
                            "{error}; code=authority_snapshot_failure_sync_failed os_error={}",
                            AuthorityTreeSnapshot::os_error_code(&sync_error)
                        ),
                    };
                    return Err(finalize_failed_authority_snapshot(
                        &cleanup_context,
                        primary,
                    ));
                }
            }
        }
        if !AuthorityTreeSnapshot::absolute_directory_binding_matches(
            &cleanup_context.expected_snapshot_parent,
            &snapshot_parent,
        )
        .map_err(|error| {
            finalize_failed_authority_snapshot(
                &cleanup_context,
                format!(
                    "code=authority_snapshot_root_parent_revalidate_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                ),
            )
        })? {
            return Err(finalize_failed_authority_snapshot(
                &cleanup_context,
                "code=authority_snapshot_root_parent_rebound".into(),
            ));
        }
        let final_root_metadata = backup_root_file.metadata().map_err(|error| {
            finalize_failed_authority_snapshot(
                &cleanup_context,
                format!(
                    "code=authority_snapshot_root_validate_failed os_error={}",
                    AuthorityTreeSnapshot::os_error_code(&error)
                ),
            )
        })?;
        let final_root_entry =
            AuthorityTreeSnapshot::stat_destination_at(&snapshot_parent, &backup_root_name)
                .map_err(|error| {
                    finalize_failed_authority_snapshot(
                        &cleanup_context,
                        format!(
                            "code=authority_snapshot_root_entry_revalidate_failed os_error={}",
                            AuthorityTreeSnapshot::os_error_code(&error)
                        ),
                    )
                })?;
        if !AuthorityTreeSnapshot::destination_entry_matches_file(
            &final_root_entry,
            &final_root_metadata,
            libc::S_IFDIR,
        ) {
            return Err(finalize_failed_authority_snapshot(
                &cleanup_context,
                "code=authority_snapshot_root_rebound".into(),
            ));
        }
        AuthorityTreeSnapshot::sync_snapshot_completion(&backup_root_file, &snapshot_parent)
            .map_err(|error| {
                finalize_failed_authority_snapshot(
                    &cleanup_context,
                    format!(
                        "code=authority_snapshot_completion_sync_failed os_error={}",
                        AuthorityTreeSnapshot::os_error_code(&error)
                    ),
                )
            })?;
        let snapshot = Self {
            backup_root,
            cleanup_context,
            cleanup_ticket: Some(cleanup_ticket),
            trees,
            science_root_path: auth_dir.to_path_buf(),
            science_root,
            science_opaque_bindings,
            config: config.clone(),
            app: AppAuthoritySnapshot::capture(state),
            preserve_recovery: false,
            cleanup_prepared: false,
        };
        let replay_manifest = snapshot.durable_replay_manifest_bytes().map_err(|error| {
            finalize_failed_authority_snapshot(&snapshot.cleanup_context, error)
        })?;
        snapshot
            .persist_private_manifest(DURABLE_AUTHORITY_REPLAY_MANIFEST, &replay_manifest)
            .map_err(|error| {
                finalize_failed_authority_snapshot(&snapshot.cleanup_context, error)
            })?;
        #[cfg(test)]
        if SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .one_click_exit_after_capture
            .as_ref()
            == Some(&config_dir.to_path_buf())
        {
            std::process::exit(86);
        }
        Ok(snapshot)
    }

    #[cfg(test)]
    pub(super) fn restore(
        &mut self,
        config_dir: &Path,
        state: &SharedAppState,
        proxy_action: ProxyAction,
    ) -> Result<(), String> {
        let mut errors = Vec::new();
        let science_restore_allowed = match self.validate_science_restore_root() {
            Ok(()) => true,
            Err(error) => {
                errors.push(error);
                false
            }
        };
        for tree in &mut self.trees {
            if tree.scope == AuthoritySnapshotScope::ScienceData && !science_restore_allowed {
                continue;
            }
            if let Err(error) = tree.restore() {
                errors.push(error);
            }
        }
        if let Err(error) =
            config::save_to(config_dir, &self.config).map_err(|error| error.to_string())
        {
            errors.push(error);
        }
        if let Err(error) = self.app.restore(state, proxy_action) {
            errors.push(error);
        }
        if errors.is_empty() {
            return self
                .cleanup_when_expendable()
                .map(|_| ())
                .map_err(String::from);
        }
        self.preserve_recovery = true;
        Err(errors.join("; "))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn restore_with_gateway<R: Runtime>(
        &mut self,
        app: &tauri::AppHandle<R>,
        config_dir: &Path,
        state: &SharedAppState,
        lifecycle: &lifecycle::Lifecycle,
        auth_proof: Option<&crate::codex_auth_supervisor::CodexAuthReadyProof>,
        proxy_action: ProxyAction,
        runtime_transaction: &RuntimeTransactionRestoreExpectation,
    ) -> Result<(), String> {
        let current = config::load_from(config_dir).map_err(|error| error.to_string())?;
        let authority_matches = match runtime_transaction {
            RuntimeTransactionRestoreExpectation::Exact(expected) => {
                current.runtime_transaction.as_ref() == expected.as_ref()
            }
            RuntimeTransactionRestoreExpectation::ExactPreservingCompensation {
                runtime_transaction,
                compensation,
            } => {
                current.runtime_transaction == *runtime_transaction
                    && current.runtime_compensation.as_ref() == Some(compensation)
            }
            RuntimeTransactionRestoreExpectation::ExactConfig(expected) => current == **expected,
        };
        if !authority_matches {
            self.preserve_recovery = true;
            return Err("one-click compensation found drifted config authority; preserved the current config, authority, runtime state, and recovery snapshot".into());
        }
        if proxy_action == ProxyAction::Restarted {
            lock(state).stop_proxy();
        }
        let mut errors = Vec::new();
        let science_restore_allowed = match self.validate_science_restore_root() {
            Ok(()) => true,
            Err(error) => {
                errors.push(error);
                false
            }
        };
        for tree in &mut self.trees {
            if tree.scope == AuthoritySnapshotScope::ScienceData && !science_restore_allowed {
                continue;
            }
            if let Err(error) = tree.restore() {
                errors.push(error);
            }
        }
        let config_restore = match runtime_transaction {
            RuntimeTransactionRestoreExpectation::Exact(expected) => {
                config::update_result(config_dir, |current| {
                    if current.runtime_transaction.as_ref() != expected.as_ref() {
                        return Err("one-click compensation found a retargeted runtime journal; preserved the current config and recovery snapshot".into());
                    }
                    *current = self.config.clone();
                    Ok(((), true))
                })
            }
            RuntimeTransactionRestoreExpectation::ExactPreservingCompensation {
                runtime_transaction,
                compensation,
            } => config::update_result(config_dir, |current| {
                if current.runtime_transaction != *runtime_transaction
                    || current.runtime_compensation.as_ref() != Some(compensation)
                {
                    return Err("one-click compensation found a retargeted durable compensation journal; preserved the current config and recovery snapshot".into());
                }
                *current = self.config.clone();
                current.runtime_compensation = Some(compensation.clone());
                Ok(((), true))
            }),
            RuntimeTransactionRestoreExpectation::ExactConfig(expected) => {
                config::update_result(config_dir, |current| {
                    if current != expected.as_ref() {
                        return Err("one-click compensation found drifted config authority; preserved the current config and recovery snapshot".into());
                    }
                    *current = self.config.clone();
                    Ok(((), true))
                })
            }
        };
        if let Err(error) = config_restore {
            errors.push(error);
        }
        if let Err(error) =
            self.app
                .restore_with_gateway(app, state, lifecycle, auth_proof, proxy_action)
        {
            errors.push(error);
        }
        #[cfg(test)]
        {
            let mut seams = SANDBOX_SESSION_TEST_SEAMS
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(canary) = seams.rollback_diagnostic_canary.clone() {
                seams.rollback_diagnostic_snapshot = Some(self.backup_root.clone());
                errors.push(format!("test-only rollback diagnostic {canary}"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            self.preserve_recovery = true;
            Err(errors.join("; "))
        }
    }

    pub(super) fn restore_durable_authority(
        &mut self,
        config_dir: &Path,
        state: &SharedAppState,
        expected_runtime_transaction: &Option<config::RuntimeTransactionRecord>,
        expected_compensation: &config::RuntimeCompensationJournal,
    ) -> Result<(), String> {
        let current = config::load_from(config_dir).map_err(|error| error.to_string())?;
        let mut restored_config = self.config.clone();
        restored_config.runtime_compensation = Some(expected_compensation.clone());
        let already_restored = current == restored_config;
        if current.runtime_compensation.as_ref() != Some(expected_compensation)
            || (!already_restored && current.runtime_transaction != *expected_runtime_transaction)
        {
            return Err(
                "durable compensation replay found drifted config authority; preserved current state"
                    .into(),
            );
        }
        {
            let app = lock(state);
            if app.proxy.is_some() || app.sandbox.is_some() {
                return Err(
                    "durable compensation replay found process-local runtime owners; refused stale authority restore"
                        .into(),
                );
            }
        }
        if already_restored {
            return Ok(());
        }
        let mut errors = Vec::new();
        let science_restore_allowed = match self.validate_science_restore_root() {
            Ok(()) => true,
            Err(error) => {
                errors.push(error);
                false
            }
        };
        for tree in &mut self.trees {
            if tree.scope == AuthoritySnapshotScope::ScienceData && !science_restore_allowed {
                continue;
            }
            if let Err(error) = tree.restore() {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            let before = self.config.clone();
            if let Err(error) = config::update_result(config_dir, |current| {
                if current.runtime_transaction != *expected_runtime_transaction
                    || current.runtime_compensation.as_ref() != Some(expected_compensation)
                {
                    return Err(
                        "durable compensation replay authority changed during restore; preserved current config"
                            .into(),
                    );
                }
                *current = before.clone();
                current.runtime_compensation = Some(expected_compensation.clone());
                Ok(((), true))
            }) {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            self.preserve_recovery = true;
            Err(errors.join("; "))
        }
    }

    pub(super) fn cleanup_when_expendable(
        &mut self,
    ) -> Result<AuthorityCleanupOutcome, AuthorityCleanupFailure> {
        self.preserve_recovery = true;
        if self.cleanup_ticket.is_none() {
            self.cleanup_ticket = Some(register_authority_cleanup(&self.cleanup_context)?);
        }
        let ticket = self.cleanup_ticket.as_ref().ok_or_else(|| {
            AuthorityCleanupFailure::new(
                AuthorityCleanupPhase::SnapshotRegistration,
                "cleanup_register_failed：事务快照清理票据缺失。",
            )
        })?;
        let cleanup_ticket = prepare_registered_authority_cleanup(&self.cleanup_context, ticket)?;
        self.cleanup_ticket = Some(cleanup_ticket);
        self.cleanup_prepared = true;
        let ticket = self.cleanup_ticket.as_ref().ok_or_else(|| {
            AuthorityCleanupFailure::new(
                AuthorityCleanupPhase::SnapshotRegistration,
                "cleanup_register_failed：cleanup-only 票据缺失。",
            )
        })?;
        match finalize_registered_authority_cleanup(&self.cleanup_context, ticket) {
            Ok(outcome) => {
                self.preserve_recovery = false;
                self.cleanup_prepared = true;
                Ok(outcome)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn prepare_success(
        &mut self,
        value: &mut Value,
    ) -> Result<(), AuthorityCleanupFailure> {
        match self.cleanup_when_expendable() {
            Ok(AuthorityCleanupOutcome::Cleared) => Ok(()),
            Err(error) if self.cleanup_prepared && error.cleanup_requirement().is_some() => {
                self.preserve_recovery = true;
                if let Some(object) = value.as_object_mut() {
                    object.insert("status".into(), Value::String("degraded".into()));
                    object.insert(
                        "recovery_status".into(),
                        Value::String("cleanup_required".into()),
                    );
                    object.insert(
                        "cleanup_message".into(),
                        Value::String("运行事务已完成，但私有事务快照需要稍后安全清理。".into()),
                    );
                }
                Ok(())
            }
            Err(error) => {
                self.preserve_recovery = true;
                Err(error)
            }
        }
    }

    pub(super) fn commit(&mut self) {
        if !self.cleanup_prepared {
            let _ = self.cleanup_when_expendable();
        }
    }
}

impl Drop for OneClickAuthoritySnapshot {
    fn drop(&mut self) {
        // Drop can run during panic unwinding after protected state changed.
        // Only explicit success or fully successful compensation may publish
        // ActiveRecovery -> CleanupOnly and remove the recovery snapshot.
        self.preserve_recovery = true;
    }
}
