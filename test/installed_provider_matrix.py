#!/usr/bin/env python3
"""Safety-first controller for installed/local-mock provider acceptance.

The controller prepares an isolated per-case HOME, starts the strict provider
scenario API, freezes exact pre-run receipts, and owns the one-shot guarded
launch so validation and exec cannot be split across callers.  It never broadly
signals an installed application; later GUI actions remain with the root driver.

No command in this module reads process argv.  Process inspection is limited to
PID/PPID/comm, executable identity, and explicitly selected loopback listeners.
Secrets stay in the 0600 config file or process memory and are never returned by
the JSON controller.
"""

from __future__ import annotations

import argparse
import copy
import errno
import hashlib
import http.client
import json
import os
import plistlib
import re
import secrets as secrets_module
import shlex
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Iterable, List, Mapping, Optional, Sequence, Tuple

try:
    from _loopback_ports import FORBIDDEN_PORTS, MAX_BIND_ATTEMPTS, LoopbackPortReservation
    from provider_mock_scenarios import (
        ACTION_TYPES,
        SCHEMA as MOCK_MANIFEST_SCHEMA,
        SCHEMA_VERSION as MOCK_MANIFEST_VERSION,
        Scenario,
        ScenarioStep,
        load_manifest,
        scenario_from_steps,
        start_scenario,
    )
except ImportError:  # ``python -m unittest test.test_...``
    from test._loopback_ports import FORBIDDEN_PORTS, MAX_BIND_ATTEMPTS, LoopbackPortReservation
    from test.provider_mock_scenarios import (
        ACTION_TYPES,
        SCHEMA as MOCK_MANIFEST_SCHEMA,
        SCHEMA_VERSION as MOCK_MANIFEST_VERSION,
        Scenario,
        ScenarioStep,
        load_manifest,
        scenario_from_steps,
        start_scenario,
    )


APP_BUNDLE = Path("/Applications/CSSwitch.app")
EXPECTED_BUNDLE_ID = "com.csswitch.menubar"
EXPECTED_EXECUTABLE = "desktop"
GATEWAY_EXECUTABLE = "csswitch-gateway"
FIXED_PATH_SECRET = "6f2b0bbd37f98f6f9f8d9e3c8f7a2b10"
FAKE_API_KEY = "csswitch-installed-fake-key-never-use"
CONTROLLER_SCHEMA = "csswitch.installed-provider-controller.v1"
PRE_RUN_MANIFEST_SCHEMA = "csswitch.isolated-live-pre-run-manifest.v1"
FIXTURE_RECEIPT_SCHEMA = "csswitch.isolated-live-fixture-receipt.v1"
PROVIDER_RECEIPT_SCHEMA = "csswitch.loopback-provider-launch-receipt.v1"
NETWORK_RECEIPT_SCHEMA = "csswitch.network-isolation-receipt.v1"
SAFE_LABEL = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.:-]{0,95}$")
MAX_CONTROL_LINE = 65_536
SANDBOX_EXEC = Path("/usr/bin/sandbox-exec")
NETWORK_SANDBOX_PROFILE = """(version 1)
(allow default)
(deny network-outbound)
(allow network-outbound (remote ip "localhost:*"))
"""
NETWORK_POLICY = "deny-egress-except-ip-loopback-and-owned-science-unix-control"
NETWORK_PROBE_SOURCE = r"""
import json
import socket
import sys

ipv4_port = int(sys.argv[1])
ipv6_port = int(sys.argv[2])
probe_host = sys.argv[3]
result = {}

for field, family, address, payload in (
    ("ipv4_loopback_allowed", socket.AF_INET, ("127.0.0.1", ipv4_port), b"4"),
    ("ipv6_loopback_allowed", socket.AF_INET6, ("::1", ipv6_port), b"6"),
):
    client = socket.socket(family, socket.SOCK_STREAM)
    client.settimeout(2.0)
    client.connect(address)
    client.sendall(payload)
    client.close()
    result[field] = True

for field, family, kind, address in (
    ("ipv4_tcp_errno", socket.AF_INET, socket.SOCK_STREAM, ("198.18.0.54", 443)),
    ("ipv4_udp_errno", socket.AF_INET, socket.SOCK_DGRAM, ("198.18.0.54", 443)),
    ("ipv4_dns_tcp_errno", socket.AF_INET, socket.SOCK_STREAM, ("198.18.0.53", 53)),
    ("ipv4_dns_udp_errno", socket.AF_INET, socket.SOCK_DGRAM, ("198.18.0.53", 53)),
    ("ipv6_tcp_errno", socket.AF_INET6, socket.SOCK_STREAM, ("2001:db8::54", 443)),
    ("ipv6_udp_errno", socket.AF_INET6, socket.SOCK_DGRAM, ("2001:db8::54", 443)),
    ("ipv6_dns_tcp_errno", socket.AF_INET6, socket.SOCK_STREAM, ("2001:db8::53", 53)),
    ("ipv6_dns_udp_errno", socket.AF_INET6, socket.SOCK_DGRAM, ("2001:db8::53", 53)),
):
    candidate = socket.socket(family, kind)
    candidate.settimeout(1.0)
    try:
        candidate.connect(address)
    except OSError as error:
        result[field] = error.errno
    else:
        result[field] = None
    finally:
        candidate.close()

for field, kind in (
    ("system_resolver_ipc_stream_errno", socket.SOCK_STREAM),
    ("system_resolver_ipc_datagram_errno", socket.SOCK_DGRAM),
):
    candidate = socket.socket(socket.AF_UNIX, kind)
    candidate.settimeout(1.0)
    try:
        candidate.connect("/private/var/run/mDNSResponder")
    except OSError as error:
        result[field] = error.errno
    else:
        result[field] = None
    finally:
        candidate.close()

try:
    socket.getaddrinfo(probe_host, 443)
except socket.gaierror:
    result["unique_system_resolver_lookup_failed"] = True
else:
    result["unique_system_resolver_lookup_failed"] = False

print(json.dumps(result, sort_keys=True))
"""


class ControllerError(RuntimeError):
    """A fail-closed installed acceptance error."""


def _network_sandbox_profile(owned_unix_socket_subpath: Path) -> str:
    path = Path(owned_unix_socket_subpath)
    if not path.is_absolute():
        raise ControllerError("owned Unix socket subpath must be absolute")
    raw = str(path)
    if any(ord(char) < 0x20 or ord(char) == 0x7F for char in raw):
        raise ControllerError("owned Unix socket subpath contains control characters")
    quoted = raw.replace("\\", "\\\\").replace('"', '\\"')
    return (
        NETWORK_SANDBOX_PROFILE
        + f'(allow network-outbound (subpath "{quoted}"))\n'
    )


@dataclass(frozen=True)
class CaseDefinition:
    case_id: str
    template_id: str
    profile_name: str
    category: str
    api_format: str
    adapter: str
    shim: str
    model: str
    base_kind: str
    base_prefix: str
    message_path: str
    models_path: Optional[str]
    mock_scenario: str
    formal_step_id: str
    formal_variant: str
    expected_upstream_model: Optional[str] = None
    blockers: Tuple[str, ...] = ()


CASE_DEFINITIONS: Dict[str, CaseDefinition] = {
    "deepseek-off": CaseDefinition(
        "deepseek-off", "deepseek", "Installed DeepSeek off", "cn_official",
        "anthropic", "deepseek", "off", "", "native",
        "", "/deepseek/v1/messages", None,
        "installed_deepseek_matrix", "deepseek-formal-off", "basic",
    ),
    "deepseek-detect": CaseDefinition(
        "deepseek-detect", "deepseek", "Installed DeepSeek detect", "cn_official",
        "anthropic", "deepseek", "detect", "", "native",
        "", "/deepseek/v1/messages", None,
        "installed_deepseek_matrix", "deepseek-formal-detect", "tools",
    ),
    "deepseek-rewrite": CaseDefinition(
        "deepseek-rewrite", "deepseek", "Installed DeepSeek rewrite", "cn_official",
        "anthropic", "deepseek", "rewrite", "", "native",
        "", "/deepseek/v1/messages", None,
        "installed_deepseek_matrix", "deepseek-formal-rewrite-stream", "stream-tools",
    ),
    "qwen-chat": CaseDefinition(
        "qwen-chat", "qwen", "Installed Qwen Chat", "cn_official",
        "openai_chat", "qwen", "off", "qwen-plus-latest", "native",
        "", "/qwen/v1/chat/completions", None,
        "installed_qwen_matrix", "qwen-formal-chat", "basic", "qwen-plus-latest",
    ),
    "qwen-tools": CaseDefinition(
        "qwen-tools", "qwen", "Installed Qwen tools", "cn_official",
        "openai_chat", "qwen", "off", "qwen-plus-latest", "native",
        "", "/qwen/v1/chat/completions", None,
        "installed_qwen_matrix", "qwen-formal-tools-results", "tools", "qwen-plus-latest",
    ),
    "qwen-stream": CaseDefinition(
        "qwen-stream", "qwen", "Installed Qwen stream", "cn_official",
        "openai_chat", "qwen", "off", "qwen-plus-latest", "native",
        "", "/qwen/v1/chat/completions", None,
        "installed_qwen_matrix", "qwen-formal-stream", "stream", "qwen-plus-latest",
    ),
    "custom-chat": CaseDefinition(
        "custom-chat", "custom-openai", "Installed custom OpenAI Chat", "custom",
        "openai_chat", "openai-custom", "off", "glm-4.5", "loopback",
        "/openai/v1", "/openai/v1/chat/completions", "/openai/v1/models",
        "installed_openai_chat_matrix", "openai-chat-formal", "tools", "glm-4.5",
    ),
    "responses": CaseDefinition(
        "responses", "custom-openai-responses", "Installed OpenAI Responses", "custom",
        "openai_responses", "openai-responses", "off", "gpt-5.2", "loopback",
        "/responses/v1", "/responses/v1/responses", "/responses/v1/models",
        "installed_openai_responses_matrix", "responses-formal-tools-results", "tools-results", "gpt-5.2",
    ),
    "relay-force": CaseDefinition(
        "relay-force", "custom", "Installed relay force", "custom",
        "anthropic", "relay", "off", "MiniMax-M2", "loopback",
        "/relay", "/relay/v1/messages", "/relay/v1/models",
        "installed_relay_matrix", "relay-formal-force-schema", "force-tools", "MiniMax-M2",
    ),
    "kimi": CaseDefinition(
        "kimi", "kimi", "Installed Kimi", "cn_official",
        "anthropic", "relay", "off", "kimi-k2.7-code", "loopback",
        "/relay", "/relay/v1/messages", "/relay/v1/models",
        "installed_relay_matrix", "relay-formal-kimi-thinking-filter", "stream-tools", "kimi-k2.7-code",
    ),
    "siliconflow": CaseDefinition(
        "siliconflow", "siliconflow", "Installed SiliconFlow", "cn_official",
        "anthropic", "relay", "off", "deepseek-ai/DeepSeek-V4-Pro", "proxy",
        "http://api.siliconflow.cn", "/v1/messages", "/v1/models",
        "installed_relay_matrix", "siliconflow-exact-host", "tools",
        "deepseek-ai/DeepSeek-V4-Pro",
    ),
}


def _require_safe_label(value: str, field: str = "label") -> str:
    if not isinstance(value, str) or not SAFE_LABEL.fullmatch(value):
        raise ControllerError(f"unsafe {field}")
    return value


def _is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def _reject_symlink(path: Path) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    if stat.S_ISLNK(mode):
        raise ControllerError(f"refusing symlink: {path}")


def _reject_existing_symlink_components(path: Path) -> None:
    """Reject every existing component of an absolute identity path."""

    path = Path(path)
    if not path.is_absolute():
        raise ControllerError("identity path must be absolute")
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        try:
            info = current.lstat()
        except FileNotFoundError:
            raise ControllerError(f"identity path component is missing: {current}") from None
        if stat.S_ISLNK(info.st_mode):
            raise ControllerError(f"identity path traverses a symlink: {current}")


def _ensure_private_dir(path: Path) -> None:
    _reject_symlink(path)
    path.mkdir(parents=True, exist_ok=True, mode=0o700)
    _reject_symlink(path)
    if not path.is_dir():
        raise ControllerError(f"not a directory: {path}")
    path.chmod(0o700)


def _safe_write(path: Path, data: bytes, mode: int = 0o600) -> None:
    _reject_symlink(path)
    _ensure_private_dir(path.parent)
    tmp = path.parent / f".{path.name}.tmp-{os.getpid()}-{time.monotonic_ns()}"
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    flags |= getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(tmp, flags, mode)
    try:
        view = memoryview(data)
        while view:
            written = os.write(fd, view)
            view = view[written:]
        os.fsync(fd)
    finally:
        os.close(fd)
    os.replace(tmp, path)
    path.chmod(mode)


def _safe_write_at(directory_fd: int, name: str, data: bytes, mode: int = 0o600) -> None:
    """Atomically write one leaf through a held directory identity."""

    if not name or name in {".", ".."} or "/" in name or "\x00" in name:
        raise ControllerError("unsafe evidence leaf")
    tmp = f".{name}.tmp-{os.getpid()}-{time.monotonic_ns()}"
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(tmp, flags, mode, dir_fd=directory_fd)
    try:
        view = memoryview(data)
        while view:
            view = view[os.write(fd, view):]
        os.fchmod(fd, mode)
        os.fsync(fd)
    finally:
        os.close(fd)
    os.rename(tmp, name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd)


def _read_at(directory_fd: int, name: str) -> bytes:
    if not name or name in {".", ".."} or "/" in name or "\x00" in name:
        raise ControllerError("unsafe evidence leaf")
    fd = os.open(name, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0), dir_fd=directory_fd)
    try:
        chunks = []
        while True:
            chunk = os.read(fd, 1024 * 1024)
            if not chunk:
                return b"".join(chunks)
            chunks.append(chunk)
    finally:
        os.close(fd)


def _hash_tree_at(directory_fd: int, prefix: str = "") -> List[Tuple[str, str]]:
    """Hash a directory tree entirely through held no-follow directory FDs."""

    entries: List[Tuple[str, str]] = []
    for name in sorted(os.listdir(directory_fd)):
        if not name or name in {".", ".."} or "/" in name:
            raise ControllerError("evidence tree contains an unsafe leaf")
        relative = f"{prefix}/{name}" if prefix else name
        info = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if stat.S_ISLNK(info.st_mode):
            raise ControllerError("evidence closure contains a symlink")
        if stat.S_ISDIR(info.st_mode):
            child_fd = os.open(
                name,
                os.O_RDONLY | os.O_DIRECTORY | getattr(os, "O_NOFOLLOW", 0),
                dir_fd=directory_fd,
            )
            try:
                entries.extend(_hash_tree_at(child_fd, relative))
            finally:
                os.close(child_fd)
        elif stat.S_ISREG(info.st_mode):
            if relative != "hashes.sha256":
                entries.append((relative, _sha256(_read_at(directory_fd, name))))
        else:
            raise ControllerError("evidence closure contains an unsupported entry")
    return entries


