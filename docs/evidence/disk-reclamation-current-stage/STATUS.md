# 历史验证产物集中清理（v0.5.90）

本次优先清理，未继续开发、构建或新开验证。当前Rust未提交修改及两个未发布实验目录保持原样，不计为阶段通过。约74GB为历史读数，本次不重复计算以前的清理。

## 实测与归因

|项目|本次结果|
|---|---:|
|紧邻删除前 work|24,008,084 KiB / 22.895893 GiB|
|删除后 work|20,112,428 KiB / 19.180706 GiB|
|精确删除文件／去重单链接inode|102／102|
|删除文件分配空间 st_blocks×512|3,989,151,744 B / 3.715187 GiB|
|新增work压力归档|0 B|
|共享target前后|1,400,140 KiB / 1.335278 GiB；删除0 B|
|紧邻删除前后文件系统可用空间|106,247,315,456 → 110,236,594,176 B|
|df观察增量|3,989,278,720 B / 3.715305 GiB|

[原始读数及结果](result.json)、[逐文件清单](deleted.json)、[执行前占用](execute-occupancy.json)。du(work)减少与删除清单分配块之和一致；df多出的126,976B不计入成果。st_blocks不是APFS独占物理块，APFS共享／快照及其他任务可影响df，不能把df增量全部归因。首次只读du(work)24,024,060KiB与删除紧邻前读数间的变化也不计入本次释放。验收日志与本次清理凭据保留在docs，不伪装成work净回收。

## 删除范围与保留证据

|所属轮次|文件数|分配空间|
|---|---:|---:|
|layout-region-l1-smoke-02|27|3,387,432,960 B|
|t13a85-joint-01|50|401,145,856 B|
|steady-controller-smoke-l1-01|25|200,572,928 B|

- `layout-region-l1-smoke-02`：pressure.bin、work-reserve.bin及25份重复合成组件，共27文件。实际16扫描、完整读回及独立进程冷恢复已完成，wrapper退出0；后续全仓回归不依赖这个短测副本。保留[布局报告](retained-layout-report.json)、[冷恢复](retained-cold-restore-report.json)、[原index](retained-snapshot-index.json)、[wrapper原报告](retained-observer-report.json)，原raw/events、models/preview、native-restore-bound和固定源/exe仍在原位置。仅据此解除短测恢复依赖，不签性能门。
- `t13a85-joint-01`两代、`steady-controller-smoke-l1-01`一代：共75份纯合成native/meter/stub组件。这些快照的pressure/reserve早在v87退役，本次不重复计入；组件与`work/retained-component-evidence/duplicate-components-v1`或`duplicate-stubs-v1`的既存完整gzip逐字节核对、长度和SHA与各原index一致后退役。每个文件的归档映射见[清单](plan.json)。无新压力归档。

**这四个快照当前均不能直接完整恢复。** 原index/hash和无损组件归档不替代已删除压力原件。历史脚本引用旧原路径的保留承诺由本报告覆盖，不能将旧脚本直接复跑当成当前恢复能力。原始测量和index不改写。

## 完整保留与解除条件

|保留范围|原因／何时复查|
|---|---|
|formal-L2第7／8代完整快照|现行完整恢复基线；只有后继完整写入、读回、冷恢复和依赖交接成立才可退役|
|t13a50-joint-01旧格式完整快照|兼容恢复尚需原件；旧格式依赖明确解除后复查|
|steady-controller-smoke-l2-01完整故障快照及现场|暖场超时故障对照尚需；诊断结束后再决定原件保留范围|
|layout-region-l1-smoke-01整个pending|真实1MiB分配失败现场，部分pressure原件完整保留供后续调试；它本身不是完整恢复点|
|业务world、WAL、权益收据、CPU参考、negative/fault、旧兼容输入|真实数据或仍有测试依赖；用途不明先保留，不能按t13目录删|
|源码、未提交修改、固定源／依赖zip、验收exe、raw／日志|证据及后续回归依赖；[本轮实验固定来源身份](retained-provenance.json)保持|
|唯一target/release|后续回归继续复用；无独立target或非空增量缓存，不删后再重建同样缓存|

[独立依赖审查](advisor-review.md)未发现候选下游阻断；执行仍另行逐字节核验。执行前两次查ps/lsof、真实runner/模型PID已退出，work/target无打开文件或cwd占用。另一个项目构建仍在运行，只记录comm，不干预其进程／缓存。未清理其他项目或全局Cargo缓存。

保护的四份完整原件在删除前后按原index全文件长度／SHA核对；失败pending全文件SHA保留，见[删除前](protected-before.json)、[删除后](protected-after.json)。其他118,748份work文件身份、长度、mtime、链接数全部不变，本次无保留文件分配块差异。源码前后哈希一致，[前](source-before.json)／[后](source-after.json)；所有删除父目录另行fsync见[parent-fsync.json](parent-fsync.json)。完整目录分布见[盘点后](work-after-du.txt)，用途不明确的剩余内容未删除。

## 后续每阶段规则

继续执行[统一回收规则](../../validation-artifact-retention.md)：新目录先列用途、预期磁盘增量、精确下游输入和解除条件；优先复用既有输入与target。阶段终态先保存必要证据，核查进程/占用/硬链接和恢复依赖，再逐文件清理。正常短测合成副本无依赖即退役；失败原件和必要恢复基线不能仅以hash代替。结束记录释放量、剩余占用、保留原因及下一次复查条件，完成回收决策后才新开下一轮。

此次只发布清理证据与规则。新Rust实验尚未完成同源全仓验证，不随本次提交；三轮稳态、四速、8小时和真实16GB的未完成状态不变。
