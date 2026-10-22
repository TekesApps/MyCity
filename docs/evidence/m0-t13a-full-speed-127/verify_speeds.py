#!/usr/bin/env python3
"""Independent verifier for the two fixed standalone full-speed runs.

Usage:
  python3 verify_speeds.py REPO_PATH [VERIFICATION_JSON]

It reads only frozen source inputs and completed run evidence. It never launches
MyCity, Cargo, or a benchmark. The JSON report separates execution-wrapper,
functional-model, and standalone speed-target outcomes. Missing evidence fails.
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
POOL = BASE.parent / "pool-control-current-01"
FIXED_SOURCE = "ac02e7ccb7e2b9e842e5f8dd15903b14f7fff9b9426909449b36b33bbd08db6c"
FIXED_BINARY = "557b2e4d0bc89f429a07a270c9f388b508d595c8b4a1f233b795becdf75797e7"
FIXED_ARCHIVE = "8e85f7adc3dd0d0811ec8ee6fa89720512f8ba6f587b1b10de6146cfc3d81561"
FIXED_LOCK = "7da5c54a8499a07cbc71dcecce0c0095d4af3b8b6e5052783f20d188037b6dd6"
FIXED_ACCEPTANCE = "876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508"
BUILD_ROOT_FILES = {".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
TIER_FIXTURES = {
    "opening_sha256": "fixtures/m0-t13a-full-opening-candidate.json",
    "policy_sha256": "fixtures/m0-t13a-real-policy-candidate.json",
    "oracle_sha256": "fixtures/m0-t13a-expected-oracle-candidate.json",
}
RUNS = {
    "1": ("rate1-01", "speed01"),
    "10": ("rate10-02", "speed02"),
}
RETAINED_EXCLUDED_ATTEMPTS = [{
    "rate_seconds_per_game_day": 10,
    "attempt": "rate10-01",
    "run_id": "speed01",
    "classification": "controller_failure_before_model_start",
    "evidence": "controller-terminal.json",
    "included_in_functional_or_speed_results": False,
}]


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


def digest_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def digest_file(path: Path) -> str:
    h = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                h.update(block)
    except FileNotFoundError as exc:
        raise VerifyError(f"missing:{path}") from exc
    return h.hexdigest()


def require_hash(value: Any, expected: str, name: str) -> None:
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None,
            f"invalid_sha256:{name}")
    require(value == expected, f"sha256_mismatch:{name}")


def source_stream_sha256(entries: list[tuple[str, bytes]]) -> str:
    digest = hashlib.sha256()
    for name, data in sorted(entries, key=lambda row: row[0]):
        encoded = name.encode("utf-8")
        digest.update(struct.pack("<Q", len(encoded)))
        digest.update(encoded)
        digest.update(struct.pack("<Q", len(data)))
        digest.update(data)
    return digest.hexdigest()


def verify_fixed_inputs(repo: Path) -> dict[str, Any]:
    record_path = POOL / "binary-final.json"
    record = read_json(record_path)
    require_hash(record.get("source_sha256"), FIXED_SOURCE, "binary-final.source")
    require_hash(record.get("binary_sha256"), FIXED_BINARY, "binary-final.binary")
    require_hash(record.get("source_zip_sha256"), FIXED_ARCHIVE, "binary-final.source_zip")
    binary = Path(record.get("binary", "")).resolve(strict=True)
    expected_binary = (POOL / "executables" / FIXED_BINARY / "mycity-headless").resolve()
    require(binary == expected_binary, "binary_path_not_fixed")
    require_hash(digest_file(binary), FIXED_BINARY, "executable")
    archive_path = POOL / "09-fixed-cli-build" / "source.zip"
    require(Path(record.get("source_zip", "")).resolve() == archive_path.resolve(),
            "source_zip_path_not_fixed")
    require_hash(digest_file(archive_path), FIXED_ARCHIVE, "source_zip")
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
                     if name in BUILD_ROOT_FILES or name.startswith("crates/") or name.startswith("fixtures/")]
    require(len(names) == 264 and len(build_entries) == 259, "source_zip_entry_counts")
    require(source_stream_sha256(build_entries) == FIXED_SOURCE, "source_stream_hash")
    require_hash(digest_bytes(frozen["Cargo.lock"]), FIXED_LOCK, "Cargo.lock")
    require_hash(digest_bytes(frozen["fixtures/m0-t13a-acceptance.json"]),
                 FIXED_ACCEPTANCE, "acceptance_fixture")
    require(FIXED_SOURCE.encode("ascii") in binary.read_bytes(), "embedded_source_fingerprint")

    fixture_hashes: dict[str, str] = {}
    for field, name in TIER_FIXTURES.items():
        data = frozen.get(name)
        require(data is not None, f"frozen_fixture_missing:{name}")
        current = repo / name
        try:
            current_bytes = current.read_bytes()
        except OSError as exc:
            raise VerifyError(f"repo_fixture_unavailable:{current}:{exc}") from exc
        require(current_bytes == data, f"repo_fixture_differs_from_archive:{name}")
        fixture_hashes[field] = digest_bytes(data)
    return {"source_sha256": FIXED_SOURCE, "binary": str(binary),
            "binary_sha256": FIXED_BINARY, "source_zip": str(archive_path),
            "source_zip_sha256": FIXED_ARCHIVE, "build_source_entries": len(build_entries),
            "cargo_lock_sha256": FIXED_LOCK, "acceptance_fixture_sha256": FIXED_ACCEPTANCE,
            "fixtures": fixture_hashes,
            "current_checkout_is_provenance_only": True}


def check_task(business: Path, directory: str, mode: str, fixture_hashes: dict[str, str],
               artifact_names: list[str]) -> dict[str, Any]:
    task = business / directory
    request_path = task / "request.json"
    request = read_json(request_path)
    result = read_json(task / "result.json")
    require(request.get("schema") == "mycity-model-task-request-v2", f"request_schema:{directory}")
    require(request.get("tier") == "full" and request.get("mode") == mode,
            f"request_identity:{directory}")
    require(request.get("source_sha256") == FIXED_SOURCE, f"request_source:{directory}")
    for name, expected in fixture_hashes.items():
        require(request.get(name) == expected, f"request_{name}:{directory}")
    require(request.get("root") == str(business.resolve()).encode("utf-8").hex(),
            f"request_root:{directory}")
    require(result.get("schema") == "mycity-model-task-result-v2", f"result_schema:{directory}")
    require(result.get("source_sha256") == FIXED_SOURCE and result.get("tier") == "full"
            and result.get("mode") == mode, f"result_identity:{directory}")
    require(result.get("request_sha256") == digest_file(request_path), f"request_digest:{directory}")
    require(type(result.get("pid")) is int and result["pid"] > 0, f"task_pid:{directory}")
    require(type(result.get("instance_unix_ns")) is int and result["instance_unix_ns"] > 0,
            f"task_instance:{directory}")
    artifacts = result.get("artifacts")
    require(isinstance(artifacts, list) and len(artifacts) == 3, f"artifact_slots:{directory}")
    for index, name in enumerate(artifact_names):
        item = artifacts[index]
        require(isinstance(item, dict) and item.get("name") == name, f"artifact_name:{directory}:{name}")
        content = (task / name).read_bytes()
        require(item.get("bytes") == len(content) and item.get("sha256") == digest_bytes(content),
                f"artifact_hash:{directory}:{name}")
    for index in range(len(artifact_names), 3):
        require(artifacts[index] is None, f"unexpected_artifact:{directory}:{index}")
    if mode != "reference":
        stamp = result.get("root")
        require(isinstance(stamp, dict), f"root_stamp:{directory}")
        root_bytes = (task / "root.json").read_bytes()
        root = json.loads(root_bytes)
        require(stamp.get("bytes") == len(root_bytes) and stamp.get("sha256") == digest_bytes(root_bytes)
                and stamp.get("sequence") == root.get("sequence"), f"root_asset:{directory}")
    return result


def verify_handshake(observed: Path) -> dict[str, Any]:
    model = observed / "model"
    clock = read_json(observed / "observer-clock-map.json")
    child = read_json(observed / "observer-child.json")
    ready = read_json(model / "observer-clock-ready.json")
    request = read_json(model / "observer-clock-request.json")
    reply = read_json(model / "observer-clock-reply.json")
    identity = child.get("identity", {})
    pid, nonce = clock.get("supervisor_pid"), clock.get("nonce")
    require(clock.get("source_sha256") == FIXED_SOURCE, "clock_map_source")
    require(type(pid) is int and pid > 0 and identity.get("pid") == pid, "clock_pid")
    require(isinstance(nonce, str) and re.fullmatch(r"[0-9a-f]{32}", nonce) is not None,
            "clock_nonce")
    require(ready.get("schema") == "mycity-observer-clock-ready-v1" and
            request.get("schema") == "mycity-observer-clock-request-v1" and
            reply.get("schema") == "mycity-observer-clock-reply-v1", "clock_schema")
    for row in (ready, request, reply):
        require(row.get("pid") == pid and row.get("source_sha256") == FIXED_SOURCE,
                "clock_message_identity")
    require(request.get("nonce") == reply.get("nonce") == nonce, "clock_nonce_pair")
    spawn, sent, received = (clock.get(k) for k in
                             ("spawn_before_observer_ns", "request_sent_observer_ns",
                              "reply_received_observer_ns"))
    elapsed = reply.get("elapsed_ns")
    low, high, uncertainty = (clock.get(k) for k in
                              ("supervisor_origin_observer_ns_min", "supervisor_origin_observer_ns_max",
                               "uncertainty_ns"))
    values = (spawn, sent, received, elapsed, low, high, uncertainty)
    require(all(type(value) is int and value >= 0 for value in values), "clock_timestamps")
    require(spawn <= sent <= received and low <= high and uncertainty == high - low,
            "clock_order")
    require(low == max(spawn, sent - elapsed) and high == received - elapsed,
            "clock_bounds")
    return {"pid": pid, "nonce": nonce, "source_sha256": FIXED_SOURCE,
            "spawn_before_observer_ns": spawn, "request_sent_observer_ns": sent,
            "reply_received_observer_ns": received, "origin_min_ns": low,
            "origin_max_ns": high, "uncertainty_ns": uncertainty}


def verify_observer_and_wrapper(attempt: Path, rate: str, run_id: str,
                                binary: Path) -> dict[str, Any]:
    intent = read_json(attempt / "intent.json")
    receipt = read_json(attempt / "receipt.json")
    admission = read_json(attempt / "admission.json")
    live = read_json(attempt / "live.json")
    observed = attempt / "observed"
    report = read_json(observed / "observer-report.json")
    require(intent.get("schema") == "mycity-standalone-full-speed-receipt-v1" and
            receipt.get("schema") == intent.get("schema"), "wrapper_receipt_schema")
    for record in (intent, receipt, live):
        require(record.get("source_sha256") == FIXED_SOURCE and
                record.get("binary_sha256") == FIXED_BINARY, "wrapper_fixed_identity")
        require(record.get("rate_seconds_per_game_day") == int(rate) and
                record.get("run_id") == run_id, "wrapper_rate_identity")
    require(admission.get("admission", {}).get("status") == "passed" and
            admission.get("readiness", {}).get("exit_code") == 0, "admission_not_passed")
    model = observed / "model"
    command = [str(binary), "native-observe", str(observed), "--", str(binary),
               "speed-run", str(model), "full", rate]
    require(live.get("command") == command and receipt.get("command") == command,
            "wrapper_command_mismatch")
    require(receipt.get("state") == "observer_exited" and receipt.get("model_started") is True and
            receipt.get("timed_out") is False and receipt.get("observer_exit_code") == 0,
            "execution_wrapper_not_passed")
    require(receipt.get("clock_handshake", {}).get("status") == "passed" and
            receipt.get("observer_result_validation", {}).get("status") == "passed",
            "execution_receipts_not_passed")
    require(report.get("schema") == "mycity-bounded-native-group-observer-v1" and
            report.get("status") == "passed_measurement_wrapper" and
            report.get("model_exit_code") == 0 and report.get("binary_unchanged") is True,
            "native_observer_not_passed")
    require(report.get("source_sha256") == FIXED_SOURCE and
            report.get("binary_sha256") == FIXED_BINARY, "native_observer_identity")
    clock = verify_handshake(observed)
    external_activity = receipt.get("external_compile_or_heavy_mycity_observations")
    missing_activity = receipt.get("external_activity_sample_missing")
    require(isinstance(external_activity, list), "external_activity_observations_missing")
    require(isinstance(missing_activity, list), "external_activity_missing_samples_missing")
    clean_environment = len(external_activity) == 0 and len(missing_activity) == 0
    return {"status": "passed", "controller_process_exit_not_used_as_proof": True,
            "receipt_state": receipt["state"], "observer_exit_code": receipt["observer_exit_code"],
            "observer_report_status": report["status"], "clock_handshake": clock,
            "environment_integrity": {
                "external_compile_or_heavy_mycity_observations_count": len(external_activity),
                "external_compile_or_heavy_mycity_observations": external_activity,
                "external_activity_elapsed_seconds": [
                    row.get("elapsed_seconds") for row in external_activity
                    if isinstance(row, dict)
                ],
                "external_activity_sample_missing_count": len(missing_activity),
                "external_activity_sample_missing": missing_activity,
                "clean_measurement_environment": clean_environment,
            }}


def close(a: Any, b: float, label: str) -> None:
    require(isinstance(a, (int, float)) and math.isfinite(float(a)) and
            abs(float(a) - b) <= 1e-6, f"numeric_mismatch:{label}")


def verify_speed_budget(report: dict[str, Any], rate: str) -> dict[str, Any]:
    wall_per_day = int(rate)
    require(report.get("tier") == "full" and report.get("rate") == rate, "speed_report_identity")
    require(report.get("target_day_wall_seconds") == wall_per_day, "speed_rate_budget")
    close(report.get("evaluation_day_budget_seconds"), wall_per_day, "daily_budget")
    days = report.get("days")
    require(isinstance(days, list) and len(days) == 30, "speed_day_rows")
    prior_completion = 0.0
    computed_day_pass = True
    for index, row in enumerate(days, 1):
        require(row.get("day") == index, f"speed_day_order:{index}")
        completion = row.get("business_durable_seconds")
        require(isinstance(completion, (int, float)) and math.isfinite(float(completion)),
                f"speed_day_completion:{index}")
        fixed_start = (index - 1) * wall_per_day
        close(row.get("fixed_period_start_seconds"), fixed_start, f"fixed_period_start:{index}")
        expected_fixed_period = float(completion) - fixed_start
        expected_adjacent = float(completion) - prior_completion
        close(row.get("fixed_period_to_business_seconds"), expected_fixed_period,
              f"fixed_period_duration:{index}")
        close(row.get("adjacent_business_completion_seconds"), expected_adjacent,
              f"adjacent_duration:{index}")
        day_pass = expected_fixed_period <= wall_per_day
        require(row.get("budget_met") is day_pass, f"day_budget_boolean:{index}")
        computed_day_pass = computed_day_pass and day_pass
        prior_completion = float(completion)
    operating = report.get("operating_wall_seconds")
    require(isinstance(operating, (int, float)) and math.isfinite(float(operating)),
            "operating_duration")
    close(report.get("final_day_with_checkpoint_seconds"),
          float(operating) - 29 * wall_per_day, "final_day_with_checkpoint")
    final_day_pass = float(report["final_day_with_checkpoint_seconds"]) <= wall_per_day
    all_daily_pass = computed_day_pass and final_day_pass
    total_pass = float(operating) <= wall_per_day * 30
    target_pass = all_daily_pass and total_pass
    require(report.get("all_daily_budgets_met") is all_daily_pass, "all_daily_budget_summary")
    require(report.get("whole_30day_budget_met") is total_pass, "whole_month_budget_summary")
    require(report.get("speed_target_met") is target_pass, "speed_target_summary")
    return {"rate_seconds_per_game_day": wall_per_day,
            "nominal_30_day_budget_seconds": wall_per_day * 30,
            "measured_operating_wall_seconds": operating,
            "all_30_daily_periods_met": all_daily_pass,
            "whole_30day_budget_met": total_pass,
            "standalone_speed_target": "met" if target_pass else "missed"}


def verify_run(repo: Path, rate: str, run_id: str, attempt_name: str,
               fixed: dict[str, Any]) -> dict[str, Any]:
    attempt = BASE / attempt_name
    observed = attempt / "observed"
    model = observed / "model"
    business = model / "business"
    wrapper = verify_observer_and_wrapper(attempt, rate, run_id, Path(fixed["binary"]))
    manifest = read_json(model / "manifest.json")
    supervisor_report = read_json(model / "report.json")
    plan = read_json(business / "speed-manifest.json")
    speed_report = read_json(business / "report.json")
    for name, row in (("supervisor_manifest", manifest), ("supervisor_report", supervisor_report),
                      ("speed_plan", plan), ("speed_report", speed_report)):
        require(row.get("source_sha256") == FIXED_SOURCE and row.get("tier") == "full" and
                row.get("rate") == rate, f"{name}_identity:{rate}")
    require(manifest.get("schema") == "mycity-speed-supervisor-v1" and
            supervisor_report.get("schema") == "mycity-speed-supervisor-v1" and
            plan.get("schema") == "mycity-speed-plan-v1" and
            speed_report.get("schema") == "mycity-speed-driver-v1", f"speed_report_schemas:{rate}")
    require(supervisor_report.get("experiment_completed") is True, f"supervisor_completion:{rate}")
    require(speed_report.get("completed_hours") == 720 and
            speed_report.get("verified_targets") == 723 and
            speed_report.get("worker_windows") == 724, f"trajectory_counts:{rate}")
    require(speed_report.get("entire_engine_including_metadata") ==
            "equal_independent_normal_reference", f"engine_reference_claim:{rate}")
    require(speed_report.get("profile") == "L2_full_actual", f"full_profile_label:{rate}")
    for field in ("full_profile_60_actual_operating_target", "full_profile_unpaced_target"):
        require(speed_report.get(field) == "not_evaluated_other_tier_or_rate",
                f"unrelated_speed_gate:{rate}:{field}")
    require(speed_report.get("L1") == "absent" and speed_report.get("real_16GB") == "not_run" and
            speed_report.get("eight_hour") == "not_run" and
            speed_report.get("full_M0") == "not_accepted", f"scope_misclaim:{rate}")

    hashes = {name: digest_file(repo / path) for name, path in TIER_FIXTURES.items()}
    ref_result = check_task(business, "reference", "reference", hashes,
                            ["cpu-targets.jsonl", "cpu-report.json", "cpu-final-engine.json"])
    require(ref_result.get("image_sha256") is None and ref_result.get("business_sha256") is None and
            ref_result.get("root") is None, f"reference_result_shape:{rate}")
    cpu = read_json(business / "reference" / "cpu-report.json")
    require(cpu.get("schema") == "mycity-scale-reference-v1" and
            cpu.get("tier") == "full" and cpu.get("source_sha256") == FIXED_SOURCE,
            f"cpu_reference_identity:{rate}")
    for name, expected in hashes.items():
        require(cpu.get(name) == expected, f"cpu_fixture_hash:{rate}:{name}")
    oracle = read_json(repo / "fixtures/m0-t13a-expected-oracle-candidate.json")
    policy = read_json(repo / "fixtures/m0-t13a-real-policy-candidate.json")
    opening = read_json(repo / "fixtures/m0-t13a-full-opening-candidate.json")
    expected_counts = {"accounts": 64, "stocks": 128, "payroll": 64, "claims": 20000,
                       "cities": 8, "quotes": 8, "inflight_peak": 4}
    require(cpu.get("counts") == expected_counts, f"cpu_counts:{rate}")
    require(len(opening.get("accounts", [])) == 64 and len(opening.get("stocks", [])) == 128 and
            len(opening.get("sources", [])) == 192, f"opening_counts:{rate}")
    payroll = policy.get("payroll")
    claims = policy.get("claims")
    require(isinstance(payroll, list) and len(payroll) == 64 and
            isinstance(claims, list) and len(claims) == 20 and
            sum(row.get("count", 0) for row in claims) == 20000, f"policy_counts:{rate}")
    require(oracle.get("expected_work_seconds_each") == 29 * 8 * 3600 and
            len(oracle.get("expected_payroll_paid", [])) == 64 and
            oracle.get("expected_payroll_paid") ==
            [row["hourly_cents"] * 232 for row in payroll], f"wage_oracle_vector:{rate}")
    require(cpu.get("completed_hour_targets") == 720, f"cpu_hours:{rate}")
    require(cpu.get("wages_paid_total") == oracle.get("expected_wages_paid_total") == 118784,
            f"wage_oracle_total:{rate}")
    require(cpu.get("claims_paid_total") == oracle.get("expected_claims_paid_total") == 20000,
            f"claim_oracle_total:{rate}")
    require(cpu.get("cash_accounts") == oracle.get("expected_cash_accounts") and
            sum(row.get("cash", 0) for row in cpu.get("cash_accounts", [])) ==
            oracle.get("expected_cash_total") == 3000000, f"cash_oracle:{rate}")
    deliveries = cpu.get("delivery_contracts")
    require(isinstance(deliveries, list) and len(deliveries) == 8, f"delivery_count:{rate}")
    for index, item in enumerate(deliveries):
        fragment = item.get("source_fragment") or {}
        require(item.get("spec", {}).get("id") == index + 1 and item.get("status") == "Delivered" and
                item.get("delivered") == 1000 and item.get("invoice") == 2000 and
                item.get("freight") == 20 and item.get("source_cost") == 400 and
                item.get("loss_cost") == 0, f"delivery_terms:{rate}:{index + 1}")
        require(fragment.get("origin_quantity") == 20000 and fragment.get("origin_basis") == 8000 and
                fragment.get("offset") == index * 1000 and fragment.get("quantity") == 1000 and
                fragment.get("basis") == 400, f"delivery_basis:{rate}:{index + 1}")
    require(sum(item.get("invoice", 0) for item in deliveries) == oracle.get("expected_freight_invoices") and
            sum(item.get("freight", 0) for item in deliveries) == oracle.get("expected_freight_fees"),
            f"delivery_totals:{rate}")
    obligations = cpu.get("periodic_obligations")
    require(isinstance(obligations, list) and len(obligations) == 16 and
            all(item.get("arrears") == 0 and item.get("next_due") is None for item in obligations),
            f"obligations_settled:{rate}")
    inbox = cpu.get("inbox", {})
    require(inbox.get("accrued") == oracle.get("expected_inbox_fees") == 40 and
            inbox.get("paid") == 40 and inbox.get("closed") == 8, f"inbox_oracle:{rate}")
    require(cpu.get("managed_stock_records") == 14 and cpu.get("extra_stock_records") == 114 and
            cpu.get("extra_stock_nonzero_records") == 1, f"stock_scope:{rate}")
    reference_business_hash = cpu.get("business_sha256")
    require(isinstance(reference_business_hash, str) and
            re.fullmatch(r"[0-9a-f]{64}", reference_business_hash) is not None,
            f"reference_business_hash:{rate}")

    before = check_task(business, "before-reopen", "before_reopen", hashes,
                        ["root.json", "workspace.json"])
    compare = check_task(business, "compare-speed", "compare_speed", hashes,
                         ["root.json", "workspace.json"])
    image_hash = compare.get("image_sha256")
    business_hash = compare.get("business_sha256")
    require(isinstance(image_hash, str) and re.fullmatch(r"[0-9a-f]{64}", image_hash) is not None and
            isinstance(business_hash, str) and business_hash == reference_business_hash,
            f"full_state_compare:{rate}")
    require(before.get("image_sha256") == image_hash and speed_report.get("image_sha256") == image_hash,
            f"cold_image_equivalence:{rate}")
    require(speed_report.get("cold_ready_seconds") is not None, f"cold_reopen_missing:{rate}")
    speed_goal = verify_speed_budget(speed_report, rate)
    return {"rate_seconds_per_game_day": int(rate),
            "execution_wrapper": wrapper,
            "functional_model": "passed",
            "model_counts": expected_counts,
            "wages_paid_cents": cpu["wages_paid_total"],
            "claims_paid_cents": cpu["claims_paid_total"],
            "delivery_contracts": len(deliveries),
            "cash_total_cents": oracle["expected_cash_total"],
            "business_sha256": business_hash,
            "full_image_sha256": image_hash,
            "speed": speed_goal,
            "clean_measurement_environment": wrapper["environment_integrity"][
                "clean_measurement_environment"],
            "L1": "absent", "16GB": "not_run", "eight_hour": "not_run"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repo", type=Path, help="canonical MyCity checkout containing fixtures/")
    parser.add_argument("output", nargs="?", type=Path,
                        help="optional verification JSON path; default stdout")
    args = parser.parse_args()
    result: dict[str, Any] = {
        "schema": "mycity-standalone-full-speed-verification-v1",
        "scope": "two_fixed_rate_full_30day_runs",
        "functional": "not_run", "execution_wrapper": "not_run",
        "standalone_speed_targets": {}, "original_joint_M0": "not_accepted",
        "L1": "absent", "16GB": "not_run", "eight_hour": "not_run",
        "clean_speed_acceptance": "not_accepted",
        "clean_speed_acceptance_reason": "awaiting_complete_evidence",
        "retained_excluded_attempts": RETAINED_EXCLUDED_ATTEMPTS,
        "performance_acceptance": "observed rate-target results are reported separately; clean speed acceptance requires both runs to meet their rate targets with zero external heavy-work observations and zero missing activity samples; no hardware/resource gate claimed",
        "status": "failed", "runs": [], "errors": [],
    }
    try:
        repo = args.repo.resolve(strict=True)
        fixed = verify_fixed_inputs(repo)
        result["fixed_inputs"] = fixed
        for rate, (attempt_name, run_id) in RUNS.items():
            row = verify_run(repo, rate, run_id, attempt_name, fixed)
            result["runs"].append(row)
            result["standalone_speed_targets"][rate] = row["speed"]["standalone_speed_target"]
        result["functional"] = "passed_both_rates"
        result["execution_wrapper"] = "passed_both_rates"
        all_clean = all(row["clean_measurement_environment"] for row in result["runs"])
        all_speed_met = all(row["speed"]["standalone_speed_target"] == "met"
                            for row in result["runs"])
        if all_clean and all_speed_met:
            result["clean_speed_acceptance"] = "passed"
            result["clean_speed_acceptance_reason"] = "both_runs_clean_and_rate_targets_met"
        else:
            result["clean_speed_acceptance"] = "not_accepted"
            reasons = []
            if not all_clean:
                reasons.append("external_activity_or_missing_samples")
            if not all_speed_met:
                reasons.append("one_or_more_standalone_rate_targets_missed")
            result["clean_speed_acceptance_reason"] = reasons
        result["status"] = "evidence_complete"
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
