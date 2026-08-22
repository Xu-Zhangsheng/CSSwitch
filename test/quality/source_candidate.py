#!/usr/bin/env python3
"""Create or validate one immutable source-candidate record.

The record is published only after a clean exact candidate has a complete
GATE-SOURCE PASS.  It contains no reviewer prose and never claims artifact,
installed, live, signing, public, or release evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import secrets
import stat
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict

from jsonschema import Draft202012Validator, ValidationError

try:
    from validate_quality_metadata import Validator
    from run_evidence.manifest_contracts import (
        ContractViolation,
        canonical_json_bytes,
        load_canonical_json,
        validate_source_candidate_record,
    )
    from run_evidence.atomic_store import RunStoreError, _rename_exclusive
except ModuleNotFoundError:
    from test.quality.validate_quality_metadata import Validator
    from test.quality.run_evidence.manifest_contracts import (
        ContractViolation,
        canonical_json_bytes,
        load_canonical_json,
        validate_source_candidate_record,
    )
    from test.quality.run_evidence.atomic_store import RunStoreError, _rename_exclusive


ROOT = Path(__file__).resolve().parents[2]
MAX_ARTIFACT = 64 * 1024 * 1024
MAX_TOTAL = 512 * 1024 * 1024
RUN_ID_RE = re.compile(r"^[0-9a-f]{32}$")
SHA40_RE = re.compile(r"^[0-9a-f]{40}$")
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
READ_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC


class SourceCandidateError(RuntimeError):
    def __init__(
        self,
        message: str,
        *,
        published_may_exist: bool = False,
    ) -> None:
        self.published_may_exist = published_may_exist
        super().__init__(message)


def _git(repo: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args], cwd=repo, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if result.returncode != 0:
        raise SourceCandidateError("git contract failed: {}".format(" ".join(args)))
    return result.stdout.rstrip("\n")


def _identity(item: os.stat_result) -> tuple[int, int, int, int, int, int, int, int]:
    return (
        item.st_dev,
        item.st_ino,
        item.st_uid,
        item.st_mode,
        item.st_nlink,
        item.st_size,
        item.st_mtime_ns,
        item.st_ctime_ns,
    )


def _safe_gate_directory(item: os.stat_result) -> bool:
    return (
        stat.S_ISDIR(item.st_mode)
        and item.st_uid == os.geteuid()
        and stat.S_IMODE(item.st_mode) == 0o700
    )


def _open_gate_root(root: Path) -> tuple[int, tuple[int, int, int, int, int, int, int, int]]:
    if not root.is_absolute() or root.resolve() != root:
        raise SourceCandidateError("unsafe evidence root")
    fd: int | None = None
    try:
        fd = os.open(str(root), DIR_FLAGS)
        opened = os.fstat(fd)
        named = os.stat(root, follow_symlinks=False)
        if (
            not _safe_gate_directory(opened)
            or _identity(opened) != _identity(named)
        ):
            raise SourceCandidateError("unsafe evidence root")
        return fd, _identity(opened)
    except SourceCandidateError:
        if fd is not None:
            try:
                os.close(fd)
            except OSError:
                pass
        raise
    except OSError as exc:
        if fd is not None:
            try:
                os.close(fd)
            except OSError:
                pass
        raise SourceCandidateError("unsafe evidence root") from exc


def _verify_root_binding(
    root: Path,
    root_fd: int,
    expected: tuple[int, int, int, int, int, int, int, int],
) -> None:
    try:
        held = os.fstat(root_fd)
        named = os.stat(root, follow_symlinks=False)
    except OSError as exc:
        raise SourceCandidateError("evidence root binding lost") from exc
    if (
        not _safe_gate_directory(held)
        or _identity(held) != expected
        or _identity(named) != expected
    ):
        raise SourceCandidateError("evidence root binding lost")


def _open_directory_chain(
    root_fd: int,
    logical: str,
) -> list[tuple[int, str, int, tuple[int, int, int, int, int, int, int, int]]]:
    parts = logical.split("/")
    if not logical or any(part in {"", ".", ".."} for part in parts):
        raise SourceCandidateError("unsafe evidence path")
    bindings: list[
        tuple[int, str, int, tuple[int, int, int, int, int, int, int, int]]
    ] = []
    current = root_fd
    try:
        for part in parts:
            named = os.stat(part, dir_fd=current, follow_symlinks=False)
            child = os.open(part, DIR_FLAGS, dir_fd=current)
            opened = os.fstat(child)
            named_after = os.stat(part, dir_fd=current, follow_symlinks=False)
            expected = _identity(opened)
            if (
                not _safe_gate_directory(opened)
                or _identity(named) != expected
                or _identity(named_after) != expected
            ):
                os.close(child)
                raise SourceCandidateError("unsafe gate run layout")
            bindings.append((current, part, child, expected))
            current = child
        return bindings
    except SourceCandidateError:
        for _, _, child, _ in reversed(bindings):
            try:
                os.close(child)
            except OSError:
                pass
        raise
    except OSError as exc:
        for _, _, child, _ in reversed(bindings):
            try:
                os.close(child)
            except OSError:
                pass
        raise SourceCandidateError("unsafe gate run layout") from exc


def _verify_directory_bindings(
    bindings: list[
        tuple[int, str, int, tuple[int, int, int, int, int, int, int, int]]
    ],
) -> None:
    try:
        for parent, name, child, expected in bindings:
            if (
                _identity(os.fstat(child)) != expected
                or _identity(
                    os.stat(name, dir_fd=parent, follow_symlinks=False),
                )
                != expected
            ):
                raise SourceCandidateError("evidence directory binding lost")
    except OSError as exc:
        raise SourceCandidateError("evidence directory binding lost") from exc


def _close_directory_bindings(
    bindings: list[
        tuple[int, str, int, tuple[int, int, int, int, int, int, int, int]]
    ],
) -> None:
    close_failed = False
    for _, _, child, _ in reversed(bindings):
        try:
            os.close(child)
        except OSError:
            close_failed = True
    if close_failed:
        raise SourceCandidateError("evidence directory close failed")


def _single_run_id_at(root_fd: int, logical: str) -> str:
    bindings = _open_directory_chain(root_fd, logical)
    error: SourceCandidateError | None = None
    result: str | None = None
    try:
        entries = os.listdir(bindings[-1][2])
        if len(entries) != 1 or not RUN_ID_RE.fullmatch(entries[0]):
            raise SourceCandidateError("gate output must contain exactly one run")
        run_bindings = _open_directory_chain(bindings[-1][2], entries[0])
        try:
            _verify_directory_bindings(run_bindings)
        finally:
            _close_directory_bindings(run_bindings)
        _verify_directory_bindings(bindings)
        result = entries[0]
    except SourceCandidateError as exc:
        error = exc
    except OSError as exc:
        error = SourceCandidateError("unsafe gate run layout")
        error.__cause__ = exc
    try:
        _close_directory_bindings(bindings)
    except SourceCandidateError as exc:
        if error is None:
            error = exc
    if error is not None:
        raise error
    if result is None:
        raise SourceCandidateError("unsafe gate run layout")
    return result


def _read_regular_at(root_fd: int, logical: str) -> bytes:
    parts = logical.split("/")
    if (
        not logical
        or logical.startswith("/")
        or any(part in {"", ".", ".."} for part in parts)
    ):
        raise SourceCandidateError("unsafe evidence path")
    bindings = _open_directory_chain(root_fd, "/".join(parts[:-1]))
    parent_fd = bindings[-1][2]
    leaf = parts[-1]
    fd: int | None = None
    error: SourceCandidateError | None = None
    raw: bytes | None = None
    try:
        before = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        fd = os.open(leaf, READ_FLAGS, dir_fd=parent_fd)
        opened = os.fstat(fd)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_uid != os.geteuid()
            or opened.st_nlink != 1
            or opened.st_size < 0
            or opened.st_size > MAX_ARTIFACT
            or _identity(before) != _identity(opened)
        ):
            raise SourceCandidateError("unsafe evidence artifact")
        chunks: list[bytes] = []
        remaining = opened.st_size
        while remaining:
            chunk = os.read(fd, min(65536, remaining))
            if not chunk:
                raise SourceCandidateError("short evidence artifact read")
            chunks.append(chunk)
            remaining -= len(chunk)
        if os.read(fd, 1):
            raise SourceCandidateError("long evidence artifact read")
        after = os.fstat(fd)
        named_after = os.stat(leaf, dir_fd=parent_fd, follow_symlinks=False)
        if (
            _identity(after) != _identity(opened)
            or _identity(named_after) != _identity(opened)
        ):
            raise SourceCandidateError("evidence artifact drift")
        _verify_directory_bindings(bindings)
        raw = b"".join(chunks)
    except SourceCandidateError as exc:
        error = exc
    except OSError as exc:
        error = SourceCandidateError("unsafe evidence artifact")
        error.__cause__ = exc
    if fd is not None:
        try:
            os.close(fd)
        except OSError as exc:
            if error is None:
                error = SourceCandidateError("evidence artifact close failed")
                error.__cause__ = exc
    try:
        _close_directory_bindings(bindings)
    except SourceCandidateError as exc:
        if error is None:
            error = exc
    if error is not None:
        raise error
    if raw is None:
        raise SourceCandidateError("unsafe evidence artifact")
    return raw


def load_gate_artifacts(
    root: Path,
    candidate_sha: str,
) -> Dict[str, bytes]:
    if not SHA40_RE.fullmatch(candidate_sha):
        raise SourceCandidateError("invalid candidate identity")
    root_fd, root_identity = _open_gate_root(root)
    artifacts: Dict[str, bytes] = {}
    error: SourceCandidateError | None = None
    try:
        evidence_run_id = _single_run_id_at(root_fd, "evidence/runs")
        state_run_id = _single_run_id_at(root_fd, "state/runs")
        if evidence_run_id != state_run_id:
            raise SourceCandidateError("gate state/evidence run mismatch")
        run_id = evidence_run_id
        evidence_prefix = "evidence/runs/{}/".format(run_id)
        state_prefix = "state/runs/{}/".format(run_id)
        required = {
            "run-manifest.json": evidence_prefix + "run-manifest.json",
            "completion-seal.json": evidence_prefix + "completion-seal.json",
            "snapshot/source-snapshot-manifest.json": (
                state_prefix + "snapshot/source-snapshot-manifest.json"
            ),
            "evidence-manifest.json": evidence_prefix + "evidence-manifest.json",
        }
        artifacts = {
            logical: _read_regular_at(root_fd, physical)
            for logical, physical in required.items()
        }
        run = load_canonical_json(artifacts["run-manifest.json"])
        seal = load_canonical_json(artifacts["completion-seal.json"])
        snapshot = load_canonical_json(
            artifacts["snapshot/source-snapshot-manifest.json"],
        )
        evidence = load_canonical_json(artifacts["evidence-manifest.json"])
        if not all(isinstance(item, dict) for item in (run, seal, snapshot, evidence)):
            raise SourceCandidateError("invalid gate artifact")
        if (
            run.get("run_id") != run_id
            or seal.get("run_id") != run_id
            or snapshot.get("run_id") != run_id
            or evidence.get("run_id") != run_id
            or run.get("head_sha") != candidate_sha
            or snapshot.get("head_sha") != candidate_sha
            or (seal.get("aggregate_decision"), seal.get("runner_exit"))
            != ("PASS", 0)
        ):
            raise SourceCandidateError("gate run does not bind exact PASS candidate")
        refs = list(evidence.get("test_results", [])) + list(
            evidence.get("source_observations", []),
        )
        for ref in refs:
            if not isinstance(ref, dict) or not isinstance(ref.get("path"), str):
                raise SourceCandidateError("invalid evidence reference")
            artifacts[ref["path"]] = _read_regular_at(
                root_fd,
                evidence_prefix + ref["path"],
            )
        if sum(len(raw) for raw in artifacts.values()) > MAX_TOTAL:
            raise SourceCandidateError("evidence set too large")
        _verify_root_binding(root, root_fd, root_identity)
    except SourceCandidateError as exc:
        error = exc
    except (ContractViolation, UnicodeError, ValueError) as exc:
        error = SourceCandidateError("invalid gate artifact")
        error.__cause__ = exc
    try:
        os.close(root_fd)
    except OSError as exc:
        if error is None:
            error = SourceCandidateError("evidence root close failed")
            error.__cause__ = exc
    if error is not None:
        raise error
    return artifacts


def build_record(repo: Path, evidence_root: Path, candidate_sha: str) -> dict[str, Any]:
    repo = repo.resolve()
    head = _git(repo, "rev-parse", "HEAD")
    if head != candidate_sha or _git(repo, "status", "--porcelain=v1"):
        raise SourceCandidateError("candidate creation requires clean exact HEAD")
    validator = Validator(repo)
    if not validator.run("impact-release"):
        raise SourceCandidateError("impact-release must PASS before source-candidate publication: {}".format("; ".join(validator.errors)))
    lineage = validator.lineage
    previous = lineage["previous_release"]
    development = lineage["development_source"]
    base = previous["peeled_sha"]
    change_set = validator.diff_change_set(base, candidate_sha)
    change_ids = sorted(validator.current_change_ids(base, candidate_sha, []))
    if not change_ids:
        raise SourceCandidateError("candidate has no current ChangeRecordV1")
    artifacts = load_gate_artifacts(evidence_root, candidate_sha)
    run = load_canonical_json(artifacts["run-manifest.json"])
    seal = load_canonical_json(artifacts["completion-seal.json"])
    snapshot = load_canonical_json(artifacts["snapshot/source-snapshot-manifest.json"])
    evidence = load_canonical_json(artifacts["evidence-manifest.json"])
    if not all(isinstance(item, dict) for item in (run, seal, snapshot, evidence)):
        raise SourceCandidateError("gate artifacts must be objects")
    ref = lambda path: {"path": "source/" + path, "sha256": hashlib.sha256(artifacts[path]).hexdigest()}
    record = {
        "schema": "source-candidate-record.v1",
        "development_line": development["line"],
        "comparison_base": previous,
        "candidate_head_sha": candidate_sha,
        "change_set_sha256": hashlib.sha256(canonical_json_bytes(change_set)).hexdigest(),
        "change_set": change_set,
        "change_ids": change_ids,
        "run_id": run.get("run_id"),
        "run_manifest": ref("run-manifest.json"),
        "completion_seal": ref("completion-seal.json"),
        "source_snapshot_manifest": ref("snapshot/source-snapshot-manifest.json"),
        "evidence_manifest": ref("evidence-manifest.json"),
        "created_at": datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z"),
    }
    schema = json.loads((repo / "quality/schema/source-candidate-record.v1.schema.json").read_text("utf-8"))
    try:
        Draft202012Validator(schema).validate(record)
        validation_artifacts = {
            "source/" + path: raw for path, raw in artifacts.items()
        }
        validate_source_candidate_record(record, validation_artifacts)
    except (ContractViolation, ValidationError, KeyError, TypeError, ValueError) as exc:
        raise SourceCandidateError("source candidate validation failed") from exc
    return record


def _safe_publication_directory(item: os.stat_result) -> bool:
    return (
        stat.S_ISDIR(item.st_mode)
        and item.st_uid == os.geteuid()
        and not stat.S_IMODE(item.st_mode) & 0o022
    )


def _verify_publication_bindings(
    repo: Path,
    repo_fd: int,
    quality_fd: int,
    directory_fd: int,
    expected: tuple[tuple[int, int], tuple[int, int], tuple[int, int]],
    *,
    published_may_exist: bool = False,
) -> None:
    try:
        held = (
            os.fstat(repo_fd),
            os.fstat(quality_fd),
            os.fstat(directory_fd),
        )
        named = (
            os.stat(repo, follow_symlinks=False),
            os.stat("quality", dir_fd=repo_fd, follow_symlinks=False),
            os.stat(
                "source-candidates",
                dir_fd=quality_fd,
                follow_symlinks=False,
            ),
        )
    except OSError as exc:
        raise SourceCandidateError(
            "source candidate directory binding lost",
            published_may_exist=published_may_exist,
        ) from exc
    for opened, current, identity in zip(held, named, expected):
        if (
            not _safe_publication_directory(opened)
            or (opened.st_dev, opened.st_ino) != identity
            or (current.st_dev, current.st_ino) != identity
        ):
            raise SourceCandidateError(
                "source candidate directory binding lost",
                published_may_exist=published_may_exist,
            )


def _readback_published_record(
    directory_fd: int,
    leaf: str,
    expected_identity: tuple[int, int],
    expected_raw: bytes,
) -> None:
    fd: int | None = None
    error: SourceCandidateError | None = None
    try:
        fd = os.open(leaf, READ_FLAGS, dir_fd=directory_fd)
        opened = os.fstat(fd)
        if (
            not stat.S_ISREG(opened.st_mode)
            or opened.st_uid != os.geteuid()
            or opened.st_nlink != 1
            or opened.st_size != len(expected_raw)
            or (opened.st_dev, opened.st_ino) != expected_identity
        ):
            raise SourceCandidateError(
                "source candidate published file binding lost",
                published_may_exist=True,
            )
        chunks: list[bytes] = []
        remaining = opened.st_size
        while remaining:
            chunk = os.read(fd, min(65536, remaining))
            if not chunk:
                raise SourceCandidateError(
                    "short source candidate published readback",
                    published_may_exist=True,
                )
            chunks.append(chunk)
            remaining -= len(chunk)
        after = os.fstat(fd)
        named = os.stat(leaf, dir_fd=directory_fd, follow_symlinks=False)
        if (
            os.read(fd, 1)
            or b"".join(chunks) != expected_raw
            or (after.st_dev, after.st_ino) != expected_identity
            or (named.st_dev, named.st_ino) != expected_identity
        ):
            raise SourceCandidateError(
                "source candidate published file drift",
                published_may_exist=True,
            )
    except SourceCandidateError as exc:
        error = exc
    except OSError as exc:
        error = SourceCandidateError(
            "source candidate published readback failed",
            published_may_exist=True,
        )
        error.__cause__ = exc
    if fd is not None:
        try:
            os.close(fd)
        except OSError as exc:
            if error is None:
                error = SourceCandidateError(
                    "source candidate published readback close failed",
                    published_may_exist=True,
                )
                error.__cause__ = exc
    if error is not None:
        raise error


def publish_record(repo: Path, evidence_root: Path, candidate_sha: str) -> Path:
    repo = repo.resolve()
    record = build_record(repo, evidence_root, candidate_sha)
    directory = repo / "quality/source-candidates"
    directory.mkdir(mode=0o755, parents=False, exist_ok=True)
    target = directory / "{}.json".format(candidate_sha)
    temp_leaf = ".source-candidate.{}.tmp".format(secrets.token_hex(16))
    repo_fd: int | None = None
    quality_fd: int | None = None
    directory_fd: int | None = None
    fd: int | None = None
    temp_identity: tuple[int, int] | None = None
    published = False
    error: SourceCandidateError | None = None
    try:
        repo_fd = os.open(str(repo), DIR_FLAGS)
        quality_fd = os.open("quality", DIR_FLAGS, dir_fd=repo_fd)
        directory_fd = os.open(
            "source-candidates",
            DIR_FLAGS,
            dir_fd=quality_fd,
        )
        identities = tuple(
            (item.st_dev, item.st_ino)
            for item in (
                os.fstat(repo_fd),
                os.fstat(quality_fd),
                os.fstat(directory_fd),
            )
        )
        _verify_publication_bindings(
            repo,
            repo_fd,
            quality_fd,
            directory_fd,
            identities,
        )
        flags = os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
        fd = os.open(temp_leaf, flags, 0o600, dir_fd=directory_fd)
        temp_item = os.fstat(fd)
        temp_identity = (temp_item.st_dev, temp_item.st_ino)
        raw = canonical_json_bytes(record)
        written = 0
        while written < len(raw):
            count = os.write(fd, raw[written:])
            if count <= 0:
                raise SourceCandidateError("short source candidate write")
            written += count
        os.fchmod(fd, 0o644)
        os.fsync(fd)
        final_temp = os.fstat(fd)
        named_temp = os.stat(temp_leaf, dir_fd=directory_fd, follow_symlinks=False)
        if (
            not stat.S_ISREG(final_temp.st_mode)
            or final_temp.st_uid != os.geteuid()
            or final_temp.st_nlink != 1
            or final_temp.st_size != len(raw)
            or (final_temp.st_dev, final_temp.st_ino) != temp_identity
            or (named_temp.st_dev, named_temp.st_ino) != temp_identity
        ):
            raise SourceCandidateError("source candidate temporary file drift")
        try:
            os.close(fd)
        except OSError as exc:
            fd = None
            raise SourceCandidateError(
                "source candidate temporary close failed",
            ) from exc
        fd = None
        _verify_publication_bindings(
            repo,
            repo_fd,
            quality_fd,
            directory_fd,
            identities,
        )
        try:
            _rename_exclusive(directory_fd, temp_leaf, target.name)
        except RunStoreError as exc:
            raise SourceCandidateError(
                "source candidate no-clobber publication failed",
                published_may_exist=exc.published_may_exist,
            ) from exc
        published = True
        _readback_published_record(
            directory_fd,
            target.name,
            temp_identity,
            raw,
        )
        try:
            os.fsync(directory_fd)
        except OSError as exc:
            raise SourceCandidateError(
                "source candidate publication durability uncertain",
                published_may_exist=True,
            ) from exc
        _readback_published_record(
            directory_fd,
            target.name,
            temp_identity,
            raw,
        )
        _verify_publication_bindings(
            repo,
            repo_fd,
            quality_fd,
            directory_fd,
            identities,
            published_may_exist=True,
        )
    except SourceCandidateError as exc:
        error = exc
    except OSError as exc:
        error = SourceCandidateError(
            "source candidate publication I/O failed",
            published_may_exist=published,
        )
        error.__cause__ = exc
    if fd is not None:
        try:
            os.close(fd)
        except OSError as exc:
            if error is None:
                error = SourceCandidateError(
                    "source candidate temporary close failed",
                    published_may_exist=published,
                )
                error.__cause__ = exc
    if directory_fd is not None:
        if not published and temp_identity is not None:
            try:
                named_temp = os.stat(
                    temp_leaf,
                    dir_fd=directory_fd,
                    follow_symlinks=False,
                )
                if (named_temp.st_dev, named_temp.st_ino) == temp_identity:
                    os.unlink(temp_leaf, dir_fd=directory_fd)
            except OSError:
                pass
        try:
            os.close(directory_fd)
        except OSError as exc:
            if error is None:
                error = SourceCandidateError(
                    "source candidate directory close failed",
                    published_may_exist=published,
                )
                error.__cause__ = exc
    for label, parent_fd in (
        ("quality", quality_fd),
        ("repository", repo_fd),
    ):
        if parent_fd is None:
            continue
        try:
            os.close(parent_fd)
        except OSError as exc:
            if error is None:
                error = SourceCandidateError(
                    "source candidate {} directory close failed".format(label),
                    published_may_exist=published,
                )
                error.__cause__ = exc
    if error is not None:
        raise error
    return target


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Publish one exact source-candidate record")
    parser.add_argument("create", choices=("create",))
    parser.add_argument("--repo", default=str(ROOT))
    parser.add_argument("--evidence-root", required=True)
    parser.add_argument("--candidate", required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        path = publish_record(Path(args.repo), Path(args.evidence_root), args.candidate)
    except SourceCandidateError as exc:
        print(
            "SOURCE-CANDIDATE status=FAIL published_may_exist={} reason={}".format(
                str(exc.published_may_exist).lower(),
                exc,
            )
        )
        return 1
    print("SOURCE-CANDIDATE status=PASS path={}".format(path.relative_to(Path(args.repo).resolve())))
    return 0


if __name__ == "__main__":
    sys.exit(main())
