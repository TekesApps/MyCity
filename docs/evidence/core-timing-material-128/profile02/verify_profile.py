#!/usr/bin/env python3
"""Independent verifier for the frozen full-tier unpaced timing diagnostic.

This tool reads completed evidence only. It never starts the model, Cargo, or
any benchmark. It keeps functional evidence, timing visibility, and resource /
performance acceptance as distinct outcomes.

Usage:
  python3 verify_profile.py REPO_PATH [OUTPUT_JSON]

The frozen transcript stores response payload hashes and the explicitly
borrowed `sim_host_timing` summary for successful Status responses, not the
complete Status payload. The verifier uses only that summary, segments
cumulative counters by sim-host process instance, and never treats payload
hashes as timing data.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import struct
import sys
import zipfile
from pathlib import Path
from typing import Any


BASE = Path(__file__).resolve().parent
BUILD_ROOT_FILES = {".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
EXPECTED_SOURCE = "db4577273a37fdbeeb95c092e20356ba2386bdb0dd311f2b6020de2bb89f0daf"
EXPECTED_BINARY = "83b7a28460c8d82ae1b5c5c17ca154359648d6d3937103f973a8fa988fcc373a"
EXPECTED_SOURCE_ZIP = "ac4fed1cf8d4f915f7d69b2947e4795310f56e8aba2775a4eb386b01536cfdea"
EXPECTED_ACCEPTANCE = "876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508"
FIXTURES = {
    "opening_sha256": "fixtures/m0-t13a-full-opening-candidate.json",
    "policy_sha256": "fixtures/m0-t13a-real-policy-candidate.json",
    "oracle_sha256": "fixtures/m0-t13a-expected-oracle-candidate.json",
}
MAX_TRANSCRIPT_LINE = 128 * 1024
TIMING_PHASES = (
    "run_required",
    "cache_recover_rebind_reset",
    "input_encode_publish",
    "worker_spawn_register_submit_to_first_validated_response",
    "worker_progress_wait_after_first_response",
    "candidate_read_and_import",
    "persistent_accept_execute",
    "status_air_checkpoint_digest",
    "status_business_digest",
    "status_state_digest",
)


class VerifyError(Exception):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise VerifyError(message)


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise VerifyError(f"missing:{path}") from exc
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise VerifyError(f"invalid_json:{path}:{exc}") from exc
    require(isinstance(value, dict), f"json_object_expected:{path}")
    return value


def sha_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
    except FileNotFoundError as exc:
        raise VerifyError(f"missing:{path}") from exc
    return digest.hexdigest()


def require_sha(value: Any, expected: str, label: str) -> None:
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None,
            f"invalid_sha256:{label}")
    require(value == expected, f"sha256_mismatch:{label}")


def source_stream_sha256(entries: list[tuple[str, bytes]]) -> str:
    digest = hashlib.sha256()
    for name, data in sorted(entries, key=lambda row: row[0]):
        encoded = name.encode("utf-8")
        digest.update(struct.pack("<Q", len(encoded)))
        digest.update(encoded)
        digest.update(struct.pack("<Q", len(data)))
        digest.update(data)
    return digest.hexdigest()


def verify_fixed_inputs(repo: Path, descriptor_root: Path) -> dict[str, Any]:
    manifest = read_json(descriptor_root / "prepared-manifest.json")
    pinned = manifest.get("fixed_diagnostic_record")
    require(isinstance(pinned, dict), "prepared_manifest_diagnostic_descriptor_missing")
    record_path = Path(pinned.get("path", "")).resolve(strict=True)
    record = read_json(record_path)
    source = record.get("source_sha256")
    binary_sha = record.get("binary_sha256")
    source_zip_sha = record.get("source_zip_sha256")
    require(source == EXPECTED_SOURCE and binary_sha == EXPECTED_BINARY and
            source_zip_sha == EXPECTED_SOURCE_ZIP, "diagnostic_record_not_current_v128_02")
    for label, value in (("source", source), ("binary", binary_sha), ("source_zip", source_zip_sha)):
        require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None,
                f"invalid_diagnostic_record:{label}")
    require(pinned.get("source_sha256") == source and pinned.get("binary_sha256") == binary_sha and
            pinned.get("source_zip_sha256") == source_zip_sha, "descriptor_record_identity_mismatch")
    if pinned.get("record_sha256") is not None:
        require_sha(sha_file(record_path), pinned["record_sha256"], "diagnostic_record")
    binary = Path(record.get("binary", "")).resolve(strict=True)
    require_sha(sha_file(binary), binary_sha, "executable")
    archive_path = Path(record.get("source_zip", "")).resolve(strict=True)
    require_sha(sha_file(archive_path), source_zip_sha, "source_zip")
    try:
        with zipfile.ZipFile(archive_path) as archive:
            names = archive.namelist()
            require(archive.testzip() is None, "source_zip_crc")
            require(len(names) == len(set(names)), "source_zip_duplicate_paths")
            entries = [(name, archive.read(name)) for name in names]
    except (OSError, zipfile.BadZipFile, KeyError) as exc:
        raise VerifyError(f"source_zip_invalid:{exc}") from exc
    frozen = dict(entries)
    build_entries = [(name, data) for name, data in entries
                     if name in BUILD_ROOT_FILES or name.startswith("crates/")
                     or name.startswith("fixtures/")]
    require(len(build_entries) == record.get("source_files"), "source_zip_build_entry_count")
    require(source_stream_sha256(build_entries) == source, "source_stream_hash")
    require(source.encode("ascii") in binary.read_bytes(), "embedded_source_fingerprint")
    acceptance_data = frozen.get("fixtures/m0-t13a-acceptance.json")
    require(acceptance_data is not None, "frozen_acceptance_fixture_missing")
    acceptance = pinned.get("acceptance_fixture_sha256")
    require(acceptance == EXPECTED_ACCEPTANCE, "descriptor_acceptance_fixture_identity")
    require_sha(sha_bytes(acceptance_data), acceptance, "acceptance_fixture")

    require("pending" not in str(pinned.get("status", "")).lower(),
            "prepared_manifest_descriptor_pending")
    runner_sha = manifest.get("prepared_files_sha256", {}).get("run_profile.py")
    require_sha(sha_file(descriptor_root / "run_profile.py"), runner_sha, "pinned_runner")
    fixture_hashes: dict[str, str] = {}
    for field, name in FIXTURES.items():
        data = frozen.get(name)
        require(data is not None, f"frozen_fixture_missing:{name}")
        current = repo / name
        try:
            current_data = current.read_bytes()
        except OSError as exc:
            raise VerifyError(f"repo_fixture_unavailable:{current}:{exc}") from exc
        require(current_data == data, f"repo_fixture_differs_from_archive:{name}")
        fixture_hashes[field] = sha_bytes(data)
    return {
        "source_sha256": source,
        "binary": str(binary),
        "binary_sha256": binary_sha,
        "source_zip": str(archive_path),
        "source_zip_sha256": source_zip_sha,
        "archive_entries": len(names),
        "build_source_entries": len(build_entries),
        "acceptance_fixture_sha256": acceptance,
        "runner_sha256": runner_sha,
        "fixtures": fixture_hashes,
        "declared_run_root": manifest.get("run_root"),
        "current_checkout_source_comparison": "not_used_for_binary_identity",
    }


def check_task(business: Path, directory: str, mode: str, source: str,
               fixture_hashes: dict[str, str], names: list[str]) -> dict[str, Any]:
    task = business / directory
    request_path = task / "request.json"
    request = read_json(request_path)
    result = read_json(task / "result.json")
    require(request.get("schema") == "mycity-model-task-request-v2", f"request_schema:{directory}")
    require(request.get("tier") == "full" and request.get("mode") == mode,
            f"request_identity:{directory}")
    require(request.get("source_sha256") == source, f"request_source:{directory}")
    for key, digest in fixture_hashes.items():
        require(request.get(key) == digest, f"request_{key}:{directory}")
    require(request.get("root") == str(business.resolve()).encode("utf-8").hex(),
            f"request_root:{directory}")
    require(result.get("schema") == "mycity-model-task-result-v2", f"result_schema:{directory}")
    require(result.get("source_sha256") == source and result.get("tier") == "full"
            and result.get("mode") == mode, f"result_identity:{directory}")
    require(result.get("request_sha256") == sha_file(request_path), f"request_digest:{directory}")
    require(type(result.get("pid")) is int and result["pid"] > 0, f"task_pid:{directory}")
    require(type(result.get("instance_unix_ns")) is int and result["instance_unix_ns"] > 0,
            f"task_instance:{directory}")
    artifacts = result.get("artifacts")
    require(isinstance(artifacts, list) and len(artifacts) == 3, f"artifact_slots:{directory}")
    for index, name in enumerate(names):
        item = artifacts[index]
        require(isinstance(item, dict) and item.get("name") == name,
                f"artifact_name:{directory}:{name}")
        content = (task / name).read_bytes()
        require(item.get("bytes") == len(content) and item.get("sha256") == sha_bytes(content),
                f"artifact_hash:{directory}:{name}")
    for index in range(len(names), 3):
        require(artifacts[index] is None, f"unexpected_artifact:{directory}:{index}")
    if mode != "reference":
        stamp = result.get("root")
        require(isinstance(stamp, dict), f"root_stamp:{directory}")
        root_bytes = (task / "root.json").read_bytes()
        root = json.loads(root_bytes)
        require(stamp.get("bytes") == len(root_bytes) and stamp.get("sha256") == sha_bytes(root_bytes)
                and stamp.get("sequence") == root.get("sequence"), f"root_asset:{directory}")
    return result


def verify_clock(root: Path, source: str) -> dict[str, Any]:
    observed = root / "observed"
    model = observed / "model"
    clock = read_json(observed / "observer-clock-map.json")
    child = read_json(observed / "observer-child.json")
    ready = read_json(model / "observer-clock-ready.json")
    request = read_json(model / "observer-clock-request.json")
    reply = read_json(model / "observer-clock-reply.json")
    pid = clock.get("supervisor_pid")
    nonce = clock.get("nonce")
    require(clock.get("source_sha256") == source, "clock_source")
    require(type(pid) is int and pid > 0 and child.get("identity", {}).get("pid") == pid,
            "clock_pid")
    require(isinstance(nonce, str) and re.fullmatch(r"[0-9a-f]{32}", nonce) is not None,
            "clock_nonce")
    for row, schema in ((ready, "mycity-observer-clock-ready-v1"),
                        (request, "mycity-observer-clock-request-v1"),
                        (reply, "mycity-observer-clock-reply-v1")):
        require(row.get("schema") == schema and row.get("pid") == pid and
                row.get("source_sha256") == source, f"clock_message:{schema}")
    require(request.get("nonce") == reply.get("nonce") == nonce, "clock_nonce_pair")
    spawn, sent, received, elapsed, low, high, uncertainty = (
        clock.get(key) for key in (
            "spawn_before_observer_ns", "request_sent_observer_ns",
            "reply_received_observer_ns", "unused",
            "supervisor_origin_observer_ns_min", "supervisor_origin_observer_ns_max",
            "uncertainty_ns"))
    # The model-provided elapsed clock belongs to the reply; keep it separate from the observer map.
    elapsed = reply.get("elapsed_ns")
    values = (spawn, sent, received, elapsed, low, high, uncertainty)
    require(all(type(value) is int and value >= 0 for value in values), "clock_timestamps")
    require(spawn <= sent <= received and low <= high and uncertainty == high - low,
            "clock_order")
    require(low == max(spawn, sent - elapsed) and high == received - elapsed,
            "clock_bounds")
    return {"status": "passed", "pid": pid, "nonce": nonce, "source_sha256": source,
            "spawn_before_observer_ns": spawn, "request_sent_observer_ns": sent,
            "reply_received_observer_ns": received, "origin_bounds_ns": [low, high],
            "uncertainty_ns": uncertainty}


def verify_transcript_timings(path: Path) -> dict[str, Any]:
    """Audit all Status request/response pairs without pretending hashes are payloads."""
    requests: dict[int, dict[str, Any]] = {}
    responses: dict[int, list[dict[str, Any]]] = {}
    total_lines = 0
    try:
        with path.open("rb") as stream:
            for raw in stream:
                total_lines += 1
                require(len(raw) <= MAX_TRANSCRIPT_LINE, f"transcript_line_bound:{total_lines}")
                require(raw.endswith(b"\n"), f"transcript_unterminated:{total_lines}")
                try:
                    row = json.loads(raw)
                except (UnicodeError, json.JSONDecodeError) as exc:
                    raise VerifyError(f"transcript_invalid_json:{total_lines}:{exc}") from exc
                data = row.get("data") if isinstance(row, dict) else None
                require(isinstance(data, dict), f"transcript_data:{total_lines}")
                if type(data.get("request")) is int:
                    command = data.get("command")
                    if isinstance(command, dict) and command.get("kind") == "status":
                        request_id = data["request"]
                        require(request_id not in requests, f"transcript_duplicate_status_request:{request_id}")
                        requests[request_id] = row
                if type(data.get("response")) is int:
                    responses.setdefault(data["response"], []).append(row)
    except FileNotFoundError as exc:
        raise VerifyError(f"missing:{path}") from exc
    except OSError as exc:
        raise VerifyError(f"transcript_io:{path}:{exc}") from exc

    require(len(requests) >= 723, "transcript_status_request_count")
    segments: list[dict[str, Any]] = []
    missing_timing = 0
    unavailable_reasons: set[str] = set()
    for request_id in sorted(requests):
        candidates = [row for row in responses.get(request_id, [])
                     if row.get("data", {}).get("kind") == "result"]
        require(len(candidates) == 1, f"transcript_status_response_count:{request_id}")
        response = candidates[0]["data"]
        require(response.get("result_ok") is True, f"transcript_status_result:{request_id}")
        clock = response.get("sim_clock")
        require(isinstance(clock, dict), f"transcript_status_clock_missing:{request_id}")
        process_identity = (clock.get("pid"), clock.get("instance_unix_ns"))
        require(type(process_identity[0]) is int and process_identity[0] > 0 and
                type(process_identity[1]) is int and process_identity[1] > 0,
                f"transcript_status_process_identity:{request_id}")
        host_elapsed = clock.get("elapsed_ns")
        require(type(host_elapsed) is int and host_elapsed >= 0,
                f"transcript_status_host_elapsed:{request_id}")
        if not segments or segments[-1]["process_identity"] != list(process_identity):
            segments.append({"segment": len(segments) + 1,
                             "process_identity": list(process_identity),
                             "status_count": 0,
                             "first_request_id": request_id,
                             "first_host_elapsed_ns": host_elapsed,
                             "last_request_id": request_id,
                             "last_host_elapsed_ns": host_elapsed,
                             "scope_notes": None,
                             "_first_timing": None,
                             "_last_timing": None})
        segment = segments[-1]
        segment["status_count"] += 1
        segment["last_request_id"] = request_id
        require(host_elapsed >= segment["last_host_elapsed_ns"],
                f"transcript_status_clock_regressed:{segment['segment']}:{request_id}")
        segment["last_host_elapsed_ns"] = host_elapsed
        timing = response.get("sim_host_timing")
        if not isinstance(timing, dict):
            missing_timing += 1
            unavailable_reasons.add("response_did_not_include_sim_host_timing")
            continue
        require(timing.get("schema") == "mycity-sim-timing-v1", f"timing_schema:{request_id}")
        require(isinstance(timing.get("scope_notes"), dict), f"timing_scope_notes:{request_id}")
        if segment["scope_notes"] is None:
            segment["scope_notes"] = timing["scope_notes"]
        else:
            require(timing["scope_notes"] == segment["scope_notes"],
                    f"timing_scope_notes_changed:{segment['segment']}:{request_id}")
        samples: dict[str, dict[str, int]] = {}
        for phase in TIMING_PHASES:
            sample = timing.get(phase)
            require(isinstance(sample, dict), f"timing_phase_missing:{request_id}:{phase}")
            count, total_ns, max_ns = (sample.get(key) for key in ("count", "total_ns", "max_ns"))
            require(all(type(value) is int and value >= 0 for value in (count, total_ns, max_ns)),
                    f"timing_phase_values:{request_id}:{phase}")
            require(max_ns <= total_ns and (count != 0 or (total_ns == 0 and max_ns == 0)),
                    f"timing_phase_consistency:{request_id}:{phase}")
            samples[phase] = {"count": count, "total_ns": total_ns, "max_ns": max_ns}
        if segment["_first_timing"] is None:
            segment["_first_timing"] = samples
        previous = segment["_last_timing"]
        if previous is not None:
            for phase in TIMING_PHASES:
                for metric in ("count", "total_ns", "max_ns"):
                    require(samples[phase][metric] >= previous[phase][metric],
                            f"timing_counter_regressed:{segment['segment']}:{phase}:{metric}")
        segment["_last_timing"] = samples

    for segment in segments:
        first, last = segment.pop("_first_timing"), segment.pop("_last_timing")
        if first is None or last is None:
            segment["cumulative_phase_summary"] = None
            continue
        phase_summary = {}
        for phase in TIMING_PHASES:
            phase_summary[phase] = {
                "first_status_cumulative": first[phase],
                "last_status_cumulative": last[phase],
                "between_first_and_last_status_delta": {
                    "count": last[phase]["count"] - first[phase]["count"],
                    "total_ns": last[phase]["total_ns"] - first[phase]["total_ns"],
                    "max_ns_is_not_additive": last[phase]["max_ns"],
                },
            }
        segment["cumulative_phase_summary"] = phase_summary

    require(total_lines > 0, "transcript_empty")
    return {
        "transcript_sha256": sha_file(path),
        "transcript_rows": total_lines,
        "status_request_count": len(requests),
        "status_responses_verified": len(requests),
        "process_instance_segments": segments,
        "sim_host_timing_reports_recovered": len(requests) - missing_timing,
        "sim_host_timing_status": "verified" if missing_timing == 0 else "unavailable_payload_hash_only",
        "sim_host_timing_missing_status_count": missing_timing,
        "sim_host_timing_unavailable_reasons": sorted(unavailable_reasons),
        "aggregation_rule": "group consecutive Status calls by (sim_clock.pid, sim_clock.instance_unix_ns); retain cumulative first/last counters and compute deltas only inside a process segment; never sum snapshots or cross process restarts",
        "segment_boundary_interpretation": "process instance changes delimit a new SimHost timing counter lifetime; the transcript carries no city-id label, so segment ordinals are reported without inventing a city identity",
        "state_bytes": {
            "fixed_sim_timings_bytes": 240,
            "source": "fixed-size SimTimings definition and existing source/unit contract",
            "included_in_timing_summary": False,
            "additional_budget_charge_bytes": 0,
        },
        "scope_notes_by_process_segment": {
            str(segment["segment"]): segment.get("scope_notes") for segment in segments
        },
    }


def verify_terminal_and_observer(root: Path, fixed: dict[str, Any]) -> dict[str, Any]:
    intent = read_json(root / "intent.json")
    launch = read_json(root / "launch.json")
    terminal = read_json(root / "terminal.json")
    source = fixed["source_sha256"]
    binary_sha = fixed["binary_sha256"]
    require(intent.get("schema") == "mycity-unpaced-full-timing-v128-v1" and
            intent.get("model_started") is False, "intent_identity")
    require(launch.get("source_sha256") == source and launch.get("binary_sha256") == binary_sha,
            "launch_identity")
    command = launch.get("command")
    observed = root / "observed"
    model = observed / "model"
    expected_command = [fixed["binary"],
                        "native-observe", str(observed), "--",
                        fixed["binary"],
                        "speed-run", str(model), "full", "unpaced"]
    require(command == expected_command, "launch_command")
    require(terminal.get("state") == "functional_result_validated_not_performance" and
            terminal.get("model_started") is True and terminal.get("observer_exit_code") == 0 and
            terminal.get("timed_out") is False, "terminal_functional_state")
    require(terminal.get("source_sha256") in (None, source) and
            terminal.get("binary_sha256") in (None, binary_sha), "terminal_identity")
    require(terminal.get("source_binary_identity_unchanged") is True and
            terminal.get("clock_status") == "passed" and
            terminal.get("observer_report_status") == "passed", "terminal_identity_or_clock")
    observer = read_json(observed / "observer-report.json")
    require(observer.get("schema") == "mycity-bounded-native-group-observer-v1" and
            observer.get("status") == "passed_measurement_wrapper" and
            observer.get("model_exit_code") == 0 and observer.get("binary_unchanged") is True,
            "native_observer_result")
    require(observer.get("source_sha256") == source and observer.get("binary_sha256") == binary_sha,
            "native_observer_identity")
    clock = verify_clock(root, source)
    activities = terminal.get("external_compile_or_heavy_activity")
    missing = terminal.get("external_activity_sampling_errors")
    require(isinstance(activities, list) and isinstance(missing, list), "external_activity_fields")
    return {
        "functional_wrapper": "passed",
        "observer_exit_code": terminal["observer_exit_code"],
        "timeout": terminal["timed_out"],
        "clock": clock,
        "external_activity_count": len(activities),
        "external_activity": activities,
        "external_sampling_error_count": len(missing),
        "external_sampling_errors": missing,
        "clean_measurement_environment": len(activities) == 0 and len(missing) == 0,
        "performance_acceptance": "not_claimed",
    }


def verify_business(repo: Path, root: Path, fixed: dict[str, Any]) -> dict[str, Any]:
    model = root / "observed" / "model"
    source = fixed["source_sha256"]
    business = model / "business"
    manifest = read_json(model / "manifest.json")
    supervisor = read_json(model / "report.json")
    plan = read_json(business / "speed-manifest.json")
    speed = read_json(business / "report.json")
    for label, row in (("manifest", manifest), ("supervisor", supervisor),
                       ("plan", plan), ("speed_report", speed)):
        require(row.get("source_sha256") == source and row.get("tier") == "full",
                f"{label}_identity")
        require(row.get("rate") == "unpaced", f"{label}_not_unpaced")
    require(manifest.get("schema") == "mycity-speed-supervisor-v1" and
            supervisor.get("schema") == "mycity-speed-supervisor-v1" and
            plan.get("schema") == "mycity-speed-plan-v1" and
            speed.get("schema") == "mycity-speed-driver-v1", "speed_schema")
    require(supervisor.get("experiment_completed") is True, "supervisor_incomplete")
    require(speed.get("completed_hours") == 720 and speed.get("verified_targets") == 723 and
            speed.get("worker_windows") == 724, "full_trajectory_counts")
    require(speed.get("entire_engine_including_metadata") ==
            "equal_independent_normal_reference", "engine_cpu_reference_comparison")
    require(speed.get("profile") == "L2_full_actual", "full_tier_profile")
    require(speed.get("L1") == "absent" and speed.get("real_16GB") == "not_run" and
            speed.get("eight_hour") == "not_run" and speed.get("full_M0") == "not_accepted",
            "scope_misclaim")
    require(speed.get("target_day_wall_seconds") is None and
            speed.get("full_profile_unpaced_target") in ("passed_this_run", "failed_this_run"),
            "unpaced_misclassified_as_fixed_rate")

    hashes = fixed["fixtures"]
    reference = check_task(business, "reference", "reference", source, hashes,
                           ["cpu-targets.jsonl", "cpu-report.json", "cpu-final-engine.json"])
    require(reference.get("image_sha256") is None and reference.get("business_sha256") is None and
            reference.get("root") is None, "reference_result_shape")
    cpu = read_json(business / "reference" / "cpu-report.json")
    opening = read_json(repo / "fixtures/m0-t13a-full-opening-candidate.json")
    policy = read_json(repo / "fixtures/m0-t13a-real-policy-candidate.json")
    oracle = read_json(repo / "fixtures/m0-t13a-expected-oracle-candidate.json")
    expected_counts = {"accounts": 64, "stocks": 128, "payroll": 64, "claims": 20000,
                       "cities": 8, "quotes": 8, "inflight_peak": 4}
    require(cpu.get("schema") == "mycity-scale-reference-v1" and cpu.get("tier") == "full" and
            cpu.get("source_sha256") == source and cpu.get("counts") == expected_counts,
            "cpu_reference_identity_or_counts")
    for key, digest in hashes.items():
        require(cpu.get(key) == digest, f"cpu_reference_{key}")
    require(len(opening.get("accounts", [])) == 64 and len(opening.get("stocks", [])) == 128 and
            len(opening.get("sources", [])) == 192, "opening_basis_counts")
    payroll, claims = policy.get("payroll"), policy.get("claims")
    require(isinstance(payroll, list) and len(payroll) == 64 and isinstance(claims, list) and
            len(claims) == 20 and sum(item.get("count", 0) for item in claims) == 20000,
            "policy_basis_counts")
    require(oracle.get("expected_work_seconds_each") == 29 * 8 * 3600 and
            oracle.get("expected_payroll_paid") == [row["hourly_cents"] * 232 for row in payroll],
            "payroll_oracle_vector")
    require(cpu.get("completed_hour_targets") == 720 and
            cpu.get("wages_paid_total") == oracle.get("expected_wages_paid_total") == 118784 and
            cpu.get("claims_paid_total") == oracle.get("expected_claims_paid_total") == 20000,
            "wages_claims_oracle")
    require(cpu.get("cash_accounts") == oracle.get("expected_cash_accounts") and
            sum(row.get("cash", 0) for row in cpu.get("cash_accounts", [])) ==
            oracle.get("expected_cash_total") == 3000000, "cash_oracle")
    deliveries = cpu.get("delivery_contracts")
    require(isinstance(deliveries, list) and len(deliveries) == 8, "delivery_count")
    for index, item in enumerate(deliveries):
        fragment = item.get("source_fragment") or {}
        require(item.get("spec", {}).get("id") == index + 1 and
                item.get("status") == "Delivered" and item.get("delivered") == 1000 and
                item.get("invoice") == 2000 and item.get("freight") == 20 and
                item.get("source_cost") == 400 and item.get("loss_cost") == 0,
                f"delivery_terms:{index + 1}")
        require(fragment.get("origin_quantity") == 20000 and fragment.get("origin_basis") == 8000 and
                fragment.get("offset") == index * 1000 and fragment.get("quantity") == 1000 and
                fragment.get("basis") == 400, f"delivery_basis:{index + 1}")
    require(sum(item.get("invoice", 0) for item in deliveries) ==
            oracle.get("expected_freight_invoices") and
            sum(item.get("freight", 0) for item in deliveries) ==
            oracle.get("expected_freight_fees"), "delivery_totals")
    obligations = cpu.get("periodic_obligations")
    require(isinstance(obligations, list) and len(obligations) == 16 and
            all(item.get("arrears") == 0 and item.get("next_due") is None for item in obligations),
            "obligations")
    inbox = cpu.get("inbox", {})
    require(inbox.get("accrued") == oracle.get("expected_inbox_fees") == 40 and
            inbox.get("paid") == 40 and inbox.get("closed") == 8, "inbox_oracle")
    require(cpu.get("managed_stock_records") == 14 and cpu.get("extra_stock_records") == 114 and
            cpu.get("extra_stock_nonzero_records") == 1, "stock_scope")
    ref_business_hash = cpu.get("business_sha256")
    require(isinstance(ref_business_hash, str) and re.fullmatch(r"[0-9a-f]{64}", ref_business_hash),
            "reference_business_hash")

    before = check_task(business, "before-reopen", "before_reopen", source, hashes,
                        ["root.json", "workspace.json"])
    compare = check_task(business, "compare-speed", "compare_speed", source, hashes,
                         ["root.json", "workspace.json"])
    image_hash = compare.get("image_sha256")
    business_hash = compare.get("business_sha256")
    require(isinstance(image_hash, str) and re.fullmatch(r"[0-9a-f]{64}", image_hash) and
            business_hash == ref_business_hash, "full_state_compare")
    require(before.get("image_sha256") == image_hash and speed.get("image_sha256") == image_hash,
            "cold_checkpoint_image_equivalence")
    require(speed.get("cold_ready_seconds") is not None, "cold_reopen_missing")
    transcript = verify_transcript_timings(business / "transcript.jsonl")
    return {
        "functional_model": "passed",
        "full_tier_counts": expected_counts,
        "hours": 720,
        "verified_targets": 723,
        "worker_windows": 724,
        "wages_paid_cents": cpu["wages_paid_total"],
        "claims_paid_cents": cpu["claims_paid_total"],
        "delivery_contracts": len(deliveries),
        "periodic_obligations": len(obligations),
        "cash_total_cents": oracle["expected_cash_total"],
        "full_business_sha256": business_hash,
        "full_engine_image_sha256": image_hash,
        "cold_image_unchanged": True,
        "unpaced_report_speed_target": speed.get("speed_target_met"),
        "unpaced_is_not_1s_or_10s_acceptance": True,
        "timing_transcript_audit": transcript,
        "performance_acceptance": "not_claimed",
        "L1": "absent",
        "eight_hour": "not_run",
        "real_16GB": "not_run",
        "full_M0": "not_accepted",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repo", type=Path, help="canonical MyCity checkout containing fixtures/")
    parser.add_argument("evidence_root", type=Path,
                        help="explicit completed diagnostic root; never auto-selects an attempt")
    parser.add_argument("--descriptor-root", type=Path, default=BASE,
                        help="directory containing the matching prepared-manifest.json and runner")
    parser.add_argument("--output", type=Path, help="optional JSON output; default stdout")
    args = parser.parse_args()
    result: dict[str, Any] = {
        "schema": "mycity-full-unpaced-profile-verification-v1",
        "scope": "full-tier unpaced functional and source-segment diagnostic only",
        "functional": "not_run",
        "timing_profile": "not_run",
        "external_activity_status": "not_run",
        "performance_acceptance": "not_claimed",
        "L1": "absent",
        "eight_hour": "not_run",
        "real_16GB": "not_run",
        "full_M0": "not_accepted",
        "retained_excluded_attempt": {
            "root": str(BASE.parent / "timing-full-v128-01"),
            "classification": "root-abort-prelaunch",
            "model_started": False,
            "included_in_functional_or_timing_results": False,
        },
        "status": "failed",
        "errors": [],
    }
    try:
        repo = args.repo.resolve(strict=True)
        evidence_root = args.evidence_root.resolve(strict=True)
        descriptor_root = args.descriptor_root.resolve(strict=True)
        fixed = verify_fixed_inputs(repo, descriptor_root)
        require(fixed.get("declared_run_root") == str(evidence_root), "evidence_root_descriptor_mismatch")
        result["fixed_inputs"] = fixed
        wrapper = verify_terminal_and_observer(evidence_root, fixed)
        result["wrapper"] = wrapper
        business = verify_business(repo, evidence_root, fixed)
        result["business"] = business
        result["functional"] = "passed"
        result["timing_profile"] = business["timing_transcript_audit"]["sim_host_timing_status"]
        result["external_activity_status"] = (
            "clean_observed" if wrapper["clean_measurement_environment"] else "polluted_or_sampling_incomplete"
        )
        result["status"] = ("functional_verified_timing_unavailable"
                            if result["timing_profile"] != "verified" else "evidence_complete")
    except (VerifyError, OSError, TypeError, ValueError, AttributeError, KeyError) as exc:
        result["errors"].append(str(exc))
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    else:
        sys.stdout.write(rendered)
    return 0 if result["status"] == "evidence_complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())
