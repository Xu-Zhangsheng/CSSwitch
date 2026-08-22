use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::codex_models;

pub(super) struct RequestHead {
    pub(super) method: String,
    pub(super) target: String,
    pub(super) headers: HashMap<String, String>,
}

const REQUEST_HEAD_DEADLINE: Duration = Duration::from_secs(10);
const REQUEST_BODY_DEADLINE: Duration = Duration::from_secs(30);

fn read_with_deadline(
    stream: &mut TcpStream,
    buf: &mut [u8],
    deadline: Instant,
    timeout_detail: &str,
) -> Result<usize, String> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| timeout_detail.to_string())?;
    stream
        .set_read_timeout(Some(remaining))
        .map_err(|error| error.to_string())?;
    stream.read(buf).map_err(|error| {
        if matches!(
            error.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ) {
            timeout_detail.to_string()
        } else {
            error.to_string()
        }
    })
}

pub(super) fn read_head(stream: &mut TcpStream) -> Result<RequestHead, String> {
    let deadline = Instant::now() + REQUEST_HEAD_DEADLINE;
    let mut buf = Vec::with_capacity(4096);
    let mut byte = [0_u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        let n = read_with_deadline(stream, &mut byte, deadline, "request headers timed out")?;
        if n == 0 {
            return Err("empty request".to_string());
        }
        buf.push(byte[0]);
        if buf.len() > 64 * 1024 {
            return Err("request headers too large".to_string());
        }
    }
    let text = std::str::from_utf8(&buf).map_err(|_| "invalid request headers".to_string())?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or("missing request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("missing method")?.to_string();
    let target = parts.next().ok_or("missing target")?.to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    Ok(RequestHead {
        method,
        target,
        headers,
    })
}

pub(super) fn content_length(headers: &HashMap<String, String>) -> Result<usize, String> {
    let Some(raw) = headers.get("content-length") else {
        return Ok(0);
    };
    let parsed = raw
        .parse::<i64>()
        .map_err(|_| "invalid Content-Length".to_string())?;
    if parsed < 0 {
        return Err("invalid Content-Length".to_string());
    }
    Ok(parsed as usize)
}

pub(super) fn read_body(stream: &mut TcpStream, len: usize) -> Result<Vec<u8>, String> {
    let deadline = Instant::now() + REQUEST_BODY_DEADLINE;
    let mut body = vec![0_u8; len];
    let mut offset = 0;
    while offset < len {
        let read = read_with_deadline(
            stream,
            &mut body[offset..],
            deadline,
            "request body timed out",
        )?;
        if read == 0 {
            return Err("request body ended early".into());
        }
        offset += read;
    }
    Ok(body)
}

fn json_bytes(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).unwrap_or_else(|_| b"{\"error\":\"internal\"}".to_vec())
}

pub(super) fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

pub(super) fn write_json(stream: &mut TcpStream, status: u16, reason: &str, value: Value) {
    let body = json_bytes(value);
    write_response(stream, status, reason, "application/json", &body);
}

pub(super) fn write_codex_models_response(
    stream: &mut TcpStream,
    snapshot: &codex_models::CodexModelsSnapshot,
) {
    let body = json_bytes(snapshot.response_body());
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nx-csswitch-model-source: {}\r\nx-csswitch-model-age-seconds: {}\r\nconnection: close\r\n\r\n",
        body.len(),
        snapshot.source().as_str(),
        snapshot.age_seconds(),
    );
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

pub(super) fn typed_error_json(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    error_type: &str,
    message: &str,
) {
    write_json(
        stream,
        status,
        reason,
        json!({
            "type": "error",
            "error": {
                "type": error_type,
                "message": message,
            },
        }),
    );
}

pub(super) fn forbidden_json(stream: &mut TcpStream) {
    typed_error_json(stream, 403, "Forbidden", "permission_error", "forbidden");
}

pub(super) fn invalid_request_json(stream: &mut TcpStream, detail: &str) {
    typed_error_json(stream, 400, "Bad Request", "invalid_request_error", detail);
}

pub(super) fn route_unknown_json(stream: &mut TcpStream, detail: &str) {
    typed_error_json(stream, 400, "Bad Request", "route_unknown", detail);
}

pub(super) fn codex_model_unavailable_json(stream: &mut TcpStream, detail: &str) {
    typed_error_json(
        stream,
        503,
        "Service Unavailable",
        "codex_model_unavailable",
        detail,
    );
}

pub(super) fn request_too_large_json(stream: &mut TcpStream) {
    typed_error_json(
        stream,
        413,
        "Payload Too Large",
        "invalid_request_error",
        "request body is too large",
    );
}

pub(super) fn not_found_json(stream: &mut TcpStream, path: &str) {
    typed_error_json(stream, 404, "Not Found", "not_found_error", path);
}

pub(super) fn api_error_json(stream: &mut TcpStream, status: u16, detail: &str) {
    typed_error_json(stream, status, status_reason(status), "api_error", detail);
}

pub(super) fn upstream_protocol_error_json(stream: &mut TcpStream, detail: &str) {
    typed_error_json(
        stream,
        502,
        "Bad Gateway",
        "upstream_protocol_error",
        detail,
    );
}

pub(super) fn status_reason(status: u16) -> &'static str {
    reqwest::StatusCode::from_u16(status)
        .ok()
        .and_then(|status| status.canonical_reason())
        .unwrap_or("Error")
}

pub(super) fn dequery(path: &str) -> &str {
    path.split_once('?').map(|(p, _)| p).unwrap_or(path)
}

pub(super) fn models_error_json(
    stream: &mut TcpStream,
    status: u16,
    error_kind: &str,
    upstream_status: Option<u16>,
    message: &str,
) {
    write_json(
        stream,
        status,
        status_reason(status),
        json!({
            "error_kind": error_kind,
            "upstream_status": upstream_status,
            "message": message,
        }),
    );
}
