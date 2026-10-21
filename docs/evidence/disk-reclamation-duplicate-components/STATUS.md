# 历史验证重复组件集中回收

先暂停开发与验证，按运行进程、cwd／打开文件、后续恢复依赖、文件类别和硬链接检查，再执行精确清单。没有停止其他项目进程，没有启动 Cargo 或模型负载。原约 74GB 为旧估计，此前回收不计入本次成果。

## 当前实测

| 口径 | 本次结果 |
|---|---:|
| 准备前 work | 18,293,564 KiB，17.446 GiB |
| 新增完整组件无损归档 | 26,718,208 B，25.480 MiB |
| 删除前 work（含新归档） | 18,319,656 KiB |
| 删除后 work | 15,525,616 KiB，14.806 GiB |
| 精确删除／设备与 inode 去重 | 46 文件／46 单链接 inode |
| 删除文件分配空间 | 2,861,096,960 B |
| 扣除归档后净分配减少 | **2,834,378,752 B，2.640 GiB** |
| 删除窗口 df 可用空间观察增量 | 2,860,109,824 B，独立记录、不全部归因于清理 |
| 删除窗口末 df 可用空间 | 117,111,406,592 B |
| 共享 target | 1,090,804 KiB，删除窗口不变；删除 0 B |

`st_blocks × 512` 代表文件分配空间，不证明 APFS 独占物理块已归还。df 还受 APFS 共享块、快照及其他任务影响。准备前至结束的 df 变化与删除窗口变化也不同，原始读数全部保存在 [prepare-metrics.json](prepare-metrics.json) 与 [result.json](result.json)，不把其差额或历史删除量计作本次成果。

## 删除范围与证据保留

仅删除 [14 轮简表](removed-rounds.md) 对应的 46 份 `native.json`／`meter.json`，这些正常快照的 pressure 此前已经退役。不是按 t13 目录名批删。逐字节比对确认所有候选分别与 v70 保留原件一致，归档后实际解压再次核对完整字节、长度及 SHA；归档、父目录、精确计划、审查记录均先持久化再 unlink。执行前再次复查进程／占用和文件身份。

- 完整原字节归档：`work/retained-component-evidence/duplicate-components-v1/native.json.gz`、`meter.json.gz`。归档总分配占用约 25.48 MiB，而不是 hash 替代原文件。v70 原始组件另完整保留。
- [plan.json](plan.json)／[deleted.json](deleted.json)：每个原路径、所属轮次／快照、原 index 路径及 SHA、文件长度／SHA、inode／mtime／分配块、归档映射和理由。原 manifest、历史测量、日志和收据均未改写。
- 所列旧组件**原路径已不存在，旧布局不能直接完整恢复**。必要时仅可从 gzip 在新的临时目录提取对应组件并核对；这既不重建旧 pressure，也不恢复完整快照可用性。完整归档本身列为证据保留，不纳入临时缓存清理。
- [advisor 独立审查](advisor-review.md)实际检查计划、脚本和引用，不将主线程解压／SHA 检查冒充 advisor 实测。脚本引用清单含仍有效的 business／fault-reference 依赖，相关业务文件完整保留。

## 完整保留与核验

- `work/t13a70-joint-01/model/layout/snapshots/snapshot-000001`、`snapshot-000002`，以及 `work/t13a50-joint-01/model/layout/snapshots/snapshot-000001`：三份各 29 个索引原文件完整保留，清理前后全部重新核对长度及 SHA，见 [protected-before.json](protected-before.json)、[protected-after.json](protected-after.json)。仍是联合恢复和旧格式兼容输入；只有替代基线实测通过且依赖解除后才可退役。
- 所有 `.pending`、negative fixture、非单链接文件保留。故障／损坏索引／硬链接关系不可因组件内容相同而删除。
- 实际业务 world、WAL、收据、权益、fault-reference、v68 完整兼容副本及 v76／v77 业务恢复输入保留。经济和恢复输入不按压力替身处理。
- 全部固定源码／依赖快照、验收可执行文件、必要日志和故障证据保留。92,533 个其余既有 work 文件元数据逐项未变，原始盘点见 work-before.json.gz；无目录批删。
- 当前 Rust／Cargo.lock／未跟踪新源码的状态和字节指纹前后相同，见 source-wip-before.json、[source-wip-after.json](source-wip-after.json)。本次发布只提交回收文档／证据，不提交尚未完成的模型 WIP。
- 共享 target 保留复用，incremental 无有量缓存，未确认有无需再用的独立 target；其余组件用途不明则保留到引用／替代关系明确。没有清理其他项目或全局 Cargo 缓存。

## 后续规则与验收边界

执行[阶段回收规则](../../validation-artifact-retention.md)：终态 → 归档固定源码／依赖／exe 和必要结果 → 明确下一阶段精确输入 → 检查进程、cwd、打开文件、硬链接 → 持久化精确退役清单 → 删除 → 核验保护文件 → 分别记 st_blocks、du、df。重复输入优先复用；需要复制的须预先写明终态退役条件。每阶段报告回收项或保留理由及解除条件。

此阶段只完成磁盘回收及证据可用性核验，不是模型、性能或恢复运行测试；正式 8 小时、完整 M0 和真实 16GB 设备门状态不变。角色 workspace 仍是未提交、未完成的开发阶段。
