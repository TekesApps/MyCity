# 2026-10-21 后续历史产物回收（v0.5.68）

本阶段只清理验证产物和维护记录；不修改或发布当前未提交的Rust模型。此前v0.5.65已清理58.16GiB，本次独立记账，不重复计入。当前盘点不再是旧估计的74GB。

| 项目 | 本次实测 |
|---|---|
| work清理前 | 29,886,348 KiB，28.50 GiB |
| work清理后 | 17,995,564 KiB，17.16 GiB |
| 删除分配空间（inode去重） | 12,176,162,816 B，11.34 GiB |
| 删除逻辑长度 | 12,176,161,240 B |
| df可用空间观察增量 | 12,176,478,208 B |
| df清理后可用空间 | 127,747,547,136 B，118.97 GiB |
| target保留 | 754,196 KiB，736.52 MiB，删除窗口未变化 |

删除4个路径／4个单链接inode；work的du降幅与分配空间相符。df观察增量另多315,392B，不归因于本次清理。st_blocks不证明APFS独占物理块。本次没有清理target或其他项目／全局缓存，没有创建新target。

## 删除与恢复可用性

- `work/t13a64-joint-01/model/layout/snapshots/snapshot-000001/pressure.bin`
- `work/t13a64-joint-01/model/layout/snapshots/snapshot-000002/pressure.bin`
- `work/t13a65-joint-01/model/layout/snapshots/snapshot-000001/pressure.bin`
- `work/t13a65-joint-01/model/layout/snapshots/snapshot-000002/pressure.bin`

v64旧基线已被发布的v66 joint-02完整基线替代；第一次v65联合实验缺顶层heap footer，其失败日志仍完整保留。源码、脚本及[advisor独立依赖审查](advisor-review.md)未找到这4份压力原文件的后续恢复依赖。各轮真实业务world／WAL、native／stub、index、manifest、收据、源快照／依赖及可执行文件保留，未按目录批删。

**上述四快照现均不能直接完整恢复。** hash／index仅证明历史身份；需对应版本生成器重建原pattern／page layout／mutation iteration并核对完整index后，才能恢复可用性声明。历史当时成功恢复的事实不改写。

v64保留证据：`docs/evidence/m0-t13a-hotpath/t13a64-joint-01`及该专题原始日志；v65失败证据：`docs/evidence/m0-t13a-helper-compare/t13a65-joint-01`、`joint-01-audits`及`failure-notes.md`。逐文件身份、理由、位置和恢复状态见[实际清单](deletion-result.json)，不是仅保存hash后宣称可恢复。

## 保留与核验

最新成功基线`work/t13a65-joint-02`两代29文件完整快照、v50 snapshot-000001旧格式恢复依赖、当前候选完整兼容world均保留。真实业务存档、固定源码／依赖、验收exe、收据和故障证据不退役；剩余其他用途未明确的中间文件先保留。target仍供当前构建与后续验证复用，不删除未确认过期的依赖。

删除前没有活跃模型／编译或work文件占用，见[进程](processes-before.txt)及[打开文件](open-files-before.txt)。81,116个未选中work文件的inode／长度／mtime／分配块／链接数前后不变。源码／fixture／固定exe逐hash不变；三份保护快照按原index存在且长度符合，业务原world及完整兼容副本各243文件hash相符，见[保留核验](preservation-check.json)。这不是新恢复或性能验收。

后续执行[阶段回收规则](../../validation-artifact-retention.md)：终态归档后逐文件退役，不在测量中清理；保留当前／最新完整／旧格式依赖，复用共享target。下一次有新完整成功基线替代或恢复依赖解除时复查，不能自动删除当前保护清单。
