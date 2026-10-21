# 2026-10-21 历史验证产物回收

本轮等待联合诊断正常退出，再归档本轮证据、复查进程和打开文件，无模型运行或文件占用后才执行精确清单。advisor 独立核查恢复依赖，未发现保护清单外必须保留的完整恢复基线。未停止其他任务，未修改游戏代码。

| 测量 | 结果 |
|---|---|
| 清理前 work | 90,838,548 KiB，约 86.63 GiB |
| 清理后 work | 29,853,212 KiB，约 28.47 GiB |
| 删除分配空间，按 inode 去重 | 62,448,984,064 bytes，约 58.16 GiB |
| 文件系统可用空间变化，观察值 | +62,458,707,968 bytes，约 58.17 GiB |
| 清理后文件系统可用空间 | 106,846,334,976 bytes，约 99.51 GiB |
| 保留共享 target | 698,224 KiB，约 681.86 MiB，清理窗口未变化 |

删除 41 个路径、39 个 inode；两组硬链接各有两个路径，只计算一次。work 的 du 降幅与删除分配空间一致。df 增幅另多 9,723,904 bytes，可能受 APFS／其他任务影响，不计为本次成果；st_blocks 也不证明 APFS 独占物理块。

## 删除范围与证据

只移除已终止旧轮次的 `pressure.bin`／`work-reserve.bin`，包括无后续完整恢复依赖的历史压力副本和失败 `.pending` 的压力部分；没有按目录批删。简短轮次清单见 [removed-runs.md](removed-runs.md)，逐文件原身份、删除理由及证据位置见 [deletion-result.json](deletion-result.json)。生成器仍在对应版本源码快照中；恢复已退役载荷需要重新生成并核对完整原索引，不能用 hash 代替文件。

对应专题新增 availability 清单并在 STATUS 顶部覆盖旧保留承诺；原始 manifest／hash 不改写。所列快照不能直接完整恢复，保留 native／stub 和 index 只能支持组件核验；失败 pending 从未是完整恢复点。历史成功恢复仍是当时发生过的事实。

## 保留及验证

- 完整保留 `t13a65-joint-01/02` 当前阶段、`t13a64-joint-01` 最新发布基线两代，以及 `t13a50-joint-01/model/layout/snapshots/snapshot-000001` 旧格式兼容恢复依赖。完整原文件没有用 hash 替代。
- 保留各轮真实业务存档／WAL、native／stub 组件、索引、日志、收据、故障证据、源码／固定依赖快照和验收可执行文件。用途不明确的内容保留。
- 80,874 个未删除 raw 文件的 inode／长度／mtime／分配块／链接数前后不变。7 个保护快照逐索引文件存在且长度符合原索引，见 [protected-snapshots-check.json](protected-snapshots-check.json)；此检查不是新一次完整恢复或全量重哈希验收。
- 共享 target 仍供后续构建使用；未发现 work 内独立 target，未删除其缓存或全局 Cargo 缓存。没有为本次检查新建 target。
- 未提交 Rust 改动保留。本次只验证回收及保留，不代表 T13 性能或完整 M0 通过。

## 后续回收

执行 [验证产物回收规则](../../validation-artifact-retention.md)：每阶段结束归档后按依赖退役旧合成压力文件，保留明确的当前／最新／兼容恢复基线；复用 target，测量期间不清理。每阶段应记录“已回收”或保留理由与下次复查条件。

## 记录口径

`inventory-before.json` 是运行期间初次盘点，按路径求和，含硬链接重复和后续增长，不能作为最终释放量。`pre-delete.json` 和 `deletion-result.json` 是实际删除窗口的前后基准。`deletion-plan.json` 是执行前允许清单，实际完成以 result 为准。`processes-before.txt`、`open-files-before.txt` 保存执行前检查，操作脚本为 `reclaim.py`。
