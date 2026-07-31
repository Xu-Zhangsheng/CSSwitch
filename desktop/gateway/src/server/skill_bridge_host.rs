use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::config::GatewayConfig;

const BRIDGE_REPLAY_WINDOW_SECONDS: u64 = 185;
const BRIDGE_HEARTBEAT_SECONDS: u64 = 2;

#[cfg(unix)]
#[derive(Clone)]
pub(super) struct BridgeProgress {
    pub(super) phase: String,
    pub(super) message: String,
    pub(super) sequence: u64,
}

#[cfg(unix)]
fn valid_bridge_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(unix)]
pub(super) fn acquire_bridge_host_lock(bridge: &std::path::Path) -> Result<std::fs::File, String> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    let lock_path = bridge.join(".csswitch-host.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&lock_path)
        .map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err("refusing unsafe Skill bridge host lock".into());
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("Skill bridge already has an active host".into());
    }
    Ok(file)
}

#[cfg(unix)]
fn bridge_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(unix)]
fn bridge_operation_timeout(operation: &str) -> u64 {
    if operation == "install" {
        crate::skill_install::BRIDGE_INSTALL_RESPONSE_TIMEOUT_SECONDS
    } else {
        60
    }
}

#[cfg(unix)]
pub(super) fn write_bridge_status(
    bridge: &std::path::Path,
    id: &str,
    operation: &str,
    started_at: u64,
    timeout_seconds: u64,
    progress: &BridgeProgress,
) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;

    let now = bridge_now();
    let payload = json!({
        "schema_version": csswitch_skill_install_core::SCHEMA_VERSION,
        "status": "PROCESSING",
        "request_id": id,
        "operation": operation,
        "phase": progress.phase,
        "message": progress.message,
        "sequence": progress.sequence,
        "started_at": started_at,
        "updated_at": now,
        "elapsed_seconds": now.saturating_sub(started_at),
        "timeout_seconds": timeout_seconds,
        "deadline_at": started_at.saturating_add(timeout_seconds),
        "terminal_grace_seconds": 5,
        "poll_after_seconds": 3,
        "response_filename": format!("{id}.response.json")
    });
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = bridge.join(format!(
        ".{id}.status.{}.{}.{}.tmp",
        std::process::id(),
        progress.sequence,
        nonce
    ));
    let target = bridge.join(format!("{id}.status.json"));
    let _ = std::fs::remove_file(&temp);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(&mut file, &payload).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    std::fs::rename(&temp, &target).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        error.to_string()
    })
}

#[cfg(unix)]
fn bridge_final_response_exists(path: &std::path::Path) -> Result<bool, String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    match std::fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_file()
                && metadata.uid() == unsafe { libc::geteuid() }
                && metadata.permissions().mode() & 0o077 == 0 =>
        {
            Ok(true)
        }
        Ok(_) => Err("invalid existing Skill bridge response".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(unix)]
