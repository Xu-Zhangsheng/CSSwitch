//! A narrow, authenticated loopback control surface for CSSwitch's companion CLI.
//! It exposes only status, one-click start, and an opt-in bounded provider probe.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use crate::{config, lock, proc, SharedAppState, SharedLifecycle};

const MAX_REQUEST: usize = 16 * 1024;
const PROBE_TIMEOUT_MS: u64 = 15_000;

fn record_path() -> PathBuf { config::default_dir().join("control.json") }

fn write_record(port: u16, token: &str) -> Result<(), String> {
    let path = record_path();
    let parent = path.parent().ok_or("控制记录路径无父目录")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(".control-{}", config::new_id()));
    let payload = serde_json::to_vec(&json!({
        "schema_version": 1, "port": port, "token": token, "pid": std::process::id(),
    })).map_err(|e| e.to_string())?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&payload).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    fs::rename(temporary, path).map_err(|e| e.to_string())
}

pub(crate) fn remove_record() { let _ = fs::remove_file(record_path()); }

pub(crate) fn start(app: tauri::AppHandle, state: SharedAppState, lifecycle: SharedLifecycle) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let token = proc::gen_secret().map_err(|e| e.to_string())?;
    write_record(port, &token)?;
    thread::Builder::new().name("csswitch-control".into()).spawn(move || loop {
        match listener.accept() {
            Ok((stream, address)) if address.ip().is_loopback() => {
                let app = app.clone(); let state = state.clone(); let lifecycle = lifecycle.clone(); let token = token.clone();
                let _ = thread::Builder::new().name("csswitch-control-request".into()).spawn(move || handle(stream, &token, app, state, lifecycle));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(50)),
            Err(_) => break,
        }
    }).map_err(|e| e.to_string())?;
    Ok(())
}

fn probe_upstream(state: &SharedAppState) -> Value {
    let (port, secret, model) = {
        let st = lock(state);
        let cfg = match config::load_from(&config::default_dir()) { Ok(value) => value, Err(error) => return json!({"ok":false,"kind":"config_error","message":error.to_string()}) };
        (st.proxy_port, st.secret.clone(), cfg.active_profile().map(|p| p.model.clone()).unwrap_or_default())
    };
    if port == 0 || secret.is_empty() || model.trim().is_empty() { return json!({"ok":false,"kind":"not_ready"}); }
    let body = json!({"model":model,"max_tokens":1,"messages":[{"role":"user","content":"ping"}]});
    let code = proc::http_post_status(port, Some(&secret), "/v1/messages", body.to_string().as_bytes(), PROBE_TIMEOUT_MS);
    let kind = match code {
        Some(200..=299) => "ok", Some(401 | 403) => "auth", Some(429) => "rate_limited",
        Some(400..=499) => "request_rejected", Some(500..=599) => "upstream_error",
        Some(_) => "unexpected_status", None => "timeout_or_network",
    };
    json!({"ok":kind=="ok","kind":kind,"http_status":code})
}

fn handle(mut stream: TcpStream, token: &str, app: tauri::AppHandle, state: SharedAppState, lifecycle: SharedLifecycle) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut bytes = Vec::new(); let mut chunk = [0u8; 2048];
    while bytes.len() < MAX_REQUEST {
        match stream.read(&mut chunk) {
            Ok(0) => break, Ok(size) => { bytes.extend_from_slice(&chunk[..size]); if bytes.windows(4).any(|w| w == b"\r\n\r\n") { break; } }, Err(_) => break,
        }
    }
    let request = String::from_utf8_lossy(&bytes);
    let authorized = request.lines().any(|line| line.strip_prefix("X-CSSwitch-Control: ").is_some_and(|value| value == token));
    let first = request.lines().next().unwrap_or("");
    let body = if !authorized { json!({"error":"unauthorized"})
    } else if first.starts_with("GET /v1/status ") { crate::commands::runtime::status_for_control(&state)
    } else if first.starts_with("POST /v1/start ") {
        match crate::commands::runtime::one_click_login_cmd(app, state, lifecycle, None) { Ok(value) => value, Err(error) => json!({"status":"error","message":error.to_string()}) }
    } else if first.starts_with("POST /v1/check ") {
        let e2e = request.contains("\r\n\r\n{\"e2e\":true}") || request.contains("\"e2e\": true");
        json!({"status":crate::commands::runtime::status_for_control(&state),"upstream":if e2e { probe_upstream(&state) } else { json!({"ok":null,"kind":"not_requested"}) }})
    } else { json!({"error":"not_found"}) };
    let status = if authorized { "200 OK" } else { "401 Unauthorized" };
    let payload = body.to_string();
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", payload.len(), payload);
    let _ = stream.write_all(response.as_bytes());
}
