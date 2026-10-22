# 2MiB observed-ceiling retry

New unique run after the controller rejected legitimate 2MiB maps under a stale 1MiB observer type bound. Candidate Rust/source/CLI/input/normal thresholds unchanged. Observer extent ceiling now matches authenticated Rust MAX_EXTENT_QUANTA32; tests accept exactly2MiB and reject an extra64KiB. This changes format validation, not performance budget. Prior failure and postfailure full cold restores remain in joint01. Never restart prior PID or replace prior data. New stage starts only after prior retention decision is durable.

实际运行已完成，见 [结果与回收](STATUS.md)。normal仍失败，两代本轮快照已退役，不再直接完整恢复。