pub(super) fn write_bridge_response_once(
    bridge: &std::path::Path,
    id: &str,
    response: &Value,
) -> Result<bool, String> {
    use std::os::unix::fs::OpenOptionsExt;

    let target = bridge.join(format!("{id}.response.json"));
    if bridge_final_response_exists(&target)? {
        return Ok(false);
    }
    let temp = bridge.join(format!(".{id}.response.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&temp);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|error| error.to_string())?;
    serde_json::to_writer(&mut file, response).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    let linked = match std::fs::hard_link(&temp, &target) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(error) => {
            let _ = std::fs::remove_file(&temp);
            return Err(error.to_string());
        }
    };
    std::fs::remove_file(&temp).map_err(|error| error.to_string())?;
    Ok(linked)
}

#[cfg(unix)]
fn cleanup_bridge_processing(bridge: &std::path::Path, id: &str) {
    let _ = std::fs::remove_file(bridge.join(format!("{id}.processing")));
    let _ = std::fs::remove_file(bridge.join(format!("{id}.status.json")));
}

#[cfg(unix)]
pub(super) fn finalize_bridge_processing(
    bridge: &std::path::Path,
    id: &str,
    response: &Value,
) -> Result<(), String> {
    write_bridge_response_once(bridge, id, response)?;
    cleanup_bridge_processing(bridge, id);
    Ok(())
}

#[cfg(unix)]
pub(super) fn recover_orphaned_bridge_processing(bridge: &std::path::Path) -> Result<(), String> {
    for entry in std::fs::read_dir(bridge).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(id) = name.strip_suffix(".processing") else {
            continue;
        };
        if !valid_bridge_id(id) {
            continue;
        }
        let response = json!({
            "schema_version": csswitch_skill_install_core::SCHEMA_VERSION,
            "status": "REQUEST_INTERRUPTED",
            "request_id": id,
            "retryable": true,
            "request_terminal": true,
            "automatic_retry_allowed": false,
            "directory_commit": null,
            "attach_state": "UNKNOWN",
            "restart_required": false,
            "message": "CSSwitch 在处理该请求期间中断。宿主已清理遗留 .processing；请重新调用相同工具，让 CSSwitch 验证真实落盘和绑定状态后安全恢复。"
        });
        finalize_bridge_processing(bridge, id, &response)?;
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn start_skill_install_bridge(cfg: &GatewayConfig) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    let (Some(bridge), Some(data_dir), Some(bridge_token)) = (
        &cfg.skill_bridge_dir,
        &cfg.skill_data_dir,
        &cfg.skill_bridge_token,
    ) else {
        return Ok(());
    };
    let name = bridge
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if !bridge.is_absolute() || !name.starts_with("CSSwitch-Skill-Bridge-") {
        return Err("refusing unsafe Skill install bridge path".into());
    }
    reject_bridge_symlinks(bridge)?;
    match std::fs::symlink_metadata(bridge) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err("refusing non-directory Skill install bridge path".into())
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            builder.create(bridge).map_err(|error| error.to_string())?;
        }
        Err(error) => return Err(error.to_string()),
    }
    std::fs::set_permissions(bridge, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    let metadata = std::fs::metadata(bridge).map_err(|error| error.to_string())?;
    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err("refusing Skill install bridge owned by another user".into());
    }
    let host_lock = acquire_bridge_host_lock(bridge)?;
    recover_orphaned_bridge_processing(bridge)?;
    let bridge = bridge.clone();
    let data_dir = data_dir.clone();
    let bridge_token = bridge_token.clone();
    let science_host_context = cfg.science_host_context.clone();
    thread::spawn(move || {
        let _host_lock = host_lock;
        let mut used_request_ids = HashMap::new();
        loop {
            let entries = match std::fs::read_dir(&bridge) {
                Ok(entries) => entries,
                Err(_) => return,
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Some(id) = name.strip_suffix(".request.json") else {
                    continue;
                };
                if !valid_bridge_id(id) {
                    continue;
                }
                let processing = bridge.join(format!("{id}.processing"));
                if std::fs::rename(entry.path(), &processing).is_err() {
                    continue;
                }
                let target = bridge.join(format!("{id}.response.json"));
                match bridge_final_response_exists(&target) {
                    Ok(true) => {
                        cleanup_bridge_processing(&bridge, id);
                        continue;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        eprintln!("Skill bridge refused existing response for {id}: {error}");
                        let progress = BridgeProgress {
                            phase: "finalization_failed".into(),
                            message:
                                "检测到非法的既有最终响应；请求未执行，.processing 已保留供安全恢复"
                                    .into(),
                            sequence: 0,
                        };
                        let _ = write_bridge_status(
                            &bridge,
                            id,
                            "unknown",
                            bridge_now(),
                            bridge_operation_timeout("unknown"),
                            &progress,
                        );
                        continue;
                    }
                }
                let request = read_regular_bridge_request(&processing)
                    .ok()
                    .and_then(|body| serde_json::from_slice::<Value>(&body).ok());
                let operation = request
                    .as_ref()
                    .and_then(|request| request.get("operation"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string();
                let started_at = bridge_now();
                let timeout_seconds = bridge_operation_timeout(&operation);
                let progress = Arc::new(Mutex::new(BridgeProgress {
                    phase: "accepted".into(),
                    message: "宿主已接收唯一请求，正在开始处理".into(),
                    sequence: 0,
                }));
                if let Ok(snapshot) = progress.lock() {
                    let _ = write_bridge_status(
                        &bridge,
                        id,
                        &operation,
                        started_at,
                        timeout_seconds,
                        &snapshot,
                    );
                }
                let (stop_tx, stop_rx) = mpsc::channel();
                let heartbeat_bridge = bridge.clone();
                let heartbeat_id = id.to_string();
                let heartbeat_operation = operation.clone();
                let heartbeat_progress = Arc::clone(&progress);
                let heartbeat = thread::spawn(move || loop {
                    match stop_rx.recv_timeout(Duration::from_secs(BRIDGE_HEARTBEAT_SECONDS)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if let Ok(snapshot) = heartbeat_progress.lock() {
                                let _ = write_bridge_status(
                                    &heartbeat_bridge,
                                    &heartbeat_id,
                                    &heartbeat_operation,
                                    started_at,
                                    timeout_seconds,
                                    &snapshot,
                                );
                            }
                        }
                    }
                });
                let mut report_progress = |phase: &str, message: &str| {
                    if let Ok(mut state) = progress.lock() {
                        state.phase = phase.to_string();
                        state.message = message.to_string();
                        state.sequence = state.sequence.saturating_add(1);
                        let _ = write_bridge_status(
                            &bridge,
                            id,
                            &operation,
                            started_at,
                            timeout_seconds,
                            &state,
                        );
                    }
                };
                let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    request.map_or_else(bridge_request_failed, |request| {
                        if crate::skill_install::validate_bridge_request(
                            &bridge_token,
                            id,
                            &request,
                        )
                        .is_err()
                            || bridge_request_is_replay(
                                &mut used_request_ids,
                                id,
                                request
                                    .get("issued_at")
                                    .and_then(Value::as_u64)
                                    .unwrap_or_default(),
                                bridge_now(),
                            )
                        {
                            bridge_request_failed()
                        } else {
                            crate::skill_install::handle_bridge_request_with_progress(
                                &data_dir,
                                science_host_context.as_ref(),
                                &request,
                                &mut report_progress,
                            )
                        }
                    })
                }))
                .unwrap_or_else(|_| bridge_request_internal_failed());
                let _ = stop_tx.send(());
                let _ = heartbeat.join();
                match finalize_bridge_processing(&bridge, id, &response) {
                    Ok(()) => {}
                    Err(error) => {
                        eprintln!("Skill bridge final response write failed for {id}: {error}");
                        let snapshot = BridgeProgress {
                            phase: "finalization_failed".into(),
                            message: "最终响应写入失败；.processing 已保留，gateway 重启后会恢复"
                                .into(),
                            sequence: u64::MAX,
                        };
                        let _ = write_bridge_status(
                            &bridge,
                            id,
                            &operation,
                            started_at,
                            timeout_seconds,
                            &snapshot,
                        );
                    }
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
    });
    Ok(())
}

