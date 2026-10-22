# Retry01实际终态：观察器启动拒绝

20项辅助回归通过后实际启动冻结source32c。观察器PID48002退出2：File exists。控制器错误、observer.stderr、terminal及真实源均保留；未创建任何压力数据或快照，不能签运行/VM/性能。真实Rust native_observer.rs:194独占创建其root，Python helper先创建observed违反此契约。原PID已终态，不重启该句柄。修复放在全新retry02，旧失败不覆盖。
