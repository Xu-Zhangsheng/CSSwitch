fn process_text_paths(pid: u32) -> Option<Vec<PathBuf>> {
    let pid_text = pid.to_string();
    let text_files = Command::new("/usr/sbin/lsof")
        .args(["-nP", "-a", "-p", &pid_text, "-d", "txt", "-Fn"])
        .output()
        .ok()?;
    if !text_files.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&text_files.stdout)
            .lines()
            .filter_map(|line| line.strip_prefix('n'))
            .filter_map(|path| Path::new(path).canonicalize().ok())
            .collect(),
    )
}

fn listener_runtime_pid(port: u16, runtime: &ScienceRuntimeIdentity) -> Option<u32> {
    if !runtime.is_current() {
        return None;
    }
    let pid = unique_listener_pid(port)?;
    #[cfg(test)]
    {
        let pid_text = pid.to_string();
        if test_listener_marker_matches(&pid_text, runtime) {
            return Some(pid);
        }
    }
    let expected = runtime.path.canonicalize().ok()?;
    process_text_paths(pid)?
        .into_iter()
        .any(|path| path == expected)
        .then_some(pid)
}

fn listener_uses_runtime(port: u16, runtime: &ScienceRuntimeIdentity) -> bool {
    listener_runtime_pid(port, runtime).is_some()
}

#[cfg(test)]
fn test_listener_marker_matches(pid: &str, runtime: &ScienceRuntimeIdentity) -> bool {
    if std::env::var("CSSWITCH_TEST_FAKE_SCIENCE_IDENTITY")
        .ok()
        .as_deref()
        != Some("1")
    {
        return false;
    }
    let Some(configured) = std::env::var_os("SCIENCE_BIN").map(PathBuf::from) else {
        return false;
    };
    if configured.canonicalize().ok() != runtime.path.canonicalize().ok() {
        return false;
    }
    std::fs::read_to_string(sandbox_data_dir().join("fake-science/pid"))
        .ok()
        .is_some_and(|recorded| recorded.trim() == pid)
}

/// Return the sandbox UI URL, falling back to the plain localhost port.
pub(crate) fn sandbox_url(port: u16, runtime: &ScienceRuntimeIdentity) -> String {
    let home = sandbox_home();
    let data_dir = sandbox_data_dir();
    if runtime.is_current() {
        if let Ok(out) = Command::new(&runtime.path)
            .arg("url")
            .arg("--data-dir")
            .arg(&data_dir)
            .env("HOME", &home)
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(url) = first_http_url(&s) {
                return url;
            }
        }
    }
    format!("http://127.0.0.1:{port}")
}

fn runtime_status(runtime: &ScienceRuntimeIdentity) -> Option<bool> {
    if !runtime.is_current() {
        return None;
    }
    let out = Command::new(&runtime.path)
        .arg("status")
        .arg("--data-dir")
        .arg(sandbox_data_dir())
        .env("HOME", sandbox_home())
        .output()
        .ok()?;
    // Some Science builds use a non-zero exit to mean "not running" while
    // still returning a valid {"running":false} payload. Accept only that
    // negative result; a non-zero positive or malformed response stays unknown.
    runtime_status_value(&out)
}

pub(crate) fn probe_known_runtime(
    port: u16,
    runtime: &ScienceRuntimeIdentity,
) -> SandboxScienceState {
    let status = runtime_status(runtime);
    let health_ready = proc::http_health(port, None, 400);
    let port_accepts_tcp = health_ready || loopback_port_accepts_tcp(port);
    let listener_matches_runtime = status == Some(true)
        && health_ready
        && listener_uses_runtime(port, runtime)
        && managed_launch_identity_matches(port, runtime);
    classify_known_runtime_state(
        status,
        health_ready,
        port_accepts_tcp,
        listener_matches_runtime,
    )
}

pub(crate) fn probe_sandbox_runtime(
    port: u16,
) -> Result<(SandboxScienceState, Option<ScienceRuntimeIdentity>), String> {
    probe_sandbox_runtime_cached(port, &ScienceVersionCache::default())
}

