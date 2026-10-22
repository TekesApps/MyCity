import hashlib
import json
import tempfile
import unittest
import zipfile
from pathlib import Path

from preflight import FIXTURES, stream_source_sha256, verify_freeze


class FrozenInputPreflightTests(unittest.TestCase):
    def make_fixture(self, directory):
        root = Path(directory)
        (root / "fixtures").mkdir()
        entries = [
            ("Cargo.lock", b"lock-bytes"),
            (FIXTURES[0], b"acceptance-fixture"),
            (FIXTURES[1], b"opening-fixture"),
        ]
        source = stream_source_sha256(entries)
        archive = root / "frozen-source.zip"
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
            for name, content in entries:
                output.writestr(name, content)
        for name, content in entries[1:]:
            (root / name).write_bytes(content)
        binary = root / "fixed-cli"
        binary.write_bytes(b"test-executable:" + source.encode())
        digest = lambda data: hashlib.sha256(data).hexdigest()
        fixture_hashes = {name: digest(data) for name, data in entries[1:]}
        source_info = {
            "source_sha256": source,
            "binary_sha256": digest(binary.read_bytes()),
            "acceptance_sha256": fixture_hashes[FIXTURES[0]],
            "opening_sha256": fixture_hashes[FIXTURES[1]],
        }
        freeze_info = {
            "source_sha256": source,
            "source_zip_sha256": digest(archive.read_bytes()),
            "source_files": len(entries),
            "lock_sha256": digest(entries[0][1]),
            "acceptance_sha256": fixture_hashes[FIXTURES[0]],
            "full_opening_sha256": fixture_hashes[FIXTURES[1]],
        }
        return source_info, freeze_info, archive, binary, root

    def test_validates_frozen_stream_binary_and_only_canonical_fixtures(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, freeze, archive, binary, root = self.make_fixture(temporary)
            # A different current Rust checkout is intentionally irrelevant.
            (root / "crates").mkdir()
            (root / "crates/current-only.rs").write_text("new integrated code")
            result = verify_freeze(
                source, freeze, archive, binary, root,
                expected_source=source["source_sha256"], expected_entry_count=3,
            )
            self.assertEqual(result["status"], "passed_static_frozen_input_preflight")
            self.assertIn("fixtures_only", result["current_checkout_comparison"])

    def test_rejects_fixture_byte_drift(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, freeze, archive, binary, root = self.make_fixture(temporary)
            (root / FIXTURES[0]).write_bytes(b"changed-current-fixture")
            with self.assertRaises(AssertionError):
                verify_freeze(
                    source, freeze, archive, binary, root,
                    expected_source=source["source_sha256"], expected_entry_count=3,
                )

    def test_rejects_wrong_embedded_source_fingerprint(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, freeze, archive, binary, root = self.make_fixture(temporary)
            binary.write_bytes(b"wrong-executable")
            source["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
            with self.assertRaises(AssertionError):
                verify_freeze(
                    source, freeze, archive, binary, root,
                    expected_source=source["source_sha256"], expected_entry_count=3,
                )


if __name__ == "__main__":
    unittest.main()
