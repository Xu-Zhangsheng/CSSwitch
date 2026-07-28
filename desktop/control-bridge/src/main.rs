//! External, update-independent CSSwitch control bridge.
//!
//! This binary owns its loopback listener and authentication record. It only
//! uses the stable config/control core crates; it never imports Tauri, opens
//! CSSwitch.app, or calls an app-private IPC endpoint.

use csswitch_config_core::{ConfigStore, StoreError};
use csswitch_control_core::{CapabilitySet, ProtocolInfo};
use getrandom::getrandom;
use serde_json::{json, Map, Value};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

const MAX_REQUEST: usize = 1024 * 1024;
const EXTERNAL_RECORD: &str = "external-control.json";

fn csswitch_dir() -> PathBuf {
    env::var_os("CSSWITCH_BRIDGE_DIR").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".csswitch")
    })
}

fn random_hex(bytes: usize) -> Result<String, String> {
    let mut value = vec![0_u8; bytes];
    getrandom(&mut value).map_err(|error| error.to_string())?;
    Ok(value.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn write_record(root: &PathBuf, port: u16, token: &str) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|error| error.to_string())?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())?;
    let record = root.join(EXTERNAL_RECORD);
    let temporary = root.join(format!(".{EXTERNAL_RECORD}-{}", random_hex(8)?));
    let bytes = serde_json::to_vec(&json!({
        "schema_version": 1,
        "protocol_version": "csswitch-control/1",
        "port": port,
        "pid": std::process::id(),
        "token": token,
    })).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())?;
    fs::rename(temporary, record).map_err(|error| error.to_string())
}

fn json_response(stream: &mut TcpStream, status: &str, value: Value) {
    let body = value.to_string();
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
    let _ = stream.write_all(response.as_bytes());
}

fn error(code: &str, message: &str) -> Value { json!({"error": {"code": code, "message": message}}) }

fn templates() -> Value {
    let values = [
        "deepseek", "glm", "xiaomi", "siliconflow", "kimi", "minimax",
        "openrouter", "qwen", "opencode-go-openai", "opencode-go-anthropic",
        "grok", "gemini", "codex", "custom-openai", "custom-openai-responses", "custom"
    ].into_iter().map(|id| json!({"id": id})).collect::<Vec<_>>();
    json!({"templates": values})
}

fn read_request(stream: &mut TcpStream) -> Result<(String, Vec<(String, String)>, Option<Value>), String> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end;
    loop {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 { return Err("empty request".into()); }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.len() > MAX_REQUEST { return Err("request too large".into()); }
        if let Some(index) = bytes.windows(4).position(|window| window == b"\r\n\r\n") { header_end = index + 4; break; }
    }
    let headers = std::str::from_utf8(&bytes[..header_end]).map_err(|_| "request is not UTF-8".to_string())?;
    let mut lines = headers.split("\r\n");
    let request_line = lines.next().ok_or("missing request line")?.to_string();
    let pairs = lines.filter_map(|line| line.split_once(':').map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_string()))).collect::<Vec<_>>();
    let content_length = pairs.iter().find(|(key, _)| key == "content-length").and_then(|(_, value)| value.parse::<usize>().ok()).unwrap_or(0);
    if content_length > MAX_REQUEST - header_end { return Err("request body too large".into()); }
    while bytes.len() < header_end + content_length {
        let read = stream.read(&mut chunk).map_err(|error| error.to_string())?;
        if read == 0 { return Err("truncated request body".into()); }
        bytes.extend_from_slice(&chunk[..read]);
    }
    let body = if content_length == 0 { None } else { Some(serde_json::from_slice(&bytes[header_end..header_end + content_length]).map_err(|_| "request body is not valid JSON".to_string())?) };
    Ok((request_line, pairs, body))
}

fn require_envelope(body: &Value) -> Result<(&str, &str), Value> {
    if body.get("confirm").and_then(Value::as_bool) != Some(true) { return Err(error("CONFIRMATION_REQUIRED", "Mutation requires confirm=true.")); }
    let intent = body.get("intent_id").and_then(Value::as_str).filter(|value| !value.is_empty()).ok_or_else(|| error("INVALID_PARAMS", "intent_id is required."))?;
    let expected = body.get("expected_config_fingerprint").and_then(Value::as_str).filter(|value| !value.is_empty()).ok_or_else(|| error("INVALID_PARAMS", "expected_config_fingerprint is required."))?;
    Ok((intent, expected))
}

