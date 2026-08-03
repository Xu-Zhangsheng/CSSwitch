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
    pub(super) one_click_exit_after_capture: Option<PathBuf>,
    pub(super) one_click_fail_first_journal: Option<PathBuf>,
    pub(super) one_click_fail_finalize_completion: Option<PathBuf>,
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
pub(crate) fn test_arm_one_click_exit_after_snapshot_capture(
    config_dir: PathBuf,
) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .one_click_exit_after_capture = Some(config_dir);
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_arm_one_click_first_journal_failure(
    config_dir: PathBuf,
) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .one_click_fail_first_journal = Some(config_dir);
    SandboxSessionTestSeamGuard
}

#[cfg(test)]
pub(crate) fn test_arm_one_click_finalize_completion_failure(
    config_dir: PathBuf,
) -> SandboxSessionTestSeamGuard {
    SANDBOX_SESSION_TEST_SEAMS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .one_click_fail_finalize_completion = Some(config_dir);
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
