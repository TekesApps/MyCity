"""Pure synthetic tests for seal.py's fail-closed evidence checks.

These fixtures do not run an old writer, Cargo, Store, or the recovery reader.
They prove only that seal.py binds and rejects synthetic inputs as specified.
"""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import prepare
import seal


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def put_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True) + "\n")


class SealFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="seal-synthetic-")
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.evidence = root / "attempt01"
        self.output = root / "writer-output"
        self.pins = root / "old-source-pins"
        self.evidence.mkdir()
        self.output.mkdir()
        self.pins.mkdir()

        self.archive = self.pins / "source.zip"
        self.archive.write_bytes(b"synthetic frozen source archive")
        self.business_sha = "b" * 64
        self.freeze = self.pins / "freeze.json"
        put_json(self.freeze, {"source_sha256": self.business_sha})

        self.patches = [
            mock.patch.object(seal, "EVIDENCE", self.evidence),
            mock.patch.object(seal, "OUTPUT", self.output),
            mock.patch.object(prepare, "OLD_EVIDENCE", self.pins),
            mock.patch.object(prepare, "OLD_ARCHIVE_SHA256", seal.digest(self.archive)),
            mock.patch.object(prepare, "OLD_MANIFEST_SHA256", seal.digest(self.freeze)),
            mock.patch.object(prepare, "OLD_BUSINESS_SHA256", self.business_sha),
            mock.patch.object(prepare, "HARNESS", "synthetic reviewed harness bytes"),
            mock.patch.object(seal, "reject_running_measurement_or_writer", lambda: None),
        ]
        for patcher in self.patches:
            patcher.start()
            self.addCleanup(patcher.stop)

    def create_fixture(self, *, receipt=True):
        exe = self.evidence / "writer-test-executable"
        exe.write_bytes(b"synthetic preserved executable")
        exe.chmod(0o755)
        harness = self.evidence / "harness.rs"
        harness.write_text(prepare.HARNESS)
        stdout = self.evidence / "writer.stdout"
        stdout.write_bytes(b"synthetic writer stdout\n")
        stderr = self.evidence / "writer.stderr"
        stderr.write_bytes(b"synthetic writer stderr\n")

        for kind in ("accepted-wal", "accepted-checkpoint"):
            world = self.output / kind
            world.mkdir()
            (world / "CURRENT.json").write_text('{"synthetic":true}\n')
        report = {
            "rules": "construction-engine-v1",
            "operating_published": True,
            "old_historical_world": False,
            "wal": {"path": "accepted-wal", "state_sha256": "1" * 64},
            "checkpoint": {"path": "accepted-checkpoint", "state_sha256": "2" * 64},
        }
        put_json(self.output / "report.json", report)

        if receipt:
            put_json(
                self.evidence / "writer-receipt.json",
                {
                    "exit_code": 0,
                    "executable_sha256": seal.digest(exe),
                    "source_archive_sha256": prepare.OLD_ARCHIVE_SHA256,
                    "freeze_manifest_sha256": prepare.OLD_MANIFEST_SHA256,
                    "business_source_sha256": prepare.OLD_BUSINESS_SHA256,
                    "exporter_harness_sha256": sha256_bytes(prepare.HARNESS.encode()),
                    "stdout_sha256": seal.digest(stdout),
                    "stderr_sha256": seal.digest(stderr),
                    "command": [
                        str(exe),
                        "export_v119_accepted_project_wal_and_checkpoint",
                        "--exact",
                        "--nocapture",
                    ],
                },
            )

    def assert_no_new_success_seals(self):
        for kind, _ in seal.KINDS:
            self.assertFalse((self.evidence / f"{kind}.manifest.json").exists())
            self.assertFalse((self.evidence / f"{kind}.seal.json").exists())

    def run_seal_quietly(self):
        with contextlib.redirect_stdout(io.StringIO()):
            seal.seal()

    def test_missing_writer_receipt_rejects_without_success_seal(self):
        self.create_fixture(receipt=False)
        with self.assertRaises((OSError, RuntimeError)):
            self.run_seal_quietly()
        self.assert_no_new_success_seals()

    def test_nonzero_writer_receipt_rejects_without_success_seal(self):
        self.create_fixture()
        receipt_path = self.evidence / "writer-receipt.json"
        receipt = json.loads(receipt_path.read_text())
        receipt["exit_code"] = 23
        put_json(receipt_path, receipt)
        with self.assertRaisesRegex(RuntimeError, "writer receipt"):
            self.run_seal_quietly()
        self.assert_no_new_success_seals()

    def test_wrong_source_pin_rejects_without_success_seal(self):
        self.create_fixture()
        with mock.patch.object(prepare, "OLD_ARCHIVE_SHA256", "0" * 64):
            with self.assertRaisesRegex(RuntimeError, "source pin mismatch"):
                self.run_seal_quietly()
        self.assert_no_new_success_seals()

    def test_missing_second_world_rejects_before_writing_either_seal(self):
        self.create_fixture()
        import shutil

        shutil.rmtree(self.output / "accepted-checkpoint")
        with self.assertRaisesRegex(RuntimeError, "retained world"):
            self.run_seal_quietly()
        self.assert_no_new_success_seals()

    def test_existing_seal_rejects_without_overwriting_it_or_sealing_peer(self):
        self.create_fixture()
        existing = self.evidence / "accepted-wal.seal.json"
        existing.write_text('{"preserve":"prior evidence"}\n')
        with self.assertRaisesRegex(RuntimeError, "overwrite"):
            self.run_seal_quietly()
        self.assertEqual(existing.read_text(), '{"preserve":"prior evidence"}\n')
        self.assertFalse((self.evidence / "accepted-checkpoint.seal.json").exists())
        self.assertFalse((self.evidence / "accepted-wal.manifest.json").exists())

    def test_evidence_mutation_during_second_hash_rejects_before_success_seal(self):
        self.create_fixture()
        target = self.evidence / "writer.stdout"
        original_digest = seal.digest
        calls = 0

        def mutate_on_recheck(path):
            nonlocal calls
            if Path(path) == target:
                calls += 1
                if calls == 2:
                    target.write_bytes(b"changed between evidence hashes\n")
            return original_digest(path)

        with mock.patch.object(seal, "digest", mutate_on_recheck):
            with self.assertRaisesRegex(RuntimeError, "evidence changed"):
                self.run_seal_quietly()
        self.assertEqual(calls, 2)
        self.assert_no_new_success_seals()

    def test_world_mutation_during_second_hash_rejects_before_success_seal(self):
        self.create_fixture()
        target = self.output / "accepted-wal" / "CURRENT.json"
        original_digest = seal.digest
        calls = 0

        def mutate_on_recheck(path):
            nonlocal calls
            if Path(path) == target:
                calls += 1
                if calls == 2:
                    target.write_text('{"synthetic":false}\n')
            return original_digest(path)

        with mock.patch.object(seal, "digest", mutate_on_recheck):
            with self.assertRaisesRegex(RuntimeError, "world changed"):
                self.run_seal_quietly()
        self.assertEqual(calls, 2)
        self.assert_no_new_success_seals()

    def test_synthetic_success_manifest_and_seal_match_controller_shape(self):
        self.create_fixture()
        self.run_seal_quietly()
        manifest_path = self.evidence / "accepted-wal.manifest.json"
        seal_path = self.evidence / "accepted-wal.seal.json"
        manifest = json.loads(manifest_path.read_text())
        record = json.loads(seal_path.read_text())

        self.assertEqual(set(manifest), {"fixture_kind", "world_path", "files"})
        self.assertEqual(manifest["fixture_kind"], "accepted-wal")
        self.assertEqual(manifest["world_path"], str(self.output / "accepted-wal"))
        self.assertEqual(manifest["files"], [{
            "path": "CURRENT.json",
            "size": len(b'{"synthetic":true}\n'),
            "sha256": sha256_bytes(b'{"synthetic":true}\n'),
        }])
        self.assertEqual(record["schema"], "mycity-v119-accepted-construction-seal-v1")
        self.assertEqual(record["fixture_kind"], "accepted-wal")
        self.assertEqual(record["rules_identity"], "construction-engine-v1")
        self.assertEqual(record["new_reader_compatibility"], "not_run")
        self.assertIs(record["old_historical_world"], False)
        for controller_field in (
            "world_manifest_sha256",
            "manifest_file_sha256",
            "writer_exit_code",
            "source_archive_sha256",
            "freeze_manifest_sha256",
            "business_source_sha256",
        ):
            self.assertIn(controller_field, record)
        self.assertEqual(record["manifest_file_sha256"], seal.digest(manifest_path))
        self.assertEqual(record["world_manifest_sha256"], seal.canonical_hash(manifest["files"]))
        self.assertIn("writer_report_sha256", record)
        self.assertTrue((self.evidence / "accepted-checkpoint.seal.json").is_file())


if __name__ == "__main__":
    unittest.main()