pub(crate) fn probe_sandbox_runtime_cached(
    port: u16,
    version_cache: &ScienceVersionCache,
) -> Result<(SandboxScienceState, Option<ScienceRuntimeIdentity>), String> {
    let health_ready = proc::http_health(port, None, 400);
    let port_accepts_tcp = health_ready || loopback_port_accepts_tcp(port);
    let candidates = runtime_probe_candidates(port, version_cache)?;
    let no_candidates = candidates.is_empty();
    let mut saw_stopped = false;
    let mut saw_running_unconfirmed = false;
    for runtime in candidates {
        match runtime_status(&runtime) {
            Some(true)
                if health_ready
                    && listener_uses_runtime(port, &runtime)
                    && managed_launch_identity_matches(port, &runtime) =>
            {
                return Ok((SandboxScienceState::RunningHealthy, Some(runtime)))
            }
            Some(true) => saw_running_unconfirmed = true,
            Some(false) => saw_stopped = true,
            None => {}
        }
    }
    if saw_running_unconfirmed {
        return Ok((SandboxScienceState::Unknown, None));
    }
    if !port_accepts_tcp && (saw_stopped || !sandbox_data_dir().exists()) {
        return Ok((SandboxScienceState::Stopped, None));
    }
    if !port_accepts_tcp && no_candidates {
        return Ok((SandboxScienceState::Stopped, None));
    }
    Ok((SandboxScienceState::Unknown, None))
}

fn stop_runtime_from_probe(
    state: SandboxScienceState,
    runtime: Option<ScienceRuntimeIdentity>,
) -> Result<Option<ScienceRuntimeIdentity>, String> {
    match (state, runtime) {
        (SandboxScienceState::Stopped, _) => Ok(None),
        (SandboxScienceState::RunningHealthy, Some(runtime)) => Ok(Some(runtime)),
        (SandboxScienceState::RunningHealthy, None) => {
            Err("Science 状态为运行中，但无法确认其 binary 身份；已拒绝按端口停止".into())
        }
        (SandboxScienceState::Unknown, _) => {
            Err("无法确认当前 Science daemon 使用的 binary；已拒绝按端口停止".into())
        }
    }
}

/// Check that the sandbox Science associated with our data-dir is running.
/// A naked `/health` response is not sufficient identity proof.
#[cfg(test)]
pub(crate) fn sandbox_running_ours(port: u16, runtime: &ScienceRuntimeIdentity) -> bool {
    probe_known_runtime(port, runtime) == SandboxScienceState::RunningHealthy
}

/// The caller has just observed a healthy response and only needs to prove the
/// listener executable identity; avoid repeating status and health CLI work.
pub(crate) fn sandbox_listener_matches_runtime(
    port: u16,
    runtime: &ScienceRuntimeIdentity,
) -> bool {
    listener_uses_runtime(port, runtime)
}

/// Stop the sandbox Science process and clear the in-memory sandbox URL.
///
/// Returns `Err` when the stop script is missing or exits non-zero, so callers
/// can report that Science may not have stopped cleanly.
pub(crate) fn stop_sandbox<R: Runtime>(
    app: &tauri::AppHandle<R>,
    sandbox: &mut Option<Child>,
    sandbox_url: &mut Option<String>,
    runtime: Option<&ScienceRuntimeIdentity>,
) -> Result<(), String> {
    stop_sandbox_with_launch_token(app, sandbox, sandbox_url, runtime, None)
}

