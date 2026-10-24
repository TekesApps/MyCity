# 空间场景到步行交通与商店结算（有限实现）

本阶段把一个已验证的空间场景接入现有有向交通图，再让居民步行可达性实际约束 Store 每日市场结算。范围严格限于**单张地图、地面、步行、单栋建筑、单条连接走廊、单条道路与一个显式道路端口**。它不是完整城市路网，也不代表规模性能或 16GB 内存验收。

## 空间如何形成交通边

输入必须先通过 `mycity_map::scene::Scene::assemble`。现有 Scene 对建筑门、门到道路锚点的连接走廊及一条二次曲线道路执行空间、地形、碰撞、方向和授权检查；当前有效路径都在同一地面高度，通行模式为 WALK。适配器不接收未经验证的坐标，也不通过“两个东西离得近”推断连接。

`mycity_map::traffic_adapter::adapt` 接收已验证的 Scene 和明确的 `AdapterInput`：地图与城市 ID、revision、稳定节点／边／入口 ID、主体 owner、连接走廊和道路的每时段容量、自由流速度、开闭状态、道路方向、OD 群体需求，以及道路起点端口的 `TerminalGrant`。端口须获准、使用 WALK、位于道路首点、与 Scene 地面高度一致，且净宽至少覆盖道路宽度。缺少授权或端点／高度／宽度不匹配时拒绝适配。

当前最多生成 3 个节点：建筑门、道路锚点、道路起点端口；最多生成 4 条有向边：门与锚点间两个方向的连接边，以及道路两个方向的边。门和锚点各自的 `can_enter`／`can_exit` 决定连接边方向；道路方向由显式 `RoadDirection` 与端口许可共同决定。端口方向遵循空间 seam 的既有语义：道路首点进入地图的 start→end 边要求端口 `can_enter`；从地图离开的 end→start 边要求 `can_exit`。曾出现的反向映射错误已修正。

道路及连接边的容量来自设施配置，不从几何宽度猜算；速度由设施配置给出。自由流时间按已验证走廊的保守长度上界除以配置速度并向上取整。关闭道路会将实际交通边标为关闭，关闭连接设施会关闭连接边；许可或方向变化会移除对应方向边。因此这些改变会改变 `Opening`、`Assignment` 与可达路径，而不是只改变外观属性。

## 从 OD 到每日交易

适配得到的 `Opening` 进入现有 `mycity_core::transport` 分配器，群体 OD 使用 WALK 单 bit。Store 固定每日 `TransportAllocationProfile` 中的 Opening、Assignment、往返 `TripPartition` 和费用规则；每日结算沿用 `Store::settle_town_market`，生成并保存当日不可变 transport proof，再由既有市场 resolver 根据 outbound 与 return 的实际 served 人数限制往返行程数和该商店的可售量。Household／shop 的入口 owner 必须分别对应市场中的居民与商户账户。

本验证将行人到店作为市场可达性因果：人能完成往返，商店才获得相应交易额度。WALK 需求没有被改写为货运；商品仍由现有市场记账结算。这里**没有**模拟货物装车、货运车辆或配送路线，也没有将步行客流解释成货运履约。

170 Store 测试覆盖三种相同规模的空间场景：

| 场景 | 交通结果 | 市场结果 |
| --- | --- | --- |
| 道路与门双向开放 | outbound 4 人、return 4 人获服务 | 实际销量 40，商店可达 |
| 道路关闭 | outbound 0 人、return 0 人获服务 | 实际销量 0，不可达 |
| 建筑门禁止进入 | outbound 4 人获服务、return 0 人获服务 | 无完整往返，实际销量 0，不可达 |

每个场景都经过 Store checkpoint、关闭后 cold open，并以原 operation ID 重放。测试核对冷开前后的 Store state hash、已引用 proof 的 hash 与字节、市场销量及原始事务 Receipt；重放保持同一状态，不重复交易。proof 的 assignment 与 opening 对应，记录 service day 和 generation。道路关闭与单向门都因此影响实际到店交易，而非只验证纯图分配。

## 实测边界和阶段结果

169 的 `mycity-map` release 测试实测 5 项通过。170 的 `mycity-storage` release 集成测试在 source unchanged 的运行中实测 1 项通过；同一集成测试依次完成上述三个 Store 场景、checkpoint、冷恢复与精确重放。完整源、命令、exit 状态、可执行文件和保留的三个 world 位于外置卷 `work/construction-utilities-current-01/170-spatial-store-current-main/`；测试输出和微型收据后续归档到[阶段证据](evidence/core-spatial-traffic-139/STATUS.md)。最终174 strict、175同源map5/Store1、176 fmt与177固定exe的CPU/RSS及独立checkpoint/proof/钱物核对已由主线完成，详见阶段证据；它们仍只属于微型功能和资源观察。

