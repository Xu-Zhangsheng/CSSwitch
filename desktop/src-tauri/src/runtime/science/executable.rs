fn is_executable_file(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    path.is_file()
        && path
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

fn is_explicit_executable_file(path: &Path) -> bool {
    is_executable_file(path)
}

fn embedded_identity_metadata_matches(details: &str, identifier: &str, team_id: &str) -> bool {
    details
        .lines()
        .any(|line| line == format!("Identifier={identifier}"))
        && details
            .lines()
            .any(|line| line == format!("TeamIdentifier={team_id}"))
}

fn official_updated_embedded_identity_metadata_matches(details: &str) -> bool {
    OFFICIAL_UPDATED_SCIENCE_IDENTIFIERS
        .iter()
        .any(|identifier| {
            embedded_identity_metadata_matches(details, identifier, OFFICIAL_SCIENCE_TEAM_ID)
        })
}

fn official_updated_identity_metadata_matches(path: &Path) -> bool {
    // Official updater executables observed in 0.1.25 use either
    // `com.anthropic.operon` or `com.anthropic.operon.cli`, including an updater
    // seeded byte-for-byte from the installed App.
    // Both currently fail strict cryptographic `codesign --verify`. Treat the
    // exact allowlisted fields only as format/identity guards; the local trust
    // boundary remains the fixed user-owned path plus SHA-256-bound runtime
    // identity below, not a claim of verified official provenance.
    let output = Command::new("/usr/bin/codesign")
        .args(["-d", "--verbose=4"])
        .arg(path)
        .stdout(Stdio::null())
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() || output.stderr.len() > 64 * 1024 {
        return false;
    }
    let details = String::from_utf8_lossy(&output.stderr);
    official_updated_embedded_identity_metadata_matches(&details)
}

fn file_is_macho(path: &Path) -> bool {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path);
    let Ok(mut file) = file else {
        return false;
    };
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() {
        return false;
    }
    matches!(
        magic,
        [0xfe, 0xed, 0xfa, 0xce]
            | [0xce, 0xfa, 0xed, 0xfe]
            | [0xfe, 0xed, 0xfa, 0xcf]
            | [0xcf, 0xfa, 0xed, 0xfe]
            | [0xca, 0xfe, 0xba, 0xbe]
            | [0xbe, 0xba, 0xfe, 0xca]
            | [0xca, 0xfe, 0xba, 0xbf]
            | [0xbf, 0xba, 0xfe, 0xca]
    )
}

fn official_updated_science_bin_for_home(
    home: &Path,
    verify_local_identity: bool,
) -> Option<PathBuf> {
    if !home.is_absolute() || home.canonicalize().ok().as_deref() != Some(home) {
        return None;
    }
    let science_dir = home.join(".claude-science");
    let bin_dir = science_dir.join("bin");
    let candidate = home.join(OFFICIAL_UPDATED_RUNTIME_RELATIVE);
    if !is_executable_file(&candidate) {
        return None;
    }
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    let uid = unsafe { libc::geteuid() };
    for directory in [home, &science_dir, &bin_dir] {
        let metadata = directory.symlink_metadata().ok()?;
        if !metadata.file_type().is_dir()
            || metadata.uid() != uid
            || metadata.permissions().mode() & 0o022 != 0
        {
            return None;
        }
    }
    let metadata = candidate.symlink_metadata().ok()?;
    if !metadata.file_type().is_file()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o111 == 0
        || metadata.permissions().mode() & 0o022 != 0
    {
        return None;
    }
    if verify_local_identity
        && (!(MIN_SCIENCE_BINARY_SIZE..=MAX_SCIENCE_BINARY_SIZE).contains(&metadata.len())
            || !file_is_macho(&candidate)
            || !official_updated_identity_metadata_matches(&candidate))
    {
        return None;
    }
    Some(candidate)
}

