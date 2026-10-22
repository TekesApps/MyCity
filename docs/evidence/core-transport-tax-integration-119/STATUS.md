# 主线交通履约与有限营业额税验证

最终主线来源为 `717148eab1b00e5fc183ee240240fa9150cc839f9a94aa96aaa8bb8a22f67f82`，243份Rust／fixture／manifest文件见[attempt02冻结清单](attempt02/freeze.json)与source.zip。工作区整合a15ca974交通履约和fb72899、a7ba774、13d49df税fixture；共享target及所有Store验证目录均在外置HP P900 USB/APFS SSD。仅Rust与CLI，无UI／GPU／AgentKit。

候选来源a15的core/storage release合计349项、strict及四场景实际通过，原始成功和失败日志保存在[candidate-a15](candidate-a15/REPORT.md)。根代理逐字节核对其93份core/storage运行时与Rust测试文件和主线一致，见[相等证明](candidate-a15/main-module-equality.json)。不能将该候选全套测试冒称为主线整仓重测。

根代理在最终主线实际完成两个example release构建、110项core lib测试、4项建设Store恢复测试、四个交通Store CLI、11个经营场景／覆盖配置以及strict all-targets与workspace fmt，全部退出0，见[命令与结果](attempt02/results.json)。固定二进制及SHA见[binaries.json](attempt02/binaries.json)；每场景原始stdout/stderr完整保留。

交通现在由Store从完整日历时点发行绑定固定profile的每日proof，再经Engine既有结算实际扣库存、付运费及记账。日1实际销量normal100、closed0、bottleneck40、expanded200；各自第二日继续履约。实测同op重放、同日重复／跨日旧proof拒绝、发布与回复丢失故障、checkpoint冷恢复、归档重放、历史proof缺失／篡改拒绝和低容量无状态／游标／紧急额度变化。四个世界是不同固定配置，**尚非同一世界的工程扩容**；单城独占分配、固定重复OD、运价与公用输入范围不扩大为全城／跨城联合。

税fixture以实际商品成交额为基数、5%未校准税率，通过次日到期的现有Timeline claim征付，保留分币余数。正常28日应征1400、实付1350、未来第29日应付50。真实成本压力配置（单位生产公用费50分、期初企业现金0、贷款第2日到期）销量100，应征500／实付450／到期欠税50、工资已付8800／欠13600、贷款已还995750／余4250。每次自身检查钱物、工资、税债余额及财政现金流，未用不同运行的现金差冒充纯税效应。

首轮[attempt01](attempt01/results.json)真实失败完整保留：原低现金fixture的贷款提供了流动性，5%税最终只剩未到期50，不能证明到期欠税。修正压力输入而非改模型保底，并移除税会影响经营时不成立的跨运行差额断言；100%税诊断只是失败分析，未选作平衡参数。原失败源码ZIP／程序／stderr仍保留，不覆盖失败结果。

基础家庭28日仍消耗期初现金6160分，不能宣称持续经营或平衡。营业额税仅为fixture，不是完整税制或债权优先级；公司真实亏损退出仍待接入。Engine本场景拥有容量4,100,856B，小于原4,194,304B，仅是结构容量；proof文件读取2MiB与schema字节界限不等于进程峰值。新增profile／proof／resolver／归档引用同时存活的heap及过程增量尚未实测，16GB不通过；不挪用旧M0预算。

阶段保留所有必要日志、失败证据、两个版本源码和固定程序、当前Store与故障／恢复样本；新Store目录仅约624KiB，仍供后续道路效果与退出兼容验证，本轮删除0B。复用共享target，未新造独立缓存；旧M0 smoke两代与190完整保护原件仍保留。M0冻结32c／59aa和原标准未改，8小时／16GB／完整M0未由此获得通过。
