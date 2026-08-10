/// A regular-file-backed output sink prevents a descendant that inherited
/// stdout/stderr from keeping the parent blocked on pipe EOF. RLIMIT_FSIZE
/// bounds what the entire private process group can write before it is killed.
struct ScienceControlOutput {
    file: fs::File,
    path: PathBuf,
}

impl ScienceControlOutput {
    fn create(label: &str) -> Option<Self> {
        for _ in 0..8 {
            let nonce = SCIENCE_CONTROL_OUTPUT_NONCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                ".csswitch-science-control-{label}-{}-{nonce}",
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
                Ok(file) => return Some(Self { file, path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return None,
            }
        }
        None
    }

    fn stdio(&self) -> Option<Stdio> {
        self.file.try_clone().ok().map(Stdio::from)
    }

    fn read_bounded(&mut self, limit: u64) -> Option<Vec<u8>> {
        if self.file.metadata().ok()?.len() > limit {
            return None;
        }
        self.file.seek(SeekFrom::Start(0)).ok()?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut self.file)
            .take(limit.saturating_add(1))
            .read_to_end(&mut bytes)
            .ok()?;
        (bytes.len() as u64 <= limit).then_some(bytes)
    }
}

impl Drop for ScienceControlOutput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn anchored_science_control_child_exited(pid: u32) -> Result<bool, i32> {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    loop {
        // SAFETY: info points to writable siginfo_t storage. WNOWAIT observes
        // without reaping, keeping the pid as an anchor for its private group.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            // SAFETY: successful waitid initializes siginfo_t; si_pid == 0 is
            // the specified WNOHANG result when no child exit is pending.
            return Ok(unsafe { info.assume_init().si_pid() } != 0);
        }
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno != libc::EINTR {
            return Err(errno);
        }
    }
}

fn kill_anchored_science_control_group(pid: u32, leader_exited: bool) -> bool {
    let Ok(pgid) = i32::try_from(pid) else {
        return false;
    };
    // SAFETY: the child was spawned with process_group(0), and the live or
    // WNOWAIT-observed leader keeps this exact pid/pgid reserved.
    if unsafe { libc::kill(-pgid, libc::SIGKILL) } == 0 {
        return true;
    }
    let errno = std::io::Error::last_os_error().raw_os_error();
    if errno == Some(libc::ESRCH) {
        return true;
    }
    cfg!(target_os = "macos") && leader_exited && errno == Some(libc::EPERM)
}

fn wait_science_control_command(mut child: Child, timeout: Duration) -> Option<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match anchored_science_control_child_exited(child.id()) {
            Ok(true) => {
                let group_stopped = kill_anchored_science_control_group(child.id(), true);
                let status = child.wait().ok()?;
                return group_stopped.then_some(status);
            }
            Ok(false) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(false) => {
                if !kill_anchored_science_control_group(child.id(), false) {
                    let _ = child.kill();
                }
                let cleanup_deadline = Instant::now() + Duration::from_secs(1);
                while Instant::now() < cleanup_deadline {
                    match anchored_science_control_child_exited(child.id()) {
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

fn run_science_control_command(
    path: &Path,
    home: &Path,
    timeout: Duration,
    stdout_limit: u64,
    stderr_limit: u64,
    configure: impl FnOnce(&mut Command),
) -> Option<Output> {
    let mut stdout = ScienceControlOutput::create("stdout")?;
    let mut stderr = ScienceControlOutput::create("stderr")?;
    let stdout_stdio = stdout.stdio()?;
    let stderr_stdio = stderr.stdio()?;
    let file_limit = stdout_limit.max(stderr_limit).saturating_add(1);
    let mut environment = super::launch_env::base_process_env();
    environment.push(("HOME".into(), home.display().to_string()));

    let mut command = Command::new(path);
    super::launch_env::apply_allowlist(&mut command, environment);
    configure(&mut command);
    command
        .stdout(stdout_stdio)
        .stderr(stderr_stdio)
        .process_group(0);
    // SAFETY: setrlimit is async-signal-safe and the closure captures only a
    // copyable integer. The limit is inherited by all descendants.
    unsafe {
        command.pre_exec(move || {
            let limit = libc::rlimit {
                rlim_cur: file_limit as libc::rlim_t,
                rlim_max: file_limit as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
    let child = command.spawn().ok()?;
    let status = wait_science_control_command(child, timeout)?;
    let stdout = stdout.read_bounded(stdout_limit)?;
    let stderr = stderr.read_bounded(stderr_limit)?;
    Some(Output {
        status,
        stdout,
        stderr,
    })
}
