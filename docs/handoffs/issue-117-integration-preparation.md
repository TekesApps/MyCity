# #117 已交付 asset-cost 模块集成准备

2026-10-28；复用 [Issue #117](https://github.com/TekesApps/MyCity/issues/117) 唯一当前集成准备。基线 `v0.5.212` / `0b6578b0270db53a6bb89dc1ac8aa3af09c1967e`，仓库 `/Users/frankqin/dev/MyCity`。本次仅将既有交付放入当前树，供 root 后续 construction 真实资产成本接线使用；不是新批量模块开发，不代表 #117 完成。

## 固定来源与文件

root 从原 patch 重构并核对原 manifest SHA 的完整固定源保留于 `work/asset-capital-runtime-20261028/frozen-candidate/`；本次没有重构来源或修改固定源。原 handoff 为该目录内 `docs/handoffs/issue-117-asset-cost.md`。原件继续保留，hash 不代替完整原件。

| 文件（相对固定源目录或当前仓库） | SHA-256 | 本次处理 |
| --- | --- | --- |
| `crates/city-core/src/asset_cost.rs` | `c96e268f34e449101f8fbe3e4b1d6a29b7e9ec3ff0efa95b52bce32e5f8aa987` | 逐字复制到当前树；无适配 |
| `crates/city-core/tests/asset_cost.rs` | `5a85c5cbd6414c0a87374015782b3b4d43575b3baf03979e6c2cf7c55618ff79` | 逐字复制到当前树；无适配 |
| `docs/handoffs/issue-117-asset-cost.md` | `adb6495e7d1f58f097ecfd7a701484dc5b010e1e363babacdd7ac5f3893f9799` | 仅阅读固定源内原件；未复制到当前 docs |

本次写权限仅以上两个 Rust 目标文件及本说明。没有修改 lib.rs、Engine、World、Store、Timeline、财务、Cargo 清单或其他文件；原有 24 项 WIP 保留。root 继续唯一负责共享核心、接线、串行编译和验收，统一回填 Issue；本准备没有提交、push、PR 或 Issue 状态变更。

## 静态接口核对与适配结论

已阅读当前 `crates/city-core/src/units.rs`、`src/lib.rs` 和 `Cargo.toml`，并完整阅读冻结模块和测试源码。仅静态观察：

- 模块使用 `units::{ArithmeticError, GameTime, Money, Quantity}`。当前单位均提供 `ZERO`、`new(i64)`、`value()`、比较、serde，以及 `checked_add` / `checked_sub` / `checked_mul(u64)`；与源码调用形状一致。`ArithmeticError` 可被现有 `From` 转为模块 `Error`。未发现需要适配的 API 形状差异；编译正确性仍未验证。
- 测试保留 `pub use mycity_core::units;` 与 `#[path = "../src/asset_cost.rs"] mod asset_cost;`。当前 lib.rs 公开 `units`，路径从 `crates/city-core/tests` 指向新增源码；源码中的 `crate::units` 在该测试 crate 中指向同一 re-export。无需为该独立测试改 lib.rs。当前库未导出 asset_cost，也未建立运行时调用。
- 当前 `mycity-core` 已依赖 serde 和 serde_json；没有新增依赖或清单改动。
- 当前 units 已有 `frozen_lot_basis_delta`。本次保持原交付的局部累计 floor 差算法，不替换为新 helper，不将原 handoff 关于历史私有 helper 的描述当作当前 API 事实；没有算法变更的适配理由。

测试文件开头的 synthetic / NOT_RUN 声明保持原样。`r(id)`、`delivered()` 和 `partial_scope()` 中 source、profile、receipt、极短寿命、金额均是合成数学契约；非真实 construction 工料、资本、交付、维修、租赁或寿命来源。

## 后续接线边界与已知限制

原 API 保持 `Snapshot::new_delivery` / `validate`、`parse_snapshot`、`plan`、`Candidate::against`。模块只计算候选增量，Source 的非零 id/revision 只表示结构断言，不认证原票；Candidate 不提供现金、产权、持久写入或全局幂等权限，`establishes_distributable_profit()` 恒 false。

后续 root 必须从真实 construction 已发生工料/资本及真实投运交付解析 ActualDelivery；#31 报价/估值不能充当 basis。真实 profile 必须提供方法、寿命、残值、同质 cost_quantum、clock 和来源；缺项不能从合成测试补齐。旧资产恢复完整原始 Snapshot，不能重新 new_delivery 洗掉年龄、basis 或期间游标。

同一真实 scope 的唯一性、跨 lot/维修/租金/资本化原票的成本切片互斥、当前权威 root 和来源版本认证、receipt lost-reply、原子发布 next/additional_lot 与成本、Timeline 必要边界、真实资金/工料/产权/保护和税/分红完整性仍由 root 沿原机制接线。本模块最近 receipt / revision guard 不能代替全局去重；serde 测试不能代替 Store/WAL 崩溃恢复。

支持范围仍为有来源固定秒直线法、可按整数分均分的同质 quantum、有限普通租赁、已发生普通修复、真实全剩余 scope 替换、前缀量退出和关联转移。活动租约下退出/替换/转移、异质或非整分分摊、其他折旧方法/日历寿命、租约更新及受保护公开/司法处置未实现。本次不改变这些业务含义或默认采纳产品政策；真实 construction 装配若与之冲突，应先退回 root 决策。

## 验证状态与冻结交接

本次仅做源码/API/引用路径静态阅读、SHA-256、逐字比较、Git 状态及空白检查；这些不构成编译或测试通过。Rust 源码与测试均保持固定源 hash。

- `cargo build/check/test/clippy/fmt`、rustc、脚本测试：`not_run`（按本次边界禁止）。
- 真实 construction adapter/Engine/World/Store/Timeline/财务接线、真实 profile 和原票来源：`not_run` / 未实现于本准备。
- 合成模块测试、真实正常/拒绝与边界用例、跨原票幂等、保存/崩溃恢复、资源与联合负载验收：`not_run`。
- root 后续唯一串行最小模块验证入口：`CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --locked --offline -p mycity-core --test asset_cost`；尚未运行，且仅能验证合成模块测试。沿现有 HP P900 work/target，不建立新缓存。

三个允许文件交回 root 后冻结；后续适配由 root 明确分配。仅本准备完成，#117 真实接线与完整验收仍开放。新说明自身 hash 由交接消息记录，避免自引用 hash。

## Root有限消费结果（2026-10-28）

上文是原准备阶段的冻结事实，未倒写其not_run。root已按#151接通自有原单工程资本、原日税/留存、停用及真实关闭、原票/冷恢复与normal资源，见[实际证据](../evidence/asset-capital-runtime-20261028/STATUS.md)。原候选其他租赁/维修/替换/关联转移等仍未据此整体验收，#117开放。
