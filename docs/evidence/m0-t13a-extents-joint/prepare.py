"""Freeze a caller-selected exact source and already-built binary; never builds."""
import argparse
import hashlib
import json
import os
import struct
import time
import zipfile
from pathlib import Path

D = Path(__file__).resolve().parent
R = D.parents[2]
WORK = R / "work/m0-t13a-extents-joint-l1-01"
FIXED_BINARIES = R / "work/candidate-binaries/r07-extents"


def source_files():
    roots = [".cargo/config.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]
    roots.extend(
        path.relative_to(R).as_posix()
        for base in ("crates", "fixtures")
        for path in (R / base).rglob("*")
        if path.is_file()
    )
    return sorted(roots)


def source_digest(files):
    digest = hashlib.sha256()
    for name in files:
        data = (R / name).read_bytes()
        encoded = name.encode()
        digest.update(struct.pack("<Q", len(encoded)))
        digest.update(encoded)
        digest.update(struct.pack("<Q", len(data)))
        digest.update(data)
    return digest.hexdigest()


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--expected-source-sha256", required=True)
    parser.add_argument("--binary", required=True, type=Path)
    args = parser.parse_args()

    assert not (D / "freeze.json").exists(), "evidence directory already initialized"
    assert not WORK.exists(), f"unique work path already exists: {WORK}"
    assert args.binary.is_file() and os.access(args.binary, os.X_OK)

    files = source_files()
    source = source_digest(files)
    assert source == args.expected_source_sha256, (source, args.expected_source_sha256)
    fixed_exe = FIXED_BINARIES / source / "mycity-headless"
    assert not fixed_exe.exists(), f"fixed executable path already exists: {fixed_exe}"
    binary_hash = sha256(args.binary)
    binary_bytes = args.binary.read_bytes()
    assert source.encode() in binary_bytes, "binary does not embed the frozen source hash"

    freeze_zip = D / "frozen-source.zip"
    with zipfile.ZipFile(freeze_zip, "x", zipfile.ZIP_DEFLATED) as archive:
        for name in files:
            archive.write(R / name, name)
    acceptance = R / "fixtures/m0-t13a-acceptance.json"
    opening = R / "fixtures/m0-t13a-full-opening-candidate.json"
    freeze = {
        "source_sha256": source,
        "source_files": len(files),
        "lock_sha256": sha256(R / "Cargo.lock"),
        "acceptance_sha256": sha256(acceptance),
        "full_opening_sha256": sha256(opening),
        "source_zip_sha256": sha256(freeze_zip),
        "started_unix_ns": time.time_ns(),
        "shared_target": "/Users/frankqin/dev/MyCity/target/release",
        "original_load_thresholds_changed": False,
        "cargo_invoked": False,
        "pressure_started": False,
    }
    (D / "freeze.json").write_text(json.dumps(freeze, indent=2) + "\n")

    fixed_exe.parent.mkdir(parents=True, exist_ok=True)
    os.link(args.binary, fixed_exe)
    source_stat = os.stat(args.binary)
    fixed_stat = os.stat(fixed_exe)
    assert (source_stat.st_dev, source_stat.st_ino) == (fixed_stat.st_dev, fixed_stat.st_ino)
    (D / "source.json").write_text(
        json.dumps(
            {
                "source_sha256": source,
                "source_files": len(files),
                "binary": str(fixed_exe),
                "binary_sha256": binary_hash,
                "lock_sha256": freeze["lock_sha256"],
                "acceptance_sha256": freeze["acceptance_sha256"],
                "opening_sha256": freeze["full_opening_sha256"],
                "scope": "same original M0-T13-A full L1 inputs and thresholds; exact integrated source includes core model and role-wiring changes; all raw costs included; no performance acceptance",
            },
            indent=2,
        )
        + "\n"
    )
    print(source)


if __name__ == "__main__":
    main()
