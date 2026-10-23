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

三个场景是独立期初配置world的因果对照，不是同一Store运行时编辑空间。当前没有空间资产更新与交通profile/revision同根提交协议；该项仍是F缺口，不能以本轮对照代替。