fn fingerprint_sha256_hex(fingerprint: &ScienceExecutableFingerprint) -> String {
    fingerprint
        .sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn secure_runtime_snapshot_root(root: &Path) -> Result<PathBuf, String> {
    if !root.is_absolute() {
        return Err("Science runtime snapshot 目录不是绝对路径".into());
    }
    let mut cursor = Some(root);
    while let Some(path) = cursor {
        match path.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("Science runtime snapshot 目录路径包含 symlink，已拒绝使用".into())
            }
            Ok(metadata) if !metadata.file_type().is_dir() => {
                return Err("Science runtime snapshot 目录路径包含非目录文件".into())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("检查 Science runtime snapshot 目录失败：{error}")),
        }
        cursor = path.parent();
    }
    fs::create_dir_all(root)
        .map_err(|error| format!("创建 Science runtime snapshot 目录失败：{error}"))?;
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("确认 Science runtime snapshot 目录失败：{error}"))?;
    if canonical != root {
        return Err("Science runtime snapshot 目录包含 symlink，已拒绝使用".into());
    }
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    let uid = unsafe { libc::geteuid() };
    let metadata = root
        .symlink_metadata()
        .map_err(|error| format!("读取 Science runtime snapshot 目录失败：{error}"))?;
    if !metadata.file_type().is_dir() || metadata.uid() != uid {
        return Err("Science runtime snapshot 目录属主或权限不安全".into());
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("收紧 Science runtime snapshot 目录权限失败：{error}"))?;
    Ok(canonical)
}

fn official_updated_snapshot_from_process_paths(
    snapshot_root: &Path,
    process_paths: &[PathBuf],
    verify_local_identity: bool,
) -> Result<Option<PathBuf>, String> {
    let metadata = match snapshot_root.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("读取 Science runtime snapshot 目录失败：{error}")),
    };
    // SAFETY: geteuid has no preconditions and does not dereference pointers.
    let uid = unsafe { libc::geteuid() };
    if !snapshot_root.is_absolute()
        || metadata.file_type().is_symlink()
        || !metadata.file_type().is_dir()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o022 != 0
        || snapshot_root.canonicalize().ok().as_deref() != Some(snapshot_root)
    {
        return Err("Science runtime snapshot 目录身份或权限不安全".into());
    }
    let mut matching = process_paths
        .iter()
        .filter(|path| path.parent() == Some(snapshot_root));
    let Some(path) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err("Science listener 映射到多个 runtime snapshot，已拒绝恢复".into());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Science runtime snapshot 文件名不可识别")?;
    let Some(expected_sha) = name.strip_prefix("claude-science-") else {
        return Err("Science listener executable 不是内容寻址 snapshot".into());
    };
    if expected_sha.len() != 64 || !expected_sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Science runtime snapshot 文件名 SHA-256 非法".into());
    }
    let metadata = path
        .symlink_metadata()
        .map_err(|error| format!("读取 Science runtime snapshot 文件失败：{error}"))?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o111 == 0
        || metadata.permissions().mode() & 0o022 != 0
        || !(MIN_SCIENCE_BINARY_SIZE..=MAX_SCIENCE_BINARY_SIZE).contains(&metadata.len())
        || path.canonicalize().ok().as_deref() != Some(path.as_path())
    {
        return Err("Science runtime snapshot 文件身份或权限不安全".into());
    }
    let fingerprint =
        science_executable_fingerprint(path).ok_or("Science runtime snapshot 内容无法确认")?;
    if fingerprint_sha256_hex(&fingerprint) != expected_sha.to_ascii_lowercase() {
        return Err("Science runtime snapshot 文件名与内容 SHA-256 不一致".into());
    }
    if verify_local_identity
        && (!file_is_macho(path) || !official_updated_identity_metadata_matches(path))
    {
        return Err("Science runtime snapshot 未通过 Mach-O/embedded metadata 复核".into());
    }
    Ok(Some(path.clone()))
}

fn official_updated_snapshot_for_listener(
    port: u16,
    snapshot_root: &Path,
    verify_local_identity: bool,
) -> Result<Option<PathBuf>, String> {
    let Some(pid) = unique_listener_pid(port) else {
        return Ok(None);
    };
    let Some(process_paths) = process_text_paths(pid) else {
        return Ok(None);
    };
    official_updated_snapshot_from_process_paths(
        snapshot_root,
        &process_paths,
        verify_local_identity,
    )
}

