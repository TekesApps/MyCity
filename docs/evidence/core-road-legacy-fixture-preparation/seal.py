#!/usr/bin/env python3
"""Seal retained old-writer outputs; never execute a writer or open a Store.

Not yet executed or tested. This does not certify new-reader compatibility.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

import prepare

EVIDENCE = prepare.HERE / "attempt01"
OUTPUT = Path("/Volumes/HP P900/dev/MyCity/work/legacy-accepted-project-119-run01")
KINDS = (("accepted-wal", "wal"), ("accepted-checkpoint", "checkpoint"))


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def canonical_hash(value):
    data = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    return hashlib.sha256(data).hexdigest()


def regular(path, root):
    if root.is_symlink() or not root.is_dir():
        raise RuntimeError(f"missing real evidence root: {root}")
    cursor = root
    for part in path.relative_to(root).parts:
        cursor = cursor / part
        if cursor.is_symlink():
            raise RuntimeError(f"symlink evidence is not allowed: {cursor}")
    path.resolve(strict=True).relative_to(root.resolve(strict=True))
    if not path.is_file():
        raise RuntimeError(f"not a regular evidence file: {path}")
    return path


def world_manifest(root):
    if root.is_symlink() or not root.is_dir():
        raise RuntimeError(f"not a real retained world: {root}")
    files = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise RuntimeError(f"world contains a symlink: {path}")
        if path.is_dir():
            continue
        regular(path, root)
        files.append({"path": path.relative_to(root).as_posix(),
                      "size": path.stat().st_size, "sha256": digest(path)})
    if not files:
        raise RuntimeError("empty retained world")
    return files


def reject_running_measurement_or_writer():
    rows = subprocess.check_output(["ps", "-axo", "pid=,comm=,args="], text=True)
    for line in rows.splitlines():
        fields = line.strip().split(None, 2)
        if len(fields) != 3 or fields[0] == str(os.getpid()):
            continue
        _, comm, command = fields
        executable = Path(comm).name
        if executable in ("mycity-headless", "writer-test-executable") or executable.startswith("legacy_accepted_fixture-"):
            raise RuntimeError(f"measurement/writer is still running: {line.strip()}")
    # No source world may still be open. Failure to inspect is not permission.
    result = subprocess.run(["lsof", "+D", str(OUTPUT)], capture_output=True, text=True)
    if result.returncode != 1 or result.stdout.strip() or result.stderr.strip():
        raise RuntimeError("cannot establish that old-writer output is unoccupied")


def write_new(path, value):
    with path.open("x") as stream:
        json.dump(value, stream, ensure_ascii=False, sort_keys=True, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())


def seal():
    reject_running_measurement_or_writer()
    pins = ((prepare.OLD_EVIDENCE / "source.zip", prepare.OLD_ARCHIVE_SHA256),
            (prepare.OLD_EVIDENCE / "freeze.json", prepare.OLD_MANIFEST_SHA256))
    for path, expected in pins:
        regular(path, prepare.OLD_EVIDENCE)
        if digest(path) != expected:
            raise RuntimeError(f"old-source pin mismatch: {path}")
    freeze = json.loads(pins[1][0].read_bytes())
    if freeze["source_sha256"] != prepare.OLD_BUSINESS_SHA256:
        raise RuntimeError("old business identity mismatch")
    evidence = {
        "writer_receipt": EVIDENCE / "writer-receipt.json",
        "writer_report": OUTPUT / "report.json",
        "writer_stdout": EVIDENCE / "writer.stdout",
        "writer_stderr": EVIDENCE / "writer.stderr",
        "writer_executable": EVIDENCE / "writer-test-executable",
        "exporter_harness": EVIDENCE / "harness.rs",
    }
    hashes = {}
    for label, path in evidence.items():
        regular(path, OUTPUT if label == "writer_report" else EVIDENCE)
        hashes[label] = digest(path)
    receipt = json.loads(evidence["writer_receipt"].read_bytes())
    expected_receipt = {
        "exit_code": 0,
        "executable_sha256": hashes["writer_executable"],
        "source_archive_sha256": prepare.OLD_ARCHIVE_SHA256,
        "freeze_manifest_sha256": prepare.OLD_MANIFEST_SHA256,
        "business_source_sha256": prepare.OLD_BUSINESS_SHA256,
        "exporter_harness_sha256": hashes["exporter_harness"],
        "stdout_sha256": hashes["writer_stdout"],
        "stderr_sha256": hashes["writer_stderr"],
    }
    if any(receipt.get(key) != value for key, value in expected_receipt.items()):
        raise RuntimeError("actual writer receipt does not bind retained source/executable/logs")
    expected_command = [str(evidence["writer_executable"]),
                        "export_v119_accepted_project_wal_and_checkpoint", "--exact", "--nocapture"]
    if receipt.get("command") != expected_command:
        raise RuntimeError("receipt must describe direct execution of preserved writer test")
    if hashes["exporter_harness"] != hashlib.sha256(prepare.HARNESS.encode()).hexdigest():
        raise RuntimeError("exporter harness differs from the reviewed source")
    if not os.access(evidence["writer_executable"], os.X_OK):
        raise RuntimeError("retained writer is not executable")
    report = json.loads(evidence["writer_report"].read_bytes())
    if (report.get("rules") != "construction-engine-v1"
            or report.get("operating_published") is not True
            or report.get("old_historical_world") is not False):
        raise RuntimeError("old writer did not export the required accepted project")
    records = []
    for kind, report_key in KINDS:
        row = report[report_key]
        if row.get("path") != kind or not re.fullmatch("[0-9a-f]{64}", row.get("state_sha256", "")):
            raise RuntimeError(f"invalid old writer state identity: {kind}")
        manifest_path = EVIDENCE / f"{kind}.manifest.json"
        seal_path = EVIDENCE / f"{kind}.seal.json"
        if manifest_path.exists() or seal_path.exists():
            raise RuntimeError("refuse to overwrite any previous seal or partial failure")
        files = world_manifest(OUTPUT / kind)
        manifest = {"fixture_kind": kind, "world_path": str(OUTPUT / kind), "files": files}
        record = {
            "schema": "mycity-v119-accepted-construction-seal-v1", "fixture_kind": kind,
            "rules_identity": report["rules"], "state_sha256": row["state_sha256"],
            "source_archive_sha256": prepare.OLD_ARCHIVE_SHA256,
            "freeze_manifest_sha256": prepare.OLD_MANIFEST_SHA256,
            "business_source_sha256": prepare.OLD_BUSINESS_SHA256,
            "world_manifest_sha256": canonical_hash(files), "writer_exit_code": 0,
            "new_reader_compatibility": "not_run", "old_historical_world": False,
        }
        for label, path in evidence.items():
            record[f"{label}_path"] = str(path)
            record[f"{label}_sha256"] = hashes[label]
        records.append((kind, files, manifest_path, seal_path, manifest, record))
    # Check both complete worlds and all evidence again before creating any seal.
    reject_running_measurement_or_writer()
    for label, path in evidence.items():
        if digest(path) != hashes[label]:
            raise RuntimeError("writer evidence changed during sealing")
    for kind, files, _, _, _, _ in records:
        if world_manifest(OUTPUT / kind) != files:
            raise RuntimeError("retained world changed during sealing")
    for _, _, manifest_path, seal_path, manifest, record in records:
        write_new(manifest_path, manifest)
        record["manifest_file_sha256"] = digest(manifest_path)
        write_new(seal_path, record)
    print("Sealed two retained old-writer fixtures; new-reader compatibility remains not_run")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seal", action="store_true", help="requires actual retained writer evidence")
    args = parser.parse_args()
    if args.seal:
        seal()
    else:
        print("planned_not_run: requires completed writer execution and retained evidence; use --seal")
