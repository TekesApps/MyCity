# 粒度收敛补充与 Issue 统筹回执（v0.5.231）

2026-11-19；文档与 Issue 台账发布 `v0.5.231`。**本阶段仅同步设计收敛补充与 GitHub / 本地 Issue 台账，不修改游戏源码、未编译或运行任何 Rust/M0 测试（`not_run`），不改变既定 M0 冻结门与硬件验收结论。**

## 1. 设计收敛补充

- [模型粒度、聚合与复杂度收敛](../../model-granularity-and-convergence.md)：追加 `v0.5.231` 节，明确权威层空间场/人时暴露/指标的定点数与扁平数组（SoA）表示边界、`Prepared` 脏标记增量粗粒度汇总层（`O(1)` 只读 `Status` 查询），以及有界汇点可达势场与批次流量预筛原则。
- [属性驱动的概率机制通则](../../regional-probabilistic-decisions.md)：追加硬契约精确日历秒守时与周期性统计倾向 `stable_id mod K` 确定性相位分桶错峰约束。

## 2. GitHub Issue 新建、父子链接与非破坏性补充

- 新增 2 项未派工 `stage:todo`、`type:capability` 有界业务子任务（`write_authority: []`，不抢占 `#159` 唯一共享线）：
  - [#161](https://github.com/TekesApps/MyCity/issues/161)（父任务 `#6`，已原生链接）：有界汇点可达势场与批次流量预筛：路网版本缓存、门禁因果与候选裁剪
  - [#162](https://github.com/TekesApps/MyCity/issues/162)（父任务 `#49`，已原生链接）：增量脏标记粗粒度汇总层：定长整数观测槽与快进摘要降本
- 补登既有开放缺陷 [#160](https://github.com/TekesApps/MyCity/issues/160) 至本地台账并确认原生归属父任务 `#21`（保持 `stage:developing`、`type:defect`）。
- 非破坏性更新 9 项既有 Issue（保留全部原文与历史合同）：
  - `#6`、`#49`：父任务子清单分别勾选补充 `#161`、`#162`；
  - `#61`、`#98`：补充进入 `city-core` 常驻层的定点数/SoA 与跨硬件精度边界；
  - `#73`：明确 `#73`（有限 offer 时刻表拼接）与 `#161`（大规模汇点可达预筛）职责分界；
  - `#101`、`#62`、`#141`、`#142`：补充硬契约精确守时与软统计倾向 `stable_id mod K` 确定性分桶错峰，以及 `#141`/`#62` 对 `#161` 预筛的引用。

## 3. 本地台账、实时回读与工作区隔离

- 同步更新 [registry.json](../../inputs/issue-ledger/registry.json)、[requirements.json](../../inputs/issue-ledger/requirements.json)、[execution-flow.json](../../inputs/issue-ledger/execution-flow.json)、[issue-execution-ledger.md](../../issue-execution-ledger.md)、[core-task-dispatch-current.md](../../core-task-dispatch-current.md) 与 [README.md](../../../README.md)。
- 实时回读 GitHub 全量 Issue 共 **161** 项（**128** Open + **33** Closed），与 `registry.json` 逐条及状态标签完全一致；变更前后快照与回执原件保存于外置 SSD 目录 `/Volumes/HP P900/dev/MyCity/work/issue-granularity-coordination-20261119/`（事前预算 `16MiB`，实际仅小 JSON 文件）。详细哈希与回读见 [receipts.json](receipts.json)。
- 游戏源码与已发布验证基线仍为 `v0.5.229`；工作区未提交的 `#159/#160` 代码与 `final11` 固定输入、用户未提交的 `AGENTS.md` 修改及 24 份 M0 保留 WIP 均保持原样未提交、未混入本发布。本阶段新验收业务 `0`、历史补关 `0`、缺陷关闭 `0`，`18` 源码包积压不变。
