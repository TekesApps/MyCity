# 工程、交通、经营与有限供电的阶段验证

本阶段集成Rust模型与CLI，没有UI、GPU、语音或AgentKit。复用原Engine、Timeline、事务、库存、容量与Store；增加V2工程重试/清场、道路验收增量与撤回、企业停运、有限Boot电力共享消费和共同空间/交通genesis。**这是限定功能阶段通过，不是完整0.1产品、完整M0、经济平衡或真实16GB验收。**

真实命令、逐次源码清单、退出码与日志见[index.json](index.json)和runs目录；每个失败仍单独保留。所有完整world、失败输入、逐轮完整源码ZIP及固定程序继续保存在外置`work/core-road-town-precommit-01`，不是只有hash。公开归档不复制GB压力载荷。源码与依赖快照见[source-manifest.json](source-manifest.json)、source.zip；最终core/storage源码与72号相同，另有headless诊断消费者的小修由77号测试和78/79/80检查覆盖。来源差别见[逐组件对照](drivers/source-equivalence.json)，不能把不同来源的运行合并冒称一次测试。

## 实际完成的验证

| 范围 | 实际证据 | 结论及边界 |
|---|---|---|
| 核心/持久化回归 | 72-final-core-storage-tests，456.310秒，exit 0/source_unchanged=true，52个可执行程序保留 | `test --release --all-targets -p mycity-core -p mycity-storage -- --test-threads=1`。含真实进程退出、事务/检查点、旧格式、交通proof、工资税债、治理库存、清场与重试；不把嵌套子进程输出重复汇总成测试数 |
| 道路初始化时钟 | 71，3/3 | 实际空间+交通Store无需先启用时钟；有发布/撤回epoch仍须时钟、整日及结算日；未来/非整日拒绝 |
| 诊断消费者 | 77，5/5 | 补齐两个运输proof写入阶段；原14个写入槽位保持，新槽单列。75编译失败与76稀疏fixture缺失的原日志保留；固定fixture从原HEAD恢复，内容未改 |
| 全工作区 | 78、79、80均exit 0/source_unchanged=true | 所有目标编译检查、release clippy `-D warnings`、Rust1.85.1 fmt通过；不是全workspace运行测试或M0性能通过 |
| 群体交通纯CLI | 37–41 | 正常8/8；断路20/25；拥堵20/20；无路0/5。边流/容量、路径成本与未完成量独立校验，同输入stdout逐字节相同。只是有界128节点/512边/512OD模型 |
| Store交通与工程 | 50–59 | 正常、断路、瓶颈、扩容、跨城来源、投运、停运、28日、撤回、未投运清场均exit 0。验收前后同世界容量2→10，销量40→200；撤回后容量2、次日销量40，历史proof与冷恢复一致 |
| 工程与停运CLI | 60–66 | 普通/亏损停运、工资退出、施工拆除、未投运清场、电力正常/机组失败通过；原库存/欠薪/债务不抹除。停运不是法院破产或法律关闭 |
| 实际三方联合 | 46–48 | 同一Store、Engine、Timeline、原料和Boot电力池。正常第二日产/销100/100，未验收100/20，机组故障0/0；三者现金守恒、两日历史重试不变、检查点冷开hash/proof一致 |
| 28日经济压力 | summaries/economy-cli-validator-20261022T171553Z-fee3cf85d7.json | 七种profile、两种营业额税组合、参数覆盖通过；税率10001bps按预期拒绝。每例现金、原料/产出、工资、税债及财政流水独立对账；运输与公用设施仍是明确profile替身 |
| 旧writer兼容 | supplement/68-legacy-reader | 冻结v119 writer实际生成已验收工程WAL/检查点并seal；新reader只读完整副本，两组均hash一致，另有历史v119交通world一致。原件前后全文件清单不变；不宣称任意历史WAL均已验证 |

三种两日联合CLI仍使用固定OD、明确承包商有效工时输入及Boot有限电量缓冲，不是内生全城通勤或实时电/水网。施工水电供应尚未接入，Boot库存隔离的拒绝测试不能替代施工供能。许可与相关方显式gate不会自动批准，但没有完整司法/行政裁判系统。道路只支持固定单工程发布/撤回，不是任意多项目叠加。