fn profile_mutation(store: &ConfigStore, body: &Value) -> Result<Value, Value> {
    let (intent_id, expected) = require_envelope(body)?;
    let action = body.get("action").and_then(Value::as_str).ok_or_else(|| error("INVALID_PARAMS", "profile action is required."))?;
    if action == "update_metadata" {
        let id = body.get("id").and_then(Value::as_str).ok_or_else(|| error("INVALID_PARAMS", "id is required."))?;
        let name = body.get("name").and_then(Value::as_str).ok_or_else(|| error("INVALID_PARAMS", "name is required."))?;
        let notes = body.get("notes").and_then(Value::as_str);
        let fingerprint = store.update_profile_metadata(expected, id, name, notes).map_err(store_error)?;
        return Ok(json!({"intent_id": intent_id, "committed": true, "config_fingerprint": fingerprint}));
    }
    let mut config = store.load().map_err(store_error)?;
    if store.fingerprint().map_err(store_error)? != expected { return Err(error("CONFIG_CHANGED", "Configuration changed; reload before writing.")); }
    let profiles = config.get_mut("profiles").and_then(Value::as_array_mut).ok_or_else(|| error("INVALID_CONFIG", "profiles must be an array."))?;
    match action {
        "create" => {
            let id = random_hex(16).map_err(|_| error("RANDOM_FAILED", "Cannot generate profile id."))?;
            let name = body.get("name").and_then(Value::as_str).ok_or_else(|| error("INVALID_PARAMS", "name is required."))?;
            let template_id = body.get("template_id").and_then(Value::as_str).ok_or_else(|| error("INVALID_PARAMS", "template_id is required."))?;
            let mut profile = Map::new();
            profile.insert("id".into(), Value::String(id.clone()));
            profile.insert("name".into(), Value::String(name.into()));
            profile.insert("template_id".into(), Value::String(template_id.into()));
            for key in ["api_format", "base_url", "model", "default_model_route_id"] { if let Some(value) = body.get(key) { profile.insert(key.into(), value.clone()); } }
            if let Some(value) = body.get("model_catalog") { profile.insert("model_catalog".into(), value.clone()); }
            if let Some(value) = body.get("role_bindings") { profile.insert("role_bindings".into(), value.clone()); }
            if let Some(value) = body.get("key").and_then(Value::as_str) { profile.insert("api_key".into(), Value::String(value.into())); }
            profiles.push(Value::Object(profile));
            let fingerprint = store.save(Some(expected), &config).map_err(store_error)?;
            Ok(json!({"intent_id": intent_id, "committed": true, "id": id, "config_fingerprint": fingerprint}))
        }
        "activate" => {
            let id = body.get("id").and_then(Value::as_str).ok_or_else(|| error("INVALID_PARAMS", "id is required."))?;
            if !profiles.iter().any(|profile| profile.get("id").and_then(Value::as_str) == Some(id)) { return Err(error("NOT_FOUND", "Profile not found.")); }
            config.as_object_mut().ok_or_else(|| error("INVALID_CONFIG", "config must be an object."))?.insert("active_id".into(), Value::String(id.into()));
            let fingerprint = store.save(Some(expected), &config).map_err(store_error)?;
            Ok(json!({"intent_id": intent_id, "committed": true, "config_fingerprint": fingerprint}))
        }
        "clear_key" | "delete" | "update_connection" => {
            let id = body.get("id").and_then(Value::as_str).ok_or_else(|| error("INVALID_PARAMS", "id is required."))?;
            let position = profiles.iter().position(|profile| profile.get("id").and_then(Value::as_str) == Some(id)).ok_or_else(|| error("NOT_FOUND", "Profile not found."))?;
            if action == "delete" {
                profiles.remove(position);
                if config.get("active_id").and_then(Value::as_str) == Some(id) { config.as_object_mut().unwrap().insert("active_id".into(), Value::String(String::new())); }
            } else {
                let profile = profiles[position].as_object_mut().ok_or_else(|| error("INVALID_CONFIG", "profile must be an object."))?;
                if action == "clear_key" { profile.insert("api_key".into(), Value::String(String::new())); }
                for key in ["api_format", "base_url", "model_catalog", "default_model_route_id", "role_bindings"] {
                    if let Some(value) = body.get(key) { profile.insert(key.into(), value.clone()); }
                }
                if let Some(value) = body.get("key").and_then(Value::as_str) { profile.insert("api_key".into(), Value::String(value.into())); }
            }
            let fingerprint = store.save(Some(expected), &config).map_err(store_error)?;
            Ok(json!({"intent_id": intent_id, "committed": true, "config_fingerprint": fingerprint}))
        }
        _ => Err(error("UNSUPPORTED", "Profile action is not supported by external bridge.")),
    }
}

