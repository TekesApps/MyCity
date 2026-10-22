# 交通到经营的真实结算接入契约（M0 实现阶段）

实现基线 v0.5.115。Store 固定每日分配配置、不可变 proof 与现有市场事务接入已完成本阶段 release 验证；本节末列出命令和边界。它不代表全城分配或 M0 性能通过。

## 可信输入与唯一结算

现有 Opening 和 Assignment 仅有交通 revision，没有 world、branch 或 service_day；resolver 的非零 generation/profile 只是对象选择器。可信原件应绑定 schema、world、branch、profile_id、service_day、generation、完整 Opening/Assignment、全商品 partitions 与 tariff。原件完整 hash 由可信持久索引绑定，不能只采信调用者的“已验证”标志。限长读取与 hash/归属/日域验证在消费前完成。

采用小结算命令：profile_id、service_day、generation、proof_sha256；沿用 Envelope.expected_revision。hash 必须在命令 payload 内，以复用 journal 的同 id 同命令重放和改 hash OperationConflict。已完成重放保持现有 journal 优先语义；新执行和 WAL 恢复依赖原件完整存在。缺失/篡改/换绑定要拒绝，不自行补造证据。配置已绑定的市场必须拒绝旧 `TownSettle` 静态报价入口；未绑定的旧 profile 才保留静态报价兼容路径。

Engine revision 对比当前 Engine；Assignment.network_revision 对比交通 Opening.revision，两个域不能强行要求数值相同。道路版本若另有权威，需单独绑定。日历或工资推进造成 Engine revision 改变，不自动使未改交通图失效。

仅在现有 prepare_town_settlement 中构造真实账户、库存、工资与 RecipeExecution 后、plan_day_with_execution/reconcile_with_execution 前替换 input.delivery。resolver 返回报价和审计，不再执行第二个 planner 或拼接另一笔独立现金/库存 batch。公用供应与运价在这一有限阶段仍为明确的外部 profile 输入。

## 容量不能跨日重放

service_day 必须等于当前完整日边界算出的结算日。一个市场一天一次、全部商品 partitions 共同消费已服务的往返 OD 配额；不能按店铺单独调用再相加。settled_day 仅防市场同日重复，不证明全城 OD 无双花。本轮应使用独占市场的可信分配凭证，或已由共同分配器明确划出的份额；无此证明则拒绝。

第二天需要可信生成的当日分配凭证。可复用未变拓扑和求解结果；不能任意改日期/换 generation 来重新授予已消费容量。旧日原件即使 hash 和 Assignment.verify 均有效，也不能当新日容量。

本阶段的最小发行权采用**期初固定每日分配配置**，不把“把任意 proof 文件写进 hash catalog”当作发行授权：`MarketProfile` 只保存 32 字节 `allocation_profile_sha256`；完整外置配置绑定 world、branch、market profile、`period_seconds=DAY`、`SingleTownExclusive`、`RepeatFixedDailyDemand`、完整 Opening/Assignment、商品 partitions 与外部运价输入。固定 Opening 本身由配置 hash 绑定，Assignment 必须对该 Opening 通过确定性验证；仅固定 partitions 不足以阻止伪造道路容量或 OD 需求。这个有限阶段不提供运行中改路，正常、断路、瓶颈、扩容通过不同的明确配置世界测试。

可信 Store writer 从当前 Timeline 的完整日边界派生 `service_day`，从 MarketState 的已消费游标以 checked-add 派生 generation；调用者不能指定下一日或 generation。它为当前完整日构造包含配置 hash、world、branch、profile、period、day、generation 及固定交通原件的不可变 proof，先同步发布按内容 hash 命名的原件，再提交包含 profile/day/generation/proof hash 的单一 settlement action。`issued_day/generation/proof_hash`、`last_consumed_generation`、`last_resolved_digest`、`settled_day` 与原有现金、库存、工资游标同一次事务/WAL 发布。proof 已提交后恢复必须读取并核验原件，不能重新求解出“等价”文件替代。固定每日需求配置允许相同 Assignment 内容在连续日再次出现；新日凭证由当前日历和单调 generation 新发，旧日 proof 本身不能跨日使用。`Caller::System` 是可信宿主边界，不承诺抵抗拥有本地存档写权限的恶意者。

