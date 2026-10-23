"""Strict source.json identity parsing shared by all control-plane consumers."""
import json
from pathlib import Path

FIXED_SOURCE_KEY = "fixed_source_sha256"
UNSUPPORTED_SOURCE_ALIAS = "source_sha256"


def require_fixed_source_sha256(source, expected=None):
    if not isinstance(source, dict):
        raise ValueError("source.json must contain an object")
    if UNSUPPORTED_SOURCE_ALIAS in source:
        raise ValueError("source.json uses unsupported key source_sha256; expected fixed_source_sha256")
    value = source.get(FIXED_SOURCE_KEY)
    if not isinstance(value, str) or len(value) != 64 or any(c not in "0123456789abcdef" for c in value):
        raise ValueError("source.json fixed_source_sha256 is missing or malformed")
    if expected is not None and value != expected:
        raise ValueError("source.json fixed_source_sha256 does not match the expected frozen source")
    return value


def load_fixed_source_document(path, expected=None):
    document = json.loads(Path(path).read_text())
    require_fixed_source_sha256(document, expected)
    return document