pub(crate) fn stop_sandbox_with_launch_token<R: Runtime>(
    app: &tauri::AppHandle<R>,
    sandbox: &mut Option<Child>,
    sandbox_url: &mut Option<String>,
    runtime: Option<&ScienceRuntimeIdentity>,
    launch_token: Option<&ScienceManagedLaunchToken>,
) -> Result<(), String> {
    if !sandbox_data_dir().exists() {
        let sandbox_port = config::load_from(&config::default_dir())
            .map_err(|error| format!("读取 Science 端口配置失败：{error}"))?
            .sandbox_port;
        let first_port_observation = loopback_port_accepts_tcp(sandbox_port);
        let second_port_observation = if first_port_observation {
            true
        } else {
            std::thread::sleep(Duration::from_millis(20));
            loopback_port_accepts_tcp(sandbox_port)
        };
        if second_port_observation {
            return Err(
                "Science data-dir 已消失，但配置端口仍有监听；未发送信号且未确认停止。".into(),
            );
        }
        kill_child(sandbox);
        *sandbox_url = None;
        return Ok(());
    }
    let recovered;
    let runtime = match runtime {
        Some(runtime) => runtime,
        None => {
            let port = config::load_from(&config::default_dir())
                .map_err(|e| format!("读取 Science 端口配置失败：{e}"))?
                .sandbox_port;
            let (state, runtime) = probe_sandbox_runtime(port)?;
            let Some(runtime) = stop_runtime_from_probe(state, runtime)? else {
                kill_child(sandbox);
                *sandbox_url = None;
                return Ok(());
            };
            recovered = runtime;
            &recovered
        }
    };
    if !runtime.is_current() {
        return Err("Science binary 在选择后发生变化；已拒绝用不同文件控制现有 daemon".into());
    }
    let sandbox_port = config::load_from(&config::default_dir())
        .map_err(|error| format!("读取 Science 端口配置失败：{error}"))?
        .sandbox_port;
    let stop_token = match launch_token {
        Some(token) => token.clone(),
        None => managed_launch_token(sandbox_port, runtime)
            .ok_or("Science managed launch 身份无法确认；已拒绝调用 stop 或发送信号")?,
    };
    if stop_token.record.port != sandbox_port
        || !managed_launch_token_is_current(&stop_token, runtime)
    {
        return Err("Science managed launch 身份在停止前发生变化；未调用 stop 或发送信号".into());
    }
    let mut err = None;
    match asset_root(app) {
        Some(root) => {
            let stop = root.join("scripts/stop-science-sandbox.sh");
            if stop.is_file() {
                let mut stop_cmd = Command::new("zsh");
                stop_cmd.arg(&stop);
                crate::runtime::launch_env::configure_science_stop_script_command(
                    &mut stop_cmd,
                    &sandbox_home(),
                    Path::new(&runtime.path),
                );
                match stop_cmd
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                {
                    Ok(s) if s.success() => {}
                    Ok(s) => err = Some(format!("停止沙箱脚本非零退出（{:?}）。", s.code())),
                    Err(e) => err = Some(format!("调用停止沙箱脚本失败：{e}")),
                }
            } else {
                err = Some(
                    "找不到打包的停止脚本，无法确认沙箱已停止（沙箱可能仍在运行）。".to_string(),
                );
            }
        }
        None => {
            err = Some(
                "定位不到资源根，取不到停止脚本，无法确认沙箱已停止（沙箱可能仍在运行）。"
                    .to_string(),
            );
        }
    }
    if err.is_none() && loopback_port_accepts_tcp(sandbox_port) {
        let pid = stop_token.record.listener_pid;
        if managed_launch_token_is_current(&stop_token, runtime) {
            // Some upstream Science builds return success and remove their
            // lockfile without terminating the daemon. The user requested
            // stop, so signal only the exact launch token whose listener,
            // process start, receipt, and canonical executable were proved
            // both before and after CLI.
            // SAFETY: kill does not dereference pointers. PID > 1 and exact
            // listener identity were checked immediately above.
            if unsafe { libc::kill(pid as i32, libc::SIGTERM) } != 0 {
                err = Some("Science stop 返回成功但精确 daemon 无法接收 TERM。".into());
            } else {
                for _ in 0..50 {
                    if !loopback_port_accepts_tcp(sandbox_port) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                if managed_launch_token_is_current(&stop_token, runtime) {
                    // SAFETY: the same launch token, including process-start
                    // identity, is revalidated after the TERM wait.
                    let _ = unsafe { libc::kill(pid as i32, libc::SIGKILL) };
                    for _ in 0..20 {
                        if !loopback_port_accepts_tcp(sandbox_port) {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                if loopback_port_accepts_tcp(sandbox_port) {
                    err = Some(
                        "Science stop 返回成功，但端口仍被占用；已拒绝把未知监听者当作停止成功。"
                            .into(),
                    );
                }
            }
        } else {
            err = Some(
                "Science stop 返回成功，但停止后的监听身份与启动记录不一致；未发送信号。".into(),
            );
        }
    }
    kill_child(sandbox);
    *sandbox_url = None;
    if err.is_none() {
        if loopback_port_accepts_tcp(sandbox_port) {
            err = Some(
                "Science stop 后配置端口重新出现监听；未确认停止且未清理 managed launch 记录。"
                    .into(),
            );
        } else if let Err(error) = clear_managed_launch_identity(&stop_token, runtime) {
            err = Some(error);
        }
    }
    match err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}