fn official_updated_snapshot_for_home(
    home: &Path,
    snapshot_root: &Path,
    verify_local_identity: bool,
) -> Result<Option<PathBuf>, String> {
    let candidate = home.join(OFFICIAL_UPDATED_RUNTIME_RELATIVE);
    if !candidate.exists() {
        return Ok(None);
    }
    let candidate = official_updated_science_bin_for_home(home, false).ok_or(
        "检测到 updater Science executable，但固定路径、属主或权限校验未通过；已拒绝静默回退旧 App",
    )?;
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&candidate)
        .map_err(|error| format!("打开 updater Science executable 失败：{error}"))?;
    let source_before = source
        .metadata()
        .map_err(|error| format!("读取 updater Science executable 失败：{error}"))?;
    if !source_before.file_type().is_file()
        || !(MIN_SCIENCE_BINARY_SIZE..=MAX_SCIENCE_BINARY_SIZE).contains(&source_before.len())
    {
        return Err("updater Science executable 大小或文件类型不安全；已拒绝静默回退旧 App".into());
    }

    let snapshot_root = secure_runtime_snapshot_root(snapshot_root)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "系统时间异常，无法创建 Science runtime snapshot")?
        .as_nanos();
    let temporary = snapshot_root.join(format!(
        ".claude-science-{}-{nonce}.tmp",
        std::process::id()
    ));
    let result = (|| -> Result<PathBuf, String> {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o500)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary)
            .map_err(|error| format!("创建 Science runtime snapshot 临时文件失败：{error}"))?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|error| format!("读取 updater Science executable 失败：{error}"))?;
            if count == 0 {
                break;
            }
            digest.update(&buffer[..count]);
            output
                .write_all(&buffer[..count])
                .map_err(|error| format!("写入 Science runtime snapshot 失败：{error}"))?;
        }
        output
            .sync_all()
            .map_err(|error| format!("持久化 Science runtime snapshot 失败：{error}"))?;
        drop(output);

        let source_after = source
            .metadata()
            .map_err(|error| format!("复核 updater Science executable 失败：{error}"))?;
        let current = candidate
            .symlink_metadata()
            .map_err(|error| format!("复核 updater Science executable 路径失败：{error}"))?;
        if source_before.dev() != source_after.dev()
            || source_before.ino() != source_after.ino()
            || source_before.size() != source_after.size()
            || source_before.mtime() != source_after.mtime()
            || source_before.mtime_nsec() != source_after.mtime_nsec()
            || source_before.mode() != source_after.mode()
            || source_after.dev() != current.dev()
            || source_after.ino() != current.ino()
            || official_updated_science_bin_for_home(home, false).as_deref()
                != Some(candidate.as_path())
        {
            return Err("updater Science executable 在快照期间发生变化；已拒绝启动，请重试".into());
        }
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o500))
            .map_err(|error| format!("收紧 Science runtime snapshot 权限失败：{error}"))?;
        if verify_local_identity
            && (!file_is_macho(&temporary)
                || !official_updated_identity_metadata_matches(&temporary))
        {
            return Err(
                "updater Science executable 未通过 Mach-O/embedded metadata 本地校验；已拒绝静默回退旧 App"
                    .into(),
            );
        }

        let sha256: [u8; 32] = digest.finalize().into();
        let name: String = sha256.iter().map(|byte| format!("{byte:02x}")).collect();
        let snapshot = snapshot_root.join(format!("claude-science-{name}"));
        match fs::hard_link(&temporary, &snapshot) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!("提交 Science runtime snapshot 失败：{error}"));
            }
        }
        let fingerprint = science_executable_fingerprint(&snapshot)
            .ok_or("Science runtime snapshot 无法重新确认")?;
        if fingerprint.sha256 != sha256
            || fingerprint.size != source_after.size()
            || fingerprint.mode & 0o022 != 0
        {
            return Err("Science runtime snapshot 内容或权限与已验证候选不一致".into());
        }
        Ok(snapshot)
    })();
    let _ = fs::remove_file(&temporary);
    result.map(Some)
}

