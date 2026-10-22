# 交通到经营的真实结算接入契约（待实现）

基线 v0.5.114。此契约来自对当前 Engine、Store、assembly、transport 和 resolver 的定向只读审查；不是功能或性能通过。唯一共享 writer 在建设阶段结束后接手，不并发修改 Engine/World/Store。

## 可信输入与唯一结算

现有 Opening 和 Assignment 仅有交通 revision，没有 world、branch 或 service_day；resolver 的非零 generation/profile 只是对象选择器。可信原件应绑定 schema、world、branch、profile_id、service_day、generation、完整 Opening/Assignment、全商品 partitions 与 tariff。原件完整 hash 由可信持久索引绑定，不能只采信调用者的“已验证”标志。限长读取与 hash/归属/日域验证在消费前完成。

采用小结算命令：profile_id、generation、proof_sha256；沿用 Envelope.expected_revision。hash 必须在命令 payload 内，以复用 journal 的同 id 同命令重放和改 hash OperationConflict。已完成重放保持现有 journal 优先语义；新执行和 WAL 恢复依赖原件完整存在。缺失/篡改/换绑定要拒绝，不自行补造证据。

Engine revision 对比当前 Engine；Assignment.network_revision 对比交通 Opening.revision，两个域不能强行要求数值相同。道路版本若另有权威，需单独绑定。日历或工资推进造成 Engine revision 改变，不自动使未改交通图失效。

仅在现有 prepare_town_settlement 中构造真实账户、库存、工资与 RecipeExecution 后、plan_day_with_execution/reconcile_with_execution 前替换 input.delivery。resolver 返回报价和审计，不再执行第二个 planner 或拼接另一笔独立现金/库存 batch。公用供应与运价在这一有限阶段仍为明确的外部 profile 输入。

## 容量不能跨日重放

service_day 必须等于当前完整日边界算出的结算日。一个市场一天一次、全部商品 partitions 共同消费已服务的往返 OD 配额；不能按店铺单独调用再相加。settled_day 仅防市场同日重复，不证明全城 OD 无双花。本轮应使用独占市场的可信分配凭证，或已由共同分配器明确划出的份额；无此证明则拒绝。

第二天需要可信生成的当日分配凭证。可复用未变拓扑和求解结果；不能任意改日期/换 generation 来重新授予已消费容量。旧日原件即使 hash 和 Assignment.verify 均有效，也不能当新日容量。

本次 service_day、generation、proof_sha256、resolved_digest 与钱、库存、settled_day 在同一 prepare/commit 中发布，并进入既有 checkpoint/WAL 恢复验证。不要把完整交通数组直接加入 Copy 型 World；已有 genesis 512KiB 上限不得静默放大。优先复用不可变文件/引用机制，量化新增 heap、读取上限、原件保留与恢复依赖。

## 最小真实 CLI 与反例

四个同输入场景：需求/库存各200、每趟20件、请求10趟；正常实际服务5趟→最多100件，断路0→0，瓶颈2→40，扩容10→200。资金/价格应保证购买力不成为额外瓶颈。验证真实 Store 收据、现金、库存、运费收款、未满足需求、冷恢复与同操作重放，不能仅打印 pureplan。

反例包括跨天旧凭证、同日新 op 再结算、同 op 换 hash、错误 world/branch/profile、伪造 Assignment、商品共用 OD 超配、原件缺失/损坏、durable hook 故障后恢复。未来多日与全城分配仍保留为完整目标，有限通过不证明完整产品或 M0 性能。
