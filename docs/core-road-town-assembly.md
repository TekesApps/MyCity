# 工程、交通与停运限定模型

本分支基于v0.5.120，合并道路投运候选d6edb2b和企业停运候选ae9a57a。其后已在外置HP P900验证指定Rust测试、构建、CLI与旧writer兼容范围；各命令结果与边界见下文。run23全目标回归早于道路genesis修复；修复后的最终core/storage all-target release回归见run72并已exit 0。本阶段的有限结果不代表64MiB全尺度物理预算、16GB设备、完整M0或完整0.1产品验收通过。

新增的[限定道路／工程／实际供电联合CLI](town-lifelines-joint.md)和[Boot电力库存接入](town-boot-electricity.md)已分别由46–48和65–66号真实CLI运行；它们将静态生产公用配额替换为Boot有限共享库存，并区分跨城出口与本地消耗成本。道路两日／28日场景保持原负载；结果仅适用于记录的固定fixture。清场和正常施工的[请求重试策略](construction-retry-policy.md)保留旧WAL语义，新规则不改写旧失败回执。

```text
town_transport road-effect NEW_WORLD
town_transport road-suspend NEW_WORLD
town_transport road-28 NEW_WORLD
town_transport road-withdraw NEW_WORLD
town_transport cross-boundary NEW_WORLD
town_suspend NEW_WORLD --loss
```

road-effect的55号CLI实际验证同一Store中的施工耗料付款、验收前道路容量2、完整结清日界验收、次日容量10，销量40→200、现金守恒和冷恢复。fixture期初成品400及其成本来源是明确输入，不追加免费库存。

road-suspend的56号CLI实际验证上述工程/图/容量绑定；fixture使用显式承包商工时及公用成本。道路容量从2变10，销量40→200；第三日停运后产量、销量和新工资为0，库存与公司现金不变，并验证历史proof冷恢复。该场景不是全城停摆机制。

独立town_suspend CLI/Store测试还负责“两店中一家停运、另一家继续销售”；本联合CLI只有一家工厂/商店，停运后可能全市场没有供应，不把这一情况称为全城停摆机制。

亏损候选以0c34651和907d01a合入本分支源码；run61实际loss CLI及run23/72的相关测试通过。`--loss`提高显式公用成本，账面权益下降、生产停止而工资成本仍发生，再提交停运；既有债权不注销，另一家商店继续销售。这里的权益投影仅为现金、库存取得成本及既有工资/债权余额之和，不等于完整会计利润或已校准的持续经营模型。

58号`road-withdraw` CLI验证拆除审批期间保留扩容，完整结算日界撤回增量，容量回到2；第三日基础销量40，工程关闭、场地释放，历史服务日proof不同且冷恢复通过。此结果不扩展为中断、任意失败回滚或其他未运行路径的通过声明。

54号`cross-boundary` CLI对同一独占市场的双城节点验证两日固定OD，往返边界来源各自保留，实际销量两日均为100，现金守恒、供应商运费收据匹配，冷开历史proof通过。独占仅指市场共同消费配额，不表示所有节点同城；期初不可变配置不是Region人口快照、迁居凭证或跨城合同履约。

限定范围：施工劳务仍是明确的承包商有效工时输入，尚未来自全城就业分配；公用容量与单价为显式接口配置，尚非实时电网/水网；运输采用固定群体OD重复规则，企业停运只过滤经营报价，不声称OD已经内生响应。道路效果只支持同一工程发布一次、撤回一次，不支持重建、多项目叠加或拆除时额外封闭基础车道。停运不是资产冻结、优先清偿或法律关闭，未知程序不会自动批准。原库存、欠薪和债务不抹除。

## 未投运工程退出的新增源码候选

整合审查发现，旧停工分支只能恢复建设，不能在未投运状态有成本地安全退出；不能以先完工投运再拆除代替。现已补入有界清场规格和历史摘要，复用原审批、施工、账务、容量和场地租约。只有带期初有效规格、从未发布运营能力的在建／停工／待验收项目才能申请；原参与方重新明确决定，审批不自动通过。开始清场不重新占地、不撤回基础容量，原材料消耗与累计成本保留；完成额外工时、接收实际废物并付费后才释放场地。

21号Store清场测试5/5通过；64号Engine清场CLI exit 0，输出为14400/28800建造工时、2400清场工时、1个废物单位及250费用，释放场地且未发布运营能力，并明确标记`model_only_not_full_legal_process=true`。59号道路清场Store CLI另验证两日销量各40、现金守恒及冷恢复。已测范围不证明完整司法/行政流程或所有换ID废物重放反例。

道路清场必须另行验证不可变交通绑定：工程已Closed但从未投运时仍保留基本道路容量和原历史凭证；普通无交通配置的清场Store测试不证明该联调。正常已投运拆除则仍须按完整已结算日界撤回工程增量。两种退出不能互相混用。新字段不提高既有Project和公共工作区预算，尺寸、持久化兼容和实际进程资源都必须实测后报告。

旧writer兼容门已完成：68号使用冻结旧writer实际生成并seal的accepted WAL、accepted checkpoint及v119 transport world；新reader在完整副本上三例均exit 0，副本起始hash与seal一致，原件未变。69号因复用的旧共享缓存缺少当前源码符号而编译失败并保留；70只清理mycity-core release缓存；71新增road-epoch测试3/3通过；72最终core/storage all-target release测试exit 0、source_unchanged=true。该功能兼容及回归测试不构成64MiB或性能证据。formal01受压力守卫提前终止，8小时与16GB设备门未完成；失败记录及所有历史失败尝试保留。证据索引为[core-road-town-integration-122状态页](evidence/core-road-town-integration-122/STATUS.md)。

## 实测范围与保留的失败历史

以下结果来自`/Volumes/HP P900/dev/MyCity/work/core-road-town-precommit-01/`，所有有结果的条目保留原`exit.json`与stdout。12（core lib）、18（town utility）、21（storage clearance）、22（town utility recovery）、28（strict clippy）、30（fmt check）、31（CLI build）、45（genesis-fix build）及49（CLI rebuild）均exit 0。20中construction retry suite单独3/3通过，随后utility recovery suite 0/2失败；该失败不抹去前一suite结果。23中52个test executable命令全部exit 0，但该整轮在道路genesis修复之前；修复后71的epoch tests 3/3、72的core/storage all-target release测试exit 0，提供最终回归证据。

失败历史保持可见：01–10和13为早期编译失败；11有一项失败；15–17为utility测试/构建失败尝试；19 construction retry policy其中一用例失败（同轮material retry与WAL suite另有通过记录）；20的后续utility suite失败；24–27 strict clippy失败；42 lifelines CLI以`town_transport_road_project_mismatch`失败；69为旧缓存误复用导致的编译失败。70清理目标包缓存，71/72是后续新缓存验证。后续修复后的通过只适用于对应范围，未删除或覆盖早期日志。formal01因系统压力守卫失败而未完成8小时；64MiB/16GB设备、完整M0仍未通过/未完成。
