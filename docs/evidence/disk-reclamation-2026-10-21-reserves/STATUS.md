# 历史验证预留页回收（v0.5.72）

本轮先停止开发与验证负载，检查进程、cwd、打开文件及后续恢复依赖后，仅删除六份确认无依赖的旧合成预留页。此前74GB为历史估计；本轮实测work为17.92GiB，过去已删除的压力数据不重复计入。

| 项目 | 本次实测 |
|---|---:|
| work清理前 | 18,785,240 KiB，17.915 GiB |
| work清理后 | 17,998,808 KiB，17.165 GiB |
| 删除分配空间／逻辑长度 | 805,306,368 B，768 MiB（6个单链接inode） |
| work的du降幅 | 805,306,368 B |
| 文件系统可用空间观察增量 | 805,335,040 B |
| df窗口末可用空间 | 118,283,776,000 B，110.160 GiB |
| 共享target保留 | 758,160 KiB，740.391 MiB，删除窗口未变 |

分配空间按设备／inode去重的st_blocks计算，不证明APFS独占物理块。df增量比删除分配空间多28,672B，不归因于本次回收；APFS共享、系统快照及其他任务仍可影响可用空间。本轮不删除target、不建立新target、不清理其他项目或全局缓存。

## 精确删除范围

每个轮次各删除snapshot-000001与snapshot-000002中的`model/layout/snapshots/<snapshot>/work-reserve.bin`，每份128MiB：

| 轮次 | 删除理由 | 保留证据 |
|---|---|---|
| t13a64-joint-01 | 旧基线已替代、pressure此前已退役，无后续reserve恢复依赖 | [原始专题](../m0-t13a-hotpath/STATUS.md)及原work中native/meter/index等 |
| t13a65-joint-01 | 首次实验已终止、pressure此前已退役，无后续reserve恢复依赖 | [失败记录](../m0-t13a-helper-compare/failure-notes.md)及原日志、组件文件 |
| t13a65-joint-02 | 已被完整v70基线替代、pressure此前已退役，无后续reserve恢复依赖 | [原始专题](../m0-t13a-helper-compare/STATUS.md)、原业务world与fault-reference |

[逐文件身份、理由及保留证据](deletion-result.json)，[advisor独立依赖审查](advisor-review.md)。**以上六个旧layout快照此前即不完整，现仍不能直接完整恢复。** 原index／hash仅为历史身份；需要对应版本生成器、原始布局、种子和迭代重建并完整校验后，才可重新声明可恢复。原manifest及历史结果不改写。

## 完整保留及核验

- `work/t13a70-joint-01`：当前最新完整两代布局快照、实际业务、故障和所有原始日志。
- `work/t13a50-joint-01/model/layout/snapshots/snapshot-000001`：仍被引用的旧格式完整恢复依赖；同轮.pending故障证据也保留。
- `work/t13a65-joint-02/model/business`及`work/t13a68-legacy-reopen`：真实业务world／WAL／收据／fault-reference与完整兼容副本。
- 全部源码、未提交改动、固定源码／依赖快照、验收可执行文件、日志和故障证据均保留。native／meter等用途涉及组件核验，尚未证明替代证据等价，本轮保留。
- 共享target用于后续构建和旧库兼容链接；未发现可明确退役的独立临时target／增量缓存，保留复用。

三个保护布局快照各29个原index文件全部存在，长度和SHA256均匹配，见[完整性核验](protected-layout-check.json)。这只是保留检查，不算新恢复／性能验收。删除前后其余85,125个work文件设备／inode／长度／mtime／分配块／硬链接数不变；[压缩盘点](work-inventory-before.json.gz)保存每文件原始元数据。没有把hash替代仍需恢复的原文件。

[进程检查](processes-before.txt)与[相关cwd／打开文件](open-files-before.txt)未发现占用work或target的进程；无运行中的MyCity负载，没有终止其他任务。实际删除以精确清单及硬链接检查为边界，不按目录名前缀批删。

## 后续阶段回收

执行[阶段回收规则](../../validation-artifact-retention.md)。每阶段终态先归档再检查依赖与占用，按精确允许清单清理；最新完整基线、旧格式兼容及实际业务默认保留。native／meter等须逐项确认组件／故障证据替代和引用解除后再复查；完整基线须有实测替代才可退役。继续复用共享target，不在性能测量中清理，也不为清理后立刻重建同样缓存制造释放量。

本轮不修改Rust游戏模型、不增加性能通过声明；M0-T13-A、正式8小时及真实16GB设备门保持原状态，未测不算通过。
