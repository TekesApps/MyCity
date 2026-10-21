# 原L1 formal实际失败

固定来源66e7e2aee0464d302738c4a1d36cfe2b2e7d17de7ac6f8d088330a2d85629138，原300秒暖场+1800秒测量；实际约308.52秒后第二次快照1MiB分配失败、子进程SIGABRT，model exit2。完整测量窗口没有完成，不计正式通过。见[failure-facts](failure-facts.json)及work/causal-clock-formal-l1-01/observed原始报告/日志。

首代完整快照及第二代pending原件保留；pending不是恢复点。第二代已有native-restore-bound、没有native-roundtrip和index，不能从上一帧free推断OOM现场或具体分配位置。未清理故障文件，也未解除旧formal7/8恢复基线依赖。

后继修复与实际复测见[重复快照工作区](../m0-t13a-snapshot-backing/PLAN.md)，不可将短测成功或静态审查代替实际跨第二次快照验证。
