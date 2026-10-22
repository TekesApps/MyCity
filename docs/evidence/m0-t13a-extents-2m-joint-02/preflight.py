"""Verify the original frozen source bundle without matching current Rust files."""
import hashlib
import json
import struct
import zipfile
from pathlib import Path

EXPECTED_SOURCE = "ade347ccd333e9f4cdbda7f08066f720bbd7649a6d58762987445750a0db4e05"
FIXTURES = (
    "fixtures/m0-t13a-acceptance.json",
    "fixtures/m0-t13a-full-opening-candidate.json",
)


def file_sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def stream_source_sha256(entries):
    """Match the original prepare.py sorted-name, LE-u64 length stream."""
    digest = hashlib.sha256()
    for name, data in sorted(entries, key=lambda item: item[0]):
        encoded = name.encode()
        digest.update(struct.pack("<Q", len(encoded)))
        digest.update(encoded)
        digest.update(struct.pack("<Q", len(data)))
        digest.update(data)
    return digest.hexdigest()


def verify_freeze(source_info, freeze_info, archive_path, binary_path, canonical_root,
                  expected_source=EXPECTED_SOURCE, expected_entry_count=231):
    expected = source_info["source_sha256"]
    assert expected == expected_source
    assert freeze_info["source_sha256"] == expected
    assert file_sha256(archive_path) == freeze_info["source_zip_sha256"]

    with zipfile.ZipFile(archive_path) as archive:
        assert archive.testzip() is None, "frozen source archive CRC failure"
        names = archive.namelist()
        assert len(names) == len(set(names)), "duplicate paths in source archive"
        assert len(names) == freeze_info["source_files"] == expected_entry_count
        entries = [(name, archive.read(name)) for name in names]
        frozen = dict(entries)

    assert stream_source_sha256(entries) == expected
    lock = frozen["Cargo.lock"]
    assert hashlib.sha256(lock).hexdigest() == freeze_info["lock_sha256"]

    fixture_hashes = {}
    for fixture in FIXTURES:
        frozen_bytes = frozen[fixture]
        canonical_bytes = (Path(canonical_root) / fixture).read_bytes()
        assert canonical_bytes == frozen_bytes, f"canonical fixture differs from frozen bytes: {fixture}"
        fixture_hashes[fixture] = hashlib.sha256(frozen_bytes).hexdigest()

    assert fixture_hashes[FIXTURES[0]] == source_info["acceptance_sha256"]
    assert fixture_hashes[FIXTURES[0]] == freeze_info["acceptance_sha256"]
    assert fixture_hashes[FIXTURES[1]] == source_info["opening_sha256"]
    assert fixture_hashes[FIXTURES[1]] == freeze_info["full_opening_sha256"]

    binary = Path(binary_path)
    binary_hash = file_sha256(binary)
    assert binary_hash == source_info["binary_sha256"]
    assert expected.encode() in binary.read_bytes(), "fixed executable lacks frozen source fingerprint"
    return {
        "status": "passed_static_frozen_input_preflight",
        "source_sha256": expected,
        "source_entries": len(names),
        "source_zip_sha256": freeze_info["source_zip_sha256"],
        "lock_sha256": freeze_info["lock_sha256"],
        "fixture_sha256": fixture_hashes,
        "binary_path": str(binary),
        "binary_sha256": binary_hash,
        "binary_embeds_source_fingerprint": True,
        "current_checkout_comparison": "fixtures_only; current non-fixture Rust source intentionally not compared to retired frozen source",
        "cargo_invoked": False,
        "pressure_started": False,
    }


def verify_retry_directory(directory, canonical_root):
    directory = Path(directory)
    source_info = json.loads((directory / "source.json").read_text())
    freeze_info = json.loads((directory / "freeze.json").read_text())
    return verify_freeze(
        source_info,
        freeze_info,
        directory / "frozen-source.zip",
        source_info["binary"],
        canonical_root,
    )


def harness_hashes(directory):
    directory = Path(directory)
    names = ("run.py", "controller.py", "topology.py", "preflight.py", "analyze.py")
    return {name: file_sha256(directory / name) for name in names}


if __name__ == "__main__":
    here = Path(__file__).resolve().parent
    repo = here.parents[2]
    receipt = verify_retry_directory(here, repo)
    receipt["harness_sha256"] = harness_hashes(here)
    (here / "retry-preflight.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))