## 经济解释与资源约束

28日道路运行真实完成28次日结算和28份已消费proof，第14日及最终冷恢复通过；只在6日有销量，最后销售在第6日，期初1000原料全耗尽。七类经济profile的销量分别为280、219、15、0、84、280、56；能源涨价使企业末现金从605300降至305600分。税压场景工资欠13600、欠税50、余债4250分，现金为0，没有保底盈利或免费补货。这些证明有限机制与传播，不证明可持续融资、完整会计或参数已平衡。

在Apple M4/32GiB、HP P900外置USB/APFS SSD上实际运行。work与共享target均在该SSD；时间/RSS记录是这些固定小场景的观察，不是独占性能测量，期间可见其他任务编译。全进程RSS：纯交通约1.58MiB；28日经济约9.27–9.33MiB；工程Engine约8.7MiB；Store交通约47.3–88.7MiB；三种实际供电联合约51.6–55.9MiB。数值见原`time -l`日志与逐例metrics，不能将RSS当phys_footprint、增量heap、公共池或16GB实机验收。

结构预算未提高：Project≤2048B、可选电力绑定≤32B、Action≤128B、JournalRecord≤4096B、Engine owned容量≤4MiB；对应断言在回归中实际通过。交通和配方仍为有界批量计算，不扩展逐人/逐车/逐设备常驻模拟。原normal 64MiB与共同8MiB不变；新整合的满规模native/stack/共享容量增量仍待实测，不能借冻结32c预算自动覆盖。

## 失败历史、审查和回收

01–27保留真实编译、fixture、断言和lint失败；42在genesis校验时拒绝，尚未创建world；69因旧writer切换后的共享core缓存误复用编译失败。70在确认无占用、固定测试和CLI程序SHA已保留后，仅执行`cargo clean --release -p mycity-core`，然后71和72重新构建验证；没有新建独立target。75未覆盖新增WritePoint导致全workspace编译失败；修复后76暴露稀疏checkout缺少原有编译fixture，再恢复固定原文件，后续检查通过。失败日志不覆盖、失败不重新记成通过。

本轮[advisor聚焦审查摘要](drivers/advisor-focused-review.md)检查V2重试、治理库存、真实电力消费、共同genesis和无epoch道路，未发现新的具体守恒或权限阻断；审查没有签64MiB、16GB或完整集成。

定向缓存清理的`du`分配统计减少774418432B（738.54MiB），SSD可用空间观察增加643170304B（613.38MiB），两者分别报告，不宣称APFS独占物理回收。见[回收收据](supplement/70-shared-cache-invalidate/receipt.json)。缓存随后被需要的验证正常重建；源码、依赖、固定exe、失败world、旧writer原件及恢复输入均保留。跨工作树切换同版本crate源码时，先留存程序和证据，再定向失效受影响包缓存；同源后续验证继续复用，不按每次测试清空。

## M0仍未完成的门

formal01原冻结负载和60秒压力守卫不改：实际约4h50m后失败，外部编译污染、缺测及原始数据全部保留。停机后[业务WAL/检查点](../m0-t13a-joint-smoke-control-03/formal01-terminal/poststop-business/README.md)和[布局57/58代](../m0-t13a-joint-smoke-control-03/formal01-terminal/poststop-layout/README.md)在完整副本冷开通过，原件未变；59.pending仍不完整，不可当恢复基线。该功能恢复不抵未到达的6h/8h。

32c完整帧normal物理仍超8431602B；三档/速度矩阵、稀有层和真实8小时仍需完成。没有真实16GB设备，`memory_16gb_gate=not_run`；`full_M0=not_accepted`。另一路新增的tier-speed入口仅为未构建/未运行源码，不随本次阶段发布，也不算验收证据。后续先在原满级同源scope定位native/stack/metadata残差，再安排剩余独占实测；不缩负载或用更小联合fixture替代。
