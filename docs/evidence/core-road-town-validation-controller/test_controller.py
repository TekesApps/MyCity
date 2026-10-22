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

    def test_world_manifest_and_copy_preserve_all_file_hashes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
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
