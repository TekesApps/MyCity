# 历史验证产物集中复查：当前实际状态

先暂停开发，检查全部进程、cwd／打开文件、恢复依赖及硬链接；没有停止其他任务，也没有启动构建或模型负载。此前约74GB是历史读数，本次不重复计入已发布清理成果。

| 口径 | 本次实测 |
|---|---:|
| work 前后 du | 15,631,264 → 15,631,264 KiB（14.907 GiB） |
| 共享 target 前后 du | 1,292,660 → 1,292,660 KiB（1.233 GiB） |
| 本次删除文件／分配空间 | **0 文件／0 B** |
| work 文件／去重 inode 分配空间 | 96,256 文件／16,006,414,336 B |
| df 可用空间前后 | 114,799,099,904 → 114,797,322,240 B |
| df 观察变化 | -1,777,664 B；未发生删除，不能归于回收 |

`st_blocks × 512` 是文件分配空间；按设备／inode去重，并非APFS独占物理空间。du含目录空间，df另受共享块、快照及其他任务影响。没有将它们相加或将历史释放量重算为本次成果。

## 分类结论及保留理由

- 当前仅三份 pressure.bin，共9,132,122,112 B；分别在 `work/t13a70-joint-01/model/layout/snapshots/snapshot-000001`、`snapshot-000002` 和 `work/t13a50-joint-01/model/layout/snapshots/snapshot-000001`。它们和三份 reserve、全部原始组件仍是下一阶段联合恢复／旧格式兼容输入，完整保留；各29项索引原文件实际重新核对长度及SHA。无已解除依赖的旧pressure可再删。
- `work/t13a65-joint-02/model/business/world` 与 `work/t13a68-legacy-reopen/world` 各243项对固定manifest逐文件SHA核验通过。v76、v77及当前 `work/t13a-role-spatial-business-01/world` 业务输入继续保留；不是压力替身，不以可再生成推定可删。
- 其余大体积 native／meter 归属上述完整基线、故障 `.pending` 或硬链接negative fixture；故障现场、坏索引及链接关系保留。已退役旧正常快照的重复组件此前已无损归档并删除，见[历史清单](../disk-reclamation-duplicate-components/removed-rounds.md)，不重复计成果。
- compute-durations.bin 是原始逐任务延时样本；业务JSON、WAL、收据、权益和归档不能按扩展名作为缓存删除。其余用途／替代关系不明确的文件继续保留，解除条件是原始验收证据等价保留且后续恢复引用明确取消。
- 固定源码／依赖快照、验收exe、必要日志、故障证据和完整无损组件归档保留。当前源码及未提交实现逐SHA核验未变；本次仅提交盘点与文档，不夹带未完成模型代码。
- 共享target继续复用；incremental、debug、tmp分配占用均为0，没有确认可删的独立target。release依赖供当前Meta／legacy后续验证使用，不删除后立即重建来制造回收。

[删除清单](deletion-manifest.json)为空；[进程及占用](process-check.json)无活动模型及work／target占用。[完整输入核验](protected-inputs.json)、[逐文件原盘点](work-inventory.json.gz)、[源码保留核验](source-preservation.json)和[原始测量](audit-result.json)保留。盘点前后96,256个work文件元数据不变。此前被删载荷的旧布局仍**不能直接完整恢复**；hash不代替恢复原件。

## 后续阶段回收

执行[统一回收规则](../../validation-artifact-retention.md)：终态后保留日志／收据／故障及固定源码、lock、exe → 列出下一阶段精确恢复输入 → 复查进程、cwd、占用、硬链接 → 精确允许清单删除 → 核验保护文件 → 分列分配空间、du、df。每个新测试目录在创建前注明用途、依赖和退出回收条件；优先直接复用已有输入和唯一target。当前三份完整布局须等替代基线真实通过并解除依赖才可退役。测量期间不清理。

本次是磁盘和保留状态检查，不是重新运行模型／恢复验收。正式8小时、完整M0和真实16GB设备门均没有因本次复查通过。
