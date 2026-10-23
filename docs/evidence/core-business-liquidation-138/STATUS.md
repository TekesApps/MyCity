# 有限企业清算、实际关闭与28日Store经营（v0.5.138）

实现复用同一Engine/World/Timeline/Store：BeginExit停止未来工作/生产、保留原有库存正常销售；SettleExit按显式法律/优先级策略实际偿付工资债务；CloseExit仅在真实无资产/负债条件满足时关闭。失败、重放、WAL和checkpoint使用原事务/回执。没有新时钟或总账。

## 实测与来源

最终source `252bd5ea4d21bcea88a0d53de499eb5fe6ec79b91568034d677f2cf02f11c3c7`，269源码/281固定输入；[退出exe](town-business-exit-final.json) SHA `3c09004a8a91db3d8cb2a72043282faaded1a3a140d2448bd5c6946d9f62167b`，[28日exe](town-business-28d-final.json) SHA `d725647c39fe3936aed1a7f0e5a2509a2106d94865ca7142b718f077a24d3411`。source ZIP完整保留在描述符的外置路径，hash `2f6466593a72730cdfe7199804e6724d285b4c5a92ad473d201d68139edafdaa`，不是只剩hash。example没有嵌入来源查询接口；绑定依据真实build、前后来源不变、完整输入归档和固定exe，不声称已执行嵌入来源自证。

最终同源161 strict全workspace/all-target、162 core146测试、163两CLI构建、165 storage库21及退出Store3测试、166 fmt均通过。161/162/163/165/166完整输入清单相同。157曾完成455项完整core/storage回归，随后有两处等价core lint简化和新example修复，因此**455不能叫最终同源全回归**；最终170测试是其部分重测，不相加成625项独立测试。46份157实际测试exe及最终测试exe独立固定，见各executables.json外置路径。

[五次真实CLI](cli-cases/results.json)退出0：两日实际生产20/销售10+10后，800工资和1200债实付，公司cash/库存归零Closed；不足现金101按两个100债清偿51/50，剩99债且Liquidating/Close拒绝。两日CLI本身保留WAL，checkpoint和丢回复/冷恢复覆盖由三个Store测试提供，不能混称每个CLI都做了checkpoint。

三个28日实际Store场景分别在第14/28日及退出操作后checkpoint冷开核hash。基线生产300/销售280，公司期末605300；低家庭现金销售219、公司599200；能源成本压力销售280、公司305600。工资均22400实付、贷款100000已还；未售原料99700、产品20/81/20和正现金残余保留，因此三例均如实Liquidating，不强行Close。[独立检查点账物核对](cli-cases/independent-oracle.json)直接读取完整最终checkpoint，按实际产量/销量复算四账户与原料/产品数量；未把CLI打印的“通过”当独立证明。

## 反例与保留限制

151/152/153初始夹具/期初余额断言及真实receipt decoder缺新Action的失败原件均保留。153在checkpoint/open真实暴露Invalid JSON，修复严格receipt decoder三Action后155通过。154失败后明确定位为测试复用已记录Overflow的op ID；修测试让旧ID仍重放失败、新ID才可结算，没改生产幂等规则。158 lint、159 example Receipt类型、160 unused-mut失败也原样保留。独立oracle首次错误地假设两日CLI存在checkpoint，失败前提另记，未改产品满足该错误。

固定opening法律/清算策略是场景授权假设，未实现估价/司法/税法或动态营业额税评估；claims/payroll绑定必须与权威Timeline完整一致，未知/不完整或不支持的资产依赖阻断。28日例使用固定贷款/工资，动态turnover tax明确未接，路线与公用仍为profile替身；既有真实道路/Boot三日场景不等于本28日全部联动。当前没有持久股权，正残值/受保护现金保持等待，不虚构唯一股东、不擅自分配、不无限财政垫付。D线必须真实接入股权/剩余分配；E/F和最终联合场景也尚未交付，因此不称全部0.1或C完整完成。

## 增量资源与保留

本阶段工作目录与共享target均在外置HP P900 USB/APFS SSD，M4/32GiB。CLI实测Engine owned 4105728B低于4194304B cap，仅余88576B；Project1664B、MarketProfile41368B、MarketState4752B、ExitPolicy1576B。它们是包含关系，不能把父子容量再次相加；新增D/E须单独计量，不能直接套原预算。

[资源观察](resource-observations.json)分别记录实际CPU/wall/RSS和完整Store文件：28日wall约3.48—3.89秒、user0.16—0.18秒/system0.54—0.68秒，最大RSS52510720B；完整每world逻辑约784KB、分配约815—819KB。计入CLI/Store/harness和OS开销，仅为微型场景观察，没做quiet性能准入，不能作为M0吞吐/normal或16GB设备门。两日CLI自报没有采进程峰值，外层time实际RSS单列，口径不同。

新CLI完整五world、165三组Store测试完整world及源/exe/失败日志保留，副本在本目录与外置原路径；[文件证据清单](evidence-manifest.json)只用于校验，不替代恢复原件。共享target复用；不新增独立target，不清理正在测量的文件。M0固定来源/负载不改，最新smoke及旧压力交接另见[v137](../m0-t13a-smoke-handoff-137/STATUS.md)，8h和真实16GB仍未完成。

阶段结束对四份逐字节相同的最终源码ZIP做硬链接去重，所有原路径及完整内容保留，分配空间减少17743872B；卷可用空间观察增加17719296B另列，非APFS独占释放。未清理target或任何业务/恢复输入，见[回收收据](stage-retention.json)。