fn settings_mutation(store: &ConfigStore, body: &Value) -> Result<Value, Value> {
    let (intent_id, expected) = require_envelope(body)?;
    let action = body.get("action").and_then(Value::as_str).ok_or_else(|| error("INVALID_PARAMS", "settings action is required."))?;
    let mut config = store.load().map_err(store_error)?;
    if store.fingerprint().map_err(store_error)? != expected { return Err(error("CONFIG_CHANGED", "Configuration changed; reload before writing.")); }
    let obj = config.as_object_mut().ok_or_else(|| error("INVALID_CONFIG", "config must be an object."))?;
    match action {
        "set_mode" => { let mode = body.get("mode").and_then(Value::as_str).filter(|value| *value == "proxy" || *value == "official").ok_or_else(|| error("INVALID_PARAMS", "mode must be proxy or official."))?; obj.insert("mode".into(), Value::String(mode.into())); }
        "set_settings" => {
            for key in ["proxy_port", "sandbox_port"] {
                if let Some(port) = body.get(key).and_then(Value::as_u64) { if port == 0 || port > 65535 || port == 8765 { return Err(error("INVALID_PARAMS", "Ports must be 1..65535 and cannot be 8765.")); } obj.insert(key.into(), Value::Number(port.into())); }
            }
            if obj.get("proxy_port") == obj.get("sandbox_port") { return Err(error("INVALID_PARAMS", "Proxy and sandbox ports must differ.")); }
            if let Some(value) = body.get("reuse_system_ssh").and_then(Value::as_bool) { obj.insert("reuse_system_ssh".into(), Value::Bool(value)); }
        }
        _ => return Err(error("UNSUPPORTED", "Runtime start/stop is not yet in external control core.")),
    }
    let fingerprint = store.save(Some(expected), &config).map_err(store_error)?;
    Ok(json!({"intent_id": intent_id, "committed": true, "config_fingerprint": fingerprint}))
}

fn store_error(store_failure: StoreError) -> Value {
    match store_failure { StoreError::Conflict => error("CONFIG_CHANGED", "Configuration changed; reload before writing."), StoreError::Schema(version) => error("SCHEMA_UNSUPPORTED", &format!("Config schema {version} is not supported.")), _ => error("CONFIG_STORE_FAILED", "Configuration store operation failed.") }
}

fn handle(mut stream: TcpStream, store: &ConfigStore, token: &str) {
    let (line, headers, body) = match read_request(&mut stream) { Ok(value) => value, Err(_) => return json_response(&mut stream, "400 Bad Request", error("INVALID_REQUEST", "Invalid HTTP request.")) };
    if stream.peer_addr().ok().map(|address| address.ip().is_loopback()) != Some(true) { return json_response(&mut stream, "403 Forbidden", error("LOOPBACK_ONLY", "Bridge accepts loopback connections only.")); }
    if headers.iter().find(|(key, _)| key == "x-csswitch-control").map(|(_, value)| value.as_str()) != Some(token) { return json_response(&mut stream, "401 Unauthorized", error("UNAUTHORIZED", "Invalid control token.")); }
    let result = match line.as_str() {
        "GET /v1/status HTTP/1.1" => match store.public_projection() {
            Ok(config) => Ok(json!({"bridge": {"external": true, "protocol": ProtocolInfo::default()}, "config": config, "runtime": {"managed_by_external_bridge": false, "state": "not_migrated"}})),
            Err(store_failure) => Err(store_error(store_failure)),
        },
        "GET /v1/capabilities HTTP/1.1" => Ok(json!({"protocol": ProtocolInfo::default(), "capabilities": CapabilitySet { status: true, config: true, profiles: true, model_catalog: true, runtime: false, provider_check: false }, "runtime": "not_migrated"})),
        "GET /v1/config HTTP/1.1" => match (store.public_projection(), store.fingerprint()) {
            (Ok(config), Ok(fingerprint)) => Ok(json!({"config": config, "config_fingerprint": fingerprint})),
            (Err(store_failure), _) | (_, Err(store_failure)) => Err(store_error(store_failure)),
        },
        "GET /v1/templates HTTP/1.1" => Ok(templates()),
        "POST /v1/check HTTP/1.1" => Ok(json!({"ok": true, "kind": "configuration_only", "end_to_end": false, "message": "Provider checks have not yet been migrated to external control core."})),
        "POST /v1/profile HTTP/1.1" => profile_mutation(store, &body.unwrap_or(Value::Null)),
        "POST /v1/mode HTTP/1.1" | "POST /v1/settings HTTP/1.1" => settings_mutation(store, &body.unwrap_or(Value::Null)),
        _ => Err(error("UNSUPPORTED", "Route is not implemented by external bridge.")),
    };
    match result { Ok(value) => json_response(&mut stream, "200 OK", value), Err(value) => json_response(&mut stream, "409 Conflict", value) }
}

fn main() -> Result<(), String> {
    let root = csswitch_dir();
    let store = ConfigStore::new(&root);
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| error.to_string())?;
    let port = listener.local_addr().map_err(|error| error.to_string())?.port();
    let token = random_hex(32)?;
    write_record(&root, port, &token)?;
    println!("{}", json!({"ready": true, "protocol": "csswitch-control/1", "port": port}));
    for stream in listener.incoming() { if let Ok(stream) = stream { handle(stream, &store, &token); } }
    Ok(())
}
