# Timeline `EndForExit` primitive

`EndForExit { id }` is a system-authorized, low-level payroll operation for a
fully completed Timeline time point. It stops future work, clears the automatic
payroll due, and moves all currently accrued earnings into `billed`. It never
transfers cash, changes `paid`, clears arrears, or rewrites the contract's
`spec.ends_at`. Thus the unpaid balance remains `billed - paid` and can be
settled later through the existing settlement operation.

Every payroll row must already be accrued through `progress.last_complete`.
An in-flight advance is rejected. A payroll opened and ended at the same
completed instant is allowed but has no wages to bill. A naturally ended row,
or a row already ended by this command, is an unchanged idempotent no-op when
`billed == accrued`; inconsistent ended rows are rejected. Engine checkpoint
export/restore preserves this behavior and does not accrue or pay a second time.
This coverage is not a Store/WAL cold-open test.

The command is not legal approval, contract cancellation, or company closure.
The original contract end remains visible, and this primitive does not model or
waive notice, severance, penalties, taxes, or other future obligations. Those
must be resolved by the higher-level exit workflow before closure.

Source coverage in `crates/city-core/tests/timeline.rs` checks model-level
behavior, including Engine checkpoint export/restore. Durable Store coverage is
in `crates/city-storage/tests/payroll_exit.rs`: it checkpoints and reopens the
world, retries the original operation, settles only the remaining arrear, and
injects precommit (`WalHeaderHalf`) and postcommit (`Applied`) response loss.
The executable scenario is
`crates/city-storage/examples/payroll_exit.rs`; it is a bounded wage/arrear
workflow, not legal company closure. The arrears fixture pays the first 7-day
cycle, then advances only to day 7 at 10:00, adding one work hour (7 cents).

This primitive was executed and verified in release mode at v0.5.120; see
[actual evidence](evidence/core-payroll-exit-integration-120/STATUS.md). To rerun
in a new disposable world directory:

```sh
cargo test -p mycity-storage --test payroll_exit -- --nocapture
cargo run -p mycity-storage --example payroll_exit -- /tmp/mycity-payroll-exit-demo
```

The tests distinguish Engine checkpoint serialization from `Store::checkpoint`
followed by `Store::open`; the latter exercises the persistent store and WAL.
Neither source existing nor a Store/WAL in-process observer test proves an
independent OS-process termination test for this exact command.
