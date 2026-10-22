# 当前历史产物集中复查（2026-10-22）

本次先暂停新增构建，检查进程及 work/target 的完整 lsof。未发现占用；未停止其他项目。construction owner 已确认其新兼容档、失败日志与源码 WIP 都在隔离 worktree，仍需后续验证，保留。

本次新增删除 **0 文件、0 B**。此前 v105 已退役 154 文件、7,550,910,464 B；v106 随后回收新增诊断 54 文件、6,748,618,752 B。本次不重复计算这些历史成果，不将约74GB旧读数与当前读数相减宣称本次释放。

当前六份 pressure.bin 与对应 reserve 全部命中190份受保护恢复/故障原件。全部原文件重新读取计算SHA，逐项匹配既有清单，未用hash替换原件。typed正式第7/8代为完整基线；v50为旧格式恢复输入；report-lifetime首代及pending第二代为恢复/故障对照，pending不是合法恢复点；r07-report-error第二代为最终v3兼容输入。解除依赖并实际交接前不删除。

其余较大占用分类：compute-durations.bin为原始延时样本，非构建缓存；native/meter、业务world、账本/WAL、收据及故障输入仍是证据或兼容输入，不按名称删除。候选可执行文件、固定源码/依赖快照保留。用途未确认的历史中间档保留到具体依赖解除；不据目录日期或t13前缀批删。已退役旧快照的直接恢复承诺仍以原availability清单为准，本次未恢复其可用性。

唯一共享target仍用于core/storage、traffic/economy及待验证extent的build/test/clippy图；当前不能证明其中某一编译图已不再使用，保留复用。没有新独立target，也不清理全局Cargo缓存。源码、未提交改动和各隔离分支均未删除。

`result.json`记录本次work/target前后du及独立文件系统可用空间；删除清单`deleted.json`为空。`inventory.json.gz`为逐文件元数据，`by-round.json`按轮次汇总inode去重的st_blocks；这是文件分配空间，不是APFS独占物理空间。`protected-verified.json`为190原件的本次重读验证；原件均仍存在。df变化不能归入本次0B删除成果。

后续阶段执行已有[回收规则](../../validation-artifact-retention.md)：新大目录先登记用途、预期峰值、精确下游路径与解除事件；共享target和既有输入优先复用；失败/中断亦须终态留存、核对原件和占用后按精确允许清单回收，完成决策再开启下一大轮。正式性能测量不与构建、盘点或清理并发。必要完整恢复原件必须实际完成替代验证和明确解除依赖后才能退役。

本次没有运行模型或性能验收，没有新增M0通过声明。