fn official_updated_science_bin() -> Result<Option<PathBuf>, String> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Ok(None);
    };
    let snapshot_root = config::default_dir().join(OFFICIAL_UPDATED_SNAPSHOT_DIR);
    official_updated_snapshot_for_home(&home, &snapshot_root, true)
}

fn science_executable_fingerprint(path: &Path) -> Option<ScienceExecutableFingerprint> {
    if !is_executable_file(path) {
        return None;
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .ok()?;
    let before = file.metadata().ok()?;
    if !before.file_type().is_file() || before.permissions().mode() & 0o111 == 0 {
        return None;
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let after = file.metadata().ok()?;
    let path_metadata = path.symlink_metadata().ok()?;
    if path_metadata.file_type().is_symlink()
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.size() != after.size()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.mode() != after.mode()
        || after.dev() != path_metadata.dev()
        || after.ino() != path_metadata.ino()
    {
        return None;
    }
    Some(ScienceExecutableFingerprint {
        device: after.dev(),
        inode: after.ino(),
        size: after.size(),
        modified_seconds: after.mtime(),
        modified_nanoseconds: after.mtime_nsec(),
        mode: after.mode(),
        sha256: digest.finalize().into(),
    })
}

fn cached_science_bin(data_dir: &Path) -> PathBuf {
    data_dir.join("bin").join("claude-science")
}

fn create_science_version_output() -> Option<(fs::File, PathBuf)> {
    for _ in 0..8 {
        let nonce = SCIENCE_VERSION_OUTPUT_NONCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            ".csswitch-science-version-{}-{nonce}",
            std::process::id()
        ));
        match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
        {
            Ok(file) => return Some((file, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return None,
        }
    }
    None
}

fn anchored_child_exited(pid: u32) -> Result<bool, i32> {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    loop {
        // SAFETY: info points to writable siginfo_t storage. WNOWAIT observes
        // but does not reap the direct child, so its pid continues to anchor
        // the private process group until Child::wait is called.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            // SAFETY: successful waitid initializes siginfo_t. si_pid == 0
            // is the specified WNOHANG result when no exit is pending.
            return Ok(unsafe { info.assume_init().si_pid() } != 0);
        }
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno != libc::EINTR {
            return Err(errno);
        }
    }
}

fn kill_anchored_science_version_group(pid: u32, leader_exited: bool) -> bool {
    let Ok(pgid) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: waitid(WNOWAIT) or a still-running direct child keeps pid/pgid
    // reserved for the process group created by process_group(0).
    if unsafe { libc::kill(-pgid, libc::SIGKILL) } == 0 {
        return true;
    }
    let errno = std::io::Error::last_os_error().raw_os_error();
    if errno == Some(libc::ESRCH) {
        return true;
    }
    // Darwin returns EPERM, rather than ESRCH, when the anchored process group
    // contains only the WNOWAIT zombie leader. A same-uid executable descendant
    // would make killpg succeed, so accept EPERM only after waitid confirmed the
    // leader's exit; never accept it on the timeout/live-leader path.
    cfg!(target_os = "macos") && leader_exited && errno == Some(libc::EPERM)
}

fn wait_science_version_probe(mut child: Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match anchored_child_exited(child.id()) {
            Ok(true) => {
                let group_stopped = kill_anchored_science_version_group(child.id(), true);
                let status = child.wait().ok()?;
                return group_stopped.then_some(status);
            }
            Ok(false) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(false) => {
                if !kill_anchored_science_version_group(child.id(), false) {
                    let _ = child.kill();
                }
                let cleanup_deadline = Instant::now() + Duration::from_secs(1);
                while Instant::now() < cleanup_deadline {
                    match anchored_child_exited(child.id()) {
                        Ok(true) => {
                            let _ = child.wait();
                            return None;
                        }
                        Ok(false) => std::thread::sleep(Duration::from_millis(20)),
                        Err(_) => break,
                    }
                }
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return None;
            }
            Err(_) => return None,
        }
    }
}

