pub(crate) const SCIENCE_BIN: &str =
    "/Applications/Claude Science.app/Contents/Resources/bin/claude-science";
pub(crate) const SCIENCE_DOWNLOAD_URL: &str = "https://claude.com/download";
pub(crate) const CACHED_ONCE_CHOICE: &str = "cached_once";
const OFFICIAL_UPDATED_RUNTIME_RELATIVE: &str = ".claude-science/bin/claude-science";
const OFFICIAL_UPDATED_SCIENCE_IDENTIFIERS: [&str; 2] =
    ["com.anthropic.operon", "com.anthropic.operon.cli"];
const OFFICIAL_SCIENCE_TEAM_ID: &str = "Q6L2SF6YDW";
const MIN_SCIENCE_BINARY_SIZE: u64 = 1024 * 1024;
const MAX_SCIENCE_BINARY_SIZE: u64 = 512 * 1024 * 1024;
const OFFICIAL_UPDATED_SNAPSHOT_DIR: &str = "runtime-snapshots/science";
const SCIENCE_VERSION_TIMEOUT: Duration = Duration::from_secs(15);
const MANAGED_LAUNCH_FILE: &str = "science-managed-launch.v1.json";
const MAX_MANAGED_LAUNCH_BYTES: u64 = 16 * 1024;
static SCIENCE_VERSION_OUTPUT_NONCE: AtomicU64 = AtomicU64::new(1);
#[cfg(test)]
static MANAGED_LAUNCH_LAST_READ_BYTES: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScienceRuntimeSource {
    Explicit,
    OfficialUpdated,
    InstalledApp,
    CachedOnce,
}

impl ScienceRuntimeSource {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::OfficialUpdated => "official_updated",
            Self::InstalledApp => "installed_app",
            Self::CachedOnce => "cached_once",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScienceRuntimeIdentity {
    pub(crate) path: PathBuf,
    pub(crate) source: ScienceRuntimeSource,
    pub(crate) version: Option<String>,
    fingerprint: ScienceExecutableFingerprint,
}

impl ScienceRuntimeIdentity {
    pub(crate) fn environment_transaction_id(&self) -> String {
        fingerprint_sha256_hex(&self.fingerprint)
    }

    pub(crate) fn skill_install_host_context(
        &self,
        sandbox_port: u16,
    ) -> Result<csswitch_skill_install_core::ScienceHostContext, String> {
        let canonical = self
            .path
            .canonicalize()
            .map_err(|_| "Science binary 不可用，无法启用 Skill attach control")?;
        if canonical != self.path {
            return Err("Science binary 不是 canonical path，无法启用 Skill attach control".into());
        }
        let version = self
            .version
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .ok_or("Science 版本未确认，无法启用 Skill attach control")?
            .clone();
        if !self.is_current() {
            return Err("Science binary 在选择后发生变化，无法启用 Skill attach control".into());
        }
        let fingerprint = &self.fingerprint;
        Ok(csswitch_skill_install_core::ScienceHostContext {
            binary: canonical,
            version,
            fingerprint: csswitch_skill_install_core::ScienceExecutableFingerprint {
                device: fingerprint.device,
                inode: fingerprint.inode,
                size: fingerprint.size,
                modified_seconds: fingerprint.modified_seconds,
                modified_nanoseconds: fingerprint.modified_nanoseconds,
                mode: fingerprint.mode,
                sha256: fingerprint_sha256_hex(fingerprint),
            },
            home: sandbox_home(),
            data_dir: sandbox_data_dir(),
            sandbox_port,
        })
    }

