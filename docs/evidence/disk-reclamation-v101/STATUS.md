# 用户指定历史验证产物集中回收（v0.5.101）

2026-10-22先暂停开发，实测、依赖审查和文件占用检查后精确删除15份文件。此前74GB是历史估计，不是本次起点；过去轮次释放量不计入本次。没有运行模型、构建或恢复测试，没有修改Rust/Cargo WIP。

|指标|删除前|删除后|
|---|---:|---:|
|work|26,220,136 KiB（25.005 GiB）|22,508,968 KiB（21.466 GiB）|
|唯一共享target|1,833,984 KiB|1,810,936 KiB（1.727 GiB）|
|文件系统可用空间|98,642,284,544 B|102,466,465,792 B|

删除文件的inode去重分配空间为 **3,823,837,184 B（3.561 GiB）**，没有新增归档。删除窗口df可用空间增加 **3,824,181,248 B**，独立记录；两者相差344,064B，不把额外变化算成本次回收。st_blocks并不证明APFS独占物理块全部归还，df亦受共享块、快照及其他任务影响。最初读数至prepare的target还有404KiB变化，不属于本次删除；精确执行窗口及全量元数据前后核验单独留存。

## 删除清单与留存证据

|轮次/类别|数量|文件分配空间|理由与保留位置|
|---|---:|---:|---|
|steady-controller-smoke-l2-01首代pressure.bin、work-reserve.bin|2|3,178,258,432 B|暖场4.658秒超过3秒，测量未开始，不是布局故障；后续正式运行完成，advisor解除载荷恢复依赖。原目录业务、pool、暖场时轴、布局index/全部组件、失败日志和对应docs/evidence/m0-t13a-steady-controller保留。|
|旧formal-L2第7/8代、runtime-vm首代、causal-clock首代、snapshot-backing首代native.json/meter.json|10|621,977,600 B|这些正常快照的压力此前已退役；逐一完整解压比对既有gzip字节相同，并核对原index长度/SHA。完整原字节保存在work/retained-component-evidence/duplicate-components-v1；原index、日志和故障pending均保留。|
|v100旧headless rlib/rmeta|3|23,601,152 B|旧依赖图缺当前强制sysctl/rustix，当前图不受影响；保留所有exe、固定源/lock、测试及raw。不是移除后马上重建同一缓存。|

逐文件轮次、路径、inode/mtime/链接数、长度、SHA、分配空间、理由和证据映射见[deleted.json](deleted.json)；[availability.json](availability.json)是本批退役的当前可用性权威，覆盖旧原路径保留承诺。**上述快照均不能直接完整恢复**。从gzip还原native/meter仅还原组件，不补回已退役pressure；hash不代替仍需恢复的原件。历史脚本保留为证据，引用这些原路径时须先处理已退役状态，不得原样声称可重跑完整恢复。

## 安全检查与保留原因

执行前后ps与lsof记录在本目录（进程表仅保留PID/PPID/elapsed/可执行路径，省略无关命令参数）：没有MyCity/Cargo/rustc运行进程，work/target无打开文件，没有停止其他任务。候选均为单链接普通文件，按持久化允许清单逐文件unlink，未批删目录。126,876份其他work/target文件的设备/inode/类型/长度/mtime/分配块/链接数前后一致；源码、Cargo.lock、现有未提交及未跟踪源码的字节一致。见[inventory-before.json.gz](inventory-before.json.gz)、[wip-before.json](wip-before.json)和[result.json](result.json)。

剩余5份pressure及5份reserve完整原件继续保留，包含以下恢复/故障依赖；其所在目录共158份原文件清理前后均重新计算SHA核验，见[protected-after.json](protected-after.json)。没有用hash替换它们。

- typed-workspace-formal-l1-01第7/8代：当前连续快照及完整冷恢复基线，无更新替代。
- t13a50-joint-01首代：旧格式完整兼容恢复原件。
- report-lifetime-formal-l1-01首代与第二代pending：旧动态借还失败对照；首代可作为输入，pending不是恢复点，保留其repay63失败实际文件状态。
- 其他所有pending、negative fixture、业务world/WAL/财务收据、延时原始样本、CPU对照、固定源/依赖/exe、既有无损证据归档：保留证据或后续兼容用途，未确认解除的不删除。没有按目录名或文件名相似性推断重复。
- 当前target继续供build/test/clippy复用；incremental为空，没有独立target或debug可回收项，其余profile/依赖图用途尚在。全局Cargo和其他项目不在范围。

[advisor记录](advisor-review.md)独立检查依赖并释放指定载荷/组件；主任务实际执行逐字节/索引/占用/元数据核验，不把这些执行冒充advisor实测。脚本[reclaim.py](reclaim.py)是本次一次性程序，执行完成后会拒绝再次运行，不泛化为未来批删许可。

后续执行[统一阶段回收门](../../validation-artifact-retention.md)：创建前登记目的、峰值占用和解除条件，终态后先回收或写明保留原因再开下一轮，复用现有target和输入。下一轮native wrapper预计新增目录仍按其retention-intent登记；本次未创建压力副本。

只完成磁盘治理；没有性能通过、8小时运行或真实16GB设备结果，原normal失败、M0未完成和置换额度0均不变。