170 的微型收据记录 Opening 的 Rust owned Vec capacity 为 400 B；Assignment 为 20,864–20,912 B，含每条 path 的 edge ID 容量；allocation profile JSON 为 2,037–2,243 B，proof JSON 为 2,111–2,315 B。`Store::owned_capacity_bytes()` 在 create／settle 后记录为 41,465,849 B、cold open 后为 41,465,913 B。该 Store 总值不等于这个交通适配器单独新增的占用；API 也明确不包含 allocator、kernel／OS 页面或 transient replay。适配与分配、Store create、日历推进、结算、checkpoint、cold open、replay 的单次 wall 计时随 170 stderr 保留，只供还原该次功能运行，不当成基准或吞吐承诺。它是一个三节点 synthetic microfixture，不能外推为整城地图、完整产品负载、整机内存或 16GB 性能结论。

开发中保留的失败证据说明了缺陷归属：156 编译失败是测试局部变量遮蔽同名 `scene` helper；167 实测失败来自测试端口夹具净宽 200 cm 小于真实夹具道路净宽 700 cm，修正夹具后没有放宽生产检查；168 实测时真实发现 adapter 把端口 `can_enter`／`can_exit` 映射方向接反，依据 seam 语义修正了生产适配器，并保留双向／单向反例。这些旧失败没有从历史中抹去；169/170 的通过只适用于各自固定源码与所述范围。

## 当前不支持

- 多栋建筑、多条相交道路、路口转向或从这个 terminal 自动接入全城已有网络；扩展需明确授权拓扑和稳定节点合并规则。
- 汽车、货运、公交、铁路、港口、机场等模式与班次／载具容量；这些不能由 WALK 图或 WALK 到店结果代替。
- 地下、空中、桥梁、隧道、坡度／高程转换或跨地图 seam 的可通行路径。Scene 当前校验过 seam 几何，但该 seam 不会自动变成此适配器里的交通路段。
- 从道路宽度自动推容量、从附近设施推连接、运行时编辑空间并同步交通 revision，以及全城多市场共享 OD 的联合分配。
- 整城路径均衡、分时客流校准、货物实际配送、市场主体间完整物流合同、图形／GPU 或 AgentKit。

这些能力仍属于产品总体设计目标或后续实现范围；本阶段仅证明一条明确获准的地面步行路径可以改变已有 Store 市场结算，并按现有持久化机制恢复和重放。

v139 的上述三个场景是独立期初配置 world 的因果对照，当时尚未实现同一 Store 的运行时编辑。后续 v147 的 Close/Restore 与 v159 的 Scene/profile 几何同根提交、历史 proof 和冷恢复，见下列现行 #25 登记及固定证据；不以旧对照冒充后续实现。

## F 同步任务与资源登记（#25 有限工作包）

这里登记已实现的单图地面 WALK、原道路/入口开闭及受权几何版本更新，不新增 worker、时钟或调度器。#25 从父 #3/#6 划出的范围是这个实际 Store 包；桥隧、其它交通模式、生产地图/OD/权利 provider 与全城承载继续由原父/后续 issue 跟踪。