    pub(crate) fn is_current(&self) -> bool {
        science_executable_fingerprint(&self.path).as_ref() == Some(&self.fingerprint)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScienceExecutableFingerprint {
    device: u64,
    inode: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    mode: u32,
    sha256: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ScienceManagedLaunchRecord {
    schema_version: u32,
    launch_id: String,
    port: u16,
    listener_pid: u32,
    process_start: String,
    runtime_path: PathBuf,
    runtime_device: u64,
    runtime_inode: u64,
    runtime_size: u64,
    runtime_modified_seconds: i64,
    runtime_modified_nanoseconds: i64,
    runtime_mode: u32,
    runtime_sha256: String,
    data_dir: PathBuf,
    data_dir_device: u64,
    data_dir_inode: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManagedLaunchFileIdentity {
    device: u64,
    inode: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    mode: u32,
    uid: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ScienceManagedLaunchToken {
    record: ScienceManagedLaunchRecord,
    receipt_file: Option<ManagedLaunchFileIdentity>,
}

#[cfg(test)]
static MANAGED_LAUNCH_COMMIT_FAILURE_ONCE_FIRED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(test)]
pub(crate) fn test_reset_managed_launch_commit_failure_once() {
    MANAGED_LAUNCH_COMMIT_FAILURE_ONCE_FIRED.store(false, std::sync::atomic::Ordering::SeqCst);
}

#[derive(Debug)]
pub(crate) struct ScienceManagedLaunchCommitError {
    message: String,
    token: Option<ScienceManagedLaunchToken>,
}

impl ScienceManagedLaunchCommitError {
    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn token(&self) -> Option<&ScienceManagedLaunchToken> {
        self.token.as_ref()
    }
}

#[derive(Clone, Debug)]
struct ScienceVersionCacheEntry {
    fingerprint: ScienceExecutableFingerprint,
    version: String,
}

/// Successful Science `--version` results, shared for one CSSwitch process.
///
/// The dedicated inner lock serializes only the rare version probe. It never
/// holds the broader AppState lock while launching an external process.
#[derive(Clone, Debug, Default)]
pub(crate) struct ScienceVersionCache {
    entries: Arc<Mutex<HashMap<PathBuf, ScienceVersionCacheEntry>>>,
}

impl ScienceVersionCache {
    fn version(&self, path: &Path) -> Option<String> {
        self.version_inner(path, false)
    }

    pub(crate) fn force_refresh(&self, path: &Path) -> Option<String> {
        self.version_inner(path, true)
    }

    fn version_inner(&self, path: &Path, force: bool) -> Option<String> {
        let mut force = force;
        for _ in 0..2 {
            let fingerprint = science_executable_fingerprint(path)?;
            let mut entries = self
                .entries
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if force {
                entries.remove(path);
                force = false;
            } else if let Some(entry) = entries.get(path) {
                if entry.fingerprint == fingerprint {
                    return Some(entry.version.clone());
                }
                entries.remove(path);
            }

            let version = safe_science_version(path)?;
            let Some(after) = science_executable_fingerprint(path) else {
                entries.remove(path);
                return None;
            };
            if after != fingerprint {
                entries.remove(path);
                continue;
            }
            entries.insert(
                path.to_path_buf(),
                ScienceVersionCacheEntry {
                    fingerprint,
                    version: version.clone(),
                },
            );
            return Some(version);
        }
        None
    }

    pub(crate) fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
    }
}

/// 沙箱可写工作目录（独立 HOME）：由构建变体配置根派生；正式构建为
/// `~/.csswitch/sandbox/home`，Acceptance 为 `~/.csswitch-acceptance/sandbox/home`。
pub(crate) fn sandbox_home() -> PathBuf {
    config::default_dir().join("sandbox").join("home")
}

/// CSSwitch 隔离 Science 的持久化 data-dir；其中的 Skill 内容由 Science 自身管理。
pub(crate) fn sandbox_data_dir() -> PathBuf {
    sandbox_home().join(".claude-science")
}

/// 端口变更是否需要拆掉现有链路（纯函数，P1-c）。代理/沙箱任一端口变了，正在跑的代理就绑在
/// 旧端口、正在跑的沙箱又把旧代理 URL 烘死了，二者与新配置不一致 → 拆掉逼下次「一键开始」按新端口重建。
pub(crate) fn settings_change_needs_teardown(
    old_proxy: u16,
    new_proxy: u16,
    old_sandbox: u16,
    new_sandbox: u16,
) -> bool {
    old_proxy != new_proxy || old_sandbox != new_sandbox
}

/// 从 `claude-science url` 的 stdout 里取**第一条**合法 http(s) URL。
pub(crate) fn first_http_url(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let t = line.trim();
        if t.starts_with("http://") || t.starts_with("https://") {
            let url = t.split_whitespace().next().unwrap_or(t);
            return Some(url.to_string());
        }
    }
    None
}
