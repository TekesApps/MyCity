#!/usr/bin/env python3
"""Write a deterministic SHA-256 manifest for this compact evidence publication."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent
OUTPUT = ROOT / "publication-manifest.json"


def main() -> None:
    entries = []
    for path in sorted(ROOT.rglob("*")):
        if not path.is_file() or path == OUTPUT or path.is_symlink():
            continue
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        entries.append(
            {
                "path": path.relative_to(ROOT).as_posix(),
                "bytes": path.stat().st_size,
                "sha256": digest.hexdigest(),
            }
        )
    manifest = {
        "schema": "mycity-compact-evidence-publication-v1",
        "scope": "all regular files in this folder except publication-manifest.json",
        "files": entries,
    }
    OUTPUT.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(f"wrote {OUTPUT} with {len(entries)} entries")


if __name__ == "__main__":
    main()
