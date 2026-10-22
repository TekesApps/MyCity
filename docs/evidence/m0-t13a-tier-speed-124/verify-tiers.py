#!/usr/bin/env python3
"""Independent functional verifier for observed small/medium/full speed runs.

Usage:
  python3 verify-tiers.py REPO_PATH EVIDENCE_ROOT [verification.json]

This checks functional identity, frozen fixture lineage, completed trajectory,
and cold-compare artifacts. It does not establish speed, memory, L1, 16-GiB,
or eight-hour acceptance.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import zipfile
from pathlib import Path
from typing import Any


class VerifyError(Exception):
    pass


def require(ok: bool, message: str) -> None:
    if not ok:
        raise VerifyError(message)


def json_file(path: Path) -> Any:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
        require(isinstance(value, dict), f"json_object_expected:{path}")
        return value
    except FileNotFoundError as exc:
        raise VerifyError(f"missing_file:{path}") from exc
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise VerifyError(f"invalid_json:{path}:{exc}") from exc


def sha_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha_file(path: Path) -> str:
    h = hashlib.sha256()
    try:
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                h.update(block)
    except FileNotFoundError as exc:
        raise VerifyError(f"missing_file:{path}") from exc
    return h.hexdigest()


def hex64(value: Any, label: str) -> str:
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None,
            f"invalid_sha256:{label}")
    return value


def check_artifacts(task_dir: Path, result: dict[str, Any], expected_names: list[str]) -> None:
    artifacts = result.get("artifacts")
    require(isinstance(artifacts, list) and len(artifacts) == 3, f"artifact_list:{task_dir}")
    for index, expected in enumerate(expected_names):
        item = artifacts[index]
        require(isinstance(item, dict), f"artifact_missing:{task_dir}:{expected}")
        require(item.get("name") == expected, f"artifact_name:{task_dir}:{expected}")
        path = task_dir / expected
        try:
            data = path.read_bytes()
        except FileNotFoundError as exc:
            raise VerifyError(f"missing_artifact:{path}") from exc
        require(item.get("bytes") == len(data), f"artifact_size:{path}")
        require(item.get("sha256") == sha_bytes(data), f"artifact_hash:{path}")
    for index in range(len(expected_names), 3):
        require(artifacts[index] is None, f"unexpected_artifact:{task_dir}:{index}")


def check_task(root: Path, directory: str, mode: str, tier: str, source_sha: str,
               fixture_hashes: dict[str, str], expected_artifacts: list[str]) -> dict[str, Any]:
    task = root / directory
    request = json_file(task / "request.json")
    result = json_file(task / "result.json")
    require(request.get("schema") == "mycity-model-task-request-v2", f"request_schema:{task}")
    require(request.get("source_sha256") == source_sha, f"request_source:{task}")
    require(request.get("tier") == tier, f"request_tier:{task}")
    require(request.get("mode") == mode, f"request_mode:{task}")
    for key, value in fixture_hashes.items():
        require(request.get(key) == value, f"request_{key}:{task}")
    request_digest = sha_file(task / "request.json")
    require(result.get("schema") == "mycity-model-task-result-v2", f"result_schema:{task}")
    require(result.get("source_sha256") == source_sha, f"result_source:{task}")
    require(result.get("tier") == tier, f"result_tier:{task}")
    require(result.get("mode") == mode, f"result_mode:{task}")
    require(result.get("request_sha256") == request_digest, f"result_request_digest:{task}")
    require(isinstance(result.get("pid"), int) and result["pid"] > 0, f"result_pid:{task}")
    require(isinstance(result.get("instance_unix_ns"), int) and result["instance_unix_ns"] > 0,
            f"result_instance:{task}")
    check_artifacts(task, result, expected_artifacts)
    root_stamp = result.get("root")
    if mode != "reference":
        require(isinstance(root_stamp, dict), f"root_stamp:{task}")
        root_bytes = (task / "root.json").read_bytes()
        require(root_stamp.get("bytes") == len(root_bytes), f"root_size:{task}")
        require(root_stamp.get("sha256") == sha_bytes(root_bytes), f"root_hash:{task}")
        root_value = json.loads(root_bytes)
        require(root_stamp.get("sequence") == root_value.get("sequence"), f"root_sequence:{task}")
    return result


def verify_excluded_failure(root: Path, old_binary_path: Path, old_binary_sha: str,
                            final_binary_sha: str) -> dict[str, Any]:
    path = root / "03-small-observed" / "observer-report.json"
    report = json_file(path)
    require(report.get("status") == "failed", "03_small_failure_not_preserved")
    require(report.get("reason") == "native_clock_timeout", "03_small_failure_reason_changed")
    command = report.get("command")
    require(isinstance(command, list) and len(command) == 5, "03_small_failure_command_shape")
    require(command[1] == "speed-run" and command[3:] == ["small", "unpaced"],
            "03_small_failure_not_small_unpaced_speed_run")
    require(old_binary_sha != final_binary_sha, "03_small_failure_expected_old_binary")
    require(report.get("binary_sha256") == old_binary_sha and report.get("binary_unchanged") is True,
            "03_small_failure_binary_mismatch")
    require(Path(command[0]).resolve() == old_binary_path, "03_small_failure_executable_path")
    require(report.get("model_exit_code") is None, "03_small_failure_exit_shape")
    return {"path": str(path), "status": "failed", "reason": "native_clock_timeout",
            "binary_sha256": old_binary_sha, "counted_as_success": False}


def verify_clock_handshake(run_root: Path, source_sha: str) -> dict[str, Any]:
    model = run_root / "model"
    mapping = json_file(run_root / "observer-clock-map.json")
    ready = json_file(model / "observer-clock-ready.json")
    request = json_file(model / "observer-clock-request.json")
    reply = json_file(model / "observer-clock-reply.json")
    child = json_file(run_root / "observer-child.json")
    child_identity = child.get("identity")
    require(isinstance(child_identity, dict), "clock_child_identity_missing")
    pid = mapping.get("supervisor_pid")
    require(isinstance(pid, int) and pid > 0, "clock_supervisor_pid_invalid")
    nonce = mapping.get("nonce")
    require(isinstance(nonce, str) and re.fullmatch(r"[0-9a-f]{32}", nonce) is not None,
            "clock_nonce_invalid")
    require(mapping.get("source_sha256") == source_sha, "clock_map_source_mismatch")
    require(ready.get("schema") == "mycity-observer-clock-ready-v1" and
            request.get("schema") == "mycity-observer-clock-request-v1" and
            reply.get("schema") == "mycity-observer-clock-reply-v1", "clock_message_schema")
    for name, record in (("ready", ready), ("request", request), ("reply", reply)):
        require(record.get("pid") == pid and record.get("source_sha256") == source_sha,
                f"clock_{name}_identity")
    require(ready.get("pid") == child_identity.get("pid") == pid, "clock_child_pid_mismatch")
    require(request.get("nonce") == reply.get("nonce") == nonce, "clock_nonce_mismatch")
    spawn = mapping.get("spawn_before_observer_ns")
    sent = mapping.get("request_sent_observer_ns")
    received = mapping.get("reply_received_observer_ns")
    elapsed = reply.get("elapsed_ns")
    low = mapping.get("supervisor_origin_observer_ns_min")
    high = mapping.get("supervisor_origin_observer_ns_max")
    uncertainty = mapping.get("uncertainty_ns")
    values = (spawn, sent, received, elapsed, low, high, uncertainty)
    require(all(isinstance(v, int) and v >= 0 for v in values), "clock_timestamp_fields")
    require(spawn <= sent <= received and low <= high and uncertainty == high - low,
            "clock_timestamp_order")
    require(low == max(spawn, sent - elapsed) and high == received - elapsed,
            "clock_interval_mismatch")
    return {"pid": pid, "nonce": nonce, "source_sha256": source_sha,
            "spawn_before_observer_ns": spawn, "request_sent_observer_ns": sent,
            "reply_received_observer_ns": received, "origin_min_ns": low,
            "origin_max_ns": high, "uncertainty_ns": uncertainty}


def verify_tier(repo: Path, evidence_root: Path, tier: str, run_name: str,
                binary_path: Path, binary_sha: str) -> dict[str, Any]:
    counts = {
        "small": {"accounts": 16, "stocks": 32, "payroll": 8, "claims": 2000,
                  "unique_payees": 8, "sources": 48, "wage_hourly_cents": 65},
        "medium": {"accounts": 32, "stocks": 64, "payroll": 32, "claims": 10000,
                   "unique_payees": 28, "sources": 96, "wage_hourly_cents": 257},
        "full": {"accounts": 64, "stocks": 128, "payroll": 64, "claims": 20000,
                 "unique_payees": 60, "sources": 192, "wage_hourly_cents": 512},
    }[tier]
    fixture_names = {
        "small": ("m0-t13a-small-opening.json", "m0-t13a-small-policy.json", "m0-t13a-small-oracle.json"),
        "medium": ("m0-t13a-medium-opening.json", "m0-t13a-medium-policy.json", "m0-t13a-medium-oracle.json"),
        "full": ("m0-t13a-full-opening-candidate.json", "m0-t13a-real-policy-candidate.json",
                 "m0-t13a-expected-oracle-candidate.json"),
    }[tier]
    opening_path, policy_path, oracle_path = (repo / "fixtures" / name for name in fixture_names)
    opening, policy, oracle = (json_file(path) for path in (opening_path, policy_path, oracle_path))
    hashes = {
        "opening_sha256": sha_file(opening_path),
        "policy_sha256": sha_file(policy_path),
        "oracle_sha256": sha_file(oracle_path),
    }
    require(opening.get("schema") is not None and policy.get("schema") is not None and oracle.get("scope") == "independent_expected_arithmetic_only_not_runtime",
            f"frozen_fixture_schema:{tier}")
    require(len(opening.get("sources", [])) == counts["sources"], f"source_record_count:{tier}")
    require(len(opening.get("accounts", [])) == counts["accounts"], f"opening_account_count:{tier}")
    require(len(opening.get("stocks", [])) == counts["stocks"], f"opening_stock_count:{tier}")
    region = opening.get("region", {})
    exchange = opening.get("exchange", {})
    boot = opening.get("boot", {})
    require(len(region.get("cities", [])) == 8 and len(region.get("routes", [])) == 2 and
            len(region.get("obligations", [])) == 16, f"fixed_region_shape:{tier}")
    require(len(exchange.get("maps", [])) == 8 and len(exchange.get("accounts", [])) == 4 and
            len(exchange.get("stocks", [])) == 14 and len(exchange.get("edges", [])) == 6 and
            len(exchange.get("quotes", [])) == 8, f"fixed_exchange_shape:{tier}")
    require(len(boot.get("stocks", [])) == 6 and len(boot.get("resources", [])) == 2 and
            len(boot.get("limits", [])) == 6, f"fixed_boot_shape:{tier}")
    require(policy.get("initial_account_count") == counts["accounts"] and
            policy.get("initial_stock_count") == counts["stocks"], f"policy_opening_counts:{tier}")
    payroll = policy.get("payroll")
    claims = policy.get("claims")
    require(isinstance(payroll, list) and len(payroll) == counts["payroll"], f"payroll_contract_count:{tier}")
    require(all(isinstance(row, dict) for row in payroll), f"payroll_contract_shape:{tier}")
    require(isinstance(claims, list) and sum(row.get("count", 0) for row in claims) == counts["claims"],
            f"claim_expanded_count:{tier}")
    require(all(isinstance(row, dict) for row in claims), f"claim_template_shape:{tier}")
    require(len({row.get("payee") for row in payroll}) == counts["unique_payees"], f"payroll_payee_count:{tier}")
    require(sum(row.get("hourly_cents", 0) for row in payroll) == counts["wage_hourly_cents"],
            f"payroll_hourly_basis:{tier}")
    require(all(row.get("ends_at") == 2592000 for row in payroll), f"payroll_end:{tier}")
    cycle_counts = {kind: sum(1 for row in payroll if row.get("cycle", {}).get("kind") == kind)
                    for kind in ("daily", "weekly", "monthly")}
    expected_cycle_counts = {"small": {"daily": 3, "weekly": 3, "monthly": 2},
                             "medium": {"daily": 11, "weekly": 11, "monthly": 10},
                             "full": {"daily": 22, "weekly": 21, "monthly": 21}}[tier]
    require(cycle_counts == expected_cycle_counts, f"payroll_cycle_counts:{tier}")
    require(all(row.get("amount") == 1 for row in claims), f"claim_unit_amount:{tier}")
    require(oracle.get("expected_claims_paid_total") == counts["claims"], f"oracle_claim_total:{tier}")
    require(sum(oracle.get("expected_payroll_paid", [])) == oracle.get("expected_wages_paid_total"),
            f"oracle_wage_sum:{tier}")
    require(len(oracle.get("expected_payroll_paid", [])) == counts["payroll"], f"oracle_payroll_vector:{tier}")
    expected_work_seconds = 29 * 8 * 3600
    require(oracle.get("expected_work_seconds_each") == expected_work_seconds, f"oracle_work_duration:{tier}")
    require(oracle.get("expected_payroll_paid") ==
            [row["hourly_cents"] * expected_work_seconds // 3600 for row in payroll],
            f"oracle_per_contract_wages:{tier}")
    require(sum(row["count"] * row["amount"] for row in claims) == counts["claims"],
            f"claim_principal_basis:{tier}")

    run_root = evidence_root / run_name
    observer = json_file(run_root / "observer-report.json")
    require(observer.get("status") == "passed_measurement_wrapper", f"observer_status:{tier}")
    require(observer.get("reason") is None and observer.get("cleanup_error") is None,
            f"observer_reason_or_cleanup:{tier}")
    require(observer.get("model_exit_code") == 0, f"observer_model_exit:{tier}")
    require(observer.get("binary_sha256") == binary_sha and observer.get("binary_unchanged") is True,
            f"observer_binary:{tier}")
    command = observer.get("command")
    model_root = (run_root / "model").resolve()
    require(isinstance(command, list) and len(command) == 5, f"observer_command_shape:{tier}")
    require(Path(command[0]).resolve() == binary_path and command[1] == "speed-run" and
            Path(command[2]).resolve() == model_root and command[3:] == [tier, "unpaced"],
            f"observer_command_identity:{tier}")

    supervisor_manifest = json_file(model_root / "manifest.json")
    supervisor_report = json_file(model_root / "report.json")
    business_root = model_root / "business"
    plan = json_file(business_root / "speed-manifest.json")
    business_report = json_file(business_root / "report.json")
    for label, value in (("supervisor_manifest", supervisor_manifest), ("supervisor_report", supervisor_report),
                         ("speed_plan", plan), ("business_report", business_report)):
        require(value.get("tier") == tier and value.get("rate") == "unpaced", f"{label}_identity:{tier}")
        require(value.get("source_sha256") == observer.get("source_sha256"), f"{label}_source:{tier}")
    source_sha = observer.get("source_sha256")
    hex64(source_sha, f"source:{tier}")
    clock = verify_clock_handshake(run_root, source_sha)
    require(supervisor_manifest.get("schema") == "mycity-speed-supervisor-v1" and
            supervisor_report.get("schema") == "mycity-speed-supervisor-v1" and
            plan.get("schema") == "mycity-speed-plan-v1" and
            business_report.get("schema") == "mycity-speed-driver-v1", f"speed_schema:{tier}")
    require(supervisor_report.get("experiment_completed") is True, f"supervisor_completed:{tier}")
    require(business_report.get("entire_engine_including_metadata") == "equal_independent_normal_reference",
            f"business_reference_equality:{tier}")
    require(business_report.get("completed_hours") == 720 and
            business_report.get("verified_targets") == 723 and
            business_report.get("worker_windows") == 724, f"trajectory_count:{tier}")
    require(isinstance(business_report.get("days"), list) and
            len(business_report["days"]) == 30 and
            [row.get("day") for row in business_report["days"]] == list(range(1, 31)),
            f"daily_trajectory:{tier}")
    require(business_report.get("target_day_wall_seconds") is None, f"unpaced_rate:{tier}")
    require(business_report.get("speed_target_met") in (True, False), f"speed_target_field:{tier}")
    require(business_report.get("L1") == "absent" and business_report.get("real_16GB") == "not_run" and
            business_report.get("eight_hour") == "not_run" and business_report.get("full_M0") == "not_accepted",
            f"acceptance_scope:{tier}")
    expected_profile = f"L2_{tier}_actual"
    require(business_report.get("profile") == expected_profile, f"tier_profile:{tier}")
    if tier != "full":
        for gate in ("full_profile_60_actual_operating_target", "full_profile_unpaced_target"):
            require(business_report.get(gate) == "not_evaluated_other_tier_or_rate", f"full_gate_leak:{tier}:{gate}")
    else:
        require(business_report.get("full_profile_unpaced_target") in ("passed_this_run", "failed_this_run"),
                "full_unpaced_gate_missing")

    reference_result = check_task(business_root, "reference", "reference", tier, source_sha, hashes,
                                  ["cpu-targets.jsonl", "cpu-report.json", "cpu-final-engine.json"])
    require(reference_result.get("image_sha256") is None and
            reference_result.get("business_sha256") is None and reference_result.get("root") is None,
            f"reference_result_shape:{tier}")
    reference = json_file(business_root / "reference" / "cpu-report.json")
    require(reference.get("tier") == tier and reference.get("source_sha256") == source_sha,
            f"reference_report_identity:{tier}")
    for key, value in hashes.items():
        require(reference.get(key) == value, f"reference_fixture_hash:{tier}:{key}")
    expected_counts = {"accounts": counts["accounts"], "stocks": counts["stocks"],
                       "payroll": counts["payroll"], "claims": counts["claims"],
                       "cities": 8, "quotes": 8, "inflight_peak": 4}
    require(reference.get("counts") == expected_counts, f"reference_counts:{tier}")
    require(reference.get("completed_hour_targets") == 720, f"reference_hours:{tier}")
    require(reference.get("wages_paid_total") == oracle.get("expected_wages_paid_total"), f"actual_wages_vs_oracle:{tier}")
    require(reference.get("claims_paid_total") == oracle.get("expected_claims_paid_total"), f"actual_claims_vs_oracle:{tier}")
    require(reference.get("cash_accounts") == oracle.get("expected_cash_accounts"), f"cash_accounts_vs_oracle:{tier}")
    require(sum(row.get("cash", 0) for row in reference.get("cash_accounts", [])) == oracle.get("expected_cash_total"),
            f"cash_total_vs_oracle:{tier}")
    ref_business_sha = hex64(reference.get("business_sha256"), f"reference_business:{tier}")

    before_result = check_task(business_root, "before-reopen", "before_reopen", tier, source_sha, hashes,
                               ["root.json", "workspace.json"])
    compare_result = check_task(business_root, "compare-speed", "compare_speed", tier, source_sha, hashes,
                                ["root.json", "workspace.json"])
    image_sha = hex64(compare_result.get("image_sha256"), f"compare_image:{tier}")
    compared_business_sha = hex64(compare_result.get("business_sha256"), f"compare_business:{tier}")
    require(before_result.get("image_sha256") == image_sha, f"cold_image_changed:{tier}")
    require(compared_business_sha == ref_business_sha, f"compare_business_mismatch:{tier}")
    require(business_report.get("image_sha256") == image_sha, f"driver_compare_image:{tier}")
    require(business_report.get("cold_ready_seconds") is not None, f"cold_restore_missing:{tier}")
    require(supervisor_report.get("tier") == tier and supervisor_report.get("rate") == "unpaced",
            f"supervisor_report_tier:{tier}")
    return {
        "tier": tier,
        "status": "functional_pass",
        "counts": expected_counts,
        "sources": counts["sources"],
        "unique_payees": counts["unique_payees"],
        "payroll_cycle_counts": cycle_counts,
        "claims_total_cents": oracle["expected_claims_paid_total"],
        "wages_total_cents": oracle["expected_wages_paid_total"],
        "cash_total_cents": oracle["expected_cash_total"],
        "completed_hours": 720,
        "verified_targets": 723,
        "worker_windows": 724,
        "observer_clock": clock,
        "business_sha256": compared_business_sha,
        "image_sha256": image_sha,
        "speed_target_met_recorded_only": business_report["speed_target_met"],
        "performance_acceptance": "not_evaluated",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repo", type=Path, help="canonical source checkout containing fixtures/")
    parser.add_argument("evidence_root", type=Path, help="tier-speed evidence root containing binary-final.json")
    parser.add_argument("output", nargs="?", type=Path,
                        help="optional verification.json destination; default stdout")
    args = parser.parse_args()
    report: dict[str, Any] = {
        "schema": "mycity-tier-functional-verification-v1",
        "scope": "functional-only",
        "L1": "absent",
        "16GB": "not_run",
        "eight_hour": "not_run",
        "performance_acceptance": "not_evaluated",
        "status": "failed",
        "tiers": [],
        "errors": [],
    }
    try:
        repo = args.repo.resolve(strict=True)
        evidence_root = args.evidence_root.resolve(strict=True)
        old_record = json_file(evidence_root / "binary.json")
        old_binary_path = Path(old_record.get("binary", "")).resolve(strict=True)
        old_binary_sha = hex64(old_record.get("binary_sha256"), "binary")
        require(sha_file(old_binary_path) == old_binary_sha, "old_binary_hash_mismatch")
        binary_record = json_file(evidence_root / "binary-final.json")
        binary_path = Path(binary_record.get("binary", "")).resolve(strict=True)
        binary_sha = hex64(binary_record.get("binary_sha256"), "binary-final")
        require(sha_file(binary_path) == binary_sha, "binary_final_hash_mismatch")
        report["binary"] = str(binary_path)
        report["binary_sha256"] = binary_sha
        report["excluded_failed_attempt"] = verify_excluded_failure(
            evidence_root, old_binary_path, old_binary_sha, binary_sha)
        for tier, run_name in (("small", "06-small-observed"),
                               ("medium", "07-medium-observed"),
                               ("full", "08-full-observed")):
            report["tiers"].append(
                verify_tier(repo, evidence_root, tier, run_name, binary_path, binary_sha)
            )
        sources = {json_file(evidence_root / run_name / "model" / "business" / "report.json").get("source_sha256")
                   for run_name in ("06-small-observed", "07-medium-observed", "08-full-observed")}
        require(len(sources) == 1, "tier_source_identity_mismatch")
        source_sha = next(iter(sources))
        optional_source = binary_record.get("source_sha256")
        if optional_source is not None:
            require(optional_source == source_sha, "binary_source_identity_mismatch")
        archive = evidence_root / "05-cli-clock-build" / "source.zip"
        require(sha_file(archive) == binary_record["source_zip_sha256"], "source_archive_hash")
        h = hashlib.sha256()
        with zipfile.ZipFile(archive) as frozen:
            require(len(frozen.namelist()) == len(set(frozen.namelist())), "duplicate_archive_names")
            names = sorted(name for name in frozen.namelist()
                           if name in (".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml")
                           or name.startswith(("crates/", "fixtures/")))
            for name in names:
                data = frozen.read(name)
                encoded = name.encode("utf-8")
                h.update(len(encoded).to_bytes(8, "little"))
                h.update(encoded)
                h.update(len(data).to_bytes(8, "little"))
                h.update(data)
        require(h.hexdigest() == source_sha, "build_source_archive_identity")
        report["build_source_files"] = len(names)
        report["source_sha256"] = source_sha
        report["status"] = "passed"
    except (VerifyError, OSError, TypeError, ValueError, AttributeError, KeyError) as exc:
        report["errors"].append(str(exc))
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
    else:
        sys.stdout.write(rendered)
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