fn safe_science_version_with_timeout(path: &Path, timeout: Duration) -> Option<String> {
    let (mut output_file, output_path) = create_science_version_output()?;
    let result = (|| {
        let stdout = output_file.try_clone().ok()?;
        let mut command = Command::new(path);
        command
            .arg("--version")
            .env("HOME", sandbox_home())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::null())
            .process_group(0);
        // SAFETY: setrlimit is async-signal-safe and the closure touches no
        // shared Rust state. The hard limit is inherited by descendants and
        // prevents a malformed version command from filling TMPDIR.
        unsafe {
            command.pre_exec(|| {
                let limit = libc::rlimit {
                    rlim_cur: 1025 as libc::rlim_t,
                    rlim_max: 1025 as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
        let child = command.spawn().ok()?;
        let status = wait_science_version_probe(child, timeout)?;
        if !status.success() {
            return None;
        }
        output_file.seek(SeekFrom::Start(0)).ok()?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut output_file)
            .take(1025)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 1024 {
            return None;
        }
        let value = String::from_utf8(bytes).ok()?;
        let value = value.lines().next()?.trim();
        if value.is_empty()
            || value.len() > 160
            || !value
                .bytes()
                .all(|byte| byte == b' ' || (0x21..=0x7e).contains(&byte))
        {
            return None;
        }
        Some(value.to_string())
    })();
    drop(output_file);
    let _ = fs::remove_file(output_path);
    result
}

fn safe_science_version(path: &Path) -> Option<String> {
    safe_science_version_with_timeout(path, SCIENCE_VERSION_TIMEOUT)
}

fn runtime_identity(
    path: PathBuf,
    source: ScienceRuntimeSource,
    version_cache: &ScienceVersionCache,
) -> Option<ScienceRuntimeIdentity> {
    for _ in 0..2 {
        let before = science_executable_fingerprint(&path)?;
        let version = version_cache.version(&path)?;
        let after = science_executable_fingerprint(&path)?;
        if before == after {
            return Some(ScienceRuntimeIdentity {
                path,
                source,
                version: Some(version),
                fingerprint: after,
            });
        }
        let _ = version_cache.force_refresh(&path);
    }
    None
}

pub(crate) fn runtime_identity_is_current(runtime: &ScienceRuntimeIdentity) -> bool {
    runtime.is_current()
}

fn explicit_science_bin() -> Result<Option<PathBuf>, String> {
    let Some(path) = std::env::var_os("SCIENCE_BIN").map(PathBuf::from) else {
        return Ok(None);
    };
    if !is_explicit_executable_file(&path) {
        return Err("显式 SCIENCE_BIN 不是安全的绝对可执行文件；已拒绝回退".into());
    }
    Ok(Some(path))
}

#[cfg(test)]
fn science_runtime_preflight_for_paths(
    data_dir: &Path,
    explicit_bin: Option<&Path>,
    app_bin: &Path,
) -> Result<Value, String> {
    science_runtime_preflight_for_paths_cached(
        data_dir,
        explicit_bin,
        None,
        app_bin,
        &ScienceVersionCache::default(),
    )
}

#[cfg(test)]
fn science_runtime_preflight_for_paths_with_updated(
    data_dir: &Path,
    explicit_bin: Option<&Path>,
    official_updated_bin: Option<&Path>,
    app_bin: &Path,
) -> Result<Value, String> {
    science_runtime_preflight_for_paths_cached(
        data_dir,
        explicit_bin,
        official_updated_bin,
        app_bin,
        &ScienceVersionCache::default(),
    )
}

fn science_runtime_preflight_for_paths_cached(
    data_dir: &Path,
    explicit_bin: Option<&Path>,
    official_updated_bin: Option<&Path>,
    app_bin: &Path,
    version_cache: &ScienceVersionCache,
) -> Result<Value, String> {
    if let Some(bin) = explicit_bin {
        if !is_explicit_executable_file(bin) {
            return Err("显式 SCIENCE_BIN 不是安全的绝对可执行文件；已拒绝回退".into());
        }
        let runtime = runtime_identity(
            bin.to_path_buf(),
            ScienceRuntimeSource::Explicit,
            version_cache,
        )
        .ok_or("显式 SCIENCE_BIN 未通过版本预检；已拒绝回退")?;
        return Ok(json!({
            "status": "installed_ready",
            "selected_source": runtime.source.code(),
            "selected_version": runtime.version,
            "cached_version": Value::Null,
            "download_url": SCIENCE_DOWNLOAD_URL,
        }));
    }
    if let Some(bin) = official_updated_bin {
        let runtime = runtime_identity(
            bin.to_path_buf(),
            ScienceRuntimeSource::OfficialUpdated,
            version_cache,
        )
        .ok_or("updater Science snapshot 未通过版本预检；已拒绝回退旧 App")?;
        return Ok(json!({
            "status": "installed_ready",
            "selected_source": runtime.source.code(),
            "selected_version": runtime.version,
            "cached_version": Value::Null,
            "download_url": SCIENCE_DOWNLOAD_URL,
        }));
    }
    if let Some(runtime) = runtime_identity(
        app_bin.to_path_buf(),
        ScienceRuntimeSource::InstalledApp,
        version_cache,
    ) {
        return Ok(json!({
            "status": "installed_ready",
            "selected_source": runtime.source.code(),
            "selected_version": runtime.version,
            "cached_version": Value::Null,
            "download_url": SCIENCE_DOWNLOAD_URL,
        }));
    }
    let cached = cached_science_bin(data_dir);
    let cached_version = version_cache.version(&cached);
    if let Some(version) = cached_version {
        return Ok(json!({
            "status": "cached_choice_required",
            "selected_source": Value::Null,
            "selected_version": Value::Null,
            "cached_version": version,
            "download_url": SCIENCE_DOWNLOAD_URL,
        }));
    }
    Ok(json!({
        "status": "missing",
        "selected_source": Value::Null,
        "selected_version": Value::Null,
        "cached_version": Value::Null,
        "download_url": SCIENCE_DOWNLOAD_URL,
    }))
}

pub(crate) fn science_runtime_preflight(
    version_cache: &ScienceVersionCache,
    _confirmed_stopped: Option<&ScienceRuntimeIdentity>,
) -> Result<Value, String> {
    if let Ok(cfg) = config::load_from(&config::default_dir()) {
        let (state, runtime) = ScienceHostAdapter::probe_cached(cfg.sandbox_port, version_cache)?;
        if state == SandboxScienceState::RunningHealthy {
            let runtime = runtime.ok_or("Science 状态为运行中，但无法确认其 binary 身份")?;
            return Ok(json!({
                "status": "installed_ready",
                "selected_source": runtime.source.code(),
                "selected_version": runtime.version,
                "cached_version": Value::Null,
                "download_url": SCIENCE_DOWNLOAD_URL,
            }));
        }
    }
    let data_dir = sandbox_data_dir();
    let explicit = explicit_science_bin()?;
    let official_updated = official_updated_science_bin()?;
    science_runtime_preflight_for_paths_cached(
        &data_dir,
        explicit.as_deref(),
        official_updated.as_deref(),
        Path::new(SCIENCE_BIN),
        version_cache,
    )
}

#[cfg(test)]
fn select_science_runtime_for_paths(
    data_dir: &Path,
    explicit_bin: Option<&Path>,
    app_bin: &Path,
    choice: Option<&str>,
) -> Result<ScienceRuntimeIdentity, String> {
    select_science_runtime_for_paths_cached(
        data_dir,
        explicit_bin,
        None,
        app_bin,
        choice,
        &ScienceVersionCache::default(),
    )
}

#[cfg(test)]
fn select_science_runtime_for_paths_with_updated(
    data_dir: &Path,
    explicit_bin: Option<&Path>,
    official_updated_bin: Option<&Path>,
    app_bin: &Path,
    choice: Option<&str>,
) -> Result<ScienceRuntimeIdentity, String> {
    select_science_runtime_for_paths_cached(
        data_dir,
        explicit_bin,
        official_updated_bin,
        app_bin,
        choice,
        &ScienceVersionCache::default(),
    )
}

fn select_science_runtime_for_paths_cached(
    data_dir: &Path,
    explicit_bin: Option<&Path>,
    official_updated_bin: Option<&Path>,
    app_bin: &Path,
    choice: Option<&str>,
    version_cache: &ScienceVersionCache,
) -> Result<ScienceRuntimeIdentity, String> {
    if let Some(bin) = explicit_bin {
        if !is_explicit_executable_file(bin) {
            return Err("显式 SCIENCE_BIN 不是安全的绝对可执行文件；已拒绝回退".into());
        }
        return runtime_identity(
            bin.to_path_buf(),
            ScienceRuntimeSource::Explicit,
            version_cache,
        )
        .ok_or_else(|| "显式 SCIENCE_BIN 未通过版本预检；已拒绝回退".to_string());
    }
    if let Some(bin) = official_updated_bin {
        return runtime_identity(
            bin.to_path_buf(),
            ScienceRuntimeSource::OfficialUpdated,
            version_cache,
        )
        .ok_or_else(|| "updater Science snapshot 未通过版本预检；已拒绝回退旧 App".to_string());
    }
    if let Some(runtime) = runtime_identity(
        app_bin.to_path_buf(),
        ScienceRuntimeSource::InstalledApp,
        version_cache,
    ) {
        return Ok(runtime);
    }
    let cached = cached_science_bin(data_dir);
    let cached_version = version_cache.version(&cached);
    if choice == Some(CACHED_ONCE_CHOICE) {
        let _ = cached_version
            .ok_or("缓存 Science 版本无法确认；请安装或更新 Claude Science 后再试")?;
        return runtime_identity(cached, ScienceRuntimeSource::CachedOnce, version_cache)
            .ok_or("缓存 Science 文件在版本确认期间发生变化；已拒绝启动".into());
    }
    if cached_version.is_some() {
        return Err("SCIENCE_RUNTIME_CHOICE_REQUIRED：请明确选择仅本次使用缓存版本，或安装/更新 Claude Science".into());
    }
    Err("找不到可用的 Claude Science App；请先安装或更新 Claude Science".into())
}

pub(crate) fn select_science_runtime_cached(
    choice: Option<&str>,
    version_cache: &ScienceVersionCache,
) -> Result<ScienceRuntimeIdentity, String> {
    let data_dir = sandbox_data_dir();
    let explicit = explicit_science_bin()?;
    let official_updated = official_updated_science_bin()?;
    select_science_runtime_for_paths_cached(
        &data_dir,
        explicit.as_deref(),
        official_updated.as_deref(),
        Path::new(SCIENCE_BIN),
        choice,
        version_cache,
    )
}

fn runtime_probe_candidates(
    port: u16,
    version_cache: &ScienceVersionCache,
) -> Result<Vec<ScienceRuntimeIdentity>, String> {
    if let Some(explicit) = explicit_science_bin()? {
        return Ok(
            runtime_identity(explicit, ScienceRuntimeSource::Explicit, version_cache)
                .into_iter()
                .collect(),
        );
    }
    let mut candidates = Vec::new();
    let snapshot_root = config::default_dir().join(OFFICIAL_UPDATED_SNAPSHOT_DIR);
    if let Some(snapshot) = official_updated_snapshot_for_listener(port, &snapshot_root, true)? {
        if let Some(runtime) = runtime_identity(
            snapshot,
            ScienceRuntimeSource::OfficialUpdated,
            version_cache,
        ) {
            candidates.push(runtime);
        }
    }
    let app = PathBuf::from(SCIENCE_BIN);
    if let Some(app) = runtime_identity(app, ScienceRuntimeSource::InstalledApp, version_cache) {
        candidates.push(app);
    }
    let cached = cached_science_bin(&sandbox_data_dir());
    if let Some(cached) = runtime_identity(cached, ScienceRuntimeSource::CachedOnce, version_cache)
    {
        candidates.push(cached);
    }
    Ok(candidates)
}
