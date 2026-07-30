//! Authority filesystem snapshot primitives (capture/copy/identity/limits).
//! No pending-cleanup orchestration and no one-click transaction policy live here.

use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

#[allow(clippy::useless_conversion)]
pub(super) fn inode_u64(value: libc::ino_t) -> Option<u64> {
    u64::try_from(value).ok()
}

pub(super) struct AuthorityTreeSnapshot {
    pub(super) scope: AuthoritySnapshotScope,
    pub(super) source: PathBuf,
    pub(super) backup: PathBuf,
    pub(super) existed: bool,
    pub(super) source_parent: Option<std::fs::File>,
    pub(super) source_name: Option<std::ffi::CString>,
    pub(super) backup_identity: Option<(u64, u64, libc::mode_t)>,
    pub(super) backup_parent: Option<std::fs::File>,
    pub(super) backup_name: Option<std::ffi::CString>,
}

pub(super) struct AuthorityDirectoryStream(*mut libc::DIR);

impl Drop for AuthorityDirectoryStream {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

pub(super) const MAX_AUTHORITY_SNAPSHOT_ENTRIES: usize = 131_072;
pub(super) const MAX_AUTHORITY_SNAPSHOT_FILE_BYTES: u64 = 512 * 1024 * 1024;
pub(super) const MAX_AUTHORITY_SNAPSHOT_TOTAL_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub(super) const MAX_AUTHORITY_FULL_COPY_FILE_BYTES: u64 = 128 * 1024 * 1024;
pub(super) const MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
pub(super) const SCIENCE_OWNED_OPAQUE_ROOTS: [&str; 5] =
    ["conda", "runtime", "seed-assets", "r-libs", "sbx-bind-src"];
pub(crate) const SCIENCE_PROTECTED_AUTHORITY_ENTRIES: [&str; 10] = [
    "encryption.key",
    ".oauth-tokens",
    "active-org.json",
    ".key-backups",
    "auth-owner.lock",
    "config.toml",
    "csswitch-ssh-bridge.v1.json",
    "mcp",
    ".csswitch-route-state.json",
    "orgs",
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum AuthoritySnapshotScope {
    ScienceData,
    SandboxState,
    CsswitchRuntime,
    ManagedReceipt,
    #[default]
    Test,
}

impl AuthoritySnapshotScope {
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::ScienceData => "science_data",
            Self::SandboxState => "sandbox_state",
            Self::CsswitchRuntime => "csswitch_runtime",
            Self::ManagedReceipt => "managed_receipt",
            Self::Test => "test",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum AuthoritySnapshotCategory {
    CondaCache,
    ScienceRuntime,
    Skills,
    OrgState,
    #[default]
    Other,
}

impl AuthoritySnapshotCategory {
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::CondaCache => "conda_cache",
            Self::ScienceRuntime => "science_runtime",
            Self::Skills => "skills",
            Self::OrgState => "org_state",
            Self::Other => "other",
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct SandboxSessionTestSeams {
    pub(super) cleanup_fault: Option<(PathBuf, String, PathBuf)>,
    pub(super) cleanup_calls: usize,
    pub(super) capture_fail_source: Option<PathBuf>,
    pub(super) authority_clone_errno: Option<(std::thread::ThreadId, i32)>,
    pub(super) authority_fallback_fail_after_create: Option<std::thread::ThreadId>,
    pub(super) authority_completion_sync_failure: Option<std::thread::ThreadId>,
    pub(super) authority_cleanup_parent_sync_failure: Option<std::thread::ThreadId>,
    pub(super) directory_barrier: Option<(PathBuf, PathBuf)>,
    pub(super) snapshot_parent_barrier: Option<(PathBuf, PathBuf)>,
    pub(super) one_click_capture: Option<(PathBuf, PathBuf, bool, u32, PathBuf)>,
    pub(super) catalog_failure_port: Option<u16>,
    pub(super) catalog_bypass_port: Option<u16>,
    pub(super) prior_restart_post_spawn_failure_port: Option<u16>,
    pub(super) prior_restart_post_spawn_identity: Option<(u32, String)>,
    pub(super) rollback_diagnostic_canary: Option<String>,
    pub(super) rollback_diagnostic_snapshot: Option<PathBuf>,
}

#[cfg(test)]
pub(super) static SANDBOX_SESSION_TEST_SEAMS: std::sync::LazyLock<
    std::sync::Mutex<SandboxSessionTestSeams>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(SandboxSessionTestSeams::default()));

#[cfg(test)]
pub(crate) struct SandboxSessionTestSeamGuard;

#[cfg(test)]
impl Drop for SandboxSessionTestSeamGuard {
    fn drop(&mut self) {
        *SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = SandboxSessionTestSeams::default();
    }
}

#[cfg(test)]
pub(crate) fn test_arm_authority_snapshot_cleanup_fault(
    scope: PathBuf,
    mode: &str,
    log: PathBuf,
) -> SandboxSessionTestSeamGuard {
    let mut seams = SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    seams.cleanup_fault = Some((scope, mode.to_string(), log));
    seams.cleanup_calls = 0;
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_arm_authority_snapshot_capture_failure(
    source: PathBuf,
) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .capture_fail_source = Some(source);
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(super) fn test_arm_authority_snapshot_clone_errno(errno: i32) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .authority_clone_errno = Some((std::thread::current().id(), errno));
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(super) fn test_arm_authority_snapshot_fallback_create_failure() -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .authority_fallback_fail_after_create = Some(std::thread::current().id());
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(super) fn test_arm_authority_snapshot_completion_sync_failure() -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .authority_completion_sync_failure = Some(std::thread::current().id());
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(super) fn test_arm_authority_cleanup_parent_sync_failure() -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .authority_cleanup_parent_sync_failure = Some(std::thread::current().id());
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_arm_authority_snapshot_directory_barrier(
    source: PathBuf,
    barrier: PathBuf,
) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .directory_barrier = Some((source, barrier));
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(super) fn test_arm_authority_snapshot_parent_barrier(
    target: PathBuf,
    barrier: PathBuf,
) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .snapshot_parent_barrier = Some((target, barrier));
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_arm_one_click_snapshot_capture(
    config_dir: PathBuf,
    observation: PathBuf,
    fail: bool,
    expected_prior_pid: u32,
    expected_receipt: PathBuf,
) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .one_click_capture = Some((
        config_dir,
        observation,
        fail,
        expected_prior_pid,
        expected_receipt,
    ));
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_arm_healthy_reopen_catalog_failure(port: u16) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .catalog_failure_port = Some(port);
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_arm_gateway_catalog_bypass(port: u16) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .catalog_bypass_port = Some(port);
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_arm_prior_restart_post_spawn_failure(port: u16) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .prior_restart_post_spawn_failure_port = Some(port);
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_prior_restart_post_spawn_identity() -> Option<(u32, String)> {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .prior_restart_post_spawn_identity
        .clone()
}

#[cfg(test)]
pub(crate) fn test_arm_rollback_diagnostic_canary(canary: &str) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .rollback_diagnostic_canary = Some(canary.to_string());
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_rollback_diagnostic_snapshot() -> Option<PathBuf> {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .rollback_diagnostic_snapshot
        .clone()
}

pub(super) fn sync_authority_cleanup_parent(parent: &std::fs::File) -> std::io::Result<()> {
    #[cfg(test)]
    if SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .authority_cleanup_parent_sync_failure
        .is_some_and(|thread| thread == std::thread::current().id())
    {
        return Err(std::io::Error::from_raw_os_error(libc::EIO));
    }
    parent.sync_all()
}

#[derive(Default)]
pub(super) struct AuthorityCopyBudget {
    pub(super) entries: usize,
    pub(super) bytes: u64,
    pub(super) full_copy_bytes: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct AuthorityDirectoryEntryIdentity {
    pub(super) name: std::ffi::OsString,
    pub(super) kind: u8,
    pub(super) device: u64,
    pub(super) inode: u64,
    pub(super) size: u64,
    pub(super) mode: u32,
    pub(super) modified_seconds: i64,
    pub(super) modified_nanoseconds: i64,
}

impl AuthorityTreeSnapshot {
    pub(super) fn os_error_code(error: &std::io::Error) -> i32 {
        error.raw_os_error().unwrap_or(-1)
    }

    pub(super) fn clone_regular_file_at(
        source_fd: i32,
        parent_fd: i32,
        destination_name: &std::ffi::CStr,
    ) -> Result<(), std::io::Error> {
        #[cfg(test)]
        if let Some((_, errno)) = SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .authority_clone_errno
            .filter(|(thread, _)| *thread == std::thread::current().id())
        {
            return Err(std::io::Error::from_raw_os_error(errno));
        }
        let result =
            unsafe { libc::fclonefileat(source_fd, parent_fd, destination_name.as_ptr(), 0) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub(super) fn open_destination_at(
        parent_fd: i32,
        destination_name: &std::ffi::CStr,
        flags: i32,
        mode: libc::mode_t,
    ) -> Result<std::fs::File, std::io::Error> {
        let fd = unsafe {
            libc::openat(
                parent_fd,
                destination_name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                libc::c_uint::from(mode),
            )
        };
        if fd < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(unsafe { std::fs::File::from_raw_fd(fd) })
        }
    }

    pub(super) fn open_directory_at(
        parent_fd: i32,
        destination_name: &std::ffi::CStr,
    ) -> Result<std::fs::File, std::io::Error> {
        Self::open_destination_at(
            parent_fd,
            destination_name,
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
        )
    }

    pub(super) fn open_absolute_directory(path: &Path) -> Result<std::fs::File, std::io::Error> {
        if !path.is_absolute() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "directory path must be absolute",
            ));
        }
        let mut directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open("/")?;
        for component in path.components() {
            match component {
                std::path::Component::RootDir => {}
                std::path::Component::Normal(name) => {
                    let name = std::ffi::CString::new(name.as_bytes()).map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "directory component contains NUL",
                        )
                    })?;
                    directory = Self::open_directory_at(directory.as_raw_fd(), &name)?;
                }
                _ => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "directory path contains unsupported component",
                    ))
                }
            }
        }
        Ok(directory)
    }

    pub(super) fn open_or_create_authority_snapshot_parent(
        config_dir: &Path,
        sandbox_home: &Path,
    ) -> Result<std::fs::File, String> {
        let expected_parent = sandbox_home
            .parent()
            .ok_or("code=authority_snapshot_root_parent_missing")?;
        if sandbox_home != config_dir.join("sandbox").join("home") {
            return Err("code=authority_snapshot_root_parent_contract_failed".into());
        }
        let config_parent = Self::open_absolute_directory(config_dir).map_err(|error| {
            format!(
                "code=authority_snapshot_config_parent_open_failed os_error={}",
                Self::os_error_code(&error)
            )
        })?;
        let initial_config_parent_metadata = config_parent.metadata().map_err(|error| {
            format!(
                "code=authority_snapshot_config_parent_validate_failed os_error={}",
                Self::os_error_code(&error)
            )
        })?;
        if !initial_config_parent_metadata.is_dir()
            || initial_config_parent_metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err("code=authority_snapshot_config_parent_identity_failed".into());
        }
        config_parent
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(|error| {
                format!(
                    "code=authority_snapshot_config_parent_chmod_failed os_error={}",
                    Self::os_error_code(&error)
                )
            })?;
        let config_parent_metadata = config_parent.metadata().map_err(|error| {
            format!(
                "code=authority_snapshot_config_parent_validate_failed os_error={}",
                Self::os_error_code(&error)
            )
        })?;
        if !config_parent_metadata.is_dir()
            || config_parent_metadata.uid() != unsafe { libc::geteuid() }
            || config_parent_metadata.permissions().mode() & 0o777 != 0o700
        {
            return Err("code=authority_snapshot_config_parent_identity_failed".into());
        }

        let parent_name = Self::destination_name(expected_parent)?;
        match Self::stat_destination_at(&config_parent, &parent_name) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match Self::mkdir_destination_at(config_parent.as_raw_fd(), &parent_name, 0o700) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => {
                        return Err(format!(
                            "code=authority_snapshot_root_parent_create_failed os_error={}",
                            Self::os_error_code(&error)
                        ))
                    }
                }
            }
            Err(error) => {
                return Err(format!(
                    "code=authority_snapshot_root_parent_entry_validate_failed os_error={}",
                    Self::os_error_code(&error)
                ))
            }
        }
        let parent_entry =
            Self::stat_destination_at(&config_parent, &parent_name).map_err(|error| {
                format!(
                    "code=authority_snapshot_root_parent_entry_validate_failed os_error={}",
                    Self::os_error_code(&error)
                )
            })?;
        #[cfg(test)]
        if let Some(barrier) = SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .snapshot_parent_barrier
            .as_ref()
            .filter(|(target, _)| target == expected_parent)
            .map(|(_, barrier)| barrier.clone())
        {
            std::fs::create_dir_all(&barrier).map_err(|error| {
                format!("test-only snapshot parent barrier create failed: {error}")
            })?;
            std::fs::write(barrier.join("ready"), b"ready\n").map_err(|error| {
                format!("test-only snapshot parent barrier arm failed: {error}")
            })?;
            let mut released = false;
            for _ in 0..200 {
                if barrier.join("release").is_file() {
                    released = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if !released {
                return Err("test-only snapshot parent barrier timed out".into());
            }
        }
        let snapshot_parent = Self::open_directory_at(config_parent.as_raw_fd(), &parent_name)
            .map_err(|error| {
                format!(
                    "code=authority_snapshot_root_parent_open_failed os_error={}",
                    Self::os_error_code(&error)
                )
            })?;
        let initial_snapshot_parent_metadata = snapshot_parent.metadata().map_err(|error| {
            format!(
                "code=authority_snapshot_root_parent_validate_failed os_error={}",
                Self::os_error_code(&error)
            )
        })?;
        if !initial_snapshot_parent_metadata.is_dir()
            || initial_snapshot_parent_metadata.uid() != unsafe { libc::geteuid() }
            || !Self::destination_entry_matches_file(
                &parent_entry,
                &initial_snapshot_parent_metadata,
                libc::S_IFDIR,
            )
        {
            return Err("code=authority_snapshot_root_parent_identity_failed".into());
        }
        snapshot_parent
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(|error| {
                format!(
                    "code=authority_snapshot_root_parent_chmod_failed os_error={}",
                    Self::os_error_code(&error)
                )
            })?;
        let snapshot_parent_metadata = snapshot_parent.metadata().map_err(|error| {
            format!(
                "code=authority_snapshot_root_parent_validate_failed os_error={}",
                Self::os_error_code(&error)
            )
        })?;
        if !snapshot_parent_metadata.is_dir()
            || snapshot_parent_metadata.uid() != unsafe { libc::geteuid() }
            || snapshot_parent_metadata.permissions().mode() & 0o777 != 0o700
            || !Self::destination_entry_matches_file(
                &parent_entry,
                &snapshot_parent_metadata,
                libc::S_IFDIR,
            )
        {
            return Err("code=authority_snapshot_root_parent_identity_failed".into());
        }
        if !Self::absolute_directory_binding_matches(config_dir, &config_parent).map_err(
            |error| {
                format!(
                    "code=authority_snapshot_config_parent_revalidate_failed os_error={}",
                    Self::os_error_code(&error)
                )
            },
        )? {
            return Err("code=authority_snapshot_config_parent_rebound".into());
        }
        if !Self::absolute_directory_binding_matches(expected_parent, &snapshot_parent).map_err(
            |error| {
                format!(
                    "code=authority_snapshot_root_parent_revalidate_failed os_error={}",
                    Self::os_error_code(&error)
                )
            },
        )? {
            return Err("code=authority_snapshot_root_parent_rebound".into());
        }
        snapshot_parent.sync_all().map_err(|error| {
            format!(
                "code=authority_snapshot_root_parent_sync_failed os_error={}",
                Self::os_error_code(&error)
            )
        })?;
        config_parent.sync_all().map_err(|error| {
            format!(
                "code=authority_snapshot_config_parent_sync_failed os_error={}",
                Self::os_error_code(&error)
            )
        })?;
        Ok(snapshot_parent)
    }

    pub(super) fn absolute_directory_binding_matches(
        path: &Path,
        pinned: &std::fs::File,
    ) -> Result<bool, std::io::Error> {
        let current = Self::open_absolute_directory(path)?;
        let expected = pinned.metadata()?;
        let actual = current.metadata()?;
        Ok(expected.is_dir()
            && actual.is_dir()
            && expected.dev() == actual.dev()
            && expected.ino() == actual.ino()
            && expected.uid() == actual.uid())
    }

    pub(super) fn read_directory_names(
        directory: &std::fs::File,
    ) -> Result<Vec<std::ffi::OsString>, std::io::Error> {
        let duplicate = unsafe { libc::fcntl(directory.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
        if duplicate < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe { libc::lseek(duplicate, 0, libc::SEEK_SET) } < 0 {
            let error = std::io::Error::last_os_error();
            unsafe {
                libc::close(duplicate);
            }
            return Err(error);
        }
        let stream = unsafe { libc::fdopendir(duplicate) };
        if stream.is_null() {
            let error = std::io::Error::last_os_error();
            unsafe {
                libc::close(duplicate);
            }
            return Err(error);
        }
        let stream = AuthorityDirectoryStream(stream);
        let mut names = Vec::new();
        loop {
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error().unwrap_or(0) == 0 {
                    break;
                }
                return Err(error);
            }
            let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            names.push(std::ffi::OsString::from_vec(name.to_vec()));
        }
        names.sort();
        Ok(names)
    }

    pub(super) fn remove_tree_at(
        destination_parent: &std::fs::File,
        destination_name: &std::ffi::CStr,
    ) -> Result<(), std::io::Error> {
        let entry = Self::stat_destination_at(destination_parent, destination_name)?;
        match entry.st_mode & libc::S_IFMT {
            libc::S_IFDIR => {
                let directory =
                    Self::open_directory_at(destination_parent.as_raw_fd(), destination_name)?;
                let metadata = directory.metadata()?;
                if !Self::destination_entry_matches_file(&entry, &metadata, libc::S_IFDIR) {
                    return Err(std::io::Error::other("directory entry identity changed"));
                }
                for child in Self::read_directory_names(&directory)? {
                    let child = std::ffi::CString::new(child.as_bytes()).map_err(|_| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "directory entry contains NUL",
                        )
                    })?;
                    Self::remove_tree_at(&directory, &child)?;
                }
                directory.sync_all()?;
                let final_entry = Self::stat_destination_at(destination_parent, destination_name)?;
                let final_metadata = directory.metadata()?;
                if !Self::destination_entry_matches_file(
                    &final_entry,
                    &final_metadata,
                    libc::S_IFDIR,
                ) {
                    return Err(std::io::Error::other(
                        "directory entry rebound before removal",
                    ));
                }
                let result = unsafe {
                    libc::unlinkat(
                        destination_parent.as_raw_fd(),
                        destination_name.as_ptr(),
                        libc::AT_REMOVEDIR,
                    )
                };
                if result != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            libc::S_IFREG | libc::S_IFLNK => {
                Self::unlink_destination_at(destination_parent.as_raw_fd(), destination_name)?;
            }
            _ => {
                return Err(std::io::Error::other(
                    "refusing to remove special authority entry",
                ))
            }
        }
        destination_parent.sync_all()
    }

    pub(super) fn mkdir_destination_at(
        parent_fd: i32,
        destination_name: &std::ffi::CStr,
        mode: libc::mode_t,
    ) -> Result<(), std::io::Error> {
        let result = unsafe { libc::mkdirat(parent_fd, destination_name.as_ptr(), mode) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub(super) fn unlink_destination_at(
        parent_fd: i32,
        destination_name: &std::ffi::CStr,
    ) -> Result<(), std::io::Error> {
        let result = unsafe { libc::unlinkat(parent_fd, destination_name.as_ptr(), 0) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub(super) fn destination_name(path: &Path) -> Result<std::ffi::CString, String> {
        let name = path
            .file_name()
            .ok_or("code=authority_snapshot_destination_name_missing")?;
        std::ffi::CString::new(name.as_bytes())
            .map_err(|_| "code=authority_snapshot_destination_name_invalid".into())
    }

    pub(super) fn cleanup_created_destination(
        destination_parent: &std::fs::File,
        destination_name: &std::ffi::CStr,
        scope: AuthoritySnapshotScope,
        category: AuthoritySnapshotCategory,
    ) -> Result<(), String> {
        Self::unlink_destination_at(destination_parent.as_raw_fd(), destination_name)
            .map_err(|error| {
                format!(
                    "code=authority_snapshot_cleanup_unlink_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                )
            })?;
        destination_parent.sync_all().map_err(|error| {
            format!(
                "code=authority_snapshot_cleanup_sync_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
        })
    }

    pub(super) fn stat_destination_at(
        destination_parent: &std::fs::File,
        destination_name: &std::ffi::CStr,
    ) -> Result<libc::stat, std::io::Error> {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
        let result = unsafe {
            libc::fstatat(
                destination_parent.as_raw_fd(),
                destination_name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if result == 0 {
            Ok(unsafe { stat.assume_init() })
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    pub(super) fn destination_entry_matches_file(
        entry: &libc::stat,
        file: &std::fs::Metadata,
        expected_kind: libc::mode_t,
    ) -> bool {
        u64::try_from(entry.st_dev).ok() == Some(file.dev())
            && inode_u64(entry.st_ino) == Some(file.ino())
            && entry.st_mode & libc::S_IFMT == expected_kind
    }

    pub(super) fn readlink_destination_at(
        destination_parent: &std::fs::File,
        destination_name: &std::ffi::CStr,
        expected_len: usize,
    ) -> Result<Vec<u8>, std::io::Error> {
        let mut bytes = vec![0u8; expected_len.saturating_add(1)];
        let length = unsafe {
            libc::readlinkat(
                destination_parent.as_raw_fd(),
                destination_name.as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        };
        if length < 0 {
            return Err(std::io::Error::last_os_error());
        }
        bytes.truncate(length as usize);
        Ok(bytes)
    }

    pub(super) fn category(
        scope: AuthoritySnapshotScope,
        root: &Path,
        current: &Path,
    ) -> AuthoritySnapshotCategory {
        if scope != AuthoritySnapshotScope::ScienceData {
            return AuthoritySnapshotCategory::Other;
        }
        let Ok(relative) = current.strip_prefix(root) else {
            return AuthoritySnapshotCategory::Other;
        };
        let components = relative
            .components()
            .filter_map(|component| component.as_os_str().to_str())
            .collect::<Vec<_>>();
        let Some(first) = components.first().copied() else {
            return AuthoritySnapshotCategory::Other;
        };
        if first == "conda" {
            return AuthoritySnapshotCategory::CondaCache;
        }
        if matches!(first, "runtime" | "seed-assets" | "r-libs" | "sbx-bind-src") {
            return AuthoritySnapshotCategory::ScienceRuntime;
        }
        if components
            .iter()
            .any(|component| matches!(*component, "skills" | "marketplace-plugins"))
        {
            return AuthoritySnapshotCategory::Skills;
        }
        if matches!(
            first,
            ".oauth-tokens"
                | ".key-backups"
                | "active-org.json"
                | "auth-owner.lock"
                | "encryption.key"
                | "mcp"
                | "orgs"
        ) {
            return AuthoritySnapshotCategory::OrgState;
        }
        AuthoritySnapshotCategory::Other
    }

    pub(super) fn stat_entry_stable(initial: &libc::stat, final_entry: &libc::stat) -> bool {
        initial.st_dev == final_entry.st_dev
            && initial.st_ino == final_entry.st_ino
            && initial.st_mode == final_entry.st_mode
            && initial.st_uid == final_entry.st_uid
            && initial.st_gid == final_entry.st_gid
            && initial.st_nlink == final_entry.st_nlink
            && initial.st_size == final_entry.st_size
            && initial.st_mtime == final_entry.st_mtime
            && initial.st_mtime_nsec == final_entry.st_mtime_nsec
    }

    pub(super) fn directory_manifest_at(
        directory: &std::fs::File,
        names: &[std::ffi::OsString],
    ) -> Result<Vec<AuthorityDirectoryEntryIdentity>, String> {
        names
            .iter()
            .map(|name| {
                let name_c = std::ffi::CString::new(name.as_bytes())
                    .map_err(|_| "code=authority_snapshot_source_name_invalid")?;
                let metadata = Self::stat_destination_at(directory, &name_c).map_err(|error| {
                    format!(
                        "code=authority_snapshot_directory_member_validate_failed os_error={}",
                        Self::os_error_code(&error)
                    )
                })?;
                let kind = match metadata.st_mode & libc::S_IFMT {
                    libc::S_IFREG => 1,
                    libc::S_IFDIR => 2,
                    libc::S_IFLNK => 3,
                    _ => 4,
                };
                Ok(AuthorityDirectoryEntryIdentity {
                    name: name.clone(),
                    kind,
                    device: u64::try_from(metadata.st_dev)
                        .map_err(|_| "code=authority_snapshot_source_device_invalid")?,
                    inode: inode_u64(metadata.st_ino)
                        .ok_or("code=authority_snapshot_source_inode_invalid")?,
                    size: u64::try_from(metadata.st_size)
                        .map_err(|_| "code=authority_snapshot_source_size_invalid")?,
                    mode: u32::from(metadata.st_mode) & 0o777,
                    modified_seconds: metadata.st_mtime,
                    modified_nanoseconds: metadata.st_mtime_nsec,
                })
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn capture(source: PathBuf, backup: PathBuf) -> Result<Self, String> {
        Self::capture_scoped(AuthoritySnapshotScope::Test, source, backup)
    }

    #[cfg(test)]
    pub(super) fn capture_scoped(
        scope: AuthoritySnapshotScope,
        source: PathBuf,
        backup: PathBuf,
    ) -> Result<Self, String> {
        let backup_parent = backup
            .parent()
            .ok_or("code=authority_snapshot_destination_parent_missing")?;
        let backup_parent_file = Self::open_absolute_directory(backup_parent).map_err(|error| {
            format!(
                "code=authority_snapshot_destination_parent_open_failed scope={} os_error={}",
                scope.code(),
                Self::os_error_code(&error)
            )
        })?;
        let backup_name = Self::destination_name(&backup)?;
        Self::capture_scoped_at(scope, source, backup, &backup_parent_file, &backup_name)
    }

    pub(super) fn capture_scoped_at(
        scope: AuthoritySnapshotScope,
        source: PathBuf,
        backup: PathBuf,
        backup_parent: &std::fs::File,
        backup_name: &std::ffi::CStr,
    ) -> Result<Self, String> {
        let mut budget = AuthorityCopyBudget::default();
        Self::capture_scoped_at_with_budget(
            scope,
            source,
            backup,
            backup_parent,
            backup_name,
            &mut budget,
        )
    }

    pub(super) fn capture_scoped_at_with_budget(
        scope: AuthoritySnapshotScope,
        source: PathBuf,
        backup: PathBuf,
        backup_parent: &std::fs::File,
        backup_name: &std::ffi::CStr,
        budget: &mut AuthorityCopyBudget,
    ) -> Result<Self, String> {
        let source_parent_path = source
            .parent()
            .ok_or("code=authority_snapshot_source_parent_missing")?;
        let source_parent = match Self::open_absolute_directory(source_parent_path) {
            Ok(parent) => parent,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    scope,
                    source,
                    backup,
                    existed: false,
                    source_parent: None,
                    source_name: None,
                    backup_identity: None,
                    backup_parent: None,
                    backup_name: None,
                })
            }
            Err(error) => {
                return Err(format!(
                    "code=authority_snapshot_source_parent_open_failed scope={} os_error={}",
                    scope.code(),
                    Self::os_error_code(&error)
                ))
            }
        };
        let source_name = Self::destination_name(&source)?;
        Self::capture_scoped_from_parent_with_budget(
            scope,
            source,
            backup,
            &source_parent,
            &source_name,
            backup_parent,
            backup_name,
            budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn capture_scoped_from_parent_with_budget(
        scope: AuthoritySnapshotScope,
        source: PathBuf,
        backup: PathBuf,
        source_parent: &std::fs::File,
        source_name: &std::ffi::CStr,
        backup_parent: &std::fs::File,
        backup_name: &std::ffi::CStr,
        budget: &mut AuthorityCopyBudget,
    ) -> Result<Self, String> {
        #[cfg(test)]
        if SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .capture_fail_source
            .as_ref()
            == Some(&source)
        {
            return Err(format!(
                "test-only authority snapshot capture failure for {}",
                source.display()
            ));
        }
        let source_parent_path = source
            .parent()
            .ok_or("code=authority_snapshot_source_parent_missing")?;
        let backup_identity = match Self::stat_destination_at(source_parent, source_name) {
            Ok(source_identity) => {
                if source_identity.st_mode & libc::S_IFMT == libc::S_IFLNK {
                    return Err(format!(
                        "code=authority_snapshot_root_symlink scope={} category=other",
                        scope.code()
                    ));
                }
                Self::copy_tree_from_at(
                    &source,
                    source_parent,
                    source_name,
                    backup_parent,
                    backup_name,
                    budget,
                    false,
                    scope,
                    &source,
                )?;
                let source_parent_still_bound =
                    Self::absolute_directory_binding_matches(
                        source_parent_path,
                        source_parent,
                    )
                    .map_err(|error| {
                        format!(
                            "code=authority_snapshot_source_parent_revalidate_failed scope={} os_error={}",
                            scope.code(),
                            Self::os_error_code(&error)
                        )
                    })?;
                if !source_parent_still_bound {
                    let primary = format!(
                        "code=authority_snapshot_source_parent_rebound scope={}",
                        scope.code()
                    );
                    return match Self::cleanup_created_destination(
                        backup_parent,
                        backup_name,
                        scope,
                        AuthoritySnapshotCategory::Other,
                    ) {
                        Ok(()) => Err(primary),
                        Err(cleanup) => Err(format!("{primary}; {cleanup}")),
                    };
                }
                let identity =
                    Self::stat_destination_at(backup_parent, backup_name)
                        .map_err(|error| {
                            format!(
                                "code=authority_snapshot_root_entry_validate_failed scope={} os_error={}",
                                scope.code(),
                                Self::os_error_code(&error)
                            )
                        })?;
                Some((
                    u64::try_from(identity.st_dev)
                        .map_err(|_| "code=authority_snapshot_root_device_invalid")?,
                    inode_u64(identity.st_ino)
                        .ok_or("code=authority_snapshot_root_inode_invalid")?,
                    identity.st_mode & libc::S_IFMT,
                ))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!(
                "code=authority_snapshot_root_metadata_failed scope={} category=other os_error={}",
                scope.code(),
                Self::os_error_code(&error)
            ))
            }
        };
        let backup_parent_handle = if backup_identity.is_some() {
            Some(backup_parent.try_clone().map_err(|error| {
                format!(
                    "code=authority_snapshot_backup_parent_pin_failed scope={} os_error={}",
                    scope.code(),
                    Self::os_error_code(&error)
                )
            })?)
        } else {
            None
        };
        let backup_name_handle = backup_identity.as_ref().map(|_| backup_name.to_owned());
        let source_parent_handle = source_parent.try_clone().map_err(|error| {
            format!(
                "code=authority_snapshot_source_parent_pin_failed scope={} os_error={}",
                scope.code(),
                Self::os_error_code(&error)
            )
        })?;
        Ok(Self {
            scope,
            source,
            backup,
            existed: backup_identity.is_some(),
            source_parent: Some(source_parent_handle),
            source_name: Some(source_name.to_owned()),
            backup_identity,
            backup_parent: backup_parent_handle,
            backup_name: backup_name_handle,
        })
    }

    pub(super) fn charge_entry(
        budget: &mut AuthorityCopyBudget,
        file_bytes: u64,
        scope: AuthoritySnapshotScope,
        category: AuthoritySnapshotCategory,
    ) -> Result<(), String> {
        budget.entries = budget.entries.checked_add(1).ok_or_else(|| {
            format!(
                "code=authority_snapshot_entry_overflow scope={} category={}",
                scope.code(),
                category.code()
            )
        })?;
        if budget.entries > MAX_AUTHORITY_SNAPSHOT_ENTRIES {
            return Err(format!(
                "code=authority_snapshot_entry_limit scope={} category={} observed_entries={} entry_limit={MAX_AUTHORITY_SNAPSHOT_ENTRIES}",
                scope.code(),
                category.code(),
                budget.entries
            ));
        }
        if file_bytes > MAX_AUTHORITY_SNAPSHOT_FILE_BYTES {
            return Err(format!(
                "code=authority_snapshot_file_limit scope={} category={} observed_bytes={file_bytes} file_limit={MAX_AUTHORITY_SNAPSHOT_FILE_BYTES}",
                scope.code(),
                category.code()
            ));
        }
        budget.bytes = budget.bytes.checked_add(file_bytes).ok_or_else(|| {
            format!(
                "code=authority_snapshot_total_overflow scope={} category={} observed_entries={}",
                scope.code(),
                category.code(),
                budget.entries
            )
        })?;
        if budget.bytes > MAX_AUTHORITY_SNAPSHOT_TOTAL_BYTES {
            return Err(format!(
                "code=authority_snapshot_total_limit scope={} category={} observed_total_bytes={} total_limit={MAX_AUTHORITY_SNAPSHOT_TOTAL_BYTES} observed_entries={}",
                scope.code(),
                category.code(),
                budget.bytes,
                budget.entries
            ));
        }
        Ok(())
    }

    pub(super) fn charge_full_copy(
        budget: &mut AuthorityCopyBudget,
        file_bytes: u64,
        scope: AuthoritySnapshotScope,
        category: AuthoritySnapshotCategory,
    ) -> Result<(), String> {
        if file_bytes > MAX_AUTHORITY_FULL_COPY_FILE_BYTES {
            return Err(format!(
                "code=authority_snapshot_clone_required scope={} category={} observed_bytes={file_bytes} full_copy_file_limit={MAX_AUTHORITY_FULL_COPY_FILE_BYTES}",
                scope.code(),
                category.code()
            ));
        }
        budget.full_copy_bytes =
            budget
                .full_copy_bytes
                .checked_add(file_bytes)
                .ok_or_else(|| {
                    format!(
                        "code=authority_snapshot_full_copy_overflow scope={} category={}",
                        scope.code(),
                        category.code()
                    )
                })?;
        if budget.full_copy_bytes > MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES {
            return Err(format!(
                "code=authority_snapshot_clone_required scope={} category={} observed_full_copy_bytes={} full_copy_total_limit={MAX_AUTHORITY_FULL_COPY_TOTAL_BYTES}",
                scope.code(),
                category.code(),
                budget.full_copy_bytes
            ));
        }
        Ok(())
    }

    pub(super) fn sync_snapshot_completion(
        backup_root: &std::fs::File,
        snapshot_parent: &std::fs::File,
    ) -> Result<(), std::io::Error> {
        #[cfg(test)]
        if SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .authority_completion_sync_failure
            .is_some_and(|thread| thread == std::thread::current().id())
        {
            return Err(std::io::Error::from_raw_os_error(libc::EIO));
        }
        backup_root.sync_all()?;
        snapshot_parent.sync_all()
    }

    #[cfg(test)]
    pub(super) fn copy_tree(
        source: &Path,
        backup: &Path,
        budget: &mut AuthorityCopyBudget,
        allow_symlink: bool,
        scope: AuthoritySnapshotScope,
        root: &Path,
    ) -> Result<(), String> {
        let parent = backup
            .parent()
            .ok_or("code=authority_snapshot_destination_parent_missing")?;
        let parent_file = Self::open_absolute_directory(parent).map_err(|error| {
            format!(
                "code=authority_snapshot_destination_parent_open_failed scope={} os_error={}",
                scope.code(),
                Self::os_error_code(&error)
            )
        })?;
        let backup_name = Self::destination_name(backup)?;
        let source_parent_path = source
            .parent()
            .ok_or("code=authority_snapshot_source_parent_missing")?;
        let source_parent = Self::open_absolute_directory(source_parent_path).map_err(|error| {
            format!(
                "code=authority_snapshot_source_parent_open_failed scope={} os_error={}",
                scope.code(),
                Self::os_error_code(&error)
            )
        })?;
        let source_name = Self::destination_name(source)?;
        Self::copy_tree_from_at(
            source,
            &source_parent,
            &source_name,
            &parent_file,
            &backup_name,
            budget,
            allow_symlink,
            scope,
            root,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn copy_tree_from_at(
        source_logical: &Path,
        source_parent: &std::fs::File,
        source_name: &std::ffi::CStr,
        destination_parent: &std::fs::File,
        destination_name: &std::ffi::CStr,
        budget: &mut AuthorityCopyBudget,
        allow_symlink: bool,
        scope: AuthoritySnapshotScope,
        root: &Path,
    ) -> Result<(), String> {
        let category = Self::category(scope, root, source_logical);
        let metadata = Self::stat_destination_at(source_parent, source_name).map_err(|error| {
            format!(
                "code=authority_snapshot_metadata_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
        })?;
        let source_kind = metadata.st_mode & libc::S_IFMT;
        if source_kind == libc::S_IFLNK {
            if !allow_symlink {
                return Err(format!(
                    "code=authority_snapshot_root_symlink scope={} category={}",
                    scope.code(),
                    category.code()
                ));
            }
            let expected_len = usize::try_from(metadata.st_size).map_err(|_| {
                format!(
                    "code=authority_snapshot_symlink_size_invalid scope={} category={}",
                    scope.code(),
                    category.code()
                )
            })?;
            let target_bytes =
                Self::readlink_destination_at(source_parent, source_name, expected_len).map_err(
                    |error| {
                        format!(
                    "code=authority_snapshot_symlink_read_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                )
                    },
                )?;
            Self::charge_entry(budget, target_bytes.len() as u64, scope, category)?;
            let target_name = std::ffi::CString::new(target_bytes.clone())
                .map_err(|_| "code=authority_snapshot_symlink_target_invalid")?;
            let symlink_result = unsafe {
                libc::symlinkat(
                    target_name.as_ptr(),
                    destination_parent.as_raw_fd(),
                    destination_name.as_ptr(),
                )
            };
            if symlink_result != 0 {
                let error = std::io::Error::last_os_error();
                return Err(format!(
                    "code=authority_snapshot_symlink_create_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                ));
            }
            let destination_identity =
                Self::stat_destination_at(destination_parent, destination_name)
                    .map_err(|error| {
                        format!(
                            "code=authority_snapshot_symlink_validate_failed scope={} category={} os_error={}",
                            scope.code(),
                            category.code(),
                            Self::os_error_code(&error)
                        )
                    });
            let snapshot_result = (|| -> Result<(), String> {
                let destination_identity = destination_identity?;
                if destination_identity.st_mode & libc::S_IFMT != libc::S_IFLNK {
                    return Err(format!(
                        "code=authority_snapshot_symlink_identity_failed scope={} category={}",
                        scope.code(),
                        category.code()
                    ));
                }
                let final_metadata =
                    Self::stat_destination_at(source_parent, source_name).map_err(
                        |error| {
                            format!(
                                "code=authority_snapshot_symlink_revalidate_failed scope={} category={} os_error={}",
                                scope.code(),
                                category.code(),
                                Self::os_error_code(&error)
                            )
                        },
                    )?;
                let final_target = Self::readlink_destination_at(
                    source_parent,
                    source_name,
                    target_bytes.len(),
                )
                .map_err(|error| {
                    format!(
                        "code=authority_snapshot_symlink_target_revalidate_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
                if !Self::stat_entry_stable(&metadata, &final_metadata)
                    || final_metadata.st_mode & libc::S_IFMT != libc::S_IFLNK
                    || final_target != target_bytes
                {
                    return Err(format!(
                        "code=authority_snapshot_symlink_changed scope={} category={}",
                        scope.code(),
                        category.code()
                    ));
                }
                let final_destination =
                    Self::stat_destination_at(destination_parent, destination_name)
                        .map_err(|error| {
                            format!(
                                "code=authority_snapshot_symlink_entry_revalidate_failed scope={} category={} os_error={}",
                                scope.code(),
                                category.code(),
                                Self::os_error_code(&error)
                            )
                        })?;
                let final_destination_target = Self::readlink_destination_at(
                    destination_parent,
                    destination_name,
                    target_bytes.len(),
                )
                .map_err(|error| {
                    format!(
                        "code=authority_snapshot_symlink_target_validate_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
                if final_destination.st_dev != destination_identity.st_dev
                    || final_destination.st_ino != destination_identity.st_ino
                    || final_destination.st_mode & libc::S_IFMT != libc::S_IFLNK
                    || final_destination_target != target_bytes
                {
                    return Err(format!(
                        "code=authority_snapshot_destination_rebound scope={} category={} kind=symlink",
                        scope.code(),
                        category.code()
                    ));
                }
                destination_parent.sync_all().map_err(|error| {
                    format!(
                        "code=authority_snapshot_parent_sync_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })
            })();
            if let Err(primary) = snapshot_result {
                return match Self::cleanup_created_destination(
                    destination_parent,
                    destination_name,
                    scope,
                    category,
                ) {
                    Ok(()) => Err(primary),
                    Err(cleanup) => Err(format!("{primary}; {cleanup}")),
                };
            }
            return Ok(());
        }
        if source_kind == libc::S_IFREG {
            let source_size = u64::try_from(metadata.st_size).map_err(|_| {
                format!(
                    "code=authority_snapshot_source_size_invalid scope={} category={}",
                    scope.code(),
                    category.code()
                )
            })?;
            let source_mode = u32::from(metadata.st_mode) & 0o777;
            Self::charge_entry(budget, source_size, scope, category)?;
            let mut input = Self::open_destination_at(
                source_parent.as_raw_fd(),
                source_name,
                libc::O_RDONLY,
                0,
            )
            .map_err(|error| {
                format!(
                    "code=authority_snapshot_source_open_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                )
            })?;
            let opened = input
                .metadata()
                .map_err(|error| {
                    format!(
                        "code=authority_snapshot_source_open_validate_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
            if !opened.is_file()
                || !Self::destination_entry_matches_file(&metadata, &opened, libc::S_IFREG)
                || opened.len() != source_size
            {
                return Err(format!(
                    "code=authority_snapshot_source_changed scope={} category={} phase=open",
                    scope.code(),
                    category.code()
                ));
            }
            let parent_fd = destination_parent.as_raw_fd();
            let clone_result =
                Self::clone_regular_file_at(input.as_raw_fd(), parent_fd, destination_name);
            let mut destination_created = clone_result.is_ok();
            let cloned = clone_result.is_ok();
            let snapshot_result = (|| -> Result<(), String> {
                let output = match clone_result {
                    Ok(()) => Self::open_destination_at(
                        parent_fd,
                        destination_name,
                        libc::O_RDONLY,
                        0,
                    )
                    .map_err(|error| {
                        format!(
                            "code=authority_snapshot_clone_open_failed scope={} category={} os_error={}",
                            scope.code(),
                            category.code(),
                            Self::os_error_code(&error)
                        )
                    })?,
                    Err(clone_error)
                        if matches!(
                            clone_error.raw_os_error(),
                            Some(libc::ENOTSUP) | Some(libc::EXDEV)
                        ) =>
                    {
                        Self::charge_full_copy(budget, source_size, scope, category)?;
                        let mut output = Self::open_destination_at(
                            parent_fd,
                            destination_name,
                            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
                            0o600,
                        )
                        .map_err(|error| {
                            format!(
                                "code=authority_snapshot_copy_create_failed scope={} category={} os_error={}",
                                scope.code(),
                                category.code(),
                                Self::os_error_code(&error)
                            )
                        })?;
                        destination_created = true;
                        #[cfg(test)]
                        if SANDBOX_SESSION_TEST_SEAMS
                            .lock()
                            .unwrap_or_else(|error| error.into_inner())
                            .authority_fallback_fail_after_create
                            .is_some_and(|thread| thread == std::thread::current().id())
                        {
                            return Err(format!(
                                "code=authority_snapshot_copy_injected_failure scope={} category={}",
                                scope.code(),
                                category.code()
                            ));
                        }
                        let copied = std::io::copy(&mut input, &mut output).map_err(|error| {
                            format!(
                                "code=authority_snapshot_copy_failed scope={} category={} os_error={}",
                                scope.code(),
                                category.code(),
                                Self::os_error_code(&error)
                            )
                        })?;
                        if copied != source_size {
                            return Err(format!(
                                "code=authority_snapshot_source_changed scope={} category={} phase=copy",
                                scope.code(),
                                category.code()
                            ));
                        }
                        output
                    }
                    Err(clone_error) => {
                        return Err(format!(
                            "code=authority_snapshot_clone_failed scope={} category={} os_error={}",
                            scope.code(),
                            category.code(),
                            Self::os_error_code(&clone_error)
                        ))
                    }
                };
                output
                    .set_permissions(std::fs::Permissions::from_mode(source_mode))
                    .map_err(|error| {
                        format!(
                            "code=authority_snapshot_chmod_failed scope={} category={} os_error={}",
                            scope.code(),
                            category.code(),
                            Self::os_error_code(&error)
                        )
                    })?;
                let saved = output.metadata().map_err(|error| {
                    format!(
                        "code=authority_snapshot_validate_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
                if !saved.is_file()
                    || (cloned && saved.dev() != opened.dev())
                    || (saved.dev() == opened.dev() && saved.ino() == opened.ino())
                    || saved.len() != opened.len()
                    || saved.permissions().mode() & 0o777 != source_mode
                {
                    return Err(format!(
                        "code=authority_snapshot_independence_failed scope={} category={}",
                        scope.code(),
                        category.code()
                    ));
                }
                let destination_entry =
                    Self::stat_destination_at(destination_parent, destination_name)
                        .map_err(|error| {
                            format!(
                                "code=authority_snapshot_entry_revalidate_failed scope={} category={} os_error={}",
                                scope.code(),
                                category.code(),
                                Self::os_error_code(&error)
                            )
                        })?;
                if !Self::destination_entry_matches_file(&destination_entry, &saved, libc::S_IFREG)
                {
                    return Err(format!(
                        "code=authority_snapshot_destination_rebound scope={} category={} kind=file",
                        scope.code(),
                        category.code()
                    ));
                }
                let final_entry =
                    Self::stat_destination_at(source_parent, source_name).map_err(
                        |error| {
                            format!(
                                "code=authority_snapshot_source_entry_revalidate_failed scope={} category={} os_error={}",
                                scope.code(),
                                category.code(),
                                Self::os_error_code(&error)
                            )
                        },
                    )?;
                let final_metadata = input.metadata().map_err(|error| {
                    format!(
                        "code=authority_snapshot_source_revalidate_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
                if !Self::stat_entry_stable(&metadata, &final_entry)
                    || !Self::destination_entry_matches_file(
                        &final_entry,
                        &final_metadata,
                        libc::S_IFREG,
                    )
                    || final_metadata.dev() != opened.dev()
                    || final_metadata.ino() != opened.ino()
                    || final_metadata.len() != opened.len()
                    || final_metadata.permissions().mode() & 0o777
                        != opened.permissions().mode() & 0o777
                    || final_metadata.mtime() != opened.mtime()
                    || final_metadata.mtime_nsec() != opened.mtime_nsec()
                {
                    return Err(format!(
                        "code=authority_snapshot_source_changed scope={} category={} phase=revalidate",
                        scope.code(),
                        category.code()
                    ));
                }
                output.sync_all().map_err(|error| {
                    format!(
                        "code=authority_snapshot_sync_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
                destination_parent.sync_all().map_err(|error| {
                    format!(
                        "code=authority_snapshot_parent_sync_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
                Ok(())
            })();
            if let Err(primary) = snapshot_result {
                if destination_created {
                    return match Self::cleanup_created_destination(
                        destination_parent,
                        destination_name,
                        scope,
                        category,
                    ) {
                        Ok(()) => Err(primary),
                        Err(cleanup) => Err(format!("{primary}; {cleanup}")),
                    };
                }
                return Err(primary);
            }
            return Ok(());
        }
        if source_kind != libc::S_IFDIR {
            return Err(format!(
                "code=authority_snapshot_special_file scope={} category={}",
                scope.code(),
                category.code()
            ));
        }
        let source_directory =
            Self::open_directory_at(source_parent.as_raw_fd(), source_name).map_err(
                |error| {
                    format!(
                        "code=authority_snapshot_directory_source_open_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                },
            )?;
        let source_directory_metadata = source_directory.metadata().map_err(|error| {
            format!(
                "code=authority_snapshot_directory_source_validate_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
        })?;
        if !source_directory_metadata.is_dir()
            || !Self::destination_entry_matches_file(
                &metadata,
                &source_directory_metadata,
                libc::S_IFDIR,
            )
        {
            return Err(format!(
                "code=authority_snapshot_source_changed scope={} category={} phase=directory_open",
                scope.code(),
                category.code()
            ));
        }
        let source_mode = u32::from(metadata.st_mode) & 0o777;
        Self::charge_entry(budget, 0, scope, category)?;
        Self::mkdir_destination_at(destination_parent.as_raw_fd(), destination_name, 0o700)
            .map_err(|error| {
                format!(
                "code=authority_snapshot_directory_create_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
            })?;
        let created_destination_entry =
            Self::stat_destination_at(destination_parent, destination_name)
                .map_err(|error| {
                    format!(
                        "code=authority_snapshot_directory_entry_validate_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
        let destination_directory =
            Self::open_directory_at(destination_parent.as_raw_fd(), destination_name).map_err(
                |error| {
                    format!(
                "code=authority_snapshot_directory_open_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
                },
            )?;
        destination_directory
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(|error| {
                format!(
                    "code=authority_snapshot_directory_chmod_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                )
            })?;
        let destination_metadata = destination_directory.metadata().map_err(|error| {
            format!(
                "code=authority_snapshot_directory_validate_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
        })?;
        if !destination_metadata.is_dir()
            || destination_metadata.file_type().is_symlink()
            || destination_metadata.uid() != unsafe { libc::geteuid() }
            || (destination_metadata.dev() == source_directory_metadata.dev()
                && destination_metadata.ino() == source_directory_metadata.ino())
            || !Self::destination_entry_matches_file(
                &created_destination_entry,
                &destination_metadata,
                libc::S_IFDIR,
            )
        {
            return Err(format!(
                "code=authority_snapshot_directory_identity_failed scope={} category={}",
                scope.code(),
                category.code()
            ));
        }
        let children = Self::read_directory_names(&source_directory).map_err(|error| {
            format!(
                "code=authority_snapshot_directory_enumerate_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
        })?;
        let initial_manifest = Self::directory_manifest_at(&source_directory, &children)?;
        #[cfg(test)]
        if let Some(barrier) = SANDBOX_SESSION_TEST_SEAMS
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .directory_barrier
            .as_ref()
            .filter(|(target, _)| target == source_logical)
            .map(|(_, barrier)| barrier.clone())
        {
            std::fs::create_dir_all(&barrier)
                .map_err(|error| format!("test-only snapshot barrier create failed: {error}"))?;
            std::fs::write(barrier.join("ready"), b"ready\n")
                .map_err(|error| format!("test-only snapshot barrier arm failed: {error}"))?;
            let mut released = false;
            for _ in 0..200 {
                if barrier.join("release").is_file() {
                    released = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if !released {
                return Err("test-only authority snapshot barrier timed out".into());
            }
        }
        for child in &children {
            let child_name = std::ffi::CString::new(child.as_bytes()).map_err(|_| {
                format!(
                    "code=authority_snapshot_source_name_invalid scope={} category={}",
                    scope.code(),
                    category.code()
                )
            })?;
            let child_logical = source_logical.join(child);
            Self::copy_tree_from_at(
                &child_logical,
                &source_directory,
                &child_name,
                &destination_directory,
                &child_name,
                budget,
                true,
                scope,
                root,
            )?;
        }
        let final_children =
            Self::read_directory_names(&source_directory).map_err(|error| {
                format!(
                    "code=authority_snapshot_directory_reenumerate_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                )
            })?;
        let final_manifest = Self::directory_manifest_at(&source_directory, &final_children)?;
        let final_entry =
            Self::stat_destination_at(source_parent, source_name).map_err(|error| {
                format!(
                    "code=authority_snapshot_directory_source_entry_revalidate_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                )
            })?;
        let final_metadata = source_directory.metadata().map_err(|error| {
            format!(
                "code=authority_snapshot_directory_source_revalidate_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
        })?;
        let membership_stable = initial_manifest == final_manifest && children == final_children;
        let entry_stable = Self::stat_entry_stable(&metadata, &final_entry);
        let binding_stable =
            Self::destination_entry_matches_file(&final_entry, &final_metadata, libc::S_IFDIR);
        let opened_stable = final_metadata.dev() == source_directory_metadata.dev()
            && final_metadata.ino() == source_directory_metadata.ino()
            && final_metadata.uid() == source_directory_metadata.uid()
            && final_metadata.permissions().mode() & 0o777
                == source_directory_metadata.permissions().mode() & 0o777
            && final_metadata.mtime() == source_directory_metadata.mtime()
            && final_metadata.mtime_nsec() == source_directory_metadata.mtime_nsec();
        if !membership_stable
            || !final_metadata.is_dir()
            || !entry_stable
            || !binding_stable
            || !opened_stable
        {
            let manifest_detail = initial_manifest
                .iter()
                .zip(&final_manifest)
                .enumerate()
                .find_map(|(index, (initial, final_entry))| {
                    (initial != final_entry).then(|| {
                        format!(
                            " first_mismatch_index={index} name_equal={} kind_equal={} device_equal={} inode_equal={} size_equal={} mode_equal={} mtime_equal={}",
                            initial.name == final_entry.name,
                            initial.kind == final_entry.kind,
                            initial.device == final_entry.device,
                            initial.inode == final_entry.inode,
                            initial.size == final_entry.size,
                            initial.mode == final_entry.mode,
                            initial.modified_seconds == final_entry.modified_seconds
                                && initial.modified_nanoseconds
                                    == final_entry.modified_nanoseconds
                        )
                    })
                })
                .unwrap_or_default();
            return Err(format!(
                "code=authority_snapshot_directory_changed scope={} category={} membership_stable={} entry_stable={} binding_stable={} opened_stable={} initial_entries={} final_entries={}{}",
                scope.code(),
                category.code(),
                membership_stable,
                entry_stable,
                binding_stable,
                opened_stable,
                initial_manifest.len(),
                final_manifest.len(),
                manifest_detail
            ));
        }
        destination_directory
            .set_permissions(std::fs::Permissions::from_mode(
                source_mode,
            ))
            .map_err(|error| {
                format!(
                    "code=authority_snapshot_directory_chmod_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                )
            })?;
        let final_destination_metadata =
            destination_directory.metadata().map_err(|error| {
                format!(
                    "code=authority_snapshot_directory_validate_failed scope={} category={} os_error={}",
                    scope.code(),
                    category.code(),
                    Self::os_error_code(&error)
                )
            })?;
        let final_destination_entry =
            Self::stat_destination_at(destination_parent, destination_name)
                .map_err(|error| {
                    format!(
                        "code=authority_snapshot_directory_entry_revalidate_failed scope={} category={} os_error={}",
                        scope.code(),
                        category.code(),
                        Self::os_error_code(&error)
                    )
                })?;
        if !Self::destination_entry_matches_file(
            &final_destination_entry,
            &final_destination_metadata,
            libc::S_IFDIR,
        ) {
            return Err(format!(
                "code=authority_snapshot_destination_rebound scope={} category={} kind=directory",
                scope.code(),
                category.code()
            ));
        }
        destination_directory.sync_all().map_err(|error| {
            format!(
                "code=authority_snapshot_directory_sync_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
        })?;
        destination_parent.sync_all().map_err(|error| {
            format!(
                "code=authority_snapshot_parent_sync_failed scope={} category={} os_error={}",
                scope.code(),
                category.code(),
                Self::os_error_code(&error)
            )
        })?;
        Ok(())
    }

    pub(super) fn remove_current_at(
        scope: AuthoritySnapshotScope,
        parent: &std::fs::File,
        name: &std::ffi::CStr,
    ) -> Result<(), String> {
        let metadata = match Self::stat_destination_at(parent, name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return parent.sync_all().map_err(|sync_error| {
                    format!(
                        "code=authority_restore_parent_sync_failed scope={} os_error={}",
                        scope.code(),
                        Self::os_error_code(&sync_error)
                    )
                })
            }
            Err(error) => {
                return Err(format!(
                    "code=authority_restore_metadata_failed scope={} os_error={}",
                    scope.code(),
                    Self::os_error_code(&error)
                ))
            }
        };
        if metadata.st_mode & libc::S_IFMT == libc::S_IFLNK {
            return Err(format!(
                "code=authority_restore_root_symlink scope={}",
                scope.code()
            ));
        }
        if matches!(
            metadata.st_mode & libc::S_IFMT,
            libc::S_IFDIR | libc::S_IFREG
        ) {
            Self::remove_tree_at(parent, name).map_err(|error| {
                format!(
                    "code=authority_restore_remove_failed scope={} os_error={}",
                    scope.code(),
                    Self::os_error_code(&error)
                )
            })
        } else {
            Err(format!(
                "code=authority_restore_special_file scope={}",
                scope.code()
            ))
        }
    }

    pub(super) fn validate_backup_identity(&self) -> Result<(), String> {
        let Some((expected_device, expected_inode, expected_kind)) = self.backup_identity else {
            return if self.existed {
                Err(format!(
                    "code=authority_restore_backup_identity_missing scope={}",
                    self.scope.code()
                ))
            } else {
                Ok(())
            };
        };
        let parent = self.backup_parent.as_ref().ok_or_else(|| {
            format!(
                "code=authority_restore_backup_parent_missing scope={}",
                self.scope.code()
            )
        })?;
        let name = self.backup_name.as_deref().ok_or_else(|| {
            format!(
                "code=authority_restore_backup_name_missing scope={}",
                self.scope.code()
            )
        })?;
        let parent_path = self
            .backup
            .parent()
            .ok_or("code=authority_restore_backup_parent_missing")?;
        if !Self::absolute_directory_binding_matches(parent_path, parent).map_err(|error| {
            format!(
                "code=authority_restore_backup_parent_revalidate_failed scope={} os_error={}",
                self.scope.code(),
                Self::os_error_code(&error)
            )
        })? {
            return Err(format!(
                "code=authority_restore_backup_parent_rebound scope={}",
                self.scope.code()
            ));
        }
        let current = Self::stat_destination_at(parent, name).map_err(|error| {
            format!(
                "code=authority_restore_backup_validate_failed scope={} os_error={}",
                self.scope.code(),
                Self::os_error_code(&error)
            )
        })?;
        if u64::try_from(current.st_dev).ok() != Some(expected_device)
            || inode_u64(current.st_ino) != Some(expected_inode)
            || current.st_mode & libc::S_IFMT != expected_kind
        {
            return Err(format!(
                "code=authority_restore_backup_identity_changed scope={}",
                self.scope.code()
            ));
        }
        Ok(())
    }

    pub(super) fn restore(&mut self) -> Result<(), String> {
        self.validate_backup_identity()?;
        let parent_path = self.source.parent().ok_or("隔离 authority 没有父目录")?;
        let opened_parent;
        let parent = match self.source_parent.as_ref() {
            Some(parent) => {
                if !Self::absolute_directory_binding_matches(parent_path, parent).map_err(
                    |error| {
                        format!(
                            "code=authority_restore_parent_revalidate_failed scope={} os_error={}",
                            self.scope.code(),
                            Self::os_error_code(&error)
                        )
                    },
                )? {
                    return Err(format!(
                        "code=authority_restore_parent_rebound scope={}",
                        self.scope.code()
                    ));
                }
                parent
            }
            None => {
                opened_parent = match Self::open_absolute_directory(parent_path) {
                    Ok(parent) => parent,
                    Err(error) if !self.existed && error.kind() == std::io::ErrorKind::NotFound => {
                        return Ok(())
                    }
                    Err(error) => {
                        return Err(format!(
                            "code=authority_restore_parent_open_failed scope={} os_error={}",
                            self.scope.code(),
                            Self::os_error_code(&error)
                        ))
                    }
                };
                &opened_parent
            }
        };
        let computed_source_name;
        let source_name = match self.source_name.as_deref() {
            Some(name) => name,
            None => {
                computed_source_name = Self::destination_name(&self.source)?;
                &computed_source_name
            }
        };
        Self::remove_current_at(self.scope, parent, source_name)?;
        if self.existed {
            let mut budget = AuthorityCopyBudget::default();
            let backup_root = self.backup.clone();
            let backup_parent = self.backup_parent.as_ref().ok_or_else(|| {
                format!(
                    "code=authority_restore_backup_parent_missing scope={}",
                    self.scope.code()
                )
            })?;
            let backup_name = self.backup_name.as_deref().ok_or_else(|| {
                format!(
                    "code=authority_restore_backup_name_missing scope={}",
                    self.scope.code()
                )
            })?;
            Self::copy_tree_from_at(
                &self.backup,
                backup_parent,
                backup_name,
                parent,
                source_name,
                &mut budget,
                false,
                self.scope,
                &backup_root,
            )
            .map_err(|error| format!("无法恢复隔离 authority：{error}"))?;
            self.validate_backup_identity()?;
        }
        if !Self::absolute_directory_binding_matches(parent_path, parent).map_err(|error| {
            format!(
                "code=authority_restore_parent_revalidate_failed scope={} os_error={}",
                self.scope.code(),
                Self::os_error_code(&error)
            )
        })? {
            return Err(format!(
                "code=authority_restore_parent_rebound scope={}",
                self.scope.code()
            ));
        }
        Ok(())
    }
}
