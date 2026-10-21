# v70历史重复组件集中回收

2026-10-22，先暂停开发，检查进程、cwd、打开文件及后续恢复依赖。准备与删除前均无MyCity模型进程，work／target无占用；没有终止其他任务或运行Cargo／性能负载。此前约74GB和历史回收不重复计入。

|口径|实测|
|---|---:|
|删除前work|14,781,776 KiB，14.097 GiB|
|删除后work|14,390,032 KiB，13.723 GiB|
|删除文件／独立单链接inode|50／50|
|删除文件分配空间|401,145,856 B，382.562 MiB|
|新增work归档|0 B，复用已有完整gzip|
|唯一共享target前后|1,399,788 KiB，1.335 GiB，删除0 B|
|df可用空间|119,466,823,680 → 119,868,055,552 B|
|df观察增量|401,231,872 B，独立记录，不全部归因于清理|

分配空间按删除前st_blocks×512、设备／inode去重统计，不证明APFS独占物理块归还。df受共享块、快照及其他任务影响，不能全部算作本次成果。删除前后读数均已先持久化，见[result.json](result.json)及delete-before／after-metrics.json。

## 删除清单与证据

仅t13a70-joint-01/model/layout/snapshots下snapshot-000001和000002，各25个重复合成组件，共50个。其pressure／reserve此前已退役，v85实际联合恢复已经替代；此次只删除与已有gzip完整解压字节一致、旧index长度／SHA相符、单链接且无占用的文件。未按目录前缀批删。

native、meter及23类stub是L1压力替身，不是实际市民或企业存档。[plan.json](plan.json)与[deleted.json](deleted.json)逐文件记录轮次、路径、inode、分配空间、旧index及完整归档映射。准备和执行均实际解压逐字节核对。

完整字节仍在work/retained-component-evidence/duplicate-components-v1的native.json.gz／meter.json.gz及duplicate-stubs-v1的23份gzip。保留index、models、preview、native-restore-bound及原日志／业务。**50个组件原路径已删除；v70此前即不完整，现在仍不能直接完整恢复。**归档只能提取组件，不能重建pressure／reserve。

## 保留与核验

- v85 joint-01两代及v50 joint-01旧格式snapshot-000001，三份各29项完整原文件前后全长度／SHA验证，见protected-before／after.json。3份pressure共9,132,122,112 B及3份reserve共402,653,184 B仍是恢复依赖，原样保留；须新替代基线实际通过且解除依赖后才可退役。
- 真实业务world、WAL、资金／权益收据、旧兼容world、当前恢复输入、fault-reference、pending故障现场、negative与硬链接反例保留。固定源码／依赖快照、验收exe、原始延时样本、观测日志和故障证据保留；用途不明暂保留至精确依赖确认。
- 共享target/release/incremental实测0 KiB；target主要是仍需复用的deps及可执行文件。未发现确认无用的独立target或过期缓存，不删除马上需要重建的缓存。未清理其他项目或全局Cargo。
- 其余110,899个work文件的设备、inode、长度、mtime、链接数及分配块逐项未变；无新增work文件。source-before／after.json确认源码、Cargo.lock、AGENTS和未提交代码字节一致。只提交清理文档，Rust控制器WIP不混入发布。

## 历史入口影响与后续规则

[advisor审查](advisor-review.md)未发现当前crates依赖v70组件；role-arena/diagnostic_audit.py只读取保留的v70 index。历史disk-reclamation-duplicate-components/reclaim.py、disk-reclamation-2026-10-22/reclaim.py把v70原件作为CANON，不能按历史原路径完整复跑。旧background-resident/NEXT.md的v70完整保留承诺由本availability覆盖；旧verify.py只读日志／目录集合，打印的历史冷恢复结果不证明现在可恢复v70。历史测量、index不改写。

执行[统一回收规则](../../validation-artifact-retention.md)：创建新目录前记录用途、精确下游输入和解除条件；复用既有输入及唯一target。阶段终态后先归档固定源／依赖、exe、日志／收据与故障证据，再核查替代恢复基线、进程／cwd／占用和硬链接，按精确清单删除、核验保护文件并独立记录分配空间／du／df。每阶段完成回收决策后再开下一轮，测量期间不清理。

本次仅完成磁盘回收及留存核验；正式8小时、真实16GB和完整M0状态不变。稳态控制器Rust WIP保持原样。
