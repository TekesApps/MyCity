# 历史验证产物集中清理后的现状复查

先检查进程、cwd和打开文件，再检查恢复依赖及完整原文件。当前无MyCity模型负载、无work或target占用；没有停止其他项目任务。此复查不启动构建或性能负载、不修改模型。此前74GB是历史估计，不能作为当前删除量的基准。

## 当前实测与本阶段删除

| 口径 | 结果 |
|---|---:|
| 当前work | 18,039,576 KiB，17.204 GiB |
| 当前共享target | 1,019,624 KiB，995.727 MiB |
| 当前复查新增删除 | 0 B |
| 本阶段已退役的临时WAL副本 | 245文件／245单链接inode，10,207,232 B，9.734 MiB |
| 该删除窗口work变化 | 18,049,544 → 18,039,576 KiB |
| 该删除窗口df可用空间观察增量 | 16,805,888 B |
| 超出文件分配量的df变化 | 6,598,656 B，不计作本次成果 |
| 此次只读复查窗口df变化 | -2,899,968 B，未删除任何文件，不归因于清理 |
| 复查末文件系统可用空间 | 121,737,216,000 B |

删除量来自设备／inode去重的st_blocks，不证明APFS独占物理块实际归还量；df受共享块、快照、其他任务影响。目录du和文件分配空间分别记录，不混用。共享target保留复用；未新建独立target、未清全局缓存或其他项目。

## 删除清单与证据位置

本阶段只删除`work/t13a74-wal-roundtrip`内验证完成的临时fork原文件，逐文件身份和理由见[精确计划](../m0-t13a-wal-canonical/roundtrip-retirement-plan.json)及[实际结果](../m0-t13a-wal-canonical/roundtrip-retirement.json)。实际WAL帧、重开／重试和两次检查点的结果、固定源码／Cargo.lock／验证可执行文件及日志保存在[WAL专题](../m0-t13a-wal-canonical/STATUS.md)。该临时fork已不存在，**不能直接重开恢复**；原业务world与完整兼容副本保留，hash没有替代原文件。

较早清理已分别发布，以下仅索引，不将它们再次计为本次新增释放：

| 轮次范围 | 已记录分配空间 | 理由及保留证据 |
|---|---:|---|
| 首批历史旧轮次压力及预留页 | 62,448,984,064 B | [逐轮简表](../disk-reclamation-2026-10-21/removed-runs.md)、[原结果](../disk-reclamation-2026-10-21/deletion-result.json) |
| v64与失败v65第一轮四份pressure | 12,176,162,816 B | [旧基线替代及失败证据](../disk-reclamation-2026-10-21-followup/STATUS.md) |
| 旧v66两份pressure | 6,088,081,408 B | [v70实际完整基线替代及原业务保留](../m0-t13a-background-resident/STATUS.md) |
| v64、v65第一轮、v66六份reserve | 805,306,368 B | [精确路径、理由及证据](../disk-reclamation-2026-10-21-reserves/STATUS.md) |
| v73临时保存往返副本 | 10,199,040 B | [验证终态及临时副本退役](../m0-t13a-store-stream/STATUS.md) |

被退役pressure/reserve的旧布局继续不可直接完整恢复，原manifest及历史测量不改写。不是按t13目录名删除。

## 保留范围及原因

- v70两代完整布局和v50旧格式完整布局：三份pressure共9,132,122,112 B，三份reserve共402,653,184 B；仍是下一阶段联合验证／旧格式恢复输入。各29个索引原文件均已重算SHA并核对长度，见[保留核验](protected-inputs.json)。须有实测替代基线、解除恢复依赖后才能退役。
- `work/t13a65-joint-02/model/business/world`与`work/t13a68-legacy-reopen/world`：各243个原文件完整、逐SHA一致。真实业务、WAL、收据及权益不当合成压力删除。
- 29份native、29份meter、historical .pending、逐任务compute-durations及其他日志：仍承载组件核验、性能原始样本及故障证据；尚未证明有等价保留证据或后续无依赖。按具体引用解除后复查，不能凭名称视为重复。分配空间分组见[盘点](largest-file-groups.json)。
- 源码、未提交改动、固定源／依赖快照、验收exe均保留。共享target约996MiB，无有量的incremental；当前库和历史证明关联库的依赖未逐项解除，不为清理而删后立即重建。

85,131个work原文件在只读复查前后元数据均未变，见[结果](audit-result.json)、[进程](processes.txt)、[本项目cwd及打开文件](project-open-files.txt)和[advisor独立复核](advisor-review.md)。文件完整性核验不算重新运行恢复、经济或性能验收，M0未完成／16GB未测状态不变。

## 后续回收

执行[阶段回收规则](../../validation-artifact-retention.md)：终态 → 归档必要证据／固定exe和源码依赖 → 核查下一阶段恢复输入 → 检查进程／cwd／占用／硬链接 → 精确清单删除 → 核验保护文件 → 分别记录st_blocks、du和df。复用唯一共享target，不在性能测量中清理；当阶段没有可确认项，记录保留原因和下次解除条件，不默认开新目录长期留副本。
