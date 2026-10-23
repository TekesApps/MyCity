# M0 T13-A control 05 status

State: observer source-key repair prepared; no admission wait, model launch, Cargo invocation, or pressure sampling has occurred. Source and fixed binary remain bound to the v132 final integrated descriptor; this new control does not alter or inherit j3n evidence.

The preserved j3n attempt passed the 120-second normal admission, then the observer raised `KeyError: source_sha256` before collecting a frame; its child was terminated with signal 15. The startup path now requires `fixed_source_sha256`, cross-checks the run-supplied expected frozen hash, and rejects missing, wrong-name, malformed, or mismatching source identities before creating the run directory or spawning the child. Unit tests cover these branches. `run.py`, `preflight.py`, `log_budget.py`, and `observe.py` use the same canonical source key via the lightweight `source_identity.py`; the fixed support manifest now binds 17 files (previous 16 plus helper).

New future roots are `j3p-01` (smoke) and `j3q-01` (formal), each retaining the six-character component used by the socket-path contract. Static socket inventory is regenerated for those roots. The original 2h/4h/6h/8h fault/restore timeline, 4Hz/120s admission, workload, budgets, and thresholds remain unchanged. Run pure Python checks and inspect their receipts before any separate authorization to exercise observer failure/exit paths or start a measured run.

## Pure validation receipts

- `python3 -m unittest discover -s . -p 'test_*.py' -v`: 18 tests passed, including observer startup with the valid canonical key and rejection of missing, wrong-name/legacy, alias-ambiguous, and mismatched source identities.
- `python3 preflight.py`: passed static frozen-input and support checks, 276 archive entries byte-bound to v132, 17 support-file hashes verified, and all four invocation/resolved socket paths for `j3p-01`/`j3q-01` under the 104-byte `sun_path` limit (76/77 bytes including NUL).
- No Rust/Cargo command, admission wait, observer/model process, or pressure sampling was run. The new `j3p-01`/`j3q-01` roots were not created. A separate observer lightweight failure/exit validation remains for root before any measured run.
