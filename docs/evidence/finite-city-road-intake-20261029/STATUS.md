# #21 真实道路分配与共同城市日结；#157 原资产票兼容修复

基线v0.5.220；实现/验证v0.5.221。root唯一共享writer，原输入/独立预期准备已冻结，最后0接线/0活动准备。#157有限合同已验证待实际回填关闭；完整#21/#117/#115继续Open，不以有限场景关闭父任务。

## 实际业务变化

复用v219同一住房、实际Payroll/警方到返、Boot、公服及资产28916场景。原map adapter从真实scene/terrain形成allocation，merchant入口owner7/household入口owner2，4往返×25q；Store保存immutable profile和每日proof，再通过原TownTransportSettle消费。客户端只给原operation；新增Sim `SettleTownMarket`调用原Store日结，不接受客户端quote/proof，不新建worker、求解器或第二提交口。零/保留操作号、未到完整日界拒绝，旧命令保持原路径。

三份明确初始输入，不冒充同一世界运行时改路。家庭reference-price1000是显式意愿，不增资；股本、原现金和库存不变。

|后果|开放道路日1/日2|道路关闭或门不可进日1/日2|
|---|---|---|
|实际售量|94 / 100|0 / 0|
|当日生产|94 / 100|94 / 0；先滞货，次日因output非空停该配方|
|家庭→provider1配送费|658 / 700；原独立公式7分/q|0 / 0|
|公司7现金|71955 / 72495|71015 / 70935|
|累计税/留存|280/281 → 440/441|0/-116 → 0/-596|
|最终raw/product|706 / 0|806 / 94，滞货basis263|
|最终电/水|68000 / 522000|168000 / 522000|

两组资产累计折旧均116→516，实际Payroll累计155分/56100秒/余3000；道路断供没有停止工资、折旧或保存责任。全账户现金3100000守恒。门不可进仍可能完成出程4人但不能返程，因此0完整往返；不将单向served当交付。独立手算见independent-expectations.md，并非由运行输出倒推。

Town沿既有日聚合分配模型确认其市场能力/交易，**不据此声称逐车货运到达**；警方仍沿自身原实际路线，未宣称两图同一几何或新住房场址权利。未扩展道路工程扩容、运行时交通关闭、全行业或长期自给。

## #157 的具体缺陷与最小修复

固定road02已实际复现：TownTransportSettle共用prepare_town_settlement计算资产日成本，但JournalActions::with_contexts仅允许Construction/TownSettle承载asset Context，返回model InvalidOperation、无持久票且未到半WAL。即使单补热写，asset_cost_authority::check_original的Assessment仍拒绝恢复。

只在上述两处加入已有TownTransportSettle。JournalRecord布局/Context v3、金额/原proof校验、System/单动作/日期/generation门不改。原advisor单轮静态窄核同意原因与两处范围；不将其当运行通过。

## 实际验证与复用

- 固定road04：三种初始空间道路输入的共同两日日结符合独立数量/现金/税/库存预期，6个实际独立冷子进程全部退出0。日1首次WalPayloadHalf中断未提交，日2CURRENT后丢回复已经提交；同原票热/冷重试不重复generation/折旧/费用。每组含WAL和checkpoint冷开，proof实际文件/assignment/day保留。
- 固定regression02：原18住宅120568+8456+2048=131072，World74224B；14资产旧合同（含无配置/同槽/null/copy/原票）、16建设及Core313通过/1ignored/1独立full-native诊断筛除。Storage库34通过后2项因runner缺MYCITY_QUERY_TEST_WORK失败；补齐原环境、**同程序仅补跑这2项通过**，初次失败不改为成功。旧资产Store两父及冷进程、旧无资产spatial市场和v219共同资产Store均通过。
- 固定role01：原默认栈恢复父场景（内部原模式）、旧Boot/实际Payroll/资产三种角色分支，以及新增道路开/闭两角色全部通过。实际Sim IPC读取Store proof结算，与独立Store逐票结果和最终checkpoint一致；原票IPC重试、退出/冷重启、恢复后原market票均一次。零/三保留前缀/未完整日界拒绝，不烧掉下一合法操作。
- 原59MiB授权/64MiB池不改，每角色2次workspace授予及归还平衡，allocation failures0。开/闭观测arena最大55798488B、Engine owned3690456B、Store owned42008796B（**含Engine不可相加**）；这是采样观察值，不是native总峰。creator/reference在正式角色外，单列测试工具开销。
- 当前全部最终构建源与role01 manifest逐文件一致，24份无关observer/M0 WIP SHA不变。没有更改旧fixture、M0负载/Wait/128KiB/96页门。未受影响的旧长回收/原完整旧存档字节证据按v217/218及v219有效依据复用，不重复无关720日长例，不声称当前全产品重验。

## 失败原件、资源与剩余范围

全部ZIP源码、manifest、固定程序、世界/WAL/checkpoint/proof、原票及失败日志保留于外置 `work/finite-city-road-intake-20261029`。事前2GiB预算，实际逻辑大小约1.48GB见summary；无新tree/target/timer，现有target仍为后续验证保留，没有删不明缓存。源与测量：Mac16,1/32GiB/macOS26.6.2/Rust1.85.1、HP P900 USB/APFS；不用源码内置卷代替存储环境。

road01空间seed7/城市seed107不符先拒绝，修为同一城市已声明107；road02是实际#157红例；road03的门方向预期误把有出程无返程写成双向均0，已按独立方向事实修正，原件保留。regression01命令重复--lib被Cargo拒绝，未运行；regression02环境失败及正确补跑分列。没有把这些失败记为业务通过。

本阶段新关闭业务0、历史0、有限缺陷可关闭1（实际关闭另看closure），18完整源码包未整包核销。统计起止见summary，可靠开发周期起点unknown，不按关闭窗口/tag或测试数算产品进度。完整父范围、长期供需/债务/耗尽增长、全场址几何、actual16GB/native总占用/容量速度/全M0长期/GPU/UI/Windows均未验。

下一最小场景应消费现有到期/供给与经营责任，在延长原共同时间窗时观察投入不足后的停产及义务续办；先核实际具体消费者，保持一个最低可信策略，不新写没有近期消费者的模块。PR本阶段未创建，直接main集成，发布commit/tag与实际Issue终态另回填。