def _safe_json_write(path: Path, value: Mapping[str, Any]) -> None:
    encoded = (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()
    _safe_write(path, encoded, 0o600)


def _canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n").encode()


def _safe_json_write_once(path: Path, value: Mapping[str, Any]) -> str:
    encoded = (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()
    try:
        current = path.read_bytes()
    except FileNotFoundError:
        _safe_write(path, encoded, 0o600)
        return _sha256(encoded)
    if current != encoded:
        raise ControllerError(f"immutable evidence drift: {path.name}")
    return _sha256(current)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _file_identity(path: Path) -> Dict[str, Any]:
    path = Path(path)
    _reject_existing_symlink_components(path)
    info = path.stat()
    if not stat.S_ISREG(info.st_mode):
        raise ControllerError(f"identity target is not a regular file: {path.name}")
    return {
        "path": str(path),
        "sha256": _sha256(path.read_bytes()),
        "size": info.st_size,
        "mode": stat.S_IMODE(info.st_mode),
        "device": info.st_dev,
        "inode": info.st_ino,
    }


def _tree_manifest(root: Path) -> Dict[str, Any]:
    """Return a complete, canonical no-follow identity manifest for ``root``."""

    root = Path(root)
    _reject_existing_symlink_components(root)
    if not root.is_dir():
        raise ControllerError("artifact root is not a directory")
    entries: List[Dict[str, Any]] = []
    for current, directories, filenames in os.walk(root, followlinks=False):
        current_path = Path(current)
        for name in sorted([*directories, *filenames]):
            candidate = current_path / name
            relative = candidate.relative_to(root).as_posix()
            info = candidate.lstat()
            base = {
                "path": relative,
                "mode": stat.S_IMODE(info.st_mode),
            }
            if stat.S_ISLNK(info.st_mode):
                resolved = candidate.resolve(strict=True)
                if not _is_relative_to(resolved, root):
                    raise ControllerError(
                        f"artifact symlink escapes the bundle: {relative}"
                    )
                target_info = resolved.stat()
                target_identity: Dict[str, Any] = {
                    "path": resolved.relative_to(root).as_posix(),
                    "mode": stat.S_IMODE(target_info.st_mode),
                }
                if stat.S_ISREG(target_info.st_mode):
                    target_identity.update(
                        {
                            "kind": "file",
                            "size": target_info.st_size,
                            "sha256": _sha256(resolved.read_bytes()),
                        }
                    )
                elif stat.S_ISDIR(target_info.st_mode):
                    target_identity["kind"] = "directory"
                else:
                    raise ControllerError(
                        f"artifact symlink has unsupported target: {relative}"
                    )
                base.update(
                    {
                        "kind": "symlink",
                        "target": os.readlink(candidate),
                        "target_identity": target_identity,
                    }
                )
            elif stat.S_ISDIR(info.st_mode):
                base.update({"kind": "directory"})
            elif stat.S_ISREG(info.st_mode):
                base.update(
                    {
                        "kind": "file",
                        "size": info.st_size,
                        "sha256": _sha256(candidate.read_bytes()),
                    }
                )
            else:
                raise ControllerError(f"unsupported artifact entry type: {relative}")
            entries.append(base)
    entries.sort(key=lambda item: item["path"])
    return {
        "schema": "csswitch.canonical-tree-manifest.v1",
        "root": str(root),
        "entries": entries,
        "entries_sha256": _sha256(_canonical_json_bytes(entries)),
    }


def _canonical_regular_file_list_digest(root: Path) -> str:
    """Recompute the G1 canonical ``sha256  ./relative`` file-list digest."""

    root = Path(root)
    _reject_existing_symlink_components(root)
    if not root.is_dir():
        raise ControllerError("canonical package root is not a directory")
    lines = []
    for current, directories, filenames in os.walk(root, followlinks=False):
        current_path = Path(current)
        for name in [*directories, *filenames]:
            candidate = current_path / name
            if candidate.is_symlink():
                raise ControllerError("canonical package digest rejects symlinks")
        for name in filenames:
            candidate = current_path / name
            info = candidate.lstat()
            if not stat.S_ISREG(info.st_mode):
                raise ControllerError("canonical package contains a non-regular file")
            relative = candidate.relative_to(root).as_posix()
            lines.append((relative, _sha256(candidate.read_bytes())))
    payload = "".join(
        f"{digest}  ./{relative}\n" for relative, digest in sorted(lines)
    ).encode("utf-8")
    return _sha256(payload)


def _validated_source_gate_seal(path: Path, expected_commit: str) -> Dict[str, Any]:
    """Recursively validate an authoritative 15-suite PASS completion seal."""

    path = Path(path).resolve(strict=True)
    _reject_existing_symlink_components(path)
    if path.name != "completion-seal.json":
        raise ControllerError("G1 source authority is not a completion seal")
    run_root = path.parent
    artifacts: Dict[str, bytes] = {}
    for current, directories, filenames in os.walk(run_root, followlinks=False):
        current_path = Path(current)
        for name in [*directories, *filenames]:
            candidate = current_path / name
            if candidate.is_symlink():
                raise ControllerError("source-gate evidence contains a symlink")
        for name in filenames:
            candidate = current_path / name
            artifacts[candidate.relative_to(run_root).as_posix()] = candidate.read_bytes()
    try:
        from test.quality.run_evidence.manifest_contracts import (
            load_canonical_json,
            validate_completion_seal,
        )
        from test.quality.source_gate.contracts import (
            SOURCE_SUITE_ORDER,
            aggregate_results,
        )

        seal = load_canonical_json(artifacts["completion-seal.json"])
        output_root = run_root.parents[2]
        snapshot_relative = seal["source_snapshot_manifest"]["path"]
        snapshot_disk = (
            output_root / "state" / "runs" / seal["run_id"] / snapshot_relative
        )
        _reject_existing_symlink_components(snapshot_disk)
        artifacts[snapshot_relative] = snapshot_disk.read_bytes()
        run = load_canonical_json(artifacts[seal["run_manifest"]["path"]])
        snapshot = load_canonical_json(
            artifacts[seal["source_snapshot_manifest"]["path"]]
        )
        evidence = load_canonical_json(
            artifacts[seal["evidence_manifest"]["path"]]
        )
        validate_completion_seal(seal, run, snapshot, evidence, artifacts)
        results = [
            load_canonical_json(artifacts[item["path"]])
            for item in evidence["test_results"]
        ]
        decision, runner_exit = aggregate_results(results)
    except (KeyError, TypeError, ValueError) as error:
        raise ControllerError("G1 source-gate completion seal is invalid") from error
    if (
        run.get("profile") != "source"
        or run.get("head_sha") != expected_commit
        or tuple(item.get("suite_id") for item in results) != SOURCE_SUITE_ORDER
        or (decision, runner_exit) != ("PASS", 0)
        or (seal.get("aggregate_decision"), seal.get("runner_exit")) != ("PASS", 0)
    ):
        raise ControllerError("G1 source-gate authority is not exact 15-suite PASS")
    return {
        "path": str(path),
        "sha256": _sha256(path.read_bytes()),
        "run_id": seal["run_id"],
        "head_sha": run["head_sha"],
        "suite_count": len(results),
        "aggregate_decision": decision,
        "runner_exit": runner_exit,
    }


class NetworkIsolationGuard:
    """Seatbelt policy and executable probe for loopback-only descendants."""

    def __init__(
        self,
        profile_path: Path,
        owned_unix_socket_subpath: Path,
        sandbox_exec: Path = SANDBOX_EXEC,
    ):
        self.profile_path = Path(profile_path)
        self.sandbox_exec = Path(sandbox_exec)
        requested_subpath = Path(owned_unix_socket_subpath)
        if ".." in requested_subpath.parts:
            raise ControllerError("owned Unix socket subpath must not contain '..'")
        profile_parent = self.profile_path.parent.resolve(strict=True)
        self.owned_unix_socket_subpath = requested_subpath.resolve(strict=False)
        if self.owned_unix_socket_subpath != requested_subpath:
            raise ControllerError(
                "owned Unix socket subpath must be canonical and must not traverse symlinks"
            )
        if not _is_relative_to(self.owned_unix_socket_subpath, profile_parent):
            raise ControllerError("owned Unix socket subpath escapes the private run root")
        self._profile_bytes = _network_sandbox_profile(
            self.owned_unix_socket_subpath
        ).encode("utf-8")
        try:
            current = self.profile_path.read_bytes()
        except FileNotFoundError:
            _safe_write(self.profile_path, self._profile_bytes, 0o600)
        else:
            if current != self._profile_bytes:
                raise ControllerError("network sandbox profile drift")

    @property
    def profile_sha256(self) -> str:
        current = self.profile_path.read_bytes()
        if current != self._profile_bytes:
            raise ControllerError("network sandbox profile drift")
        return _sha256(current)

    def launch_prefix(self) -> List[str]:
        executable = _file_identity(self.sandbox_exec)
        if executable["mode"] & 0o111 == 0:
            raise ControllerError("sandbox-exec is not executable")
        self.profile_sha256
        return [str(self.sandbox_exec), "-p", self._profile_bytes.decode("utf-8")]

    def verify(self) -> Dict[str, Any]:
        listeners = []
        for family, address in (
            (socket.AF_INET, ("127.0.0.1", 0)),
            (socket.AF_INET6, ("::1", 0)),
        ):
            listener = socket.socket(family, socket.SOCK_STREAM)
            listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            listener.bind(address)
            listener.listen(1)
            listener.settimeout(2.0)
            listeners.append(listener)
        ports = [int(listener.getsockname()[1]) for listener in listeners]
        if FORBIDDEN_PORTS.intersection(ports):
            for listener in listeners:
                listener.close()
            raise ControllerError("network probe selected a forbidden port")
        probe_host = f"csswitch-{secrets_module.token_hex(12)}.example.com"
        command = [
            *self.launch_prefix(),
            str(Path(sys.executable).resolve(strict=True)),
            "-I",
            "-c",
            NETWORK_PROBE_SOURCE,
            str(ports[0]),
            str(ports[1]),
            probe_host,
        ]
        try:
            result = subprocess.run(
                command,
                check=False,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                timeout=8.0,
            )
            loopback_payloads = []
            for listener in listeners:
                try:
                    connection, _ = listener.accept()
                except (OSError, socket.timeout):
                    loopback_payloads.append(b"")
                else:
                    with connection:
                        loopback_payloads.append(connection.recv(2))
        finally:
            for listener in listeners:
                listener.close()
        try:
            observation = json.loads(result.stdout)
        except (UnboundLocalError, json.JSONDecodeError) as error:
            raise ControllerError("network isolation probe did not return JSON") from error
        if not isinstance(observation, dict):
            raise ControllerError("network isolation probe returned an invalid value")
        denied_fields = (
            "ipv4_tcp_errno",
            "ipv4_udp_errno",
            "ipv4_dns_tcp_errno",
            "ipv4_dns_udp_errno",
            "ipv6_tcp_errno",
            "ipv6_udp_errno",
            "ipv6_dns_tcp_errno",
            "ipv6_dns_udp_errno",
            "system_resolver_ipc_stream_errno",
            "system_resolver_ipc_datagram_errno",
        )
        transport_blocked = all(
            observation.get(field) == errno.EPERM for field in denied_fields
        )
        ok = (
            result.returncode == 0
            and result.stderr == ""
            and loopback_payloads == [b"4", b"6"]
            and observation.get("ipv4_loopback_allowed") is True
            and observation.get("ipv6_loopback_allowed") is True
            and transport_blocked
            and observation.get("unique_system_resolver_lookup_failed") is True
        )
        receipt = {
            "schema": NETWORK_RECEIPT_SCHEMA,
            "profile_path": str(self.profile_path),
            "profile_sha256": self.profile_sha256,
            "owned_unix_socket_subpath": str(self.owned_unix_socket_subpath),
            "sandbox_exec": _file_identity(self.sandbox_exec),
            "probe": {
                **{field: observation.get(field) for field in denied_fields},
                "ipv4_loopback_allowed": observation.get("ipv4_loopback_allowed"),
                "ipv6_loopback_allowed": observation.get("ipv6_loopback_allowed"),
                "dns_transport_blocked": transport_blocked,
                "unique_system_resolver_lookup_failed_observation": observation.get(
                    "unique_system_resolver_lookup_failed"
                ),
                "exit_code": result.returncode,
            },
            "policy": NETWORK_POLICY,
            "ok": ok,
        }
        if not ok:
            raise ControllerError("network isolation self-test failed")
        return receipt


def _json_pointer_escape(part: str) -> str:
    return part.replace("~", "~0").replace("/", "~1")


def _changed_json_paths(before: Any, after: Any, prefix: str = "") -> List[str]:
    if type(before) is not type(after):
        return [prefix or "/"]
    if isinstance(before, dict):
        paths: List[str] = []
        for key in sorted(set(before) | set(after)):
            child = f"{prefix}/{_json_pointer_escape(str(key))}"
            if key not in before or key not in after:
                paths.append(child)
            else:
                paths.extend(_changed_json_paths(before[key], after[key], child))
        return paths
    if isinstance(before, list):
        paths = []
        for index in range(max(len(before), len(after))):
            child = f"{prefix}/{index}"
            if index >= len(before) or index >= len(after):
                paths.append(child)
            else:
                paths.extend(_changed_json_paths(before[index], after[index], child))
        return paths
    return [] if before == after else [prefix or "/"]


def _clone_step(
    step: ScenarioStep,
    *,
    step_id: str,
    phase: str,
    path: Optional[str] = None,
    expected_model: Optional[str] = None,
) -> Dict[str, Any]:
    checks = copy.deepcopy(step.checks)
    if expected_model:
        body = checks.setdefault("body", {})
        body.setdefault("equals", {})["/model"] = expected_model
    action = copy.deepcopy(step.action)
    if expected_model and action.get("type") in {
        "anthropic_json",
        "anthropic_sse",
        "dsml",
        "openai_chat_text_tool",
        "openai_responses_text_tool",
    }:
        action["model"] = expected_model
    return {
        "id": _require_safe_label(step_id, "step id"),
        "phase": _require_safe_label(phase, "phase"),
        "method": step.method,
        "path": path if path is not None else step.path,
        "action": action,
        "checks": checks,
    }


def build_case_scenario(case: CaseDefinition) -> Scenario:
    """Select one case from the frozen installed-family scenario fixtures."""

    catalog = load_manifest()
    source = catalog[case.mock_scenario]
    selected = [step for step in source.steps if step.phase != "formal"]
    if case.case_id == "siliconflow":
        formal = catalog["relay_siliconflow_proxy_positive"].steps[0]
    else:
        formal = next(
            (step for step in source.steps if step.step_id == case.formal_step_id),
            None,
        )
        if formal is None:
            raise ControllerError("installed mock formal step is missing")
    insert_at = next(
        (index for index, step in enumerate(selected) if step.phase == "reuse"),
        len(selected),
    )
    selected.insert(insert_at, formal)

    expected_model = case.expected_upstream_model or case.model or None
    steps: List[Dict[str, Any]] = []
    phase_ordinals: Dict[str, int] = {}
    for template in selected:
        phase_ordinals[template.phase] = phase_ordinals.get(template.phase, 0) + 1
        suffix = (
            template.phase
            if phase_ordinals[template.phase] == 1
            else f"{template.phase}-{phase_ordinals[template.phase]}"
        )
        path = template.path
        if case.base_kind == "proxy":
            path = (
                "http://api.siliconflow.cn/v1/models"
                if template.phase == "discovery"
                else "http://api.siliconflow.cn/v1/messages"
            )
        step_expected_model = expected_model
        if case.adapter == "qwen":
            step_expected_model = "qwen3.7-max"
        item = _clone_step(
            template,
            step_id=f"{case.case_id}-{suffix}",
            phase=template.phase,
            path=path,
            expected_model=(None if template.phase == "discovery" else step_expected_model),
        )
        if case.case_id == "kimi" and template.phase in {"scratch", "formal"}:
            equals = item["checks"].setdefault("body", {}).setdefault("equals", {})
            equals["/thinking/type"] = "enabled"
            equals["/thinking/budget_tokens"] = 1024
            if "scratch" in template.step_id:
                equals["/max_tokens"] = 1025
        if case.case_id == "kimi" and template.phase == "formal":
            body = item["checks"].setdefault("body", {})
            body.setdefault("required", []).extend(
                pointer
                for pointer in ("/tools/0/name", "/tools/1/name")
                if pointer not in body.setdefault("required", [])
            )
            body.setdefault("absent", []).extend(
                pointer
                for pointer in ("/tools/2", "/tool_choice")
                if pointer not in body.setdefault("absent", [])
            )
            body.setdefault("equals", {})["/stream"] = True
            item["action"] = {"type": "kimi_sse"}
        steps.append(item)
    return scenario_from_steps(
        f"installed-{case.case_id}",
        steps,
        description=f"Installed provider matrix for {case.case_id}",
        phases=("discovery", "scratch", "formal", "reuse", "restart"),
    )


class InProcessScenarioControl:
    """Unit-test-only facade matching the external control contract."""

    def __init__(self, scenario: Scenario):
        self._scenario = scenario
        self._mock = None

    def start(self) -> Dict[str, Any]:
        if self._mock is not None:
            raise ControllerError("mock already started")
        self._mock = start_scenario(
            self._scenario,
            secrets={"provider_key": FAKE_API_KEY},
        )
        return self._mock.ready()

    @property
    def expected_executable(self) -> Path:
        return Path(sys.executable).resolve(strict=True)

    @property
    def base_url(self) -> str:
        if self._mock is None:
            raise ControllerError("mock is not started")
        return self._mock.base_url

    @property
    def port(self) -> int:
        if self._mock is None:
            raise ControllerError("mock is not started")
        return self._mock.port

    def enter_phase(self, phase: str) -> None:
        if self._mock is None:
            raise ControllerError("mock is not started")
        self._mock.enter_phase(phase)

    def status(self) -> Dict[str, Any]:
        if self._mock is None:
            raise ControllerError("mock is not started")
        return self._mock.result()

    def wait(self, timeout_seconds: float) -> bool:
        if self._mock is None:
            raise ControllerError("mock is not started")
        return self._mock.wait_complete(timeout_seconds)

    def stop(self) -> Dict[str, Any]:
        if self._mock is None:
            return {
                "schema": "csswitch.provider-mock-result.v1",
                "stopped": True,
                "complete": False,
                "ok": False,
                "requests": [],
                "failures": [],
            }
        return self._mock.stop()


def _scenario_manifest_value(scenario: Scenario) -> Dict[str, Any]:
    return {
        "schema": MOCK_MANIFEST_SCHEMA,
        "version": MOCK_MANIFEST_VERSION,
        "actions": sorted(ACTION_TYPES),
        "scenarios": {
            scenario.name: {
                "description": scenario.description,
                "phases": list(scenario.phases),
                "steps": [
                    {
                        "id": step.step_id,
                        "phase": step.phase,
                        "method": step.method,
                        "path": step.path,
                        "action": copy.deepcopy(step.action),
                        "checks": copy.deepcopy(step.checks),
                    }
                    for step in scenario.steps
                ],
            }
        },
    }


class SubprocessScenarioControl:
    """Owned CLI mock using anonymous secret FDs and authenticated Unix control."""

    def __init__(self, scenario: Scenario, evidence_parent: Path):
        self._scenario = scenario
        self._parent = Path(evidence_parent)
        self.evidence_dir = self._parent / "provider-mock"
        self.manifest_path = self._parent / "provider-mock-manifest.v1.json"
        self.stderr_path = self._parent / "provider-mock.stderr.log"
        self._process: Optional[subprocess.Popen] = None
        self._client: Optional[UnixScenarioControlClient] = None
        self._token: Optional[str] = None
        self._ready: Optional[Dict[str, Any]] = None

    @property
    def expected_executable(self) -> Path:
        return Path(sys.executable).resolve(strict=True)

    @property
    def base_url(self) -> str:
        if self._client is None:
            raise ControllerError("mock is not started")
        return self._client.base_url

    @property
    def port(self) -> int:
        if self._client is None:
            raise ControllerError("mock is not started")
        return self._client.port

    @property
    def process_pid(self) -> Optional[int]:
        return self._process.pid if self._process is not None else None

    @property
    def process_alive(self) -> bool:
        return self._process is not None and self._process.poll() is None

    @staticmethod
    def _write_pipe(fd: int, payload: bytes) -> None:
        try:
            view = memoryview(payload)
            while view:
                written = os.write(fd, view)
                if written <= 0:
                    raise ControllerError("short anonymous mock input write")
                view = view[written:]
        finally:
            os.close(fd)

    def _read_ready(self, timeout_seconds: float = 8.0) -> Dict[str, Any]:
        ready_path = self.evidence_dir / "ready.json"
        deadline = time.monotonic() + timeout_seconds
        while time.monotonic() < deadline:
            if self._process is not None and self._process.poll() is not None:
                raise ControllerError(
                    f"provider mock exited before ready (exit {self._process.returncode})"
                )
            try:
                _reject_symlink(self.evidence_dir)
                directory_info = self.evidence_dir.stat()
                _reject_symlink(ready_path)
                ready_info = ready_path.stat()
                if (
                    directory_info.st_uid != os.getuid()
                    or stat.S_IMODE(directory_info.st_mode) != 0o700
                    or not stat.S_ISDIR(directory_info.st_mode)
                    or ready_info.st_uid != os.getuid()
                    or stat.S_IMODE(ready_info.st_mode) != 0o600
                    or not stat.S_ISREG(ready_info.st_mode)
                ):
                    raise ControllerError("provider mock ready evidence is not private")
                value = json.loads(ready_path.read_text(encoding="utf-8"))
                if not isinstance(value, dict):
                    raise ControllerError("provider mock ready evidence is invalid")
                return value
            except FileNotFoundError:
                time.sleep(0.02)
        raise ControllerError("provider mock did not publish ready evidence")

    def start(self) -> Dict[str, Any]:
        if self._process is not None:
            raise ControllerError("mock already started")
        _reject_symlink(self._parent)
        parent_info = self._parent.stat()
        if (
            not stat.S_ISDIR(parent_info.st_mode)
            or parent_info.st_uid != os.getuid()
            or stat.S_IMODE(parent_info.st_mode) != 0o700
        ):
            raise ControllerError("mock evidence parent must be owned 0700")
        if self.evidence_dir.exists() or self.evidence_dir.is_symlink():
            raise ControllerError("mock evidence directory must not pre-exist")
        _safe_json_write_once(self.manifest_path, _scenario_manifest_value(self._scenario))

        token_read, token_write = os.pipe()
        secrets_read, secrets_write = os.pipe()
        token = secrets_module.token_urlsafe(32)
        command = [
            str(self.expected_executable),
            str(Path(__file__).with_name("provider_mock_scenarios.py")),
            "--manifest",
            str(self.manifest_path),
            "--scenario",
            self._scenario.name,
            "--evidence-dir",
            str(self.evidence_dir),
            "--control-token-fd",
            str(token_read),
            "--secrets-fd",
            str(secrets_read),
        ]
        stderr_handle = None
        try:
            try:
                if self.stderr_path.exists() or self.stderr_path.is_symlink():
                    raise ControllerError("provider mock stderr evidence must not pre-exist")
                stderr_handle = self.stderr_path.open("xb")
                self.stderr_path.chmod(0o600)
                self._process = subprocess.Popen(
                    command,
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL,
                    stderr=stderr_handle,
                    close_fds=True,
                    pass_fds=(token_read, secrets_read),
                )
            finally:
                if stderr_handle is not None:
                    stderr_handle.close()
                os.close(token_read)
                os.close(secrets_read)
            self._write_pipe(token_write, token.encode("utf-8"))
            token_write = -1
            self._write_pipe(
                secrets_write,
                json.dumps({"provider_key": FAKE_API_KEY}, separators=(",", ":")).encode(),
            )
            secrets_write = -1
        except Exception:
            for fd in (token_write, secrets_write):
                if fd < 0:
                    continue
                try:
                    os.close(fd)
                except OSError:
                    pass
            if self._process is not None:
                self._terminate_owned_process()
            raise
        client: Optional[UnixScenarioControlClient] = None
        try:
            ready = self._read_ready()
            client = UnixScenarioControlClient(self.evidence_dir, ready, token)
            if (
                ready.get("schema") != "csswitch.provider-mock-ready.v1"
                or ready.get("scenario") != self._scenario.name
                or ready.get("owned_pid") != self._process.pid
                or ready.get("host") != "127.0.0.1"
                or ready.get("phase") is not None
            ):
                raise ControllerError("provider mock ready identity is invalid")
            self._token = token
            self._ready = copy.deepcopy(ready)
            self._client = client
            return self._client.start()
        except Exception:
            if client is not None:
                try:
                    client.stop()
                except Exception:
                    pass
            self._terminate_owned_process()
            self._token = None
            self._client = None
            self._ready = None
            raise

    def _terminate_owned_process(self) -> None:
        """Stop only the exact subprocess created by this controller instance."""

        process = self._process
        if process is None or process.poll() is not None:
            return
        process.terminate()
        try:
            process.wait(timeout=3.0)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=3.0)

    def enter_phase(self, phase: str) -> None:
        if self._client is None:
            raise ControllerError("mock is not started")
        self._client.enter_phase(phase)

    def status(self) -> Dict[str, Any]:
        if self._client is None:
            raise ControllerError("mock is not started")
        return self._client.status()

    def wait(self, timeout_seconds: float) -> bool:
        if self._client is None:
            raise ControllerError("mock is not started")
        return self._client.wait(timeout_seconds)

    def stop(self) -> Dict[str, Any]:
        if self._client is None or self._process is None:
            raise ControllerError("mock is not started")
        result = self._client.stop()
        try:
            exit_code = self._process.wait(timeout=8.0)
        except subprocess.TimeoutExpired:
            self._terminate_owned_process()
            raise ControllerError("owned provider mock did not exit after control stop") from None
        self._token = None
        self._client.forget_token()
        safe = copy.deepcopy(result)
        safe["owned_process_exit_code"] = exit_code
        return safe


class UnixScenarioControlClient:
    """Client for the reviewed authenticated ``control.sock`` protocol.

    The caller owns mock process creation and supplies the token through an
    anonymous FD.  This client keeps that token in memory only; neither ready
    evidence nor controller responses contain it.
    """

    def __init__(
        self,
        evidence_dir: Path,
        ready: Mapping[str, Any],
        token: str,
        *,
        result_timeout: float = 8.0,
    ):
        self.evidence_dir = Path(evidence_dir)
        _reject_symlink(self.evidence_dir)
        info = self.evidence_dir.stat()
        if (
            not stat.S_ISDIR(info.st_mode)
            or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o700
        ):
            raise ControllerError("mock evidence directory must be owned 0700")
        socket_name = ready.get("control_socket")
        if not isinstance(socket_name, str) or Path(socket_name).name != socket_name:
            raise ControllerError("mock ready evidence has unsafe control socket")
        if (
            not isinstance(token, str)
            or not (16 <= len(token) <= 1024)
            or any(ord(ch) < 33 for ch in token)
        ):
            raise ControllerError("invalid in-memory mock control token")
        self._ready = dict(ready)
        self._token = token
        self.socket_path = self.evidence_dir / socket_name
        if len(os.fsencode(self.socket_path)) >= 104:
            raise ControllerError("mock control socket path is too long for macOS")
        self.result_timeout = result_timeout

    @property
    def base_url(self) -> str:
        value = self._ready.get("base_url")
        if not isinstance(value, str) or not value.startswith("http://127.0.0.1:"):
            raise ControllerError("mock ready base URL is not loopback")
        return value

    @property
    def port(self) -> int:
        value = self._ready.get("port")
        if not isinstance(value, int) or value in FORBIDDEN_PORTS or not (1 <= value <= 65535):
            raise ControllerError("mock ready port is invalid")
        return value

    def start(self) -> Dict[str, Any]:
        self._validate_socket()
        return copy.deepcopy(self._ready)

    def _validate_socket(self) -> None:
        _reject_symlink(self.socket_path)
        info = os.lstat(self.socket_path)
        if (
            not stat.S_ISSOCK(info.st_mode)
            or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o600
        ):
            raise ControllerError("mock control socket must be owned 0600")

    def _request(self, command: str, **fields: Any) -> Dict[str, Any]:
        self._validate_socket()
        request = {"token": self._token, "command": command, **fields}
        encoded = json.dumps(request, separators=(",", ":")).encode() + b"\n"
        if len(encoded) > MAX_CONTROL_LINE:
            raise ControllerError("mock control request is too large")
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        client.settimeout(3.0)
        try:
            client.connect(str(self.socket_path))
            client.sendall(encoded)
            chunks = bytearray()
            while not chunks.endswith(b"\n"):
                chunk = client.recv(8192)
                if not chunk:
                    break
                chunks.extend(chunk)
                if len(chunks) > MAX_CONTROL_LINE:
                    raise ControllerError("mock control response is too large")
        finally:
            client.close()
        try:
            response = json.loads(bytes(chunks))
        except (UnicodeDecodeError, json.JSONDecodeError):
            raise ControllerError("mock control response is invalid") from None
        if not isinstance(response, dict) or response.get("ok") is not True:
            raise ControllerError("mock control command was rejected")
        return response

    def enter_phase(self, phase: str) -> None:
        self._request("enter_phase", phase=_require_safe_label(phase, "phase"))

    def status(self) -> Dict[str, Any]:
        response = self._request("status")
        status_value = response.get("status")
        if not isinstance(status_value, dict):
            raise ControllerError("mock control status is missing")
        return status_value

    def wait(self, timeout_seconds: float) -> bool:
        timeout_ms = int(max(0.0, min(timeout_seconds, 600.0)) * 1000)
        return bool(self._request("wait", timeout_ms=timeout_ms).get("completed"))

    def stop(self) -> Dict[str, Any]:
        self._request("stop")
        result_path = self.evidence_dir / "result.json"
        deadline = time.monotonic() + self.result_timeout
        while time.monotonic() < deadline:
            try:
                _reject_symlink(result_path)
                info = result_path.stat()
                if (
                    stat.S_ISREG(info.st_mode)
                    and info.st_uid == os.getuid()
                    and stat.S_IMODE(info.st_mode) == 0o600
                ):
                    value = json.loads(result_path.read_text(encoding="utf-8"))
                    if isinstance(value, dict):
                        return value
            except FileNotFoundError:
                pass
            time.sleep(0.02)
        raise ControllerError("mock result evidence did not appear after stop")

    def forget_token(self) -> None:
        self._token = ""


@dataclass(frozen=True)
class ProcessRecord:
    pid: int
    ppid: int
    comm: str


class ProcessInspector:
    """PID/PPID/comm/executable/listener inspection without process argv."""

    def process_table(self) -> List[ProcessRecord]:
        result = subprocess.run(
            ["/bin/ps", "-Ao", "pid=,ppid=,comm="],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        records = []
        for line in result.stdout.splitlines():
            parts = line.strip().split(None, 2)
            if len(parts) != 3 or not parts[0].isdigit() or not parts[1].isdigit():
                continue
            records.append(ProcessRecord(int(parts[0]), int(parts[1]), parts[2]))
        return records

    def executable_for_pid(self, pid: int) -> Optional[Path]:
        if pid <= 1:
            return None
        result = subprocess.run(
            ["/usr/sbin/lsof", "-nP", "-a", "-p", str(pid), "-d", "txt", "-Fn"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        for line in result.stdout.splitlines():
            if line.startswith("n") and len(line) > 1:
                return Path(line[1:])
        return None

    def executable_identity_for_pid(self, pid: int) -> Optional[Dict[str, Any]]:
        if pid <= 1:
            return None
        result = subprocess.run(
            [
                "/usr/sbin/lsof", "-nP", "-a", "-p", str(pid),
                "-d", "txt", "-FnDi",
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        current: Optional[Dict[str, Any]] = None
        for line in result.stdout.splitlines():
            if line == "ftxt":
                if current and {"path", "device", "inode"}.issubset(current):
                    return current
                current = {}
            elif current is not None and line.startswith("D"):
                try:
                    current["device"] = int(line[1:], 0)
                except ValueError:
                    return None
            elif current is not None and line.startswith("i"):
                try:
                    current["inode"] = int(line[1:])
                except ValueError:
                    return None
            elif current is not None and line.startswith("n"):
                current["path"] = str(Path(line[1:]).resolve(strict=False))
        if current and {"path", "device", "inode"}.issubset(current):
            return current
        return None

    def listener_owned(self, pid: int, port: int) -> bool:
        if pid <= 1 or port in FORBIDDEN_PORTS:
            return False
        result = subprocess.run(
            [
                "/usr/sbin/lsof", "-nP", "-a", "-p", str(pid),
                f"-iTCP:{port}", "-sTCP:LISTEN",
            ],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return result.returncode == 0

    def pid_alive(self, pid: int) -> bool:
        if pid <= 1:
            return False
        try:
            os.kill(pid, 0)
            return True
        except ProcessLookupError:
            return False
        except PermissionError:
            return True

    def process_group(self, pid: int) -> Optional[int]:
        if pid <= 1:
            return None
        try:
            return os.getpgid(pid)
        except ProcessLookupError:
            return None

    def pids_in_process_group(self, pgid: int) -> List[int]:
        if pgid <= 1:
            return []
        return sorted(
            record.pid
            for record in self.process_table()
            if self.process_group(record.pid) == pgid
        )

    def process_start_marker(self, pid: int) -> Optional[str]:
        if pid <= 1:
            return None
        result = subprocess.run(
            ["/bin/ps", "-p", str(pid), "-o", "lstart="],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        value = result.stdout.strip()
        return value or None

    def children(self, parent_pid: int) -> List[ProcessRecord]:
        return [record for record in self.process_table() if record.ppid == parent_pid]

    def descendants(self, parent_pid: int) -> List[ProcessRecord]:
        table = self.process_table()
        pending = [parent_pid]
        descendants: List[ProcessRecord] = []
        seen = {parent_pid}
        while pending:
            current = pending.pop()
            for record in table:
                if record.ppid != current or record.pid in seen:
                    continue
                seen.add(record.pid)
                descendants.append(record)
                pending.append(record.pid)
        return descendants

    def pids_using_path(self, root: Path) -> List[int]:
        root = Path(root)
        _reject_existing_symlink_components(root)
        result = subprocess.run(
            ["/usr/sbin/lsof", "-t", "+D", str(root)],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        if result.returncode not in {0, 1}:
            raise ControllerError("runtime-root process inventory failed")
        pids = []
        for line in result.stdout.splitlines():
            if not line.isdigit() or int(line) <= 1:
                raise ControllerError("runtime-root process inventory is malformed")
            pids.append(int(line))
        return sorted(set(pids))


def _bundle_for_executable(executable: Path) -> Optional[Path]:
    try:
        if executable.parent.name != "MacOS":
            return None
        contents = executable.parent.parent
        bundle = contents.parent
        if contents.name != "Contents" or bundle.suffix != ".app":
            return None
        return bundle
    except (AttributeError, IndexError):
        return None


def _read_bundle_info(bundle: Path) -> Dict[str, Any]:
    info_path = bundle / "Contents/Info.plist"
    _reject_existing_symlink_components(bundle)
    _reject_existing_symlink_components(info_path)
    with info_path.open("rb") as handle:
        info = plistlib.load(handle)
    if not isinstance(info, dict):
        raise ControllerError("invalid app Info.plist")
    return info


class InstalledProviderSession:
    def __init__(
        self,
        case_id: str,
        *,
        root: Optional[Path] = None,
        app_bundle: Path = APP_BUNDLE,
        allow_test_bundle: bool = False,
        expected_bundle_id: str = EXPECTED_BUNDLE_ID,
        config_dir_name: str = ".csswitch",
        inspector: Optional[ProcessInspector] = None,
        scenario_control: Optional[Any] = None,
        science_bin: Optional[Path] = None,
        g1_binding_receipt: Optional[Path] = None,
        evidence_dir: Optional[Path] = None,
        preselect_profile: bool = False,
    ):
        if case_id not in CASE_DEFINITIONS:
            raise ControllerError("unknown installed provider case")
        self.case = CASE_DEFINITIONS[case_id]
        self.app_bundle = Path(app_bundle)
        if self.app_bundle != APP_BUNDLE and not allow_test_bundle:
            raise ControllerError("non-installed app bundle is test-only")
        if config_dir_name not in {".csswitch", ".csswitch-acceptance"}:
            raise ControllerError("unsupported config data root")
        if not isinstance(expected_bundle_id, str) or not expected_bundle_id:
            raise ControllerError("expected bundle identifier is required")
        self.expected_bundle_id = expected_bundle_id
        self.config_dir_name = config_dir_name
        self._selected_science_bin = Path(science_bin) if science_bin is not None else None
        self.g1_binding_receipt_path = (
            Path(g1_binding_receipt) if g1_binding_receipt is not None else None
        )
        self.preselect_profile = bool(preselect_profile)
        self.inspector = inspector or ProcessInspector()
        try:
            Path(root).lstat() if root is not None else None
            root_preexisted = root is not None
        except FileNotFoundError:
            root_preexisted = False
        self.root = self._create_root(root)
        self._root_created_by_session = not root_preexisted
        self._workspace_destroyed = False
        self.home = self.root / "home"
        self.csswitch_dir = self.home / self.config_dir_name
        self.evidence = (
            self._create_external_evidence_dir(Path(evidence_dir))
            if evidence_dir is not None
            else self.root / "evidence"
        )
        self.tmp = self.root / "tmp"
        self.bin_dir = self.root / "bin"
        for path in (self.home, self.csswitch_dir, self.evidence, self.tmp, self.bin_dir):
            _ensure_private_dir(path)
        self._evidence_fd = os.open(
            self.evidence,
            os.O_RDONLY | os.O_DIRECTORY | getattr(os, "O_NOFOLLOW", 0),
        )
        evidence_info = os.fstat(self._evidence_fd)
        self._evidence_identity = (evidence_info.st_dev, evidence_info.st_ino)
        self.network_profile = self.root / "loopback-only.sb"
        self._network_guard = NetworkIsolationGuard(
            self.network_profile,
            self.csswitch_dir / "sandbox/home/.claude-science",
        )
        self.app_bin, self.gateway_bin = self._validate_bundle()
        self.fake_science = self.bin_dir / "claude-science"
        self._install_wrappers()
        self._scenario = build_case_scenario(self.case)
        self._proxy_reservation = LoopbackPortReservation()
        try:
            for _ in range(MAX_BIND_ATTEMPTS):
                sandbox_reservation = LoopbackPortReservation()
                if sandbox_reservation.port >= 65535:
                    sandbox_reservation.release()
                    continue
                try:
                    preview_reservation = LoopbackPortReservation(sandbox_reservation.port + 1)
                except (OSError, ValueError):
                    sandbox_reservation.release()
                    continue
                self._sandbox_reservation = sandbox_reservation
                self._preview_reservation = preview_reservation
                break
            else:
                raise ControllerError("could not reserve adjacent sandbox preview port")
        except Exception:
            self._proxy_reservation.release()
            raise
        self.proxy_port = self._proxy_reservation.port
        self.sandbox_port = self._sandbox_reservation.port
        self.preview_port = self._preview_reservation.port
        if self.preview_port != self.sandbox_port + 1 or len(
            {self.proxy_port, self.sandbox_port, self.preview_port}
        ) != 3:
            self._proxy_reservation.release()
            self._sandbox_reservation.release()
            self._preview_reservation.release()
            raise ControllerError("dynamic port collision")
        if FORBIDDEN_PORTS.intersection(
            {self.proxy_port, self.sandbox_port, self.preview_port}
        ):
            self._proxy_reservation.release()
            self._sandbox_reservation.release()
            self._preview_reservation.release()
            raise ControllerError("forbidden port selected")
        if isinstance(scenario_control, InProcessScenarioControl) and not allow_test_bundle:
            self._proxy_reservation.release()
            self._sandbox_reservation.release()
            self._preview_reservation.release()
            raise ControllerError("in-process provider mock is unit-test-only")
        self._mock = scenario_control or SubprocessScenarioControl(self._scenario, self.evidence)
        self._mock_started = False
        self._mock_stopped = False
        self._phase_snapshot: Optional[Tuple[str, int, bool]] = None
        self._config_checkpoint: Optional[Any] = None
        self._config_checkpoint_label: Optional[str] = None
        self._app_pid: Optional[int] = None
        self._runtime_records: Dict[str, Dict[str, Any]] = {}
        self._mock_result: Optional[Dict[str, Any]] = None
        self._last_log_scan: Optional[Dict[str, Any]] = None
        self._provider_receipt: Optional[Dict[str, Any]] = None
        self._fixture_receipt: Optional[Dict[str, Any]] = None
        self._pre_run_manifest: Optional[Dict[str, Any]] = None
        self._pre_run_manifest_sha256: Optional[str] = None
        self._launcher_process: Optional[subprocess.Popen] = None
        self._launcher_pgid: Optional[int] = None
        self._reopen_helper_process: Optional[subprocess.Popen] = None
        self._reopen_helper_members: Dict[int, Tuple[Path, Optional[str]]] = {}
        self._launch_consumed = False
        self._reopen_consumed = False
        self._closed = False
        self._evidence_finalized = False
        self._inventory_before: Optional[Dict[str, Any]] = None

    def _assert_evidence_binding(self) -> None:
        held = os.fstat(self._evidence_fd)
        named = os.stat(self.evidence, follow_symlinks=False)
        if (
            not stat.S_ISDIR(held.st_mode)
            or not stat.S_ISDIR(named.st_mode)
            or (held.st_dev, held.st_ino) != self._evidence_identity
            or (named.st_dev, named.st_ino) != self._evidence_identity
            or held.st_uid != os.getuid()
            or named.st_uid != os.getuid()
            or stat.S_IMODE(held.st_mode) != 0o700
            or stat.S_IMODE(named.st_mode) != 0o700
        ):
            raise ControllerError("external evidence directory binding changed")

    def _write_evidence_json(
        self, name: str, value: Mapping[str, Any], *, once: bool = False
    ) -> str:
        self._assert_evidence_binding()
        encoded = (
            json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
        ).encode("utf-8")
        if once:
            try:
                current = _read_at(self._evidence_fd, name)
            except FileNotFoundError:
                pass
            else:
                if current != encoded:
                    raise ControllerError(f"immutable evidence drift: {name}")
                return _sha256(current)
        _safe_write_at(self._evidence_fd, name, encoded, 0o600)
        self._assert_evidence_binding()
        return _sha256(encoded)

    def _open_evidence_exclusive(self, name: str):
        self._assert_evidence_binding()
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
        fd = os.open(name, flags, 0o600, dir_fd=self._evidence_fd)
        os.fchmod(fd, 0o600)
        return os.fdopen(fd, "wb")

    def _create_external_evidence_dir(self, candidate: Path) -> Path:
        if not candidate.is_absolute():
            raise ControllerError("external evidence directory must be absolute")
        if candidate.exists() or candidate.is_symlink():
            raise ControllerError("external evidence directory must not pre-exist")
        existing_parent = candidate.parent
        if not existing_parent.is_dir():
            raise ControllerError("external evidence parent must already exist")
        _reject_existing_symlink_components(existing_parent)
        canonical_parent = existing_parent.resolve(strict=True)
        canonical = canonical_parent / candidate.name
        repo_root = Path(__file__).resolve(strict=True).parents[1]
        real_home = Path(os.path.expanduser("~")).resolve(strict=True)
        if (
            canonical == self.root
            or _is_relative_to(canonical, self.root)
            or canonical == repo_root
            or _is_relative_to(canonical, repo_root)
            or canonical == real_home
            or _is_relative_to(canonical, real_home)
        ):
            raise ControllerError("external evidence directory overlaps protected state")
        open_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
        parent_fd = os.open(canonical_parent, open_flags)
        try:
            os.mkdir(candidate.name, mode=0o700, dir_fd=parent_fd)
            child_fd = os.open(candidate.name, open_flags, dir_fd=parent_fd)
            try:
                os.fchmod(child_fd, 0o700)
            finally:
                os.close(child_fd)
        finally:
            os.close(parent_fd)
        resolved = candidate.resolve(strict=True)
        if resolved != canonical:
            raise ControllerError("external evidence path changed during creation")
        return resolved

    @staticmethod
    def _create_root(root: Optional[Path]) -> Path:
        if root is None:
            short_tmp = Path("/private/tmp") if Path("/private/tmp").is_dir() else Path(tempfile.gettempdir())
            candidate = Path(tempfile.mkdtemp(prefix="csim.", dir=short_tmp))
        else:
            candidate = Path(root)
            _reject_symlink(candidate)
            candidate.mkdir(parents=True, exist_ok=True, mode=0o700)
        _reject_symlink(candidate)
        candidate.chmod(0o700)
        canonical = candidate.resolve(strict=True)
        real_home = Path(os.path.expanduser("~")).resolve(strict=True)
        if canonical == real_home or _is_relative_to(canonical, real_home):
            raise ControllerError("test root resolves inside real HOME")
        return canonical

    def _validate_bundle(self) -> Tuple[Path, Path]:
        info = _read_bundle_info(self.app_bundle)
        if info.get("CFBundleIdentifier") != self.expected_bundle_id:
            raise ControllerError("unexpected installed bundle identifier")
        executable_name = info.get("CFBundleExecutable")
        if executable_name != EXPECTED_EXECUTABLE:
            raise ControllerError("unexpected installed executable name")
        app_bin = self.app_bundle / "Contents/MacOS" / executable_name
        gateway = self.app_bundle / "Contents/MacOS" / GATEWAY_EXECUTABLE
        for path in (app_bin, gateway):
            _reject_existing_symlink_components(path)
            if not path.is_file() or not os.access(path, os.X_OK):
                raise ControllerError(f"missing installed executable: {path.name}")
        return app_bin, gateway

    def _install_wrappers(self) -> None:
        open_log = self.evidence / "fake-open.log"
        tripwire = self.evidence / "python3-tripwire.log"
        open_script = """#!/bin/sh
set -eu
test -n "${CSSWITCH_FAKE_OPEN_LOG:-}" && printf 'open-called %s\\n' "$*" >> "$CSSWITCH_FAKE_OPEN_LOG"
exit 0
"""
        security_script = "#!/bin/sh\nexit 0\n"
        python_tripwire = (
            "#!/bin/sh\nset -eu\nprintf 'python3-invoked\\n' >> "
            + shlex.quote(str(tripwire))
            + "\nexit 97\n"
        )
        server_path = self.bin_dir / "fake-science-server"
        server_script = """#!/usr/bin/python3
import http.server
import json
import os
from pathlib import Path
import socketserver
import subprocess
import sys
import urllib.parse

port = int(sys.argv[1])
state = Path(sys.argv[2])
state.mkdir(parents=True, exist_ok=True)
os.chmod(state, 0o700)
origin = f"http://127.0.0.1:{port}"
auth_cookie = f"{os.getpid():064x}"

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass
    def reject_auth(self):
        body = b'{"detail":"invalid bearer token"}'
        self.send_response(401)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_POST(self):
        if self.path != "/api/auth/nonce" or self.headers.get("Origin") != origin:
            self.reject_auth()
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            self.reject_auth()
            return
        if length <= 0 or length > 4096:
            self.reject_auth()
            return
        form = urllib.parse.parse_qs(
            self.rfile.read(length).decode("ascii"),
            keep_blank_values=True,
        )
        try:
            expected_nonce = (state / "url-nonce").read_text(encoding="ascii").strip()
        except OSError:
            self.reject_auth()
            return
        if form.get("nonce") != [expected_nonce] or form.get("dest") != ["/"]:
            self.reject_auth()
            return
        body = b'{"ok":true}'
        self.send_response(200)
        self.send_header(
            "Set-Cookie",
            f"operon_auth={auth_cookie}; Path=/; HttpOnly; SameSite=Strict",
        )
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        if self.path.startswith("/api/health"):
            cookies = {}
            for item in self.headers.get("Cookie", "").split(";"):
                if "=" in item:
                    name, value = item.split("=", 1)
                    cookies[name.strip()] = value.strip()
            if self.headers.get("Origin") != origin or cookies.get("operon_auth") != auth_cookie:
                self.reject_auth()
                return
            body = b'{"db_corruption":{"flagged":false,"kind":null},"db_migrations_skipped":false}'
        else:
            body = b'{"status":"ok","fake_science":true}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

class Server(socketserver.TCPServer):
    allow_reuse_address = False

with Server(("127.0.0.1", port), Handler) as server:
    pid = os.getpid()
    result = subprocess.run(
        ["/usr/sbin/lsof", "-nP", "-a", "-p", str(pid), "-d", "txt", "-Fn"],
        check=True, capture_output=True, text=True,
    )
    executable = next((line[1:] for line in result.stdout.splitlines() if line.startswith("n")), "")
    if not executable:
        raise RuntimeError("missing executable identity")
    values = {"pid": str(pid), "port": str(port), "executable": executable}
    for name, value in values.items():
        path = state / name
        path.write_text(value, encoding="utf-8")
        os.chmod(path, 0o600)
    ready = state / "ready"
    ready.write_text("ready", encoding="utf-8")
    os.chmod(ready, 0o600)
    server.serve_forever()
"""
        science_script = """#!/bin/sh
set -eu
cmd="${1:-}"
test "$#" -eq 0 || shift
if test "$cmd" = "--version"; then
  printf '%s\n' 'claude-science acceptance-fixture-1'
  exit 0
fi
data_dir=''
port=''
while test "$#" -gt 0; do
  case "$1" in
    --data-dir) data_dir="$2"; shift 2 ;;
    --port) port="$2"; shift 2 ;;
    *) shift ;;
  esac
done
test -n "$data_dir"
state="$data_dir/csswitch-installed-fake-science"
case "$cmd" in
  serve)
    case "$port" in ''|*[!0-9]*) exit 2 ;; esac
    test "$port" != 8765
    mkdir -p "$state"
    chmod 700 "$state"
    rm -f "$state/pid" "$state/port" "$state/executable" "$state/ready"
    /usr/bin/python3 @SERVER@ "$port" "$state" >/dev/null 2>&1 &
    child=$!
    n=0
    while test ! -f "$state/ready" && test "$n" -lt 100; do
      /bin/sleep 0.05
      n=$((n + 1))
    done
    if test ! -f "$state/ready"; then
      /bin/kill -TERM "$child" 2>/dev/null || true
      wait "$child" 2>/dev/null || true
      exit 1
    fi
    ;;
  status)
    pid="$(cat "$state/pid" 2>/dev/null || true)"
    recorded_port="$(cat "$state/port" 2>/dev/null || true)"
    case "$pid:$recorded_port" in *[!0-9:]*) echo '{"running":false}'; exit 0 ;; esac
    if /bin/kill -0 "$pid" 2>/dev/null && /usr/sbin/lsof -nP -a -p "$pid" -iTCP:"$recorded_port" -sTCP:LISTEN >/dev/null 2>&1; then
      echo '{"running":true}'
    else
      echo '{"running":false}'
      exit 0
    fi
    ;;
  url)
    recorded_port="$(cat "$state/port")"
    count="$(cat "$state/url-count" 2>/dev/null || echo 0)"
    count=$((count + 1))
    printf '%s' "$count" > "$state/url-count"
    nonce="$(printf '%064x' "$count")"
    printf '%s' "$nonce" > "$state/url-nonce"
    printf 'http://127.0.0.1:%s/?nonce=%s\\n' "$recorded_port" "$nonce"
    ;;
  stop)
    if test ! -e "$state/pid" && test ! -e "$state/port" && test ! -e "$state/executable"; then
      echo already-stopped
      exit 0
    fi
    pid="$(cat "$state/pid" 2>/dev/null || true)"
    recorded_port="$(cat "$state/port" 2>/dev/null || true)"
    recorded_exe="$(cat "$state/executable" 2>/dev/null || true)"
    expected_port="${CSSWITCH_EXPECTED_SANDBOX_PORT:-}"
    case "$pid:$recorded_port:$expected_port" in *[!0-9:]*) echo REFUSE >&2; exit 1 ;; esac
    test "$pid" -gt 1 && test "$recorded_port" = "$expected_port" && test "$recorded_port" != 8765
    actual_exe="$(/usr/sbin/lsof -nP -a -p "$pid" -d txt -Fn 2>/dev/null | /usr/bin/sed -n 's/^n//p' | /usr/bin/head -n 1)"
    test -n "$recorded_exe" && test "$actual_exe" = "$recorded_exe" || { echo REFUSE >&2; exit 1; }
    /usr/sbin/lsof -nP -a -p "$pid" -iTCP:"$recorded_port" -sTCP:LISTEN >/dev/null 2>&1 || { echo REFUSE >&2; exit 1; }
    /bin/kill -TERM "$pid"
    rm -f "$state/pid" "$state/port" "$state/executable" "$state/ready"
    echo stopped
    ;;
  *) exit 2 ;;
esac
""".replace("@SERVER@", shlex.quote(str(server_path)))
        for path, body in (
            (self.bin_dir / "open", open_script),
            (self.bin_dir / "security", security_script),
            (self.bin_dir / "python3", python_tripwire),
            (server_path, server_script),
        ):
            _safe_write(path, body.encode(), 0o700)
            _reject_symlink(path)
            path.chmod(0o700)
        native_fixture = os.environ.get("CSSWITCH_ACCEPTANCE_FAKE_SCIENCE_BIN", "")
        selected_source = self._selected_science_bin
        if selected_source is not None and native_fixture:
            raise ControllerError("Science executable was selected twice")
        if selected_source is not None or native_fixture:
            source = selected_source or Path(native_fixture)
            _reject_existing_symlink_components(source)
            source = source.resolve(strict=True)
            source_info = source.stat()
            if (
                not source.is_file()
                or source_info.st_uid != os.getuid()
                or stat.S_IMODE(source_info.st_mode) & 0o022
                or not os.access(source, os.X_OK)
                or source_info.st_size <= 0
                or (selected_source is None and source_info.st_size > 2 * 1024 * 1024)
            ):
                raise ControllerError("native fake Science fixture is unsafe")
            if selected_source is None:
                _safe_write(self.fake_science, source.read_bytes(), 0o700)
                self.science_bin = self.fake_science
            else:
                self.science_bin = source
        else:
            _safe_write(self.fake_science, science_script.encode(), 0o700)
            self.science_bin = self.fake_science
        if self.science_bin == self.fake_science:
            _reject_symlink(self.fake_science)
            self.fake_science.chmod(0o700)
        self._open_log = open_log
        self._python_tripwire = tripwire

    def _profile_base_url(self, mock_base: str) -> str:
        if self.case.base_kind == "native":
            return {
                "deepseek": "https://api.deepseek.com/anthropic",
                "qwen": "https://dashscope.aliyuncs.com/compatible-mode/v1",
            }[self.case.adapter]
        if self.case.base_kind == "proxy":
            return self.case.base_prefix
        return mock_base + self.case.base_prefix

    def _native_override(self, mock_base: str) -> str:
        if self.case.base_kind == "native":
            return mock_base + self.case.message_path
        # Status consumes this loopback-only diagnostic override for every
        # adapter.  The formal/scratch child boundary removes it for
        # relay/custom, so transport still follows the profile base (and, for
        # SiliconFlow, the owned HTTP proxy).
        return mock_base

    def _config_value(self, mock_base: str) -> Dict[str, Any]:
        profile_id = f"installed-{self.case.case_id}"
        return {
            "schema_version": 2,
            "profiles": [
                {
                    "id": profile_id,
                    "name": self.case.profile_name,
                    "template_id": self.case.template_id,
                    "category": self.case.category,
                    "api_format": self.case.api_format,
                    "base_url": self._profile_base_url(mock_base),
                    "api_key": FAKE_API_KEY,
                    "model": self.case.model,
                    "website_url": None,
                    "icon": None,
                    "icon_color": None,
                    "sort_index": 0,
                    "created_at": 1,
                    "notes": "installed-local-mock",
                }
            ],
            "active_id": profile_id if self.preselect_profile else "",
            "proxy_port": self.proxy_port,
            "sandbox_port": self.sandbox_port,
            "secret": FIXED_PATH_SECRET,
            "mode": "proxy",
            "pending_notice": None,
        }

    @property
    def config_path(self) -> Path:
        return self.csswitch_dir / "config.json"

    def _owned_path_has_symlink(self, path: Path) -> bool:
        try:
            relative = Path(path).relative_to(self.root)
        except ValueError:
            return True
        current = self.root
        for part in relative.parts:
            current = current / part
            try:
                if stat.S_ISLNK(current.lstat().st_mode):
                    return True
            except FileNotFoundError:
                return False
        return False

    def _write_config(self, mock_base: str) -> None:
        value = self._config_value(mock_base)
        _safe_json_write(self.config_path, value)
        self.config_path.chmod(0o600)
        self._config_checkpoint = copy.deepcopy(value)
        self._config_checkpoint_label = "prepared"
        self._record_config_fingerprint("prepared")

    def _record_config_fingerprint(self, label: str) -> str:
        raw = self.config_path.read_bytes()
        digest = _sha256(raw)
        self._write_evidence_json(
            f"config-{_require_safe_label(label)}.json",
            {"schema": CONTROLLER_SCHEMA, "label": label, "sha256": digest},
        )
        return digest

    @property
    def _scenario_manifest_path(self) -> Path:
        candidate = getattr(self._mock, "manifest_path", None)
        return Path(candidate) if candidate is not None else self.evidence / "provider-mock-manifest.v1.json"

    def _publish_provider_receipt(
        self, ready: Mapping[str, Any], executable: Path
    ) -> Dict[str, Any]:
        expected_manifest = _scenario_manifest_value(self._scenario)
        manifest_sha256 = self._write_evidence_json(
            self._scenario_manifest_path.name, expected_manifest, once=True
        )
        live = self._mock.status()
        receipt = {
            "schema": PROVIDER_RECEIPT_SCHEMA,
            "case": self.case.case_id,
            "scenario": self._scenario.name,
            "scenario_manifest": {
                "path": str(self._scenario_manifest_path),
                "sha256": manifest_sha256,
            },
            "process": _file_identity(Path(executable).resolve(strict=True)),
            "owned_pid": ready["owned_pid"],
            "listener": {
                "host": ready["host"],
                "port": ready["port"],
                "base_url": ready["base_url"],
                "identity_verified": True,
            },
            "phase": ready["phase"],
            "request_count": len(live.get("requests", [])),
            "failure_count": len(live.get("failures", [])),
            "issued_before_app_launch": self._app_pid is None,
        }
        if (
            receipt["listener"]["host"] != "127.0.0.1"
            or receipt["phase"] is not None
            or receipt["request_count"] != 0
            or receipt["failure_count"] != 0
            or receipt["issued_before_app_launch"] is not True
        ):
            raise ControllerError("provider launch receipt is not a clean pre-run receipt")
        self._write_evidence_json("provider-launch-receipt.json", receipt, once=True)
        self._provider_receipt = copy.deepcopy(receipt)
        return receipt

    def _publish_fixture_receipt(self) -> Dict[str, Any]:
        if self._provider_receipt is None:
            raise ControllerError("provider receipt must precede fixture freeze")
        config_identity = _file_identity(self.config_path)
        config_value = json.loads(self.config_path.read_text(encoding="utf-8"))
        fixture = {
            "schema": FIXTURE_RECEIPT_SCHEMA,
            "case_definition": {
                field: copy.deepcopy(getattr(self.case, field))
                for field in self.case.__dataclass_fields__
            },
            "dynamic_ports": {
                "gateway": self.proxy_port,
                "science": self.sandbox_port,
                "preview": self.preview_port,
                "provider": self._provider_receipt["listener"]["port"],
            },
            "config": config_identity,
            "preselected_active_profile": config_value.get("active_id")
            == f"installed-{self.case.case_id}",
            "provider_scenario_manifest": copy.deepcopy(
                self._provider_receipt["scenario_manifest"]
            ),
            "science_executable": _file_identity(self.science_bin),
            "controller_source": _file_identity(Path(__file__).resolve(strict=True)),
            "provider_source": _file_identity(
                Path(__file__).with_name("provider_mock_scenarios.py").resolve(strict=True)
            ),
            "network_profile": {
                "path": str(self.network_profile),
                "sha256": self._network_guard.profile_sha256,
            },
            "credential_class": "fixed-fake-only",
            "real_provider_credentials_present": False,
            "reserved_port_8765_used": False,
        }
        if len(set(fixture["dynamic_ports"].values())) != 4:
            raise ControllerError("fixture ports are not unique")
        self._write_evidence_json("fixture-receipt.json", fixture, once=True)
        self._fixture_receipt = copy.deepcopy(fixture)
        return fixture

    @staticmethod
    def _repo_state_receipt() -> Dict[str, Any]:
        repo_root = Path(__file__).resolve(strict=True).parents[1]
        head = subprocess.run(
            ["/usr/bin/git", "-C", str(repo_root), "rev-parse", "HEAD"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        status_result = subprocess.run(
            ["/usr/bin/git", "-C", str(repo_root), "status", "--short", "--branch"],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        if head.returncode != 0 or status_result.returncode != 0:
            raise ControllerError("repository identity could not be frozen")
        lines = status_result.stdout.splitlines()
        branch = lines[0][3:] if lines and lines[0].startswith("## ") else None
        changes = lines[1:] if branch is not None else lines
        return {
            "root": str(repo_root),
            "head": head.stdout.strip(),
            "branch": branch,
            "status_lines": changes,
            "status_sha256": _sha256(status_result.stdout.encode("utf-8")),
            "dirty": bool(changes),
            "staged_count": sum(
                1 for line in changes if len(line) >= 2 and line[0] not in {" ", "?"}
            ),
            "unstaged_count": sum(
                1 for line in changes if len(line) >= 2 and line[1] not in {" ", "?"}
            ),
            "untracked_count": sum(1 for line in changes if line.startswith("??")),
        }

    def _validated_g1_binding(self) -> Dict[str, Any]:
        if self.g1_binding_receipt_path is None:
            raise ControllerError("current G1 binding receipt is required before G2 launch")
        path = self.g1_binding_receipt_path.resolve(strict=True)
        _reject_existing_symlink_components(path)
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError as error:
            raise ControllerError("G1 binding receipt is not JSON") from error
        if not isinstance(value, dict):
            raise ControllerError("G1 binding receipt is invalid")
        csswitch = value.get("csswitch")
        science = value.get("science")
        source = value.get("source")
        if (
            value.get("schema") != "csswitch-g1-binding-receipt.v1"
            or value.get("current_g1_result") != "PASS"
            or not isinstance(csswitch, dict)
            or not isinstance(science, dict)
            or not isinstance(source, dict)
        ):
            raise ControllerError("G1 binding receipt is not a current PASS")
        sha256_pattern = re.compile(r"^[0-9a-f]{64}$")
        commit_pattern = re.compile(r"^[0-9a-f]{40}$")
        commit = source.get("commit")
        branch = source.get("branch")
        digest_values = (
            csswitch.get("artifact_record_sha256"),
            csswitch.get("canonical_bundle_digest"),
            csswitch.get("desktop_sha256"),
            csswitch.get("gateway_sha256"),
            science.get("executable_sha256"),
            science.get("package_canonical_digest"),
        )
        if (
            not isinstance(commit, str)
            or commit_pattern.fullmatch(commit) is None
            or not isinstance(branch, str)
            or not branch.strip()
            or any(
                not isinstance(item, str) or sha256_pattern.fullmatch(item) is None
                for item in digest_values
            )
        ):
            raise ControllerError("G1 binding receipt contains malformed identity fields")
        repo_root = Path(__file__).resolve(strict=True).parents[1]
        commit_check = subprocess.run(
            ["/usr/bin/git", "-C", str(repo_root), "cat-file", "-e", f"{commit}^{{commit}}"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        if commit_check.returncode != 0:
            raise ControllerError("G1 source commit is not present in the repository")
        source_gate_path_value = source.get("completion_seal_path")
        source_gate_sha256 = source.get("completion_seal_sha256")
        if (
            not isinstance(source_gate_path_value, str)
            or not isinstance(source_gate_sha256, str)
            or sha256_pattern.fullmatch(source_gate_sha256) is None
        ):
            raise ControllerError("G1 source-gate authority is missing")
        source_authority = _validated_source_gate_seal(
            Path(source_gate_path_value), commit
        )
        if source_authority["sha256"] != source_gate_sha256:
            raise ControllerError("G1 source-gate authority hash mismatch")
        hashes_path = path.parent / "hashes.sha256"
        identity_path = path.parent / "identity-hashes.json"
        _reject_existing_symlink_components(hashes_path)
        _reject_existing_symlink_components(identity_path)
        declared_hashes: Dict[str, str] = {}
        for line in hashes_path.read_text(encoding="utf-8").splitlines():
            parts = line.split(None, 1)
            if len(parts) != 2 or sha256_pattern.fullmatch(parts[0]) is None:
                raise ControllerError("G1 evidence hash list is malformed")
            declared_hashes[parts[1].removeprefix("./")] = parts[0]
        for evidence_path in (path, identity_path):
            relative = evidence_path.relative_to(path.parent).as_posix()
            if declared_hashes.get(relative) != _sha256(evidence_path.read_bytes()):
                raise ControllerError("G1 evidence closure hash mismatch")
        identity = json.loads(identity_path.read_text(encoding="utf-8"))
        if (
            not isinstance(identity, dict)
            or identity.get("schema") != "csswitch-g1-binding-observation.v1"
            or identity.get("source_commit") != commit
            or identity.get("current_recomputable_receipt") != path.name
            or identity.get("source_gate") != source_authority
        ):
            raise ControllerError("G1 identity closure does not bind the receipt")
        artifact_record = identity.get("artifact_record")
        if not isinstance(artifact_record, dict):
            raise ControllerError("G1 artifact record binding is missing")
        artifact_record_path = Path(str(artifact_record.get("path", ""))).resolve(
            strict=True
        )
        _reject_existing_symlink_components(artifact_record_path)
        if (
            artifact_record.get("sha256") != csswitch["artifact_record_sha256"]
            or _sha256(artifact_record_path.read_bytes())
            != csswitch["artifact_record_sha256"]
            or identity.get("desktop_sha256") != csswitch["desktop_sha256"]
            or identity.get("gateway_sha256") != csswitch["gateway_sha256"]
            or identity.get("csswitch_bundle", {}).get("canonical_digest")
            != csswitch["canonical_bundle_digest"]
            or identity.get("science", {}).get("executable_sha256")
            != science["executable_sha256"]
            or identity.get("science", {}).get("package_canonical_digest")
            != science["package_canonical_digest"]
        ):
            raise ControllerError("G1 identity closure conflicts with the receipt")
        artifact_record_text = artifact_record_path.read_text(encoding="utf-8")
        for expected_text in (
            commit,
            csswitch["canonical_bundle_digest"],
            csswitch["desktop_sha256"],
            csswitch["gateway_sha256"],
        ):
            if expected_text not in artifact_record_text:
                raise ControllerError("G1 artifact record does not contain exact identity")
        for authority_text in (
            "Source gate: `15/15 PASS`, aggregate `PASS`, runner exit `0`",
            "CSSwitch exact-artifact scope: `PASS`",
        ):
            if authority_text not in artifact_record_text:
                raise ControllerError("G1 artifact record lacks exact authority statement")
        expected = {
            "artifact_path": str(self.app_bundle),
            "desktop_sha256": _file_identity(self.app_bin)["sha256"],
            "gateway_sha256": _file_identity(self.gateway_bin)["sha256"],
            "bundle_id": self.expected_bundle_id,
        }
        if any(csswitch.get(key) != expected_value for key, expected_value in expected.items()):
            raise ControllerError("G1 CSSwitch identity does not match launch artifact")
        if (
            _canonical_regular_file_list_digest(self.app_bundle)
            != csswitch["canonical_bundle_digest"]
        ):
            raise ControllerError("G1 CSSwitch canonical digest does not match artifact")
        science_identity = _file_identity(self.science_bin)
        if (
            science.get("executable_path") != str(self.science_bin)
            or science.get("executable_sha256") != science_identity["sha256"]
        ):
            raise ControllerError("G1 Science identity does not match launch executable")
        package_path = Path(str(science.get("package_path", ""))).resolve(strict=True)
        _reject_existing_symlink_components(package_path)
        if not _is_relative_to(self.science_bin, package_path):
            raise ControllerError("G1 Science executable is outside the bound package")
        if (
            _canonical_regular_file_list_digest(package_path)
            != science["package_canonical_digest"]
        ):
            raise ControllerError("G1 Science canonical digest does not match package")
        return {
            "path": str(path),
            "sha256": _sha256(path.read_bytes()),
            "schema": value["schema"],
            "current_g1_result": value["current_g1_result"],
            "source": copy.deepcopy(source),
            "source_gate_authority": source_authority,
            "artifact_record": {
                "path": str(artifact_record_path),
                "sha256": csswitch["artifact_record_sha256"],
            },
            "artifact_record_sha256": csswitch.get("artifact_record_sha256"),
            "canonical_bundle_digest": csswitch.get("canonical_bundle_digest"),
            "science_package_canonical_digest": science.get(
                "package_canonical_digest"
            ),
            "science_package_path": str(package_path),
            "evidence_hashes_path": str(hashes_path),
            "identity_closure_path": str(identity_path),
        }

    def _freeze_pre_run(self, launch_env: Mapping[str, str], launch_argv: Sequence[str]) -> Dict[str, Any]:
        if self._provider_receipt is None or self._fixture_receipt is None:
            raise ControllerError("complete fixture/provider receipts are required before launch")
        if self._pre_run_manifest is not None:
            self.validate_pre_run()
            return copy.deepcopy(self._pre_run_manifest)
        if (
            launch_env.get("CSSWITCH_AUTO_BOOT_ON_LAUNCH") == "1"
            and self._fixture_receipt.get("preselected_active_profile") is not True
        ):
            raise ControllerError("auto-boot requires a preselected fixture profile")
        g1_binding = self._validated_g1_binding()
        network_receipt = self._network_guard.verify()
        network_sha256 = self._write_evidence_json(
            "network-isolation-receipt.json", network_receipt, once=True
        )
        artifact_tree = _tree_manifest(self.app_bundle)
        artifact_tree_sha256 = self._write_evidence_json(
            "artifact-tree-manifest.json", artifact_tree, once=True
        )
        science_package_tree = _tree_manifest(
            Path(g1_binding["science_package_path"])
        )
        science_package_tree_sha256 = self._write_evidence_json(
            "science-package-tree-manifest.json", science_package_tree, once=True
        )
        fixture_path = self.evidence / "fixture-receipt.json"
        provider_path = self.evidence / "provider-launch-receipt.json"
        manifest = {
            "schema": PRE_RUN_MANIFEST_SCHEMA,
            "frozen_at_utc": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
            "executor": {"uid": os.getuid(), "pid": os.getpid()},
            "authorized_scope": "isolated-live loopback fixture; no account, credential, provider, SSH, installed, signing, or release claim",
            "repository": self._repo_state_receipt(),
            "g1_binding_receipt": g1_binding,
            "artifact": {
                "bundle_path": str(self.app_bundle),
                "bundle_id": self.expected_bundle_id,
                "tree_manifest_path": str(self.evidence / "artifact-tree-manifest.json"),
                "tree_manifest_sha256": artifact_tree_sha256,
                "entries_sha256": artifact_tree["entries_sha256"],
                "desktop": _file_identity(self.app_bin),
                "gateway": _file_identity(self.gateway_bin),
            },
            "science_package": {
                "path": g1_binding["science_package_path"],
                "tree_manifest_path": str(
                    self.evidence / "science-package-tree-manifest.json"
                ),
                "tree_manifest_sha256": science_package_tree_sha256,
                "entries_sha256": science_package_tree["entries_sha256"],
                "executable": _file_identity(self.science_bin),
            },
            "fixture_receipt": {
                "path": str(fixture_path),
                "sha256": _sha256(fixture_path.read_bytes()),
            },
            "provider_launch_receipt": {
                "path": str(provider_path),
                "sha256": _sha256(provider_path.read_bytes()),
            },
            "network_isolation_receipt": {
                "path": str(self.evidence / "network-isolation-receipt.json"),
                "sha256": network_sha256,
            },
            "launch": {
                "argv": list(launch_argv),
                "environment": dict(sorted(launch_env.items())),
                "argv_sha256": _sha256(_canonical_json_bytes(list(launch_argv))),
                "environment_sha256": _sha256(_canonical_json_bytes(dict(launch_env))),
                "controller_executes_launch": True,
            },
            "deadlines_seconds": {"observe_start": 60, "stop": 60, "overall": 300},
            "network_policy": NETWORK_POLICY,
            "preconditions": {
                "current_g1_binding_pass": True,
                "artifact_identity_complete": True,
                "fixture_receipt_complete": True,
                "provider_receipt_complete": True,
                "network_isolation_self_test": True,
                "reserved_8765_absent_from_fixture": True,
            },
        }
        encoded = json.dumps(manifest, ensure_ascii=False, sort_keys=True)
        if FIXED_PATH_SECRET in encoded or FAKE_API_KEY in encoded:
            raise AssertionError("pre-run manifest contains sensitive fixture material")
        manifest_path = self.evidence / "pre-run-manifest.json"
        self._pre_run_manifest_sha256 = self._write_evidence_json(
            manifest_path.name, manifest, once=True
        )
        self._pre_run_manifest = copy.deepcopy(manifest)
        return manifest

    def validate_pre_run(self) -> Dict[str, Any]:
        if self._pre_run_manifest is None or self._pre_run_manifest_sha256 is None:
            raise ControllerError("pre-run manifest has not been frozen")
        manifest_path = self.evidence / "pre-run-manifest.json"
        if _sha256(manifest_path.read_bytes()) != self._pre_run_manifest_sha256:
            raise ControllerError("pre-run manifest drift")
        if self._repo_state_receipt() != self._pre_run_manifest["repository"]:
            raise ControllerError("pre-run repository state drift")
        if self._validated_g1_binding() != self._pre_run_manifest["g1_binding_receipt"]:
            raise ControllerError("pre-run G1 binding closure drift")
        for field in (
            "g1_binding_receipt",
            "fixture_receipt",
            "provider_launch_receipt",
            "network_isolation_receipt",
        ):
            binding = self._pre_run_manifest[field]
            if _sha256(Path(binding["path"]).read_bytes()) != binding["sha256"]:
                raise ControllerError(f"pre-run {field} drift")
        if _file_identity(self.config_path)["sha256"] != self._fixture_receipt["config"]["sha256"]:
            raise ControllerError("pre-run fixture config drift")
        for field in ("controller_source", "provider_source", "science_executable"):
            frozen = self._fixture_receipt[field]
            if _file_identity(Path(frozen["path"]))["sha256"] != frozen["sha256"]:
                raise ControllerError(f"pre-run {field} drift")
        for field in ("desktop", "gateway"):
            frozen = self._pre_run_manifest["artifact"][field]
            if _file_identity(Path(frozen["path"]))["sha256"] != frozen["sha256"]:
                raise ControllerError(f"pre-run artifact {field} drift")
        current_tree = _tree_manifest(self.app_bundle)
        if (
            current_tree["entries_sha256"]
            != self._pre_run_manifest["artifact"]["entries_sha256"]
        ):
            raise ControllerError("pre-run artifact tree drift")
        current_science_tree = _tree_manifest(
            Path(self._pre_run_manifest["science_package"]["path"])
        )
        if (
            current_science_tree["entries_sha256"]
            != self._pre_run_manifest["science_package"]["entries_sha256"]
        ):
            raise ControllerError("pre-run Science package tree drift")
        if self._network_guard.profile_sha256 != self._fixture_receipt["network_profile"]["sha256"]:
            raise ControllerError("pre-run network profile drift")
        return {
            "ok": True,
            "manifest_path": str(manifest_path),
            "manifest_sha256": self._pre_run_manifest_sha256,
        }

    def preflight_blockers(self) -> List[str]:
        blockers = list(self.case.blockers)
        if self.same_bundle_processes():
            blockers.append("same_bundle_process_running")
        return sorted(set(blockers))

    def same_bundle_processes(self) -> List[Dict[str, Any]]:
        matches = []
        for record in self.inspector.process_table():
            if Path(record.comm).name != EXPECTED_EXECUTABLE:
                continue
            executable = self.inspector.executable_for_pid(record.pid)
            if executable is None:
                # A same-named process that cannot be identified is not safe to ignore.
                matches.append({"pid": record.pid, "executable": None, "identity": "unknown"})
                continue
            bundle = _bundle_for_executable(executable)
            if bundle is None:
                continue
            try:
                bundle_id = _read_bundle_info(bundle).get("CFBundleIdentifier")
            except (ControllerError, OSError, plistlib.InvalidFileException):
                matches.append(
                    {
                        "pid": record.pid,
                        "executable": str(executable),
                        "identity": "unknown_bundle",
                    }
                )
                continue
            if bundle_id == self.expected_bundle_id:
                matches.append(
                    {"pid": record.pid, "executable": str(executable), "identity": "same_bundle"}
                )
        return matches

    def start_mock(self) -> Dict[str, Any]:
        if self._mock_started:
            raise ControllerError("mock already started")
        self._inventory_before = {
            "schema": "csswitch.isolated-live-inventory.v1",
            "stage": "before",
            "owned_processes": [],
            "reserved_ports": {
                "gateway": self.proxy_port,
                "science": self.sandbox_port,
                "preview": self.preview_port,
            },
            "same_bundle_process_count": len(self.same_bundle_processes()),
        }
        self._write_evidence_json(
            "inventory-before.json", self._inventory_before, once=True
        )
        self._write_evidence_json(
            self._scenario_manifest_path.name,
            _scenario_manifest_value(self._scenario),
            once=True,
        )
        ready = self._mock.start()
        if self._mock.port in {
            self.proxy_port,
            self.sandbox_port,
            self.preview_port,
        } | set(FORBIDDEN_PORTS):
            self._mock.stop()
            raise ControllerError("mock selected a reserved port")
        mock_pid = ready.get("owned_pid")
        if not isinstance(mock_pid, int) or mock_pid <= 1:
            self._mock.stop()
            raise ControllerError("mock owned PID is invalid")
        executable = self.inspector.executable_for_pid(mock_pid)
        expected_executables = {self._mock.expected_executable}
        if isinstance(self._mock, SubprocessScenarioControl):
            controller_executable = self.inspector.executable_for_pid(os.getpid())
            if controller_executable is not None:
                expected_executables.add(controller_executable.resolve(strict=False))
        try:
            executable_matches = (
                executable is not None
                and executable.resolve(strict=True) in expected_executables
            )
        except OSError:
            executable_matches = False
        if not executable_matches or not self.inspector.listener_owned(mock_pid, self._mock.port):
            self._mock.stop()
            raise ControllerError("mock PID/executable/listener identity not proven")
        self._mock_started = True
        self._mock_stopped = False
        self._write_config(self._mock.base_url)
        safe_ready = {
            "schema": ready["schema"],
            "scenario": ready["scenario"],
            "host": ready["host"],
            "port": ready["port"],
            "base_url": ready["base_url"],
            "owned_pid": ready["owned_pid"],
            "owned_executable": str(executable),
            "listener_verified": True,
            "phase": ready["phase"],
        }
        self._write_evidence_json("mock-ready.json", safe_ready)
        self._publish_provider_receipt(ready, executable)
        self._publish_fixture_receipt()
        return safe_ready

    def prepare_dry_run(self) -> Dict[str, Any]:
        reservation = LoopbackPortReservation()
        try:
            if reservation.port in {self.proxy_port, self.sandbox_port, self.preview_port}:
                raise ControllerError("dry-run mock port collision")
            mock_base = f"http://127.0.0.1:{reservation.port}"
            self._write_config(mock_base)
            plan = self.safe_plan(mock_base=mock_base, freeze_pre_run=False)
            plan["dry_run"] = True
            plan["do_not_execute"] = True
            plan["preflight_would_allow_launch"] = plan["launch_allowed"]
            plan["launch_allowed"] = False
            return plan
        finally:
            reservation.release()

    def _launch_environment(self, mock_base: str, *, auto_boot: bool = False) -> Dict[str, str]:
        env = {
            "HOME": str(self.home),
            "CFFIXED_USER_HOME": str(self.home),
            "TMPDIR": str(self.tmp),
            "PATH": f"{self.bin_dir}:/usr/bin:/bin:/usr/sbin:/sbin",
            "SCIENCE_BIN": str(self.science_bin),
            "CSSWITCH_ACCEPTANCE_OPEN_BIN": str(self.bin_dir / "open"),
            "CSSWITCH_EXPECTED_SANDBOX_PORT": str(self.sandbox_port),
            "CSSWITCH_ACCEPTANCE_OUTER_SANDBOX": "1",
            "CSSWITCH_TOOLUSE_SHIM": self.case.shim,
            "CSSWITCH_UPSTREAM_URL": self._native_override(mock_base),
            "CSSWITCH_FAKE_OPEN_LOG": str(self._open_log),
            "CSSWITCH_DOCTOR_CHECK_REAL_HOME": "0",
            "CSSWITCH_AUTO_BOOT_ON_LAUNCH": "1" if auto_boot else "0",
            "CSSWITCH_SCIENCE_WEBVIEW_SPIKE": "0",
            "CSSWITCH_REPO": "",
            "CSSWITCH_PROVIDER": "",
            "CSSWITCH_AUTH_TOKEN": "",
            "CSSWITCH_LAUNCH_ID": "",
            "CSSWITCH_OPENAI_BASE_URL": "",
            "CSSWITCH_OPENAI_MODEL": "",
            "CSSWITCH_RELAY_BASE_URL": "",
            "CSSWITCH_RELAY_MODEL": "",
            "CSSWITCH_RELAY_THINKING": "",
            "DEEPSEEK_API_KEY": "",
            "DASHSCOPE_API_KEY": "",
            "CSSWITCH_RELAY_KEY": "",
            "CSSWITCH_OPENAI_KEY": "",
            "ANTHROPIC_API_KEY": "",
            "ANTHROPIC_AUTH_TOKEN": "",
            "ANTHROPIC_BASE_URL": "",
            "HTTP_PROXY": "",
            "HTTPS_PROXY": "",
            "ALL_PROXY": "",
            "http_proxy": "",
            "https_proxy": "",
            "all_proxy": "",
            "NO_PROXY": "127.0.0.1,localhost,::1",
            "no_proxy": "127.0.0.1,localhost,::1",
        }
        if self.case.base_kind == "proxy":
            env.update(
                {
                    "HTTP_PROXY": mock_base,
                    "http_proxy": mock_base,
                    "HTTPS_PROXY": mock_base,
                    "https_proxy": mock_base,
                    "NO_PROXY": "",
                    "no_proxy": "",
                }
            )
        return env

    def launch_argv(self, *, auto_boot: bool = False, mock_base: Optional[str] = None) -> List[str]:
        if self.case.blockers:
            raise ControllerError("case has unresolved fail-closed blockers")
        if mock_base is None:
            if not self._mock_started:
                raise ControllerError("mock must be started before launch plan")
            mock_base = self._mock.base_url
        env = self._launch_environment(mock_base, auto_boot=auto_boot)
        argv = [*self._network_guard.launch_prefix(), "/usr/bin/env", "-i"]
        for name in sorted(env):
            argv.append(f"{name}={env[name]}")
        argv.append(str(self.app_bin))
        return argv

    def safe_plan(
        self,
        *,
        mock_base: Optional[str] = None,
        auto_boot: bool = False,
        freeze_pre_run: bool = True,
    ) -> Dict[str, Any]:
        if mock_base is None:
            if not self._mock_started:
                raise ControllerError("mock must be started before launch plan")
            mock_base = self._mock.base_url
        blockers = self.preflight_blockers()
        argv = None
        pre_run = None
        if not blockers:
            argv = self.launch_argv(auto_boot=auto_boot, mock_base=mock_base)
            encoded = json.dumps(argv, sort_keys=True)
            if FIXED_PATH_SECRET in encoded or FAKE_API_KEY in encoded:
                raise AssertionError("secret entered launch plan")
            if freeze_pre_run:
                pre_run = self._freeze_pre_run(
                    self._launch_environment(mock_base, auto_boot=auto_boot), argv
                )
        return {
            "schema": CONTROLLER_SCHEMA,
            "case": self.case.case_id,
            "app_bundle": str(self.app_bundle),
            "app_executable": str(self.app_bin),
            "gateway_executable": str(self.gateway_bin),
            "proxy_port": self.proxy_port,
            "sandbox_port": self.sandbox_port,
            "preview_port": self.preview_port,
            "mock_base_url": mock_base,
            "adapter": self.case.adapter,
            "shim": self.case.shim,
            "formal_variant": self.case.formal_variant,
            "mock_phases": self.mock_phases(),
            "blockers": blockers,
            "launch_allowed": not blockers,
            "launch_argv": argv if not freeze_pre_run else None,
            "launch_operation": "launch_guarded" if freeze_pre_run else None,
            "network_policy": NETWORK_POLICY,
            "pre_run_manifest": (
                {
                    "path": str(self.evidence / "pre-run-manifest.json"),
                    "sha256": self._pre_run_manifest_sha256,
                }
                if pre_run is not None
                else None
            ),
            "controller_launches_app": freeze_pre_run,
        }

    def _validate_provider_live(self) -> None:
        if self._provider_receipt is None:
            raise ControllerError("provider receipt is missing")
        pid = self._provider_receipt.get("owned_pid")
        executable = self._provider_receipt.get("process", {}).get("path")
        listener = self._provider_receipt.get("listener", {})
        status_value = self._mock.status()
        actual_executable = self.inspector.executable_for_pid(pid)
        if (
            not isinstance(pid, int)
            or pid <= 1
            or not self.inspector.pid_alive(pid)
            or actual_executable is None
            or str(actual_executable.resolve(strict=True)) != executable
            or not self.inspector.listener_owned(pid, listener.get("port"))
            or status_value.get("active_phase") is not None
            or len(status_value.get("requests", [])) != 0
            or len(status_value.get("failures", [])) != 0
        ):
            raise ControllerError("provider live identity drift before launch")

    def launch_guarded(self) -> Dict[str, Any]:
        """Revalidate and execute the frozen argv in one controller operation."""

        if self._launch_consumed or self._launcher_process is not None:
            raise ControllerError("guarded launch is one-shot")
        blockers = self.preflight_blockers()
        if blockers:
            raise ControllerError("refusing guarded launch while preflight is blocked")
        self.validate_pre_run()
        self._validate_provider_live()
        self._launch_consumed = True
        self._proxy_reservation.release()
        self._sandbox_reservation.release()
        self._preview_reservation.release()
        self._validate_provider_live()
        if not all(
            self._port_closed(port)
            for port in (self.proxy_port, self.sandbox_port, self.preview_port)
        ):
            raise ControllerError("reserved loopback port was acquired before launch")
        stdout_handle = self._open_evidence_exclusive("guarded-launch.stdout.log")
        try:
            stderr_handle = self._open_evidence_exclusive("guarded-launch.stderr.log")
        except Exception:
            stdout_handle.close()
            raise
        try:
            process = subprocess.Popen(
                self._pre_run_manifest["launch"]["argv"],
                stdin=subprocess.DEVNULL,
                stdout=stdout_handle,
                stderr=stderr_handle,
                close_fds=True,
                start_new_session=True,
            )
        finally:
            stdout_handle.close()
            stderr_handle.close()
        self._launcher_process = process
        self._launcher_pgid = self.inspector.process_group(process.pid)
        if self._launcher_pgid != process.pid:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=3.0)
            raise ControllerError("guarded launcher did not acquire an owned process group")
        deadline = time.monotonic() + 2.0
        actual_executable = None
        actual_executable_identity = None
        while time.monotonic() < deadline and process.poll() is None:
            actual_executable = self.inspector.executable_for_pid(process.pid)
            actual_executable_identity = self.inspector.executable_identity_for_pid(
                process.pid
            )
            if actual_executable is not None and actual_executable_identity is not None:
                break
            time.sleep(0.02)
        frozen_app = self._pre_run_manifest["artifact"]["desktop"]
        current_app = _file_identity(self.app_bin)
        if (
            process.poll() is not None
            or actual_executable is None
            or actual_executable_identity is None
            or actual_executable.resolve(strict=False) != self.app_bin
            or actual_executable_identity["path"] != str(self.app_bin)
            or actual_executable_identity["device"] != frozen_app["device"]
            or actual_executable_identity["inode"] != frozen_app["inode"]
            or any(
                current_app[field] != frozen_app[field]
                for field in ("sha256", "size", "mode", "device", "inode")
            )
        ):
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=3.0)
            raise ControllerError("launched Desktop identity does not match frozen artifact")
        receipt = {
            "schema": "csswitch.guarded-launch-receipt.v1",
            "pid": process.pid,
            "pgid": self._launcher_pgid,
            "process_start_marker": self.inspector.process_start_marker(process.pid),
            "pre_run_manifest_sha256": self._pre_run_manifest_sha256,
            "controller_owned": True,
            "executable": current_app,
            "exit_code": None,
        }
        self._write_evidence_json("guarded-launch-receipt.json", receipt)
        return {"pid": process.pid, "controller_owned": True}

    def mock_phases(self) -> List[Dict[str, Any]]:
        counts: Dict[str, int] = {}
        for step in self._scenario.steps:
            counts[step.phase] = counts.get(step.phase, 0) + 1
        phases = []
        if self.case.base_kind == "native":
            phases.append({"phase": "discovery", "mock_request_expected": False, "steps": 0})
        for phase in ("discovery", "scratch", "formal", "reuse", "restart"):
            if counts.get(phase):
                phases.append(
                    {"phase": phase, "mock_request_expected": True, "steps": counts[phase]}
                )
        return phases

    def enter_phase(self, phase: str) -> Dict[str, Any]:
        _require_safe_label(phase, "phase")
        if not self._mock_started:
            raise ControllerError("mock not started")
        status = self._mock.status()
        request_count = len(status.get("requests", []))
        no_request = self.case.base_kind == "native" and phase == "discovery"
        self._mock.enter_phase(phase)
        self._phase_snapshot = (phase, request_count, no_request)
        return {
            "phase": phase,
            "mock_request_expected": not no_request,
            "request_count_before": request_count,
        }

    def finish_phase(self, phase: str) -> Dict[str, Any]:
        if self._phase_snapshot is None or self._phase_snapshot[0] != phase:
            raise ControllerError("phase was not entered")
        _, before, no_request = self._phase_snapshot
        result = self._mock.status()
        requests = result.get("requests", [])
        after = len(requests)
        expected_ids = [step.step_id for step in self._scenario.steps if step.phase == phase]
        consumed_ids = [item.get("step") for item in requests]
        if no_request:
            ok = after == before
        else:
            ok = all(step_id in consumed_ids for step_id in expected_ids)
        self._phase_snapshot = None
        return {
            "phase": phase,
            "ok": ok,
            "requests_added": after - before,
            "expected_steps": len(expected_ids),
            "terminal_failure": bool(result.get("failures")),
        }

    def _live_guarded_launch_receipt(self) -> Dict[str, Any]:
        if self._launcher_process is None or self._launcher_process.poll() is not None:
            raise ControllerError("controller-owned primary app is not alive")
        try:
            raw = _read_at(self._evidence_fd, "guarded-launch-receipt.json")
            receipt = json.loads(raw)
        except (FileNotFoundError, UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ControllerError("guarded launch receipt is unavailable") from error
        pid = receipt.get("pid")
        start_marker = receipt.get("process_start_marker")
        if (
            receipt.get("schema") != "csswitch.guarded-launch-receipt.v1"
            or receipt.get("controller_owned") is not True
            or not isinstance(pid, int)
            or pid <= 1
            or pid != self._launcher_process.pid
            or not isinstance(start_marker, str)
            or not start_marker
            or not self.inspector.pid_alive(pid)
            or self.inspector.executable_for_pid(pid) != self.app_bin
            or self.inspector.process_start_marker(pid) != start_marker
        ):
            raise ControllerError("controller-owned primary app identity drift")
        return receipt

    def observe_app(self, timeout_seconds: float = 8.0) -> Dict[str, Any]:
        """Observe the exact installed process after root executes launch_argv."""

        deadline = time.monotonic() + timeout_seconds
        while time.monotonic() < deadline:
            matches = []
            for record in self.inspector.process_table():
                executable = self.inspector.executable_for_pid(record.pid)
                if executable == self.app_bin:
                    matches.append(record.pid)
            if len(matches) == 1:
                if self._launcher_process is not None:
                    receipt = self._live_guarded_launch_receipt()
                    if matches[0] != receipt["pid"]:
                        raise ControllerError("observed app is not the guarded primary")
                self._app_pid = matches[0]
                value = {"pid": matches[0], "executable": str(self.app_bin), "identity_verified": True}
                self._write_evidence_json("app-identity.json", value)
                return value
            if len(matches) > 1:
                raise ControllerError("multiple exact installed app processes observed")
            time.sleep(0.05)
        raise ControllerError("exact installed app process not observed")

    def reopen_guarded(self, timeout_seconds: float = 8.0) -> Dict[str, Any]:
        """Trigger the production single-instance entry without forcing a new App."""

        if self._reopen_consumed:
            raise ControllerError("guarded reopen is one-shot")
        if self._app_pid is None or self._launcher_process is None:
            raise ControllerError("primary app identity must be observed before reopen")
        if not 0.5 <= timeout_seconds <= 30.0:
            raise ControllerError("guarded reopen timeout is out of range")
        if self._pre_run_manifest is None or self._pre_run_manifest_sha256 is None:
            raise ControllerError("pre-run manifest has not been frozen")
        manifest_path = self.evidence / "pre-run-manifest.json"
        if _sha256(manifest_path.read_bytes()) != self._pre_run_manifest_sha256:
            raise ControllerError("pre-run manifest drift before reopen")
        if self._validated_g1_binding() != self._pre_run_manifest["g1_binding_receipt"]:
            raise ControllerError("G1 binding closure drift before reopen")
        for field in ("desktop", "gateway"):
            frozen = self._pre_run_manifest["artifact"][field]
            if _file_identity(Path(frozen["path"]))["sha256"] != frozen["sha256"]:
                raise ControllerError(f"artifact {field} drift before reopen")
        if (
            _tree_manifest(self.app_bundle)["entries_sha256"]
            != self._pre_run_manifest["artifact"]["entries_sha256"]
        ):
            raise ControllerError("artifact tree drift before reopen")
        frozen_science = self._fixture_receipt["science_executable"]
        if _file_identity(Path(frozen_science["path"]))["sha256"] != frozen_science["sha256"]:
            raise ControllerError("Science executable drift before reopen")
        self._validate_provider_live()
        launch_receipt = self._live_guarded_launch_receipt()
        primary_pid = self._app_pid
        primary_start = launch_receipt["process_start_marker"]
        primary_executable = self.inspector.executable_for_pid(primary_pid)
        exact_before = sorted(
            record.pid
            for record in self.inspector.process_table()
            if self.inspector.executable_for_pid(record.pid) == self.app_bin
        )
        if (
            exact_before != [primary_pid]
            or primary_pid != launch_receipt["pid"]
            or primary_executable != self.app_bin
            or self.inspector.process_start_marker(primary_pid) != primary_start
        ):
            raise ControllerError("primary app identity drift before reopen")

        stdout_handle = self._open_evidence_exclusive("guarded-reopen.stdout.log")
        try:
            stderr_handle = self._open_evidence_exclusive("guarded-reopen.stderr.log")
        except Exception:
            stdout_handle.close()
            raise
        try:
            helper = subprocess.Popen(
                self._pre_run_manifest["launch"]["argv"],
                stdin=subprocess.DEVNULL,
                stdout=stdout_handle,
                stderr=stderr_handle,
                close_fds=True,
                start_new_session=True,
            )
            self._reopen_helper_process = helper
        finally:
            stdout_handle.close()
            stderr_handle.close()
        self._reopen_consumed = True
        # start_new_session makes the child PID its PGID. Freeze group members
        # only while the exact helper leader is still alive; after it exits the
        # numeric PGID may be reused and is never an ownership source.
        helper_pgid = helper.pid
        deadline = time.monotonic() + timeout_seconds
        try:
            while helper.poll() is None and time.monotonic() < deadline:
                self._freeze_live_reopen_helper_group(helper, helper_pgid)
                time.sleep(0.01)
        except Exception:
            self._stop_reopen_helper_process(helper)
            self._cleanup_reopen_helper_group()
            raise
        if helper.poll() is None:
            self._stop_reopen_helper_process(helper)
            self._cleanup_reopen_helper_group()
            raise ControllerError("second-instance reopen did not return to the primary app")
        helper_exit = helper.wait(timeout=3.0)
        self._reopen_helper_members.pop(helper.pid, None)
        if helper_exit != 0:
            self._cleanup_reopen_helper_group()
            raise ControllerError("second-instance reopen helper failed")

        frozen_residuals = self._reconcile_finished_reopen_helper_members()
        if frozen_residuals:
            self._cleanup_reopen_helper_group()
            raise ControllerError("second-instance reopen left a frozen residual process")

        if self.inspector.pids_in_process_group(helper_pgid):
            self._cleanup_reopen_helper_group()
            raise ControllerError("second-instance reopen left a residual process group")

        deadline = time.monotonic() + timeout_seconds
        exact_after: List[int] = []
        while time.monotonic() < deadline:
            exact_after = sorted(
                record.pid
                for record in self.inspector.process_table()
                if self.inspector.executable_for_pid(record.pid) == self.app_bin
            )
            if exact_after == [primary_pid]:
                break
            time.sleep(0.05)
        primary_preserved = (
            exact_after == [primary_pid]
            and self.inspector.pid_alive(primary_pid)
            and self.inspector.executable_for_pid(primary_pid) == self.app_bin
            and self.inspector.process_start_marker(primary_pid) == primary_start
        )
        if not primary_preserved:
            raise ControllerError("reopen did not preserve one exact primary app identity")
        receipt = {
            "schema": "csswitch.guarded-reopen-receipt.v1",
            "primary_pid": primary_pid,
            "primary_process_start_marker": primary_start,
            "primary_executable": _file_identity(self.app_bin),
            "helper_pid": helper.pid,
            "helper_pgid": helper_pgid,
            "helper_exit_code": helper_exit,
            "helper_process_group_empty": True,
            "exact_app_pids_before": exact_before,
            "exact_app_pids_after": exact_after,
            "primary_identity_preserved": True,
            "second_persistent_app_absent": True,
        }
        self._write_evidence_json("guarded-reopen-receipt.json", receipt, once=True)
        return receipt

    def _reconcile_finished_reopen_helper_members(self) -> List[int]:
        frozen_residuals: List[int] = []
        for pid, (expected_executable, expected_start) in list(
            self._reopen_helper_members.items()
        ):
            if not self.inspector.pid_alive(pid):
                self._reopen_helper_members.pop(pid, None)
                continue
            current_start = self.inspector.process_start_marker(pid)
            if current_start is not None and current_start != expected_start:
                self._reopen_helper_members.pop(pid, None)
                continue
            current_executable = self.inspector.executable_for_pid(pid)
            if (
                current_start == expected_start
                and current_executable is not None
                and current_executable != expected_executable
            ):
                self._reopen_helper_members[pid] = (current_executable, current_start)
            frozen_residuals.append(pid)
        return frozen_residuals

    @staticmethod
    def _stop_reopen_helper_process(helper: subprocess.Popen) -> None:
        if helper.poll() is not None:
            return
        helper.terminate()
        try:
            helper.wait(timeout=3.0)
        except subprocess.TimeoutExpired:
            helper.kill()
            helper.wait(timeout=3.0)

    def _freeze_live_reopen_helper_group(
        self, helper: subprocess.Popen, pgid: int
    ) -> bool:
        if helper.poll() is not None:
            self._reopen_helper_members.pop(helper.pid, None)
            return False
        helper_executable = self.inspector.executable_for_pid(helper.pid)
        helper_start = self.inspector.process_start_marker(helper.pid)
        helper_alive = self.inspector.pid_alive(helper.pid)
        helper_group = self.inspector.process_group(helper.pid)
        if (
            not helper_alive
            or helper_executable is None
            or not helper_start
            or helper_group != pgid
        ):
            if helper.poll() is not None or not helper_alive:
                self._reopen_helper_members.pop(helper.pid, None)
                return False
            raise ControllerError("guarded reopen helper identity or process group drift")
        group_pids = self.inspector.pids_in_process_group(pgid)
        for pid, existing in list(self._reopen_helper_members.items()):
            if pid in group_pids:
                continue
            if not self.inspector.pid_alive(pid):
                self._reopen_helper_members.pop(pid, None)
                continue
            current_identity = (
                self.inspector.executable_for_pid(pid),
                self.inspector.process_start_marker(pid),
            )
            if current_identity[1] == existing[1]:
                if current_identity[0] is not None:
                    self._reopen_helper_members[pid] = current_identity
                raise ControllerError("guarded reopen frozen member left process group")
            self._reopen_helper_members.pop(pid, None)

        owned: Dict[int, Tuple[Path, Optional[str]]] = {}
        for pid in group_pids:
            executable = self.inspector.executable_for_pid(pid)
            start_marker = self.inspector.process_start_marker(pid)
            if executable is None or not start_marker:
                if (
                    not self.inspector.pid_alive(pid)
                    or self.inspector.process_group(pid) != pgid
                ):
                    # A short-lived launch descendant may disappear between the
                    # process-group snapshot and identity inspection. Never
                    # adopt that PID, but do not turn its confirmed departure
                    # into an ownership failure either.
                    self._reopen_helper_members.pop(pid, None)
                    continue
                raise ControllerError("guarded reopen group member identity unavailable")
            owned[pid] = (executable, start_marker)
        if helper.pid not in owned:
            if helper.poll() is not None or not self.inspector.pid_alive(helper.pid):
                self._reopen_helper_members.pop(helper.pid, None)
                return False
            raise ControllerError("guarded reopen helper missing from its process group")
        if (
            helper.poll() is not None
            or not self.inspector.pid_alive(helper.pid)
            or self.inspector.executable_for_pid(helper.pid) != helper_executable
            or self.inspector.process_start_marker(helper.pid) != helper_start
            or self.inspector.process_group(helper.pid) != pgid
        ):
            if helper.poll() is not None or not self.inspector.pid_alive(helper.pid):
                self._reopen_helper_members.pop(helper.pid, None)
            return False
        additions: Dict[int, Tuple[Path, Optional[str]]] = {}
        for pid, identity in owned.items():
            current_executable = self.inspector.executable_for_pid(pid)
            current_start = self.inspector.process_start_marker(pid)
            current_group = self.inspector.process_group(pid)
            if not self.inspector.pid_alive(pid):
                # The member departed after the first identity read. It was
                # never newly frozen, and any stale frozen ownership must be
                # revoked before this numeric PID can be recycled.
                self._reopen_helper_members.pop(pid, None)
                continue
            if current_group != pgid:
                existing = self._reopen_helper_members.get(pid)
                if existing is not None and existing[1] == current_start:
                    if current_executable is not None:
                        self._reopen_helper_members[pid] = (
                            current_executable,
                            current_start,
                        )
                    raise ControllerError("guarded reopen frozen member left process group")
                self._reopen_helper_members.pop(pid, None)
                continue
            if current_executable != identity[0] or current_start != identity[1]:
                raise ControllerError("guarded reopen group member identity drift")
            existing = self._reopen_helper_members.get(pid)
            if existing is None:
                additions[pid] = identity
            elif existing != identity and pid != helper.pid:
                raise ControllerError("guarded reopen frozen member identity drift")
            elif existing != identity and existing[1] != identity[1]:
                raise ControllerError("guarded reopen helper start identity drift")
            # The exact controller-owned Popen leader may exec through
            # sandbox-exec/env/Desktop without changing PID/start identity.
            # Preserve its first frozen identity; Popen remains the only
            # authority used to terminate that leader after an exec transition.
        self._reopen_helper_members.update(additions)
        return True

    def _cleanup_reopen_helper_group(self) -> List[int]:
        owned = dict(self._reopen_helper_members)
        if not owned:
            return []
        for pid, (expected_executable, expected_start) in owned.items():
            if (
                self.inspector.pid_alive(pid)
                and self.inspector.executable_for_pid(pid) == expected_executable
                and self.inspector.process_start_marker(pid) == expected_start
            ):
                try:
                    os.kill(pid, 15)
                except ProcessLookupError:
                    pass
        deadline = time.monotonic() + 3.0
        while time.monotonic() < deadline:
            remaining = [
                pid
                for pid, (expected_executable, expected_start) in owned.items()
                if self.inspector.pid_alive(pid)
                and self.inspector.executable_for_pid(pid) == expected_executable
                and self.inspector.process_start_marker(pid) == expected_start
            ]
            if not remaining:
                for pid in owned:
                    self._reopen_helper_members.pop(pid, None)
                return []
            time.sleep(0.05)
        for pid, (expected_executable, expected_start) in owned.items():
            if (
                self.inspector.pid_alive(pid)
                and self.inspector.executable_for_pid(pid) == expected_executable
                and self.inspector.process_start_marker(pid) == expected_start
            ):
                try:
                    os.kill(pid, 9)
                except ProcessLookupError:
                    pass
        remaining = [
            pid
            for pid, (expected_executable, expected_start) in owned.items()
            if self.inspector.pid_alive(pid)
            and self.inspector.executable_for_pid(pid) == expected_executable
            and self.inspector.process_start_marker(pid) == expected_start
        ]
        for pid in owned:
            if pid not in remaining:
                self._reopen_helper_members.pop(pid, None)
        return remaining

    def inspect_children(self) -> Dict[str, Any]:
        if self._app_pid is None:
            raise ControllerError("app PID not observed")
        children = []
        python_count = 0
        for record in self.inspector.children(self._app_pid):
            executable = self.inspector.executable_for_pid(record.pid)
            name = executable.name if executable else Path(record.comm).name
            is_python = "python" in name.lower()
            python_count += int(is_python)
            children.append(
                {
                    "pid": record.pid,
                    "comm": Path(record.comm).name,
                    "executable": str(executable) if executable else None,
                    "python_like": is_python,
                }
            )
        return {"app_pid": self._app_pid, "children": children, "python_direct_children": python_count}

    def inspect_sidecar(self) -> Dict[str, Any]:
        if self._app_pid is None:
            raise ControllerError("app PID not observed")
        matches = []
        for record in self.inspector.children(self._app_pid):
            executable = self.inspector.executable_for_pid(record.pid)
            if executable == self.gateway_bin and self.inspector.listener_owned(record.pid, self.proxy_port):
                matches.append(record.pid)
        if len(matches) != 1:
            raise ControllerError("exact installed sidecar/listener identity is not unique")
        return {
            "pid": matches[0],
            "executable": str(self.gateway_bin),
            "port": self.proxy_port,
            "identity_verified": True,
        }

    @staticmethod
    def _safe_json_shape(raw: bytes) -> Dict[str, Any]:
        try:
            value = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError):
            return {"json": False, "top_level": None, "keys": []}
        if isinstance(value, dict):
            return {"json": True, "top_level": "object", "keys": sorted(value)[:32]}
        if isinstance(value, list):
            return {"json": True, "top_level": "array", "keys": []}
        return {"json": True, "top_level": type(value).__name__, "keys": []}

    def _http_request(
        self,
        method: str,
        path: str,
        *,
        body: Optional[bytes] = None,
        headers: Optional[Mapping[str, str]] = None,
        timeout: float = 4.0,
    ) -> Tuple[int, Dict[str, str], bytes]:
        conn = http.client.HTTPConnection("127.0.0.1", self.proxy_port, timeout=timeout)
        conn.request(method, path, body=body, headers=dict(headers or {}))
        response = conn.getresponse()
        raw = response.read(1_048_577)
        status = response.status
        response_headers = {name.lower(): value for name, value in response.getheaders()}
        conn.close()
        if len(raw) > 1_048_576:
            raise ControllerError("response exceeds evidence limit")
        return status, response_headers, raw

    def inspect_health(self) -> Dict[str, Any]:
        missing, _, _ = self._http_request("GET", "/health")
        wrong, _, _ = self._http_request("GET", "/wrong-installed-secret/health")
        correct, _, raw = self._http_request("GET", f"/{FIXED_PATH_SECRET}/health")
        if FIXED_PATH_SECRET.encode() in raw or FAKE_API_KEY.encode() in raw:
            raise ControllerError("health response leaked sensitive test material")
        try:
            body = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ControllerError("health response is not JSON") from error
        identity = {
            "gateway": body.get("gateway"),
            "provider": body.get("provider"),
            "shim": body.get("shim"),
            "launch_id": body.get("launch_id"),
        }
        ok = (
            missing == 403
            and wrong == 403
            and correct == 200
            and identity["gateway"] == "rust"
            and identity["provider"] == self.case.adapter
            and identity["shim"] == self.case.shim
            and isinstance(identity["launch_id"], str)
            and bool(identity["launch_id"])
        )
        return {
            "ok": ok,
            "missing_secret_status": missing,
            "wrong_secret_status": wrong,
            "correct_secret_status": correct,
            **identity,
            "sensitive_material_absent": True,
        }

    def _formal_payload(self) -> bytes:
        payload: Dict[str, Any] = {
            "model": "claude-opus-4-8",
            "max_tokens": 1_000_000,
            "thinking": {"type": "auto"},
            "messages": [{"role": "user", "content": "installed local mock ping"}],
        }
        if "tools" in self.case.formal_variant:
            payload["tools"] = [
                {
                    "name": "lookup",
                    "description": "local mock tool",
                    "input_schema": {
                        "type": "object",
                        "properties": {"query": {"type": "string"}},
                        "required": ["query"],
                    },
                }
            ]
            if self.case.case_id in {"relay-force", "kimi"}:
                payload["tools"].append(
                    {
                        "name": "empty",
                        "description": "local mock empty-schema tool",
                        "input_schema": {},
                    }
                )
            payload["tool_choice"] = {"type": "tool", "name": "lookup"}
        if self.case.case_id in {"qwen-tools", "responses"}:
            payload["messages"] = [
                {"role": "user", "content": "installed local mock ping"},
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_1",
                            "name": "lookup",
                            "input": {"query": "local mock"},
                        }
                    ],
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_1",
                            "content": "local mock result",
                        }
                    ],
                },
            ]
        if self.case.formal_variant.startswith("stream"):
            payload["stream"] = True
        return json.dumps(payload, separators=(",", ":")).encode()

    def send_formal(self) -> Dict[str, Any]:
        payload = self._formal_payload()
        status, headers, raw = self._http_request(
            "POST",
            f"/{FIXED_PATH_SECRET}/v1/messages",
            body=payload,
            headers={"Content-Type": "application/json"},
            timeout=10.0,
        )
        if FIXED_PATH_SECRET.encode() in raw or FAKE_API_KEY.encode() in raw:
            raise ControllerError("formal response leaked sensitive test material")
        content_type = headers.get("content-type", "").split(";", 1)[0]
        result = {
            "status": status,
            "content_type": content_type,
            "response_bytes": len(raw),
            "response_shape": self._safe_json_shape(raw),
            "variant": self.case.formal_variant,
            "sensitive_material_absent": True,
        }
        self._write_evidence_json(f"formal-{self.case.case_id}.json", result)
        return result

    def checkpoint_config(self, label: str) -> Dict[str, Any]:
        label = _require_safe_label(label)
        raw = self.config_path.read_bytes()
        value = json.loads(raw)
        self._config_checkpoint = copy.deepcopy(value)
        self._config_checkpoint_label = label
        return {"label": label, "sha256": self._record_config_fingerprint(label)}

    def inspect_config(self, allowed_paths: Iterable[str] = ()) -> Dict[str, Any]:
        if self._config_checkpoint is None:
            raise ControllerError("config checkpoint is missing")
        raw = self.config_path.read_bytes()
        current = json.loads(raw)
        changed = sorted(_changed_json_paths(self._config_checkpoint, current))
        allowed = sorted(set(allowed_paths))
        unexpected = [path for path in changed if path not in allowed]
        return {
            "checkpoint": self._config_checkpoint_label,
            "sha256": _sha256(raw),
            "changed_paths": changed,
            "allowed_paths": allowed,
            "unexpected_paths": unexpected,
            "ok": not unexpected,
        }

    def scan_logs(self) -> Dict[str, Any]:
        paths: List[Path] = []
        logs = self.csswitch_dir / "logs"
        if logs.is_dir():
            paths.extend(path for path in logs.rglob("*") if path.is_file() and not path.is_symlink())
        paths.extend(
            path
            for path in self.evidence.rglob("*")
            if path.is_file() and not path.is_symlink()
        )
        needles = (FIXED_PATH_SECRET.encode(), FAKE_API_KEY.encode())
        matches = 0
        for path in paths:
            if path == self.config_path:
                continue
            data = path.read_bytes()
            matches += sum(data.count(needle) for needle in needles)
        result = {
            "sensitive_log_match_count": matches,
            "python_tripwire_invoked": self._python_tripwire.exists(),
            "files_scanned": len(paths),
            "ok": matches == 0 and not self._python_tripwire.exists(),
        }
        self._last_log_scan = copy.deepcopy(result)
        return result

    def record_runtime(self, label: str) -> Dict[str, Any]:
        label = _require_safe_label(label)
        health = self.inspect_health()
        if not health["ok"]:
            raise ControllerError("runtime health identity mismatch")
        sidecar = self.inspect_sidecar()
        record = {
            "label": label,
            "pid": sidecar["pid"],
            "executable": sidecar["executable"],
            "port": sidecar["port"],
            "launch_id": health["launch_id"],
            "provider": health["provider"],
            "shim": health["shim"],
            "gateway": health["gateway"],
        }
        self._runtime_records[label] = record
        self._write_evidence_json(f"runtime-{label}.json", record)
        return record

    def compare_runtime(self, before: str, after: str, relation: str) -> Dict[str, Any]:
        if relation not in {"reuse", "restart"}:
            raise ControllerError("relation must be reuse or restart")
        first = self._runtime_records.get(before)
        second = self._runtime_records.get(after)
        if first is None or second is None:
            raise ControllerError("runtime record is missing")
        same_pid = first["pid"] == second["pid"]
        same_launch = first["launch_id"] == second["launch_id"]
        ok = (same_pid and same_launch) if relation == "reuse" else (not same_pid and not same_launch)
        return {
            "relation": relation,
            "before": before,
            "after": after,
            "same_pid": same_pid,
            "same_launch_id": same_launch,
            "ok": ok,
        }

    def inspect_fake_science(self) -> Dict[str, Any]:
        state = self.csswitch_dir / "sandbox/home/.claude-science/csswitch-installed-fake-science"
        if self._owned_path_has_symlink(state):
            raise ControllerError("fake Science state traverses a symlink")
        try:
            pid = int((state / "pid").read_text().strip())
            port = int((state / "port").read_text().strip())
            recorded_exe = Path((state / "executable").read_text().strip())
        except (FileNotFoundError, ValueError):
            raise ControllerError("fake Science owned state is incomplete")
        actual_exe = self.inspector.executable_for_pid(pid)
        ok = (
            pid > 1
            and port == self.sandbox_port
            and port not in FORBIDDEN_PORTS
            and actual_exe == recorded_exe
            and self.inspector.listener_owned(pid, port)
        )
        value = {
            "pid": pid,
            "port": port,
            "executable": str(recorded_exe),
            "identity_verified": ok,
        }
        if ok:
            self._write_evidence_json("fake-science-identity.json", value)
        return value

    def stop_fake_science(self) -> Dict[str, Any]:
        env = {
            "HOME": str(self.home),
            "PATH": f"{self.bin_dir}:/usr/bin:/bin:/usr/sbin:/sbin",
            "CSSWITCH_EXPECTED_SANDBOX_PORT": str(self.sandbox_port),
        }
        data_dir = self.csswitch_dir / "sandbox/home/.claude-science"
        if self._owned_path_has_symlink(data_dir):
            raise ControllerError("fake Science data directory traverses a symlink")
        result = subprocess.run(
            [str(self.fake_science), "stop", "--data-dir", str(data_dir)],
            check=False,
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        return {"ok": result.returncode == 0, "exit_code": result.returncode}

    def stop_mock(self) -> Dict[str, Any]:
        result = self._mock.stop()
        self._mock_stopped = True
        safe = copy.deepcopy(result)
        self._mock_result = copy.deepcopy(safe)
        self._write_evidence_json("mock-result.json", safe)
        return safe

    def export_sanitized_summary(self, destination: Path) -> Dict[str, Any]:
        """Export counts/identity outcomes only; never copy raw config or hits."""

        destination = Path(destination)
        if not destination.is_absolute() or destination.name in {"", ".", ".."}:
            raise ControllerError("sanitized summary destination must be an absolute file")
        _reject_symlink(destination)
        parent = destination.parent.resolve(strict=True)
        parent_info = parent.stat()
        if (
            not stat.S_ISDIR(parent_info.st_mode)
            or parent_info.st_uid != os.getuid()
            or stat.S_IMODE(parent_info.st_mode) != 0o700
        ):
            raise ControllerError("sanitized summary parent must be owned 0700")
        repo_root = Path(__file__).resolve(strict=True).parents[1]
        real_home = Path(os.path.expanduser("~")).resolve(strict=True)
        canonical_destination = parent / destination.name
        if (
            canonical_destination == repo_root
            or _is_relative_to(canonical_destination, repo_root)
            or canonical_destination == self.root
            or _is_relative_to(canonical_destination, self.root)
            or canonical_destination == real_home
            or _is_relative_to(canonical_destination, real_home)
        ):
            raise ControllerError("sanitized summary must be outside repo, real HOME, and test root")
        if self._owned_path_has_symlink(self.config_path):
            raise ControllerError("config path traverses a symlink")
        config_sha256 = _sha256(self.config_path.read_bytes())
        mock_result = self._mock_result or {}
        cleanup = self.verify_cleanup()
        summary = {
            "schema": "csswitch.installed-provider-summary.v1",
            "case": self.case.case_id,
            "config_sha256": config_sha256,
            "runtime": [
                {
                    "label": label,
                    "gateway": record.get("gateway"),
                    "provider": record.get("provider"),
                    "shim": record.get("shim"),
                }
                for label, record in sorted(self._runtime_records.items())
            ],
            "mock": {
                "scenario": mock_result.get("scenario"),
                "protocol_complete": bool(mock_result.get("protocol_complete")),
                "final_ok": bool(mock_result.get("final_ok")),
                "request_count": len(mock_result.get("requests", [])),
                "failure_count": len(mock_result.get("failures", [])),
            },
            "log_scan": copy.deepcopy(self._last_log_scan),
            "cleanup": {
                "ok": cleanup["ok"],
                "ports_closed": copy.deepcopy(cleanup["ports_closed"]),
                "alive_owned_pid_count": sum(
                    int(item["alive"]) for item in cleanup["owned_pids"]
                ),
            },
        }
        encoded = json.dumps(summary, ensure_ascii=False, sort_keys=True)
        if FIXED_PATH_SECRET in encoded or FAKE_API_KEY in encoded:
            raise AssertionError("sanitized summary invariant failed")
        _safe_json_write(canonical_destination, summary)
        return {"exported": True, "destination": str(canonical_destination)}

    @staticmethod
    def _port_closed(port: int) -> bool:
        if port in FORBIDDEN_PORTS:
            raise ControllerError("refusing cleanup probe of forbidden port")
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(0.25)
        try:
            return sock.connect_ex(("127.0.0.1", port)) != 0
        finally:
            sock.close()

    def verify_cleanup(self) -> Dict[str, Any]:
        self._proxy_reservation.release()
        self._sandbox_reservation.release()
        self._preview_reservation.release()
        ports = {
            "proxy": self.proxy_port,
            "sandbox": self.sandbox_port,
            "preview": self.preview_port,
        }
        if self._mock_started:
            ports["mock"] = self._mock.port
        port_results = {name: self._port_closed(port) for name, port in ports.items()}
        pid_results = []
        seen = set()
        for record in [
            *self._runtime_records.values(),
            ({"pid": self._app_pid, "executable": str(self.app_bin)} if self._app_pid else {}),
            (
                {
                    "pid": self._launcher_process.pid,
                    "executable": str(self.app_bin),
                }
                if self._launcher_process is not None
                else {}
            ),
        ]:
            pid = record.get("pid")
            if not isinstance(pid, int) or pid in seen:
                continue
            seen.add(pid)
            alive = self.inspector.pid_alive(pid)
            actual = self.inspector.executable_for_pid(pid) if alive else None
            expected = record.get("executable")
            pid_results.append(
                {
                    "pid": pid,
                    "alive": alive,
                    "identity_match": alive and str(actual) == expected,
                }
            )
        descendant_roots = {
            pid
            for pid in (
                self._app_pid,
                self._launcher_process.pid if self._launcher_process is not None else None,
            )
            if isinstance(pid, int)
        }
        for parent_pid in descendant_roots:
            for record in self.inspector.descendants(parent_pid):
                if record.pid in seen:
                    continue
                seen.add(record.pid)
                alive = self.inspector.pid_alive(record.pid)
                actual = self.inspector.executable_for_pid(record.pid) if alive else None
                pid_results.append(
                    {
                        "pid": record.pid,
                        "alive": alive,
                        "identity_match": alive and actual is not None,
                        "source": "desktop-or-launcher-descendant",
                    }
                )
        for pgid, source in ((self._launcher_pgid, "guarded-process-group"),):
            if pgid is None:
                continue
            for pid in self.inspector.pids_in_process_group(pgid):
                if pid in seen:
                    continue
                seen.add(pid)
                alive = self.inspector.pid_alive(pid)
                actual = self.inspector.executable_for_pid(pid) if alive else None
                pid_results.append(
                    {
                        "pid": pid,
                        "alive": alive,
                        "identity_match": alive and actual is not None,
                        "source": source,
                    }
                )
        for pid, (expected_executable, expected_start) in self._reopen_helper_members.items():
            if pid in seen:
                continue
            seen.add(pid)
            alive = self.inspector.pid_alive(pid)
            actual = self.inspector.executable_for_pid(pid) if alive else None
            current_start = self.inspector.process_start_marker(pid) if alive else None
            pid_results.append(
                {
                    "pid": pid,
                    "alive": alive,
                    "identity_match": (
                        alive
                        and actual == expected_executable
                        and current_start == expected_start
                    ),
                    "source": "guarded-reopen-frozen-member",
                }
            )
        root_pids = [
            pid for pid in self.inspector.pids_using_path(self.root) if pid != os.getpid()
        ]
        for pid in root_pids:
            if pid in seen:
                continue
            seen.add(pid)
            alive = self.inspector.pid_alive(pid)
            actual = self.inspector.executable_for_pid(pid) if alive else None
            pid_results.append(
                {
                    "pid": pid,
                    "alive": alive,
                    "identity_match": alive and actual is not None,
                    "source": "runtime-root-open-fd",
                }
            )
        if isinstance(self._mock, SubprocessScenarioControl) and self._mock.process_pid:
            mock_pid = self._mock.process_pid
            alive = self._mock.process_alive
            actual = self.inspector.executable_for_pid(mock_pid) if alive else None
            pid_results.append(
                {
                    "pid": mock_pid,
                    "alive": alive,
                    "identity_match": (
                        alive
                        and actual is not None
                        and actual.resolve(strict=False) == self._mock.expected_executable
                    ),
                }
            )

        fake_state_valid = True
        fake_state = self.csswitch_dir / "sandbox/home/.claude-science/csswitch-installed-fake-science"
        if self._owned_path_has_symlink(fake_state):
            fake_state_valid = False
        elif fake_state.exists():
            if not fake_state.is_dir():
                fake_state_valid = False
            else:
                try:
                    entries = list(fake_state.iterdir())
                    if entries:
                        fake_pid = int(
                            (fake_state / "pid").read_text(encoding="utf-8").strip()
                        )
                        fake_executable = (
                            fake_state / "executable"
                        ).read_text(encoding="utf-8").strip()
                        if fake_pid <= 1 or not fake_executable:
                            raise ValueError
                        alive = self.inspector.pid_alive(fake_pid)
                        actual = self.inspector.executable_for_pid(fake_pid) if alive else None
                        pid_results.append(
                            {
                                "pid": fake_pid,
                                "alive": alive,
                                "identity_match": alive and str(actual) == fake_executable,
                            }
                        )
                except (FileNotFoundError, OSError, ValueError):
                    fake_state_valid = False

        ok = (
            all(port_results.values())
            and fake_state_valid
            and not any(item["alive"] for item in pid_results)
        )
        return {
            "ports_closed": port_results,
            "owned_pids": pid_results,
            "fake_science_state_valid": fake_state_valid,
            "runtime_root_open_pids": root_pids,
            "ok": ok,
        }

    def finalize_evidence(self) -> Dict[str, Any]:
        """Publish cleanup, observations, manifest, then a terminal hash closure."""

        if self._evidence_finalized:
            return {
                "finalized": True,
                "hashes_path": str(self.evidence / "hashes.sha256"),
                "hashes_sha256": _sha256(_read_at(self._evidence_fd, "hashes.sha256")),
            }
        if self._mock_started and not self._mock_stopped:
            raise ControllerError("provider mock must stop before evidence finalization")
        cleanup = self.verify_cleanup()
        if not cleanup["ok"]:
            raise ControllerError("cleanup must pass before evidence finalization")
        after = {
            "schema": "csswitch.isolated-live-inventory.v1",
            "stage": "after",
            "owned_processes": copy.deepcopy(cleanup["owned_pids"]),
            "ports_closed": copy.deepcopy(cleanup["ports_closed"]),
            "runtime_root_open_pids": copy.deepcopy(
                cleanup["runtime_root_open_pids"]
            ),
        }
        cleanup_receipt = {
            "schema": "csswitch.isolated-live-cleanup-receipt.v1",
            **copy.deepcopy(cleanup),
            "driver_verified": True,
        }
        decision = (
            "INCONCLUSIVE(reason=controller-does-not-aggregate-runtime-observations)"
            if self._launch_consumed
            else "INCONCLUSIVE(reason=pre-run-only)"
        )
        observations = {
            "schema": "csswitch.isolated-live-observations.v1",
            "case": self.case.case_id,
            "decision": decision,
            "pre_run_manifest_validated": self._pre_run_manifest is not None,
            "network_isolation_receipt": (
                str(self.evidence / "network-isolation-receipt.json")
                if self._pre_run_manifest is not None
                else None
            ),
            "cleanup_ok": True,
        }
        events = (
            json.dumps(
                {
                    "event": "evidence-finalized",
                    "case": self.case.case_id,
                    "decision": decision,
                    "cleanup_ok": True,
                },
                ensure_ascii=False,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode("utf-8")
        self._write_evidence_json("inventory-after.json", after, once=True)
        self._write_evidence_json("cleanup.json", cleanup_receipt, once=True)
        self._write_evidence_json("observations.json", observations, once=True)
        _safe_write_at(self._evidence_fd, "events.ndjson", events, 0o600)
        manifest = {
            "schema": "csswitch.isolated-live-evidence-manifest.v1",
            "case": self.case.case_id,
            "decision": decision,
            "pre_run_manifest": (
                {
                    "path": str(self.evidence / "pre-run-manifest.json"),
                    "sha256": self._pre_run_manifest_sha256,
                }
                if self._pre_run_manifest is not None
                else None
            ),
            "fixture_receipt": str(self.evidence / "fixture-receipt.json"),
            "provider_receipt": str(self.evidence / "provider-launch-receipt.json"),
            "inventory_before": str(self.evidence / "inventory-before.json"),
            "inventory_after": str(self.evidence / "inventory-after.json"),
            "cleanup": str(self.evidence / "cleanup.json"),
            "observations": str(self.evidence / "observations.json"),
            "events": str(self.evidence / "events.ndjson"),
        }
        self._write_evidence_json("manifest.json", manifest, once=True)
        self._assert_evidence_binding()
        lines = _hash_tree_at(self._evidence_fd)
        hash_bytes = "".join(
            f"{digest}  ./{relative}\n" for relative, digest in sorted(lines)
        ).encode("utf-8")
        _safe_write_at(self._evidence_fd, "hashes.sha256", hash_bytes, 0o600)
        if _hash_tree_at(self._evidence_fd) != lines:
            raise ControllerError("evidence hash closure changed during finalization")
        self._assert_evidence_binding()
        self._evidence_finalized = True
        return {
            "finalized": True,
            "decision": decision,
            "file_count": len(lines),
            "hashes_path": str(self.evidence / "hashes.sha256"),
            "hashes_sha256": _sha256(hash_bytes),
        }

    def destroy_workspace(self) -> Dict[str, bool]:
        """Remove only this session's private root after owned cleanup is proven."""

        if self._workspace_destroyed:
            return {"root_removed": True}
        if not self._root_created_by_session:
            raise ControllerError("refusing to remove a pre-existing workspace root")
        _reject_symlink(self.root)
        info = self.root.stat()
        if (
            not stat.S_ISDIR(info.st_mode)
            or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o700
            or self.root.resolve(strict=True) != self.root
        ):
            raise ControllerError("workspace root identity is not private and canonical")
        real_home = Path(os.path.expanduser("~")).resolve(strict=True)
        if self.root == real_home or _is_relative_to(self.root, real_home):
            raise ControllerError("refusing workspace removal inside real HOME")
        cleanup = self.verify_cleanup()
        if not cleanup["ok"]:
            raise ControllerError("refusing workspace removal before owned cleanup passes")
        shutil.rmtree(self.root)
        if self.root.exists() or self.root.is_symlink():
            raise ControllerError("workspace removal did not complete")
        self._workspace_destroyed = True
        return {"root_removed": True}

    def close(self) -> None:
        if self._closed:
            return
        self._closed = True
        owned_descendants: Dict[int, Tuple[Path, Optional[str]]] = dict(
            self._reopen_helper_members
        )
        descendant_roots = {
            pid
            for pid in (
                self._app_pid,
                self._launcher_process.pid if self._launcher_process is not None else None,
            )
            if isinstance(pid, int)
        }
        for parent_pid in descendant_roots:
            for record in self.inspector.descendants(parent_pid):
                executable = self.inspector.executable_for_pid(record.pid)
                if executable is not None:
                    owned_descendants[record.pid] = (
                        executable,
                        self.inspector.process_start_marker(record.pid),
                    )
        for pgid in (self._launcher_pgid,):
            if pgid is None:
                continue
            for pid in self.inspector.pids_in_process_group(pgid):
                executable = self.inspector.executable_for_pid(pid)
                if executable is not None:
                    owned_descendants[pid] = (
                        executable,
                        self.inspector.process_start_marker(pid),
                    )
        if not self._workspace_destroyed and self.root.exists():
            for pid in self.inspector.pids_using_path(self.root):
                if pid == os.getpid():
                    continue
                executable = self.inspector.executable_for_pid(pid)
                if executable is not None:
                    owned_descendants[pid] = (
                        executable,
                        self.inspector.process_start_marker(pid),
                    )
        if self._launcher_process is not None and self._launcher_process.poll() is None:
            self._launcher_process.terminate()
            try:
                self._launcher_process.wait(timeout=3.0)
            except subprocess.TimeoutExpired:
                self._launcher_process.kill()
                self._launcher_process.wait(timeout=3.0)
        if self._reopen_helper_process is not None:
            self._stop_reopen_helper_process(self._reopen_helper_process)
        for pgid in (self._launcher_pgid,):
            if pgid is None:
                continue
            for pid in self.inspector.pids_in_process_group(pgid):
                executable = self.inspector.executable_for_pid(pid)
                if executable is not None and pid not in owned_descendants:
                    owned_descendants[pid] = (
                        executable,
                        self.inspector.process_start_marker(pid),
                    )
        if not self._workspace_destroyed and self.root.exists():
            for pid in self.inspector.pids_using_path(self.root):
                if pid == os.getpid():
                    continue
                executable = self.inspector.executable_for_pid(pid)
                if executable is not None and pid not in owned_descendants:
                    owned_descendants[pid] = (
                        executable,
                        self.inspector.process_start_marker(pid),
                    )
        for pid, (expected_executable, expected_start) in owned_descendants.items():
            if pid == (self._launcher_process.pid if self._launcher_process else None):
                continue
            actual = self.inspector.executable_for_pid(pid)
            if (
                not self.inspector.pid_alive(pid)
                or actual != expected_executable
                or self.inspector.process_start_marker(pid) != expected_start
            ):
                continue
            try:
                os.kill(pid, 15)
            except ProcessLookupError:
                pass
        descendant_deadline = time.monotonic() + 3.0
        while time.monotonic() < descendant_deadline:
            if not any(self.inspector.pid_alive(pid) for pid in owned_descendants):
                break
            time.sleep(0.05)
        for pid, (expected_executable, expected_start) in owned_descendants.items():
            actual = self.inspector.executable_for_pid(pid)
            if (
                self.inspector.pid_alive(pid)
                and actual == expected_executable
                and self.inspector.process_start_marker(pid) == expected_start
            ):
                try:
                    os.kill(pid, 9)
                except ProcessLookupError:
                    pass
        if self._launcher_process is not None:
            try:
                receipt_raw = _read_at(self._evidence_fd, "guarded-launch-receipt.json")
            except FileNotFoundError:
                receipt_raw = None
            if receipt_raw is not None:
                receipt = json.loads(receipt_raw)
                receipt["exit_code"] = self._launcher_process.poll()
                self._write_evidence_json("guarded-launch-receipt.json", receipt)
        if self._mock_started and not self._mock_stopped:
            self._mock.stop()
            self._mock_stopped = True
        self._proxy_reservation.release()
        self._sandbox_reservation.release()
        self._preview_reservation.release()

    def __enter__(self) -> "InstalledProviderSession":
        return self

    def __exit__(self, _exc_type, _exc, _traceback) -> None:
        self.close()


def _scrub_error(message: str) -> str:
    return message.replace(FIXED_PATH_SECRET, "<redacted>").replace(FAKE_API_KEY, "<redacted>")


def _dispatch(session: InstalledProviderSession, command: Mapping[str, Any]) -> Any:
    op = command.get("op")
    if op == "start_mock":
        return session.start_mock()
    if op == "plan":
        return session.safe_plan(auto_boot=bool(command.get("auto_boot", False)))
    if op == "pre_run_check":
        return session.validate_pre_run()
    if op == "launch_guarded":
        return session.launch_guarded()
    if op == "observe_app":
        return session.observe_app(float(command.get("timeout_seconds", 8.0)))
    if op == "reopen_guarded":
        return session.reopen_guarded(float(command.get("timeout_seconds", 8.0)))
    if op == "enter_phase":
        return session.enter_phase(str(command.get("phase", "")))
    if op == "finish_phase":
        return session.finish_phase(str(command.get("phase", "")))
    if op == "health":
        return session.inspect_health()
    if op == "formal":
        return session.send_formal()
    if op == "children":
        return session.inspect_children()
    if op == "sidecar":
        return session.inspect_sidecar()
    if op == "record_runtime":
        return session.record_runtime(str(command.get("label", "")))
    if op == "compare_runtime":
        return session.compare_runtime(
            str(command.get("before", "")),
            str(command.get("after", "")),
            str(command.get("relation", "")),
        )
    if op == "config_checkpoint":
        return session.checkpoint_config(str(command.get("label", "")))
    if op == "config_check":
        allowed = command.get("allowed_paths", [])
        if not isinstance(allowed, list) or not all(isinstance(item, str) for item in allowed):
            raise ControllerError("allowed_paths must be a string array")
        return session.inspect_config(allowed)
    if op == "log_scan":
        return session.scan_logs()
    if op == "fake_science":
        return session.inspect_fake_science()
    if op == "stop_fake_science":
        return session.stop_fake_science()
    if op == "mock_status":
        return session._mock.status()
    if op == "mock_wait":
        return {"complete": session._mock.wait(float(command.get("timeout_seconds", 0.0)))}
    if op == "stop_mock":
        return session.stop_mock()
    if op == "cleanup_check":
        return session.verify_cleanup()
    if op == "finalize_evidence":
        return session.finalize_evidence()
    if op == "export_summary":
        destination = command.get("destination")
        if not isinstance(destination, str):
            raise ControllerError("summary destination must be a string")
        return session.export_sanitized_summary(Path(destination))
    if op == "destroy_workspace":
        return session.destroy_workspace()
    if op == "close":
        session.close()
        return {"closed": True}
    raise ControllerError("unknown controller operation")


def run_json_lines(session: InstalledProviderSession) -> int:
    hello = {
        "ok": True,
        "event": "controller_ready",
        "schema": CONTROLLER_SCHEMA,
        "case": session.case.case_id,
        "test_root": str(session.root),
        "controller_launches_app": True,
    }
    print(json.dumps(hello, sort_keys=True), flush=True)
    for raw_line in sys.stdin:
        if len(raw_line.encode("utf-8", "replace")) > MAX_CONTROL_LINE:
            response = {"ok": False, "error": "control line too large"}
            print(json.dumps(response, sort_keys=True), flush=True)
            continue
        try:
            command = json.loads(raw_line)
            if not isinstance(command, dict):
                raise ControllerError("controller command must be an object")
            result = _dispatch(session, command)
            response = {"ok": True, "op": command.get("op"), "result": result}
        except Exception as error:  # JSON control must stay alive for safe cleanup.
            response = {
                "ok": False,
                "error_type": type(error).__name__,
                "error": _scrub_error(str(error)),
            }
        encoded = json.dumps(response, ensure_ascii=False, sort_keys=True)
        if FIXED_PATH_SECRET in encoded or FAKE_API_KEY in encoded:
            encoded = json.dumps(
                {"ok": False, "error": "controller redaction invariant failed"}, sort_keys=True
            )
        print(encoded, flush=True)
        if command.get("op") == "close" and response.get("ok"):
            return 0
    session.close()
    return 0


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", required=True, choices=sorted(CASE_DEFINITIONS))
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--jsonl", action="store_true")
    args = parser.parse_args(argv)
    if not args.dry_run and not args.jsonl:
        parser.error("choose --dry-run or --jsonl")
    session = InstalledProviderSession(args.case)
    destroy_after_dry_run = False
    try:
        if args.dry_run:
            result = session.prepare_dry_run()
            encoded = json.dumps(result, ensure_ascii=False, sort_keys=True)
            if FIXED_PATH_SECRET in encoded or FAKE_API_KEY in encoded:
                raise AssertionError("dry-run output leaked sensitive material")
            print(encoded)
            destroy_after_dry_run = True
            return 0
        return run_json_lines(session)
    finally:
        session.close()
        if destroy_after_dry_run:
            session.destroy_workspace()


if __name__ == "__main__":
    raise SystemExit(main())
