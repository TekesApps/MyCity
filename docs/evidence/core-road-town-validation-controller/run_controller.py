#!/usr/bin/env python3
"""Guarded, artifact-preserving release validation controller for MyCity town models.

Default behavior is plan-only. Execution requires --run, an exact expected commit, the
external work/target links, and a clean source checkout. The controller never deletes runs.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import uuid


ROOT = Path(__file__).resolve().parents[3]
DEFAULT_WORK_LINK = Path("/Users/frankqin/dev/MyCity/work")
DEFAULT_WORK_VOLUME = Path("/Volumes/HP P900/dev/MyCity/work")
DEFAULT_TARGET_LINK = Path("/Users/frankqin/dev/MyCity/target")
DEFAULT_TARGET_VOLUME = Path("/Volumes/HP P900/dev/MyCity/target")
TOOLCHAIN = "+1.85.1"
RULES_TRANSPORT_V119 = "m0-town-transport-fixed-profile-v1"
RULES_CONSTRUCTION_V119 = "construction-engine-v1"
# Set only from the preserved v119 source.zip and its fixed business-source manifest.
# Until those immutable identifiers are checked into the controller evidence, legacy
# construction compatibility must remain blocked.
V119_SOURCE_ARCHIVE = Path(
    "/Users/frankqin/dev/MyCity/docs/evidence/core-transport-tax-integration-119/attempt02/source.zip"
)
V119_SOURCE_ARCHIVE_SHA256 = "75d641c5696873580e8ef2a92606d49d9e6fa6dfe4489b7439c9886e5cda7bbe"
V119_FREEZE_MANIFEST = Path(
    "/Users/frankqin/dev/MyCity/docs/evidence/core-transport-tax-integration-119/attempt02/freeze.json"
)
V119_FREEZE_MANIFEST_SHA256 = "3eaac12110dca5e07bd008fe31406eff03771b554faf040ef2cdb90b97c2c0f6"
V119_BUSINESS_SOURCE_SHA256 = "717148eab1b00e5fc183ee240240fa9150cc839f9a94aa96aaa8bb8a22f67f82"
V119_SEAL_ROOT = Path(
    "/Users/frankqin/dev/MyCity/docs/evidence/core-road-legacy-fixture-preparation/attempt01"
)
V119_OLD_OUTPUT_ROOT = Path(
    "/Volumes/HP P900/dev/MyCity/work/legacy-accepted-project-119-run01"
)
V119_FIXTURE_KINDS = ("accepted-wal", "accepted-checkpoint")
SOURCE_SNAPSHOT_PATHS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
    "crates",
    "fixtures",
    "docs/evidence/core-construction-compat/attempt-01/old-v106-world",
    "docs/evidence/core-construction-compat/attempt-01/old-v106-storage-probe.json",
    "docs/evidence/m0-t13a-pe1-short/reference-worlds.json",
)
M0_HEADLESS_COMMANDS = {
    "joint-endurance",
    "layout-session",
    "scale-endurance",
    "steady",
    "speed",
    "rare",
}


class Refusal(RuntimeError):
    """A preflight or validation condition requires the run to stop."""


def valid_sha256(value: str) -> bool:
    return len(value) == 64 and all(ch in "0123456789abcdef" for ch in value)


def process_refusal(comm: str, command: str) -> str | None:
    """Return why a live process blocks validation, if it should block."""
    name = Path(comm.strip()).name.lower()
    if name in {"cargo", "rustc", "cargo-clippy", "clippy-driver", "rustdoc"}:
        return f"active {name} process"
    text = f"{comm} {command}".lower()
    words = re.findall(r"(?<![a-z0-9_-])[a-z0-9][a-z0-9_-]*(?![a-z0-9_-])", text)
    has_m0_headless_command = any(
        executable == "mycity-headless" and subcommand in M0_HEADLESS_COMMANDS
        for executable, subcommand in zip(words, words[1:])
    )
    if has_m0_headless_command:
        return "active MyCity headless M0 measurement process"
    if "mycity" in text and any(word in words for word in ("m0", "measure", "benchmark", "smoke")):
        return "active MyCity M0 measurement process"
    if "m0-t13a" in words or "m0_t13a" in words:
        return "active MyCity M0-T13-A measurement process"
    return None


def reject_live_processes() -> None:
    result = subprocess.run(
        ["ps", "-axo", "pid=,comm=,args="],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise Refusal(f"ps failed with exit {result.returncode}: {result.stderr.strip()}")
    blocked: list[str] = []
    for line in result.stdout.splitlines():
        parts = line.strip().split(None, 2)
        if len(parts) != 3:
            continue
        pid, comm, command = parts
        reason = process_refusal(comm, command)
        if reason:
            blocked.append(f"pid={pid} {reason}: {command}")
    if blocked:
        raise Refusal("live process gate refused the run:\n" + "\n".join(blocked))


def verify_link(link: Path, expected: Path, label: str) -> Path:
    if not link.is_symlink():
        raise Refusal(f"{label} must remain the configured symlink: {link}")
    try:
        resolved = link.resolve(strict=True)
        target = expected.resolve(strict=True)
    except OSError as exc:
        raise Refusal(f"{label} external volume is unavailable: {exc}") from exc
    if resolved != target or not resolved.is_dir():
        raise Refusal(f"{label} link mismatch: {link} -> {resolved}, expected {target}")
    return resolved


def run_git(repo: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise Refusal(f"git {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def verify_source(repo: Path, expected_commit: str) -> None:
    actual = run_git(repo, "rev-parse", "HEAD")
    if actual != expected_commit:
        raise Refusal(f"source HEAD mismatch: actual={actual}, expected={expected_commit}")
    dirty = run_git(repo, "status", "--porcelain", "--untracked-files=all")
    if dirty:
        raise Refusal("source checkout is not clean:\n" + dirty)


def digest_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def preserve_controller_support(run_dir: Path) -> dict[str, object]:
    """Keep the actual control source separately from the Rust build snapshot."""
    source_root = Path(__file__).resolve().parent
    destination = run_dir / "controller-source"
    destination.mkdir(exist_ok=False)
    files = []
    for name in ("run_controller.py", "test_controller.py", "README.md"):
        source = source_root / name
        if source.is_symlink() or not source.is_file():
            raise Refusal(f"controller support must be a regular retained file: {name}")
        data = source.read_bytes()
        with (destination / name).open("xb") as stream:
            stream.write(data)
        files.append({"path": name, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    manifest = {"schema": "mycity-road-town-controller-source-v1", "files": files}
    exclusive_json(destination / "manifest.json", manifest)
    return manifest


def tree_manifest(root: Path) -> list[dict[str, object]]:
    if root.is_symlink() or not root.is_dir():
        raise Refusal(f"world must be a real directory, not a symlink: {root}")
    root = root.resolve(strict=True)
    manifest: list[dict[str, object]] = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise Refusal(f"symlink inside copied world is not allowed: {path}")
        if path.is_dir():
            continue
        if not path.is_file():
            raise Refusal(f"unsupported filesystem entry in world: {path}")
        manifest.append(
            {
                "path": path.relative_to(root).as_posix(),
                "size": path.stat().st_size,
                "sha256": digest_file(path),
            }
        )
    return manifest


def copy_world_verified(
    source: Path,
    destination: Path,
    work_root: Path,
    evidence_dir: Path,
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    if source.is_symlink():
        raise Refusal(f"compatibility input path cannot be a symlink: {source}")
    source_real = source.resolve(strict=True)
    try:
        source_real.relative_to(work_root)
    except ValueError as exc:
        raise Refusal(f"compatibility input must be a copied world under external work: {source_real}") from exc
    before = tree_manifest(source_real)
    exclusive_json(evidence_dir / "source-before.json", before)
    destination.mkdir(parents=False, exist_ok=False)
    try:
        for entry in before:
            rel = Path(str(entry["path"]))
            source_file = source_real / rel
            target_file = destination / rel
            target_file.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source_file, target_file)
    except OSError:
        exclusive_json(evidence_dir / "copy-partial.json", tree_manifest(destination))
        raise
    copied = tree_manifest(destination)
    after = tree_manifest(source_real)
    exclusive_json(evidence_dir / "source-after.json", after)
    exclusive_json(evidence_dir / "copy-manifest.json", copied)
    if before != copied or before != after:
        raise Refusal(f"compatibility copy/hash verification failed: {source_real}")
    return before, after


def verify_original_world_unchanged(
    source: Path, expected: list[dict[str, object]], evidence_dir: Path, phase: str
) -> None:
    actual = tree_manifest(source)
    exclusive_json(evidence_dir / f"source-{phase}.json", actual)
    if actual != expected:
        raise Refusal(f"original compatibility world changed {phase}: {source}")


def canonical_json_sha256(value: object) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def verify_v119_source_pins() -> dict[str, str]:
    if V119_SOURCE_ARCHIVE.is_symlink() or not V119_SOURCE_ARCHIVE.is_file():
        raise Refusal(f"fixed v119 source archive is unavailable: {V119_SOURCE_ARCHIVE}")
    if V119_FREEZE_MANIFEST.is_symlink() or not V119_FREEZE_MANIFEST.is_file():
        raise Refusal(f"fixed v119 freeze manifest is unavailable: {V119_FREEZE_MANIFEST}")
    archive_hash = digest_file(V119_SOURCE_ARCHIVE)
    freeze_hash = digest_file(V119_FREEZE_MANIFEST)
    if archive_hash != V119_SOURCE_ARCHIVE_SHA256:
        raise Refusal("fixed v119 source.zip does not match the pinned archive hash")
    if freeze_hash != V119_FREEZE_MANIFEST_SHA256:
        raise Refusal("fixed v119 freeze.json does not match the pinned hash")
    freeze_value = json.loads(V119_FREEZE_MANIFEST.read_bytes())
    if not isinstance(freeze_value, dict) or freeze_value.get("source_sha256") != V119_BUSINESS_SOURCE_SHA256:
        raise Refusal("fixed v119 freeze.json does not bind the 717148 business-source hash")
    return {
        "source_archive_sha256": archive_hash,
        "freeze_manifest_sha256": freeze_hash,
        "business_source_sha256": V119_BUSINESS_SOURCE_SHA256,
    }


def rooted_file(path: Path, root: Path, label: str) -> Path:
    if root.is_symlink() or path.is_symlink():
        raise Refusal(f"{label} path or root must not be a symlink: {path}")
    try:
        resolved_root = root.resolve(strict=True)
        unresolved_relative = path.absolute().relative_to(root.absolute())
        cursor = root
        for component in unresolved_relative.parts:
            cursor = cursor / component
            if cursor.is_symlink():
                raise Refusal(f"{label} has a symlink component: {cursor}")
        resolved = path.resolve(strict=True)
        resolved.relative_to(resolved_root)
    except (OSError, ValueError) as exc:
        raise Refusal(f"{label} must resolve inside {root}: {path}") from exc
    if not resolved.is_file():
        raise Refusal(f"{label} is not a regular file: {resolved}")
    return resolved


def verify_v119_construction_seal(
    fixture_kind: str,
    source_world: Path,
    manifest_path: Path,
    seal_path: Path,
    rules: str,
    state_hash: str,
    work_root: Path,
    source_pins: dict[str, str],
) -> dict[str, object]:
    if fixture_kind not in V119_FIXTURE_KINDS:
        raise Refusal(f"unknown accepted-construction fixture kind: {fixture_kind}")
    expected_world = V119_OLD_OUTPUT_ROOT / fixture_kind
    if source_world.is_symlink() or source_world.resolve(strict=True) != expected_world.resolve(strict=True):
        raise Refusal(f"{fixture_kind} world must be the sealed old-writer output: {expected_world}")
    try:
        source_world.resolve(strict=True).relative_to(work_root.resolve(strict=True))
    except (OSError, ValueError) as exc:
        raise Refusal("accepted-construction worlds must be under the external work volume") from exc
    expected_manifest = V119_SEAL_ROOT / f"{fixture_kind}.manifest.json"
    expected_seal = V119_SEAL_ROOT / f"{fixture_kind}.seal.json"
    if manifest_path.resolve(strict=True) != expected_manifest.resolve(strict=True):
        raise Refusal(f"manifest path must be the fixed fixture manifest: {expected_manifest}")
    if seal_path.resolve(strict=True) != expected_seal.resolve(strict=True):
        raise Refusal(f"seal path must be the fixed root-issued seal: {expected_seal}")
    manifest_file = rooted_file(manifest_path, V119_SEAL_ROOT, "construction file manifest")
    seal_file = rooted_file(seal_path, V119_SEAL_ROOT, "construction seal")
    actual_files = tree_manifest(source_world)
    manifest_value = json.loads(manifest_file.read_bytes())
    if not isinstance(manifest_value, dict):
        raise Refusal("construction manifest must be a JSON object")
    if manifest_value != {
        "fixture_kind": fixture_kind,
        "world_path": str(expected_world),
        "files": actual_files,
    }:
        raise Refusal(f"{fixture_kind} source world does not match its frozen full-file manifest")
    manifest_hash = digest_file(manifest_file)
    world_hash = canonical_json_sha256(actual_files)
    seal_value = json.loads(seal_file.read_bytes())
    if not isinstance(seal_value, dict):
        raise Refusal("construction seal must be a JSON object")
    required = {
        "schema": "mycity-v119-accepted-construction-seal-v1",
        "fixture_kind": fixture_kind,
        "rules_identity": rules,
        "state_sha256": state_hash,
        "source_archive_sha256": source_pins["source_archive_sha256"],
        "freeze_manifest_sha256": source_pins["freeze_manifest_sha256"],
        "business_source_sha256": source_pins["business_source_sha256"],
        "manifest_file_sha256": manifest_hash,
        "world_manifest_sha256": world_hash,
        "writer_exit_code": 0,
    }
    for key, expected in required.items():
        if seal_value.get(key) != expected:
            raise Refusal(f"{fixture_kind} root seal mismatch at {key}")
    evidence_files = {
        "writer_receipt": ("writer_receipt_path", "writer_receipt_sha256", V119_SEAL_ROOT, "writer-receipt.json"),
        "writer_report": ("writer_report_path", "writer_report_sha256", V119_OLD_OUTPUT_ROOT, "report.json"),
        "writer_stdout": ("writer_stdout_path", "writer_stdout_sha256", V119_SEAL_ROOT, "writer.stdout"),
        "writer_stderr": ("writer_stderr_path", "writer_stderr_sha256", V119_SEAL_ROOT, "writer.stderr"),
        "writer_executable": ("writer_executable_path", "writer_executable_sha256", V119_SEAL_ROOT, "writer-test-executable"),
        "exporter_harness": ("exporter_harness_path", "exporter_harness_sha256", V119_SEAL_ROOT, "harness.rs"),
    }
    verified_evidence: dict[str, dict[str, str]] = {}
    for label, (path_key, hash_key, evidence_root, expected_name) in evidence_files.items():
        evidence_path = evidence_root / expected_name
        sealed_path = seal_value.get(path_key)
        expected_hash = seal_value.get(hash_key)
        if sealed_path != str(evidence_path) or not isinstance(expected_hash, str) or not valid_sha256(expected_hash):
            raise Refusal(f"{fixture_kind} seal has invalid {label} evidence reference")
        evidence_path = rooted_file(evidence_path, evidence_root, label)
        actual_hash = digest_file(evidence_path)
        if actual_hash != expected_hash:
            raise Refusal(f"{fixture_kind} {label} does not match its sealed SHA256")
        verified_evidence[label] = {"path": str(evidence_path), "sha256": actual_hash}
    receipt = json.loads((V119_SEAL_ROOT / "writer-receipt.json").read_bytes())
    if not isinstance(receipt, dict) or receipt.get("exit_code") != 0:
        raise Refusal("frozen-v119 writer receipt does not report exit_code 0")
    if receipt.get("executable_sha256") != verified_evidence["writer_executable"]["sha256"]:
        raise Refusal("frozen-v119 writer receipt executable hash does not match preserved executable")
    return {
        "fixture_kind": fixture_kind,
        "manifest_path": str(manifest_file),
        "manifest_file_sha256": manifest_hash,
        "world_manifest_sha256": world_hash,
        "seal_path": str(seal_file),
        "seal_file_sha256": digest_file(seal_file),
        "seal": seal_value,
        "verified_evidence": verified_evidence,
    }


def exclusive_json(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2, sort_keys=True)
        stream.write("\n")


def save_source_snapshot(repo: Path, commit: str, run_dir: Path) -> dict[str, str]:
    archive_path = run_dir / "source.tar"
    lock_path = run_dir / "Cargo.lock"
    scope_path = run_dir / "source-scope.json"
    scope_entries: list[dict[str, object]] = []
    for path in SOURCE_SNAPSHOT_PATHS:
        listing = subprocess.run(
            ["git", "-C", str(repo), "ls-tree", "-r", "-z", commit, "--", path],
            check=False,
            capture_output=True,
        )
        if listing.returncode != 0:
            raise Refusal(f"git ls-tree failed for source scope {path}")
        for record in listing.stdout.split(b"\0"):
            if not record:
                continue
            header, raw_path = record.split(b"\t", 1)
            mode, kind, object_id = header.decode("ascii").split()
            if kind != "blob":
                continue
            content = subprocess.run(
                ["git", "-C", str(repo), "cat-file", "blob", object_id],
                check=False,
                capture_output=True,
            )
            if content.returncode != 0:
                raise Refusal(f"git cat-file failed for source scope {raw_path!r}")
            scope_entries.append(
                {
                    "path": raw_path.decode("utf-8", errors="strict"),
                    "mode": mode,
                    "git_blob": object_id,
                    "size": len(content.stdout),
                    "sha256": hashlib.sha256(content.stdout).hexdigest(),
                }
            )
    scope_entries.sort(key=lambda item: str(item["path"]))
    exclusive_json(
        scope_path,
        {"commit": commit, "included_paths": list(SOURCE_SNAPSHOT_PATHS), "files": scope_entries},
    )
    with archive_path.open("xb") as archive:
        result = subprocess.run(
            ["git", "-C", str(repo), "archive", "--format=tar", commit, *SOURCE_SNAPSHOT_PATHS],
            stdout=archive,
            stderr=subprocess.PIPE,
            check=False,
        )
    if result.returncode != 0:
        raise Refusal(f"git archive failed: {result.stderr.decode(errors='replace')}")
    shutil.copy2(repo / "Cargo.lock", lock_path)
    snapshot = {
        "commit": commit,
        "source_scope_sha256": digest_file(scope_path),
        "source_archive_sha256": digest_file(archive_path),
        "cargo_lock_sha256": digest_file(lock_path),
    }
    exclusive_json(run_dir / "source-snapshot.json", snapshot)
    return snapshot


def clean_environment(artifact_root: Path, target_root: Path) -> dict[str, str]:
    env = os.environ.copy()
    env.pop("RUST_MIN_STACK", None)
    for key in tuple(env):
        if key.startswith("MYCITY_"):
            env.pop(key, None)
    env["CARGO_TARGET_DIR"] = str(target_root)
    env["MYCITY_TEST_ARTIFACT_ROOT"] = str(artifact_root)
    return env


def run_command(
    label: str,
    argv: list[str],
    repo: Path,
    run_dir: Path,
    target_root: Path,
    index: int,
    expected_commit: str,
    capture_process_rss: bool = False,
) -> None:
    # A process may start between two commands, so the gate is per-command.
    reject_live_processes()
    verify_source(repo, expected_commit)
    prefix = f"{index:03d}-{label}"
    command_dir = run_dir / prefix
    command_dir.mkdir(exist_ok=False)
    artifact_root = command_dir / "test-artifacts"
    artifact_root.mkdir()
    stdout_path = command_dir / "stdout.log"
    stderr_path = command_dir / "stderr.log"
    metadata_path = command_dir / "command.json"
    command_argv = ["/usr/bin/time", "-l", *argv] if capture_process_rss else argv
    exclusive_json(
        metadata_path,
        {
            "argv": argv,
            "executed_argv": command_argv,
            "cwd": str(repo),
            "label": label,
            "process_rss_capture": capture_process_rss,
        },
    )
    exclusive_json(
        command_dir / "source-before.json",
        {
            "head": run_git(repo, "rev-parse", "HEAD"),
            "status": run_git(repo, "status", "--porcelain", "--untracked-files=all"),
        },
    )
    env = clean_environment(artifact_root, target_root)
    started = time.monotonic()
    with stdout_path.open("x", encoding="utf-8") as stdout, stderr_path.open("x", encoding="utf-8") as stderr:
        result = subprocess.run(command_argv, cwd=repo, env=env, stdout=stdout, stderr=stderr, check=False)
    wall_seconds = time.monotonic() - started
    # Capture source identity immediately after the child exits, even on failure.
    source_after = {
        "head": run_git(repo, "rev-parse", "HEAD"),
        "status": run_git(repo, "status", "--porcelain", "--untracked-files=all"),
    }
    exclusive_json(command_dir / "source-after.json", source_after)
    source_changed = source_after["head"] != expected_commit or bool(source_after["status"])
    stderr_text = stderr_path.read_text(encoding="utf-8", errors="replace")
    max_rss_line = None
    max_rss_match = re.search(
        r"^\s*maximum resident set size[^\n]*$",
        stderr_text,
        flags=re.MULTILINE | re.IGNORECASE,
    )
    if max_rss_match:
        max_rss_line = max_rss_match.group(0).strip()
    runtime_metrics = {
        "wall_seconds": wall_seconds,
        "max_rss_line": max_rss_line,
        "scope": "whole CLI process including runtime; not model-only heap and not an M0 performance result",
    }
    binary_receipt: dict[str, object] = {"status": "not_found"}
    if result.returncode != 0:
        running_paths = re.findall(r"\bRunning\b.*?\(([^()\r\n]+)\)", stderr_text)
        if running_paths:
            candidate = Path(running_paths[-1])
            if not candidate.is_absolute():
                candidate = repo / candidate
            candidate = candidate.resolve(strict=False)
            try:
                candidate.relative_to(target_root.resolve(strict=True))
            except (ValueError, OSError):
                candidate = Path()
            if candidate.is_file():
                failure_bin_dir = command_dir / "failure-executables"
                failure_bin_dir.mkdir(exist_ok=False)
                saved = failure_bin_dir / candidate.name
                shutil.copy2(candidate, saved)
                binary_receipt = {
                    "status": "saved_exact_running_executable",
                    "reported_path": str(candidate),
                    "saved_path": str(saved),
                    "sha256": digest_file(saved),
                }
    exclusive_json(
        command_dir / "exit.json",
        {
            "label": label,
            "exit_code": result.returncode,
            "failure_executable": binary_receipt,
            "runtime_metrics": runtime_metrics,
        },
    )
    print(f"{label}: exit={result.returncode} logs={command_dir}", flush=True)
    if source_changed:
        raise Refusal(f"source checkout changed during command: {label}; command evidence retained")
    if result.returncode != 0:
        if binary_receipt["status"] == "not_found":
            exclusive_json(command_dir / "failure-executable.json", {"status": "not_found_in_this_command_stderr"})
        raise Refusal(f"command failed; preserving artifacts and stopping: {label} exit={result.returncode}")


def run_plan() -> list[tuple[str, list[str]]]:
    cargo = ["cargo", TOOLCHAIN]
    return [
        ("fmt", cargo + ["fmt", "--all", "--", "--check"]),
        ("core-release-tests", cargo + ["test", "--locked", "--offline", "--release", "--all-targets", "-p", "mycity-core"]),
        ("storage-release-tests", cargo + ["test", "--locked", "--offline", "--release", "--all-targets", "-p", "mycity-storage"]),
        ("storage-release-examples", cargo + ["build", "--locked", "--offline", "--release", "--examples", "-p", "mycity-storage"]),
        ("strict-clippy", cargo + ["clippy", "--locked", "--offline", "--release", "--all-targets", "-p", "mycity-core", "-p", "mycity-storage", "--", "-D", "warnings"]),
    ]


def print_plan(args: argparse.Namespace) -> None:
    print("PLAN ONLY: no commands will run. Execution requires --run and --expected-commit.")
    for label, argv in run_plan():
        print(f"{label}: {' '.join(argv)}")
    scenarios = ("normal", "closed", "bottleneck", "expanded", "cross-boundary", "road-effect", "road-suspend", "road-28", "road-withdraw")
    for scenario in scenarios:
        print(
            f"town_transport/{scenario}: <target>/release/examples/town_transport {scenario} <new-world-dir>"
        )
    print("town_suspend: <target>/release/examples/town_suspend <new-world-dir>")
    print("town_suspend loss: <target>/release/examples/town_suspend <new-world-dir> --loss")
    print("payroll_exit: <target>/release/examples/payroll_exit <new-world-dir>")
    print(
        "compat transport (explicit only): reopen-check <verified-copy> "
        f"{RULES_TRANSPORT_V119} <v119-reported-state-sha256>"
    )
    print(
        "compat accepted construction: pass both accepted-wal and accepted-checkpoint "
        "world/manifest/seal/rules/hash groups; each gets a separate copied Store reopen-check"
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--plan", action="store_true", help="print commands only (default)")
    mode.add_argument("--run", action="store_true", help="execute after all safety gates pass")
    parser.add_argument("--repo", type=Path, default=ROOT)
    parser.add_argument("--expected-commit")
    parser.add_argument("--work-link", type=Path, default=DEFAULT_WORK_LINK)
    parser.add_argument("--work-volume", type=Path, default=DEFAULT_WORK_VOLUME)
    parser.add_argument("--target-link", type=Path, default=DEFAULT_TARGET_LINK)
    parser.add_argument("--target-volume", type=Path, default=DEFAULT_TARGET_VOLUME)
    parser.add_argument("--artifact-base", type=Path)
    parser.add_argument("--compat-transport-world", type=Path)
    parser.add_argument("--compat-transport-hash")
    parser.add_argument("--compat-construction-world", type=Path, action="append")
    parser.add_argument("--compat-construction-manifest", type=Path, action="append")
    parser.add_argument("--compat-construction-seal", type=Path, action="append")
    parser.add_argument(
        "--compat-construction-provenance", dest="compat_construction_seal", type=Path, action="append"
    )
    parser.add_argument("--compat-construction-rules", action="append")
    parser.add_argument("--compat-construction-hash", action="append")
    return parser


def do_run(args: argparse.Namespace) -> None:
    if not args.expected_commit or not re.fullmatch(r"[0-9a-f]{40,64}", args.expected_commit):
        raise Refusal("--run requires an explicit full lowercase --expected-commit")

    # The live-process gate intentionally runs before filesystem snapshots or directory creation.
    reject_live_processes()
    work_root = verify_link(args.work_link, args.work_volume, "work")
    target_root = verify_link(args.target_link, args.target_volume, "target")
    repo = args.repo.resolve(strict=True)
    verify_source(repo, args.expected_commit)

    if bool(args.compat_transport_world) != bool(args.compat_transport_hash):
        raise Refusal("transport compatibility requires both --compat-transport-world and hash")
    construction_worlds = args.compat_construction_world or []
    construction_manifests = args.compat_construction_manifest or []
    construction_seals = args.compat_construction_seal or []
    construction_rules = args.compat_construction_rules or []
    construction_hashes = args.compat_construction_hash or []
    construction_counts = {
        len(construction_worlds),
        len(construction_manifests),
        len(construction_seals),
        len(construction_rules),
        len(construction_hashes),
    }
    if construction_counts != {0} and construction_counts != {2}:
        raise Refusal(
            "accepted-construction compatibility requires exactly two complete groups: "
            "one WAL and one checkpoint world, each with manifest, seal, rules, and hash"
        )
    for name, digest in (
        ("transport", args.compat_transport_hash),
    ):
        if digest is not None and not valid_sha256(digest):
            raise Refusal(f"{name} expected state hash must be 64 lowercase hex chars")
    construction_seal_evidence: dict[str, dict[str, object]] = {}
    if construction_worlds:
        source_pins = verify_v119_source_pins()
        for source, manifest, seal, rules, digest in zip(
            construction_worlds,
            construction_manifests,
            construction_seals,
            construction_rules,
            construction_hashes,
        ):
            if rules != RULES_CONSTRUCTION_V119:
                raise Refusal(f"construction rules must equal {RULES_CONSTRUCTION_V119}")
            if not valid_sha256(digest):
                raise Refusal("construction expected state hash must be 64 lowercase hex chars")
            fixture_kind = manifest.name.removesuffix(".manifest.json")
            checked = verify_v119_construction_seal(
                fixture_kind,
                source,
                manifest,
                seal,
                rules,
                digest,
                work_root,
                source_pins,
            )
            fixture_kind = str(checked["fixture_kind"])
            if fixture_kind in construction_seal_evidence:
                raise Refusal(f"duplicate construction fixture: {fixture_kind}")
            construction_seal_evidence[fixture_kind] = checked
        if set(construction_seal_evidence) != set(V119_FIXTURE_KINDS):
            raise Refusal("both accepted-wal and accepted-checkpoint seals are required")

    artifact_base = args.artifact_base or (work_root / "core-road-town-validation-controller")
    if artifact_base.is_symlink():
        raise Refusal("artifact base cannot be a symlink")
    artifact_base_resolved = artifact_base.resolve(strict=False)
    try:
        artifact_base_resolved.relative_to(work_root)
    except ValueError as exc:
        raise Refusal("artifact base must be under the external work volume") from exc
    artifact_base.mkdir(parents=True, exist_ok=True)
    if artifact_base.is_symlink() or not artifact_base.is_dir():
        raise Refusal("artifact base is not a regular directory")
    run_name = dt.datetime.now(dt.timezone.utc).strftime("run-%Y%m%dT%H%M%SZ-") + uuid.uuid4().hex[:12]
    run_dir = artifact_base / run_name
    run_dir.mkdir(parents=True, exist_ok=False)
    controller_support = preserve_controller_support(run_dir)
    exclusive_json(
        run_dir / "preflight.json",
        {
            "expected_commit": args.expected_commit,
            "repo": str(repo),
            "work_link": str(args.work_link),
            "work_resolved": str(work_root),
            "target_link": str(args.target_link),
            "target_resolved": str(target_root),
            "controller": str(Path(__file__).resolve()),
            "controller_support": controller_support,
            "compat_transport": bool(args.compat_transport_world),
            "compat_construction": bool(construction_worlds),
            "v119_source_pins": source_pins if construction_worlds else None,
        },
    )
    snapshot = save_source_snapshot(repo, args.expected_commit, run_dir)
    print(f"run={run_dir}\nsource={json.dumps(snapshot, sort_keys=True)}", flush=True)

    index = 1
    for label, argv in run_plan():
        run_command(label, argv, repo, run_dir, target_root, index, args.expected_commit)
        index += 1

    binaries = run_dir / "executables"
    binaries.mkdir(exist_ok=False)
    for name in ("town_transport", "town_suspend", "payroll_exit"):
        source = target_root / "release" / "examples" / name
        if not source.is_file():
            raise Refusal(f"required example executable was not built: {source}")
        destination = binaries / name
        shutil.copy2(source, destination)
        exclusive_json(
            binaries / f"{name}.sha256.json",
            {"path": str(destination), "sha256": digest_file(destination)},
        )

    for name, argv in (
        ("town-transport-normal", [str(binaries / "town_transport"), "normal"]),
        ("town-transport-closed", [str(binaries / "town_transport"), "closed"]),
        ("town-transport-bottleneck", [str(binaries / "town_transport"), "bottleneck"]),
        ("town-transport-expanded", [str(binaries / "town_transport"), "expanded"]),
        ("town-transport-cross-boundary", [str(binaries / "town_transport"), "cross-boundary"]),
        ("town-transport-road-effect", [str(binaries / "town_transport"), "road-effect"]),
        ("town-transport-road-suspend", [str(binaries / "town_transport"), "road-suspend"]),
        ("town-transport-road-28", [str(binaries / "town_transport"), "road-28"]),
        ("town-transport-road-withdraw", [str(binaries / "town_transport"), "road-withdraw"]),
        ("town-suspend", [str(binaries / "town_suspend")]),
        ("town-suspend-loss", [str(binaries / "town_suspend")]),
        ("payroll-exit", [str(binaries / "payroll_exit")]),
    ):
        world = run_dir / "worlds" / name
        world.parent.mkdir(exist_ok=True)
        extra = ["--loss"] if name == "town-suspend-loss" else []
        run_command(
            name,
            argv + [str(world), *extra],
            repo,
            run_dir,
            target_root,
            index,
            args.expected_commit,
            capture_process_rss=True,
        )
        index += 1

    compat_runs: list[tuple[str, Path, str, str, dict[str, object] | None]] = []
    if args.compat_transport_world and args.compat_transport_hash:
        compat_runs.append(
            ("transport", args.compat_transport_world, RULES_TRANSPORT_V119, args.compat_transport_hash, None)
        )
    for source, digest, rules, manifest, seal in zip(
        construction_worlds,
        construction_hashes,
        construction_rules,
        construction_manifests,
        construction_seals,
    ):
        fixture_kind = manifest.name.removesuffix(".manifest.json")
        compat_runs.append(
            (
                f"construction-{fixture_kind}",
                source,
                rules,
                digest,
                construction_seal_evidence[fixture_kind],
            )
        )
    for kind, source, rules, digest, proof in compat_runs:
        compat_name = kind.replace("/", "-")
        compat_dir = run_dir / "compat" / f"{compat_name}-world"
        compat_dir.parent.mkdir(parents=True, exist_ok=True)
        evidence_dir = compat_dir.parent / f"{compat_name}-manifests"
        evidence_dir.mkdir(exist_ok=False)
        original_manifest, _ = copy_world_verified(source, compat_dir, work_root, evidence_dir)
        if proof is not None:
            exclusive_json(run_dir / "compat" / f"{compat_name}-seal-evidence.json", proof)
        argv = [str(binaries / "town_transport"), "reopen-check", str(compat_dir), rules, digest]
        verify_original_world_unchanged(source, original_manifest, evidence_dir, "before-open")
        run_command(
            f"compat-{compat_name}-reopen",
            argv,
            repo,
            run_dir,
            target_root,
            index,
            args.expected_commit,
            capture_process_rss=True,
        )
        verify_original_world_unchanged(source, original_manifest, evidence_dir, "after-open")
        exclusive_json(evidence_dir / "copy-after-open.json", tree_manifest(compat_dir))
        index += 1

    exclusive_json(
        run_dir / "sequence.json",
        {
            "commands_completed": index - 1,
            "next_action": "independent review required; this file is not an overall acceptance",
        },
    )
    print(f"command sequence ended; review each receipt and evidence directory: {run_dir}")


def main() -> int:
    args = build_parser().parse_args()
    if not args.run:
        print_plan(args)
        return 0
    try:
        do_run(args)
    except (Refusal, OSError, ValueError) as exc:
        print(f"REFUSED/STOPPED: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