proof 严格 JSON 的当前保守字段上界为 **1,571,328 字节**，独立读入上限 2 MiB；修改 transport 字段或数组上限时需重算上界。解码先限字节、lexical guard，再对 Opening/Assignment/partitions 及每条 path 的 edge ids 使用预分配 bounded visitor 并拒绝额外元素，未知字段拒绝。Store 复用既有 16 MiB 文件 backing 暂存，proof/配置数组不进入 Copy World；genesis 512 KiB 与 Engine 4 MiB 预算不扩大。配置解码、每日 proof 编码/解码、Assignment.verify 重算和 resolver 的新增峰值需单列实测，字节上限不能替代 heap 峰值验收。**当前配置、proof、校验与 resolver 同时存活时的新增峰值尚未实测；2 MiB 是文件输入上限，不代表内存峰值，也不能沿用此前基线预算作通过结论。**

冷恢复除 WAL 外还会校验 checkpoint 内最后一个市场游标引用的实际 proof，并按 archive 顺序逐条核验 checkpoint 归档中的 transport action 原件；不将历史 receipt 或 proof hash 全部汇集成无界列表。已绑定 profile 的 `MarketState` 必须满足初始日无 transport receipt、已结算日有 receipt、issued/consumed day 与 generation 对齐且 proof hash 非零。proof 与 allocation 原件在文件发布前先通过正常容量准入，按新文件和临时发布空间收费；每日结算同时预留已有 envelope/WAL 空间，已存在且内容一致的同 hash 文件不重复收费。本阶段的真实 CLI 覆盖 checkpoint 冷恢复、缺失/损坏原件拒绝和低容量无副作用反例。

本次 service_day、generation、proof_sha256、resolved_digest 与钱、库存、settled_day 在同一 prepare/commit 中发布，并进入既有 checkpoint/WAL 恢复验证。不要把完整交通数组直接加入 Copy 型 World；已有 genesis 512KiB 上限不得静默放大。优先复用不可变文件/引用机制，量化新增 heap、读取上限、原件保留与恢复依赖。

## 最小真实 CLI 与反例

四个同输入场景：需求/库存各200、每趟20件、请求10趟；正常实际服务5趟→最多100件，断路0→0，瓶颈2→40，扩容10→200。资金/价格应保证购买力不成为额外瓶颈。验证真实 Store 收据、现金、库存、运费收款、未满足需求、冷恢复与同操作重放，不能仅打印 pureplan。

反例包括跨天旧凭证、同日新 op 再结算、同 op 换 hash、错误 world/branch/profile、伪造 Assignment、商品共用 OD 超配、原件缺失/损坏、durable hook 故障后恢复。未来多日与全城分配仍保留为完整目标，有限通过不证明完整产品或 M0 性能。

## 本阶段实测记录

- 最终源码通过 `cargo +1.85.1 fmt --all -- --check`、`git diff --check`、`cargo +1.85.1 clippy --locked --offline --release -p mycity-core -p mycity-storage --all-targets -- -D warnings`，以及 `cargo +1.85.1 test --locked --offline --release -p mycity-core -p mycity-storage`；合计 **349 项**单元、集成与文档测试通过。
- 真实 Store CLI 日一销售为 normal **100**、closed **0**、bottleneck **40**、expanded **200**。四场景均验证第二日重新签发、同日二次结算拒绝、同 op 重放及改 proof 冲突、proof 发布/WAL Applied 故障恢复、checkpoint 冷开归档重放、归档 proof 篡改或缺失拒绝，以及低额度拒绝不改变 cursor/state/紧急保留空间。每笔运费均与实际收款账户变化一致。机器可读原始输出与命令日志保存在外置盘 `work/transport-settlement-attempt-01/`。
- Release/strict 验证使用 macOS 上的 Rust/Cargo 1.85.1，`--locked --offline`，共享构建目录 `/Users/frankqin/dev/MyCity/target`（链接到 HP P900 外置 SSD）；不是 16GB 内存或性能测试。proof 输入上限 2 MiB、固定模型区预算和文件分配额已覆盖；新增配置、proof、解码、求解与 resolver 同时存活的进程 heap 峰值仍 **未测**。本阶段不声称 M0 性能验收或全城交通—市场协调完成。
