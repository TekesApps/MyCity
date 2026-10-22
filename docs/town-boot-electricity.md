# Town生产与Boot电力库存接入候选

本切口只接一个Town、一个Boot运营主体和一种公用商品：电力。它消费Boot已完成产出的有限电力缓冲，复用Engine事务、Timeline、原Stock和Boot出口账。65/66号固定fixture CLI均exit 0，验证两日正常供电与机组失败对照；46/48又在道路、建设和Boot电力联合fixture中验证正常及机组失败两例。范围不是实时电网、日内供电连续性、多供应商市场或水电混合计量。

实测命令为`cargo run --locked --offline --release -p mycity-core --example town_utility`及带`-- --generator-failure`的对照，使用`fixtures/core-town-utility-opening.json`。正常模式第一日零供电，机组完成后第二日产出/销量100/100，电力导出100,000单位并向运营方付款，现金守恒；失败模式第二日仍无电、无产量、无销量。两种模式都通过checkpoint状态相等检查。stdout明确标注`live_transport=false`和`self_funding=not_demonstrated`。联合证据46/48中，Boot缓冲与工程、道路共用原料及现金边界；不得扩展为持续盈利结论。

`MarketProfile.utility_supply`是可选期初固定绑定，包含Boot运营账户、`stocks[1]`电力库存ID、每个配方utility整数单位对应的库存整数数量和每utility单位的固定电价。字段缺省且不序列化时保持旧profile表示；非空时必须验证实际Boot身份、库存owner和Electricity商品，不能由调用者提供“今日供给”。固定换算为正整数、价格为正，溢出拒绝。向下取整余数仍留在原库存，不凭空变成整单位供给。

绑定世界的配方`utility_account`必须等于供应者，供应者不能同时充当该配方雇主；旧`available_utility_per_day`和`utility_cost_per_output_q`必须为零，明确由实际供给及绑定电价取代。配送utility surcharge必须为零，避免同义费用重复收取。运费仍是独立明确接口；此接入本身不替换交通数据。

在完整日界结算时，Timeline与Boot时间必须一致。Engine读取当前候选库存，只把已入库数量换算成一个共享池，全部配方共享同一供应者额度。启动中、失败或尚未完成的产出不计可供数量；作业Resource/Lease不转成电量。实际产量仍受工资工时、原料、空产出lot、现金和需求规则约束，不保证利润或持续经营。

实际生产按计划中真实utility数量调用既有`Boot::export(0, quantity, ledger)`，扣原库存并累计`exports[0]`和`exported_basis`。消费者按实际耗量向供应者付款并纳入商品成本；Boot作业已经支付的燃料／承包费用不重复执行。市场服务日游标、生产耗料、产出、电力出口、成本及付款在同一个Prepared中提交；失败全部不发布，旧operation ID重试不能第二次消费。Boot内耗、跨城出口及Town结算都修改同一库存，前一次已消费的电量不能再用日初快照重复售出。

实现经`Boot::export_local_electricity`复用上述出口原语，并保留累计`local_electricity_exported`及`local_electricity_basis`作为总出口的子集。旧零值字段省略，非零本地出口必须有匹配的Town绑定。原因是原Exchange精确恒等式只覆盖跨城库存／在途／损耗；现在其数量等式明确加入本地消费，成本等式明确加入本地出口原始basis。消费者付款不是这项basis，也不冒充Exchange的跨城销售收入。不能简单把等式改成小于等于，不能因本地用电而放弃有Exchange世界的校验。

## 已测范围与仍待测门

65号normal及66号generator-failure CLI均exit 0；46号联合normal、48号联合generator-failure也exit 0。观察到的初始库存、机组60秒后完成/故障处理、导出量与basis、运营方付款、现金守恒和冷开结果只覆盖这些固定fixture，不证明长期生产平衡。

- 实际供给不足限制产量；两个配方总耗电不超过同一库存。
- 缺钱、缺料或没有有效工时不消费电力；库存未免费补充。
- Boot内耗或先发生的跨城出口降低后续可供量；未完成／失败作业不创造可用产出。
- 消费者费用准确进入供应者账户，Boot物量及成本出口恒等式仍成立。
- 事务持久化失败、冷恢复及同op重试不重复出口、付款或生产。
- 错误库存、owner、commodity、换算、时钟和残留静态配额拒绝；旧无绑定world能按原规则恢复。
- 交通proof与本电力绑定的联合范围由46/48两日CLI覆盖；单独65/66不证明道路联调。71的road-epoch测试3/3通过，72最终core/storage all-target release测试exit 0；69旧共享缓存编译失败及70定向清理的历史保留。正式8小时M0因压力守卫提前结束，16GB设备门未运行。实际状态入口为[core-road-town-integration-122](evidence/core-road-town-integration-122/STATUS.md)。

复用既有最多16个配方和同一规划池，不新增逐设备主体、第二账本或另一条日历。新增Town配置数据为一个固定长度可选绑定；临时换算沿用原配方与共享池，计算为O(配方数)，不新建按居民数量增长的数据。新增绑定不超过32字节的尺寸断言及旧无绑定兼容测试已由18/23/72覆盖；65/66记录了小fixture CLI RSS，68的旧writer无绑定world在完整副本上恢复通过。64MiB全尺度预算与16GB设备门仍待测。冻结M0来源及其标准不作修改。

另外有两个Boot本地出口分类累计量，不是另一份可花费库存。可选绑定不超过32字节的断言及兼容测试已由18/23/72的Rust测试覆盖。65/66记录了CLI RSS，但该两日小fixture的RSS记录不代表64MiB全尺度预算或完整物理验收通过；68旧writer生成的无绑定world由新reader在副本上恢复通过。16GB设备门和新整合分支的全尺度物理测量仍未运行。冻结32c来源不含本功能，其formal01已压力守卫失败，不能替代此分支测量。
