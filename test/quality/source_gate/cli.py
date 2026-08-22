#!/usr/bin/python3
"""The sole fixed public entry for the trusted source gate."""
from __future__ import annotations

import json
import os
import stat
import sys
from collections.abc import Callable, Sequence
from pathlib import Path
from typing import Any


_REPO_ROOT = Path(__file__).resolve().parents[3]
_USAGE = (
    "run",
    "--output-root",
)
_SAFE_ERROR_CODES = {
    "unsafe output root": "output-root-unsafe",
    "output root must be empty euid-owned 0700 directory": (
        "output-root-not-empty-owned-0700"
    ),
    "output root binding lost": "output-root-binding-lost",
    "source temp socket capacity": "output-root-path-too-long",
}


class SourceCliPreflightError(RuntimeError):
    pass


def _emit_error(code: str, runner_exit: int) -> None:
    try:
        sys.stderr.write(
            _canonical({
                "error": "source-gate",
                "reason": code,
                "runner_exit": runner_exit,
            }).decode("utf-8")
        )
        sys.stderr.flush()
    except BaseException:
        # Diagnostics must never alter the fail-closed exit path.
        pass


def _safe_error_code(exc: BaseException) -> str:
    return _SAFE_ERROR_CODES.get(str(exc), "internal-failure")


def _canonical(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        + b"\n"
    )


def _parse(argv: Sequence[str]) -> str:
    if (
        not isinstance(argv, Sequence)
        or isinstance(argv, (str, bytes))
        or len(argv) != 3
        or tuple(argv[:2]) != _USAGE
        or not isinstance(argv[2], str)
        or not argv[2]
    ):
        raise ValueError("usage")
    return argv[2]


def _validate_output_root(value: str) -> tuple[str, int]:
    if (
        not isinstance(value, str)
        or not value.startswith("/")
        or value == "/"
        or value.endswith("/")
        or "//" in value
        or os.path.normpath(value) != value
        or os.path.realpath(value) != value
        or value == str(_REPO_ROOT)
        or value.startswith(str(_REPO_ROOT) + os.sep)
    ):
        raise SourceCliPreflightError("unsafe output root")
    fd: int | None = None
    try:
        fd = os.open(
            value,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        held = os.fstat(fd)
        named = os.stat(value, follow_symlinks=False)
        if (
            not stat.S_ISDIR(held.st_mode)
            or held.st_uid != os.geteuid()
            or stat.S_IMODE(held.st_mode) != 0o700
            or held.st_nlink != 2
            or (
                held.st_dev, held.st_ino, held.st_mode, held.st_uid,
                held.st_nlink, held.st_mtime_ns, held.st_ctime_ns,
            )
            != (
                named.st_dev, named.st_ino, named.st_mode, named.st_uid,
                named.st_nlink, named.st_mtime_ns, named.st_ctime_ns,
            )
            or os.listdir(fd)
        ):
            raise SourceCliPreflightError(
                "output root must be empty euid-owned 0700 directory",
            )
        return value, fd
    except SourceCliPreflightError:
        if fd is not None:
            os.close(fd)
        raise
    except OSError as exc:
        if fd is not None:
            os.close(fd)
        raise SourceCliPreflightError("unsafe output root") from exc


def _assert_output_root_binding(root: str, root_fd: int) -> None:
    public_fd: int | None = None
    try:
        held = os.fstat(root_fd)
        named = os.stat(root, follow_symlinks=False)
        public_fd = os.open(
            root,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC,
        )
        public = os.fstat(public_fd)
    except OSError as exc:
        raise SourceCliPreflightError("output root binding lost") from exc
    finally:
        if public_fd is not None:
            try:
                os.close(public_fd)
            except OSError:
                pass
    identity = (held.st_dev, held.st_ino)
    if (
        not all(stat.S_ISDIR(item.st_mode) for item in (held, named, public))
        or any(item.st_uid != os.geteuid() for item in (held, named, public))
        or any(
            stat.S_IMODE(item.st_mode) != 0o700
            for item in (held, named, public)
        )
        or identity != (named.st_dev, named.st_ino)
        or identity != (public.st_dev, public.st_ino)
    ):
        raise SourceCliPreflightError("output root binding lost")


def _main(
    argv: Sequence[str],
    executor: Callable[[str, int], tuple[int, Any | None]],
) -> int:
    try:
        requested = _parse(argv)
    except ValueError:
        _emit_error("usage", 64)
        return 64
    try:
        root, root_fd = _validate_output_root(requested)
    except SourceCliPreflightError as exc:
        _emit_error(_safe_error_code(exc), 2)
        return 2
    try:
        rc, line = executor(root, root_fd)
        _assert_output_root_binding(root, root_fd)
    except KeyboardInterrupt:
        _emit_error("interrupted", 130)
        return 130
    except SourceCliPreflightError as exc:
        _emit_error(_safe_error_code(exc), 12)
        return 12
    except BaseException as exc:
        _emit_error(_safe_error_code(exc), 12)
        return 12
    finally:
        try:
            os.close(root_fd)
        except OSError:
            pass
    if line is not None:
        sys.stdout.buffer.write(_canonical(line))
        sys.stdout.buffer.flush()
    return int(rc)


def main() -> int:
    if sys.flags.isolated != 1:
        _emit_error("isolated-python-required", 2)
        return 2
    try:
        _parse(sys.argv[1:])
    except ValueError:
        _emit_error("usage", 64)
        return 64
    if str(_REPO_ROOT) not in sys.path:
        sys.path.insert(0, str(_REPO_ROOT))
    try:
        from test.quality.source_gate.runtime import execute_source_gate
    except BaseException:
        _emit_error("runtime-import-failed", 2)
        return 2
    return _main(sys.argv[1:], execute_source_gate)


if __name__ == "__main__":
    raise SystemExit(main())