#[cfg(unix)]
pub(super) fn bridge_request_is_replay(
    used: &mut HashMap<String, u64>,
    id: &str,
    issued_at: u64,
    now: u64,
) -> bool {
    used.retain(|_, issued| now.saturating_sub(*issued) <= BRIDGE_REPLAY_WINDOW_SECONDS);
    used.insert(id.to_string(), issued_at).is_some()
}

#[cfg(unix)]
fn reject_bridge_symlinks(path: &std::path::Path) -> Result<(), String> {
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("refusing symlink in Skill install bridge path".into())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn read_regular_bridge_request(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};

    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "invalid Skill bridge request")?;
    let metadata = file
        .metadata()
        .map_err(|_| "invalid Skill bridge request")?;
    if !metadata.is_file()
        || metadata.len() > 1024 * 1024
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o022 != 0
    {
        return Err("invalid Skill bridge request".into());
    }
    let mut body = Vec::with_capacity(metadata.len() as usize);
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut body)
        .map_err(|_| "invalid Skill bridge request")?;
    if body.len() > 1024 * 1024 {
        return Err("invalid Skill bridge request".into());
    }
    Ok(body)
}

#[cfg(unix)]
fn bridge_request_failed() -> Value {
    json!({
        "schema_version": csswitch_skill_install_core::SCHEMA_VERSION,
        "status":"REQUEST_FAILED",
        "message":"本地 Skill 请求非法或已处理",
        "directory_commit":false,
        "restart_required":false
    })
}

#[cfg(unix)]
fn bridge_request_internal_failed() -> Value {
    json!({
        "schema_version": csswitch_skill_install_core::SCHEMA_VERSION,
        "status":"REQUEST_FAILED",
        "message":"本地 Skill 请求处理异常；宿主已停止该请求，可安全重试相同工具",
        "retryable":true,
        "directory_commit":null,
        "restart_required":false
    })
}

#[cfg(not(unix))]
pub(super) fn start_skill_install_bridge(_cfg: &GatewayConfig) -> Result<(), String> {
    Ok(())
}