| 契约 | 当前规则、执行方与验证路径 |
|---|---|
| 激活与周期 | 显式提案/批准/Apply/Withdraw 命令，同步 CPU 执行；无周期 tick、后台 worker、独立任务队列或新 IPC。候选重建是新计算承诺；已接纳的撤回、原票重试、恢复及历史解释不能因未激活而丢失。 |
| 两种时间 | 计算任务没有自有 clock_kind；业务适用日沿原 Timeline 的完整日边界，candidate effective day 必须为 completed_day+1。不是另建次日唤醒/激活任务，不回拨时钟。Withdraw 不触发候选生效，不要求午夜或候选尚未到期。 |
| 事实与最小状态 | 真实 caller、原 Request/Approve/Withdraw/Apply 与 operation/receipt、双角色授权、Scene/profile/hash/revision、adapter basis、当日 proof 和必要的完整持久来源。几何相交/候选生成不授予产权或通行权。 |
| 输入与覆盖 | 固定已验证 Scene、原 transport profile 与明确 AdapterInput，world/branch/asset、父/候选 Scene input/state hash、geometry 与 transport 各自 revision、basis hash、effective day。单图已支持模式以外拒绝；没有从地图面积猜覆盖。 |
| 有效期与重新求解 | 无独立 TTL。Request/Approve/Apply 沿候选激活门重新核对父版本/hash/basis 与适用日，拒绝 stale，不热换原批准目标。Withdraw 单独核原 request/descriptor 的 scope、asset、内容 hash 和真实权限，不要求旧 parent 仍是当前版本，不重建 candidate。Request 行仅由明确 Withdraw 或成功 Apply 消费，不用超时删除义务。 |
| 候选与迭代 | 原有限 CPU 路径：验证 prior→组装 candidate Scene→adapter→transport assignment→编码 Scene/profile/descriptor；nodes≤128、edges≤512、demands≤512、assignment steps≤512，均沿原算法和整单位规则。没有新增自适应等级/近似模型或 GPU 更快结论。 |
| 唯一 writer 与提交 | 调用 Store 的宿主 PID 同步计算；Engine prepare 验证单条 SpatialGeometry Action、原 caller 与授权，再沿既有 WAL/catalog/root 提交同一 Scene/profile。未通过验证的候选不变成已发生事实；lost reply 用原 Envelope/receipt 重试，不能另发新业务 ID。 |
| CPU 持有量与工作区 | 输入≤64 KiB、Scene geometry budget 1 MiB、profile 仍受原 TRANSPORT_PROOF_SCHEMA_MAX_BYTES 约束；descriptor≤4 KiB、transport catalog≤128 KiB；权威表≤8 requests/16 approvals，并计入 Engine prepare 的 projected owned budget。Engine 4 MiB、既有 Store normal 64 MiB 等门不扩大。build_candidate 的 prior/candidate clone、assignment 与编码临时区必须另计，candidate_owned_capacity_bytes 只是保留量，尚非其瞬时峰。 |
| 缓存、IPC、GPU | 无新增 geometry 专属缓存、IPC payload、worker 或 GPU 缓冲；Store 仍使用共享 metadata/workspace，不能把共享部分记为免费或再逐任务重复收费。未测与渲染竞争或设备路径。 |
| 磁盘与增长 | content-addressed Scene/profile/descriptor、catalog、原 WAL/receipt/proof 按既有文件界限持久化；引用未解除不删除完整原件。每次新版本可增文件与历史，不能以热表有界宣称永久磁盘有界；磁盘/APFS 实际分配增量未测。 |
| 退出、失败与恢复 | 无异步 geometry worker 因而无独立取消/退休协议。拒绝 stale/权限/预算时保留原状态与已发生责任；Apply 已提交后不得通过撤销候选回滚事实。checkpoint/cold replay、旧 proof 与完整 catalog/Scene 来源保留；缺失/篡改恢复来源明确拒绝，不伪造历史。 |
| 策略与精度 | 本包仅原可信 CPU 策略，不动态降低几何/授权精度；观测厘米、原 revision/hash 和业务分币/原算法结果。误差与承载参数没有额外实测资格，不把有限分配结果解释为全城交通最优。 |

源码责任点：`city-core/src/transaction.rs` 的 SpatialGeometry prepare 与 projected budget，`city-core/src/spatial_geometry_permissions.rs` 的 descriptor/8 request/16 approval；`city-storage/src/lib.rs` 的 stage_road_geometry_candidate、validate_geometry_descriptor_for_command、owned_capacity_bytes；`city-storage/src/spatial_geometry_updates.rs` 与 spatial_transport_updates.rs 的持久编码界限；`city-map/src/geometry_revision.rs::build_candidate`。这些同步入口没有伪装成 #101 的周期 worker。

### 固定业务与资源证据

[已发布关联证据](evidence/release-association-20261024/STATUS.md)和[完整历史尝试/最终运行](evidence/core-spatial-geometry-current/STATUS.md)保留 source `be142b7b957e48acedbe7fa52f44eed13400b83dccfe6a9d44349e5afd57a0ca`、固定程序与原始日志：原 F Store 8/8、storage 反例 8/8，以及完整副本的独立新进程冷读/缺来源拒绝。几何改动后实际 travel 81→82 秒、40 件运费 6480→6560 分而销量仍为 40；原开闭路径另有同 Store 销量 40/0/40/0/20。不能因为几何案例销量相同而抹掉实际通路/成本变化，也不能把 WALK 到店当成真实货运配送。

该几何 microfixture 的 Store owned capacity 约 41,783,052–41,783,055 B，world logical files 106,250 B，均为整个固定场景而非 geometry 单独增量。邻近 1025-commit 场景的 Engine 3,608,632/4,194,304 B、Store 41,787,700/67,108,864 B 与 archive 1,045,431/134,217,728 B 是不同负载，不能借作本几何耗时/峰值。完整源码、实际程序、失败世界/成功世界及原票保留在外置 HP P900 `work/release-association-20261024/` 和原 construction-utilities/takeover 包；hash 不代替原件。

CPU 构造瞬时峰、native/RSS/stack 联合峰、物理磁盘增量、真实 16GB、GPU 与正式 M0/长压力仍 not_run；32GiB Mac 与 HP P900 USB/APFS 的有限正确性/逻辑预算证据不外推这些门。它们仍由父/联合验收 issue 负责，不把 #25 的原有限功能范围事后改写为全城性能通过。
