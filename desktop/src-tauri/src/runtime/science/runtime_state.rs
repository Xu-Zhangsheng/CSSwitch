#[cfg(test)]
fn science_status_running(out: &Output) -> bool {
    out.status.success() && science_status_value(out) == Some(true)
}

fn science_status_value(out: &Output) -> Option<bool> {
    let stdout = String::from_utf8_lossy(&out.stdout);
    for (idx, ch) in stdout.char_indices() {
        if ch != '{' {
            continue;
        }
        let mut stream =
            serde_json::Deserializer::from_str(&stdout[idx..]).into_iter::<serde_json::Value>();
        if let Some(Ok(value)) = stream.next() {
            if let Some(running) = value.get("running").and_then(|running| running.as_bool()) {
                return Some(running);
            }
        }
    }
    None
}

#[cfg(test)]
fn trusted_science_status(out: &Output) -> Option<bool> {
    match science_status_value(out) {
        Some(false) => Some(false),
        Some(true) if out.status.success() => Some(true),
        _ => None,
    }
}

fn runtime_status_value(out: &Output) -> Option<bool> {
    match science_status_value(out) {
        Some(false) => Some(false),
        Some(true) if out.status.success() => Some(true),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SandboxScienceState {
    RunningHealthy,
    Stopped,
    Unknown,
}

#[cfg(test)]
fn classify_sandbox_state(
    status: Option<bool>,
    health_ready: bool,
    port_accepts_tcp: bool,
) -> SandboxScienceState {
    match status {
        Some(true) if health_ready => SandboxScienceState::RunningHealthy,
        Some(false) if !port_accepts_tcp => SandboxScienceState::Stopped,
        _ => SandboxScienceState::Unknown,
    }
}

fn classify_known_runtime_state(
    status: Option<bool>,
    health_ready: bool,
    port_accepts_tcp: bool,
    listener_matches_runtime: bool,
) -> SandboxScienceState {
    match status {
        Some(true) if health_ready && listener_matches_runtime => {
            SandboxScienceState::RunningHealthy
        }
        Some(false) if !port_accepts_tcp => SandboxScienceState::Stopped,
        _ => SandboxScienceState::Unknown,
    }
}

fn loopback_port_accepts_tcp(port: u16) -> bool {
    #[cfg(test)]
    if std::env::var_os("CSSWITCH_TEST_PORT_OBSERVATION_TARGET")
        .and_then(|value| value.to_string_lossy().parse::<u16>().ok())
        == Some(port)
    {
        if let Some(sequence_dir) = std::env::var_os("CSSWITCH_TEST_PORT_OBSERVATION_SEQUENCE_DIR")
            .map(PathBuf::from)
            .filter(|path| path.join("armed").is_file())
        {
            let false_marker = sequence_dir.join("observed-false");
            if OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&false_marker)
                .is_ok()
            {
                return false;
            }
            let replaced_marker = sequence_dir.join("receipt-replaced");
            if !replaced_marker.is_file() {
                let replacement =
                    std::env::var_os("CSSWITCH_TEST_PORT_OBSERVATION_REPLACEMENT_RECEIPT")
                        .map(PathBuf::from);
                let target =
                    std::env::var_os("CSSWITCH_TEST_PORT_OBSERVATION_RECEIPT").map(PathBuf::from);
                let replacement_result = (|| -> std::io::Result<()> {
                    let replacement = replacement.ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "replacement receipt path is missing",
                        )
                    })?;
                    let target = target.ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "managed receipt path is missing",
                        )
                    })?;
                    let bytes = fs::read(replacement)?;
                    let temp = sequence_dir.join("concurrent-receipt.tmp");
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                        .open(&temp)?;
                    file.write_all(&bytes)?;
                    file.sync_all()?;
                    fs::rename(&temp, &target)?;
                    File::open(
                        target
                            .parent()
                            .ok_or_else(|| std::io::Error::other("receipt parent is missing"))?,
                    )?
                    .sync_all()?;
                    fs::write(&replaced_marker, b"replaced")
                })();
                if replacement_result.is_err() {
                    let _ = fs::write(sequence_dir.join("receipt-replacement-failed"), b"failed");
                }
            }
            let _ = fs::write(sequence_dir.join("observed-true"), b"true");
            return true;
        }
    }
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok()
}

fn parse_unique_listener_pid(stdout: &str) -> Option<u32> {
    let mut pids = stdout
        .lines()
        .map(str::trim)
        .filter(|pid| !pid.is_empty())
        .map(str::parse::<u32>);
    let pid = pids.next()?.ok()?;
    if pid <= 1 || pids.any(|other| other.ok() != Some(pid)) {
        return None;
    }
    Some(pid)
}

fn unique_listener_pid(port: u16) -> Option<u32> {
    let listener = Command::new("/usr/sbin/lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"])
        .output()
        .ok()?;
    if !listener.status.success() {
        return None;
    }
    parse_unique_listener_pid(&String::from_utf8(listener.stdout).ok()?)
}

fn managed_launch_path() -> PathBuf {
    config::default_dir().join(MANAGED_LAUNCH_FILE)
}

fn process_start_identity(pid: u32) -> Option<String> {
    if pid <= 1 {
        return None;
    }
    #[cfg(test)]
    if std::env::var_os("CSSWITCH_TEST_PROCESS_START_DRIFT_PID")
        .and_then(|value| value.to_string_lossy().parse::<u32>().ok())
        == Some(pid)
        && std::env::var_os("CSSWITCH_TEST_PROCESS_START_DRIFT_MARKER")
            .is_some_and(|path| PathBuf::from(path).is_file())
    {
        return Some("Mon Jan  1 00:00:00 2001".into());
    }
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "lstart="])
        .env_clear()
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.len() > 256 || !output.stderr.is_empty() {
        return None;
    }
    let identity = String::from_utf8(output.stdout).ok()?;
    let identity = identity.trim();
    if identity.is_empty()
        || identity.len() > 128
        || !identity.is_ascii()
        || identity.lines().count() != 1
    {
        return None;
    }
    Some(identity.to_string())
}

fn data_dir_identity() -> Option<(PathBuf, u64, u64)> {
    let data_dir = sandbox_data_dir().canonicalize().ok()?;
    let metadata = data_dir.symlink_metadata().ok()?;
    if !metadata.file_type().is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o022 != 0
    {
        return None;
    }
    Some((data_dir, metadata.dev(), metadata.ino()))
}

#[cfg(test)]
pub(crate) fn test_process_start_identity_for_pid(pid: u32) -> Option<String> {
    process_start_identity(pid)
}

#[cfg(test)]
pub(crate) fn test_unique_listener_pid(port: u16) -> Option<u32> {
    unique_listener_pid(port)
}
