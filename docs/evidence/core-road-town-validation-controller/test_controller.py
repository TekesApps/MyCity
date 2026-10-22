"""Pure helper tests for run_controller.py; not run during source preparation."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest


MODULE_PATH = Path(__file__).with_name("run_controller.py")
SPEC = importlib.util.spec_from_file_location("road_validation_controller", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
controller = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(controller)


class ControllerHelpersTest(unittest.TestCase):
    def test_native_macos_value_precedes_label_and_is_in_bytes(self) -> None:
        raw = "        0.00 real         0.00 user         0.00 sys\n             1196032  maximum resident set size\n              884928  peak memory footprint\n"
        parsed = controller.parse_native_time_metrics(raw)
        self.assertEqual(parsed["status"], "captured")
        self.assertEqual(parsed["max_rss_bytes"], 1196032)
        self.assertEqual(parsed["peak_footprint_bytes"], 884928)
        self.assertEqual(parsed["units"], "bytes")

    def test_missing_zero_or_ambiguous_native_memory_never_looks_like_a_capture(self) -> None:
        for raw in ("", "maximum resident set size 1196032\n", "0 maximum resident set size\n", "1024 maximum resident set size\n2048 maximum resident set size\n"):
            with self.subTest(raw=raw):
                parsed = controller.parse_native_time_metrics(raw)
                self.assertEqual(parsed["status"], "missing_or_ambiguous")
                self.assertIsNone(parsed["max_rss_bytes"])

    def test_sha256_requires_lowercase_full_length_hex(self) -> None:
        self.assertTrue(controller.valid_sha256("a" * 64))
        self.assertFalse(controller.valid_sha256("A" * 64))
        self.assertFalse(controller.valid_sha256("a" * 63))

    def test_process_gate_blocks_builders_and_m0_measurements(self) -> None:
        self.assertEqual(controller.process_refusal("cargo", "cargo test"), "active cargo process")
        self.assertEqual(controller.process_refusal("rustc", "rustc --crate-name x"), "active rustc process")
        self.assertEqual(controller.process_refusal("clippy-driver", "clippy-driver --crate-name x"), "active clippy-driver process")
        self.assertEqual(controller.process_refusal("cargo-clippy", "cargo-clippy clippy"), "active cargo-clippy process")
        self.assertEqual(controller.process_refusal("rustdoc", "rustdoc --test heavy-doc"), "active rustdoc process")
        self.assertIsNotNone(
            controller.process_refusal("python3", "python m0-t13a/run.py")
        )
        for subcommand in controller.M0_HEADLESS_COMMANDS:
            self.assertIsNotNone(
                controller.process_refusal(
                    "mycity-headless", f"mycity-headless {subcommand} --profile large"
                ),
                subcommand,
            )
            self.assertIsNotNone(
                controller.process_refusal(
                    "python3", f"observer-wrapper mycity-headless {subcommand}"
                ),
                subcommand,
            )
        self.assertIsNone(
            controller.process_refusal(
                "python3", "mycity-headless-layout-session unrelated validation-plan.py"
            )
        )
        self.assertIsNone(
            controller.process_refusal(
                "python3", "mycity-headless version joint-endurance-not-a-command"
            )
        )
        self.assertIsNone(controller.process_refusal("python3", "python validation-plan.py"))

    def test_plan_builds_core_release_examples_separately(self) -> None:
        plan = dict(controller.run_plan())
        self.assertIn("core-release-examples", plan)
        self.assertEqual(
            plan["core-release-examples"],
            ["cargo", controller.TOOLCHAIN, "build", "--locked", "--offline", "--release", "--examples", "-p", "mycity-core"],
        )

    def test_town_market_plan_covers_seven_profiles_and_two_tax_cases_without_world_dirs(self) -> None:
        binary = Path("/tmp/town_market")
        cases = controller.town_market_commands(binary)
        self.assertEqual(len(cases), 9)
        self.assertEqual(
            [case_id for _, _, case_id, _ in cases[:7]],
            ["base", "cash-shortage", "raw-shortage", "no-route", "capacity-cut", "energy-price-rise", "demand-drop"],
        )
        for _, argv, _, _ in cases:
            self.assertEqual(argv[0], str(binary))
            self.assertIn("--scenario", argv)
            self.assertNotIn("<new-world-dir>", argv)
        self.assertEqual(
            cases[7][1],
            [str(binary), "--scenario", "base", "--turnover-tax-bps", "500"],
        )
        self.assertEqual(
            cases[8][1],
            [
                str(binary), "--scenario", "base", "--company-cash", "0",
                "--debt-principal", "1000000", "--debt-due-day", "2",
                "--production-utility-cost", "50", "--turnover-tax-bps", "500",
            ],
        )

    def test_town_market_output_parser_keeps_substitute_and_capital_caveats(self) -> None:
        fields = controller.parse_town_market_output(
            "scenario=base days=28 live_transport=false live_utility=false tax_policy=not_configured",
            "base",
        )
        self.assertEqual(fields["days"], "28")
        with self.assertRaises(controller.Refusal):
            controller.parse_town_market_output(
                "scenario=base days=27 live_transport=false live_utility=false tax_policy=not_configured",
                "base",
            )

    def test_world_manifest_and_copy_preserve_all_file_hashes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp).resolve()
            source = work / "legacy-copy"
            source.mkdir()
            (source / "wal.bin").write_bytes(b"immutable wal")
            destination = work / "run" / "legacy-copy"
            destination.parent.mkdir()
            evidence = work / "run" / "manifests"
            evidence.mkdir()
            before, after = controller.copy_world_verified(source, destination, work, evidence)
            self.assertEqual(before, after)
            self.assertEqual(before, controller.tree_manifest(destination))
            self.assertTrue((evidence / "source-before.json").is_file())
            self.assertTrue((evidence / "source-after.json").is_file())

    def test_world_manifest_rejects_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "world"
            root.mkdir()
            (root / "real").write_text("data", encoding="utf-8")
            (root / "alias").symlink_to(root / "real")
            with self.assertRaises(controller.Refusal):
                controller.tree_manifest(root)


if __name__ == "__main__":
    unittest.main()
