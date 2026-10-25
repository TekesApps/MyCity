# MyCity 当前实际执行与验收关闭

2026-10-25：游戏实现已发布v0.5.190（ea310a0fdb5c620d4aa9c24ec1a6e8e8d2fe621c）；v0.5.191只同步实际关闭、父清单和台账。总体既定非UI目标继续，不重复全产品规划。当前root会话01a166fc-e444-7602-ad22-3a8551dda487是共享核心唯一writer、集成/串行验收负责人，停止的旧主会话不联系。

## 当前实际在制

#132与同线缺陷#148均已完整有限验收、回填并实际关闭。v191收尾快照为0+0；当前已将#133激活为root唯一共享接线，实际1+0，上限仍1+2。文件/版本/验收入口及原始身份已固定，原组件冻结；真实V2原P5/Stock/费用/出流及生产冷恢复专项已实跑；当前source26共享33组和source27默认栈8组已终态通过，资源口径及完整验收仍待，不提前关闭。父任务持续推进、冻结交付及24项无关observer/M0 WIP不算活跃开发，无新增worker/会话/target/定时任务。

原#55实施方facility_work.rs source04继续冻结，SHA6773e22d14dfd5964a6b5c47e2c4ccdbf9b96fe07ea9f747311689c179122003；关闭后不授予新写权。近项具体会话、文件、版本、接线与验收入口见[机器台账](inputs/issue-ledger/execution-flow.json)。

## 本阶段实际交付

- [#132](https://github.com/TekesApps/MyCity/issues/132)：原Electric301资格/在场/实际P5工时、原资金工资与成本、共享住房已付收入、Payroll双向排他、原票/篡改拒绝/恢复，完整有限合同通过后关闭；Water与通用招聘匹配未冒充完成。
- [#148](https://github.com/TekesApps/MyCity/issues/148)：修复原18单元Enable16B布局回归；World74224B，120568+8456+2048=131072B。原96B Wait拒绝/冷恢复、flat wire/null/copy与默认栈均通过，原门不变，父#4仍开放。
- 同游戏字节实际18组主回归、10组受影响、6组原默认栈及原资金保护补验通过；最新release设施Store3个独立冷恢复子进程分开计数。组间有重叠，不当作不同功能数。
- [完整证据](evidence/facility-operation-20261025/STATUS.md)、[实际关闭与时间](evidence/facility-operation-20261025/closure.json)。无PR，root直接集成；main/附注tag/目标与Issue正文/标签/关闭时间均独立回读。历史失败和中断未改写为成功。

## 下一轮验收顺序与差项

|任务|责任与当前执行状态|所需具体能力与真实接线|差项|
|---|---|---|---|
|#133|root当前唯一共享接线；原#92会话01a16ffe-dbe1-7a62-b610-c5e72d490d7e组件冻结|已验#132/v190、#144/v185、v186/187净化组件与v188费用曲线；同源Runtime/V2→原Stock/P5/Water/Store|原账专项已接且修订7；source26共享33/source27原6+新2默认栈已通过；完整资源归属与关闭证据待收口，不以历史专项替代完整合同|
|#134|root集成；原#57会话01a16a4c-ba5a-7dd1-8c04-51461a76b405冻结，阻塞|原service_delivery_runtime、member/Resource/Lease/need/资金和真实交通水质来源|实际人员、到达返程、用水与有限残责缺失；#73/#74计划不算履约，不等整个父领域关闭|
|#135|root接线；原#101会话01a1682f-f3ce-7f61-833c-a31d55a43b34冻结，排队|原task_registry与#130/v185真实due消费者→原Timeline/writer|通用source/due/dirty、版本/age/覆盖及恢复唤醒桥缺，不造第二执行器|
|#136|root接线；原#118会话01a1682f-f0de-7241-b895-0bbfa1386c5b冻结，排队|v149固定physical_capacity候选与#125原Resource/Lease共池|通用canonical/模式/时窗/占用/退出恢复桥缺；IntervalLitres仍unsupported/unknown，不能以峰值代区间守恒|
|#137|root接线；原#120当前聊天worker冻结，排队|原opening_loader/显式provider与#131同槽Store|通用factory/schema/单位clock/引用provider及失败恢复桥缺；hash不是权利认证|

拟新增adapter/tests路径激活前再固定，planned不是实现或写授权。已有积压未消化前不新开源码批次；独立准备沿原实施方补接口、独立预期、边界反例、恢复输入，不为填满名额派工。编译测试仍用原外置SSD唯一target/work串行。

## 真实交付速度与时间

初始观察2026-10-25T02:39Z的21项待集成/1项待验证基线保留。本阶段新增验收业务1（132）、历史补登记0、缺陷关闭1（148）；当前已交付积压21项待集成/0项待验证。父任务、UI延期和缺陷不混入业务积压。

基线以前的历史补登记11、原F/E验收2、原栈缺陷3分别保留，不计本轮新交付。验收时点取root完整回填评论创建时间，关闭时间取GitHub实际回读；首次可实施/完整范围源码交付起点仍unknown，故两类验收耗时仍unknown，不用Issue创建、mtime、tag或当前确认杜撰。前代组件receipt只算其候选等待，不替代当前完整合同首交。

规则持续落在AGENTS及现有JSON/文档，随阶段更新，不新增调度器/daemon/定时任务，不以总关闭比例或tag数表示产品完成度。

## 保留和未测边界

24项无关WIP、失败源码、固定程序、原票/WAL/checkpoint及必要冷恢复原件保留。已验132/148原阶段证据预算1536MiB及旧768MiB guard中断保留；当前133在source21前按明确产物预估由768MiB重规划1GiB，只改证据保留盘预算，不改运行时/M0门。本阶段无新tree/target；既有target仍供后续接线使用，原件/缓存回收0B，不声称APFS释放量。

Mac16,1/32GiB、Rust1.85.1、HP P900 USB/APFS功能结果不等于16GB/native峰值/可持续倍率/长期验收；相关硬件/M0任务仍开放。base+Housing128KiB/Engine4MiB/共同诊断8MiB/normal64MiB/default栈及原M0负载/标准不变，UI/render/GPU/AgentKit未展开。

## #133未发布阶段记录（历史source状态；当前以上表及最新进展为准）

v190的#132/#148已关，v191只同步台账。当前root将#133激活为唯一共享接线，实际1+0，不新增会话/worker或源码批次。确切文件/版本/验收入口见execution-flow.json及Issue133；原water_treatment.rs/facility_work.rs等内核冻结。现有V1在source.advance后补记工资会重算累计出水成本，不能用于V2已出流状态；原advisor窄核确认私有联合prefix→真实劳动/资金暂存→组件及Stock/成本/租约/原票复核→原Prepared一次提交的边界。修订5明确earned成本与paid收入、cycle工资前缀及有来源费用/WIP责任，不修改V1或v188物理曲线。5000/5500、质量守恒和0→3→4成本预期仅算术核对；V2实际生产/编译/测试全部not_run，#133和父#92均开放。证据仍原HP P900 work，既有合同盘预算768MiB；所有M0/World/Engine/role/normal/default栈门及24WIP/旧恢复原件保持。

#133接口准备已由原owner冻结，当前1+0。root已完成source02纯组件37项（旧24+新13）实际通过，仍不是Root真付款/Stock/Water/Store联合验收；原资金basis与cash门冲突已在版本化接口解决，跨票防重/真实付款/恢复/联合峰值仍由root接线核验。source03严格V2结构3项、旧供给9项、原18-unit门1项及成本/布局单元6项通过；source04补深层owned计费断言后结构3项/原18-unit1项再次通过。它们不替代真实联合及最新共享全回归，仍无新Issue关闭/提交/tag/PR。

#133当前修订5补齐已完成cell费用阶跃/整数产品与WIP，以及显式原保护工资留债的窄边界。root的私有真实Start绑定批/8物理段/尾cell/原Stock net和联合physical planner/stager正在source05限定验证，尚未接P5实际提交，不算生产/晚付/恢复完成。当前仍1+0，完整目标和21项积压未缩减。

#133 source07修正新fixture组身份后，真实双岗位准入/Start/零工退出6项及受影响旧设施13/供给9通过；source08当前策略/指纹字节上core6和Store原票1组、4个保留world/8独立冷恢复子进程通过。完整P5生产/实际工资费用/物料WIP与最新共享门仍待接，不能以这些零工/专项门关闭。原92独立oracle31286B已冻结，当前回到1+0；修订6明确FrozenIntegerUnitLinearV1，tiny50/49mL的converted13750/49c+WIP13200/49c=原take550c只算独立预期，不冒充生产事实。source06失败与全旧源码/程序/恢复原件保留。

## 2026-10-26 当前限定进展（未发布）

实际在制仍为1共享接线+0独立准备。source09首次P5编译的内部可见性错误与原件保留；source10真实tiny1mL/20L通过，source11 core8项进一步通过100m³直接/分段、即时/保护工资晚付四组，产品98MmL/成本177548c、工资11002c/非工资现金8280c与原owner独立oracle一致。各固定程序、完整检查点及分数WIP保留于work/water-treatment-runtime-20261025。专项不是最新共享回归，也没有关闭#133。

当前source12补强原合同费率/人数/carry来源及同票实际付款证明，开始验证真实现金拒绝与生产WAL/checkpoint冷恢复；此处仍待结果。#134、#135–137排队差项与owner边界不变，无新增派工/target/定时任务；实际关闭统计不增加。

source12已终态：加强原合同参数/carry及同票工资证明后core9通过；Store2组/6world/16独立冷child均exit0，覆盖实际生产WAL、5000泵停checkpoint、5500完成WAL和晚付款checkpoint及原票幂等。source13正验证真实原headless RunDue，不把历史selector门替代实际消费。Issue133/父92已回填并独立回读保持Open，当前仍欠用途消费/完整硬边界/最新共享和资源门，无新增业务关闭。

## 最新在制：source21统一共享验收

当前仍root1+0，原owner/原advisor均完成停写。source17原MoveStock产品→同owner登记库存的出流/质量/prefix成本/原Lease及Store3组、8world20cold通过；source20 core14含原口净容量最早安全边界及药剂/残余/接收/班次/资格门通过。source14/15测试加法预期错误、source18净口最早边界缺口及资格inclusive时点错误预期、source19可见性编译失败均保留并修正，未放宽原容量和法律时点。

source21现固定同一套代码，在原SSD唯一target/work串行33组当前core/Store/headless共享门，core6组已通过，其余仍pending；随后原6组默认栈及最终联合资源/篡改/精度门。source17或旧source13专项不代当前共享通过。证据盘预算1GiB显式预估在执行前登记，保护全部原件；16GB/native/速度/长期与UI仍不冒充完成。Issue133和父92回填/独立回读保持Open；132/148实际仍Closed，无新增业务/历史补关/缺陷关闭，21项交付积压未虚减。

## 当前执行方法：游戏存档与结算一致性

用户已明确改用正常游戏保存/加载/续玩、写盘中断、回复丢失、重复结算与旧存档兼容回归；不构造攻击状态或新增安全专项。此前未实测推演不当成已复现产品缺陷，也不形成新派工。

source21已终态33组共享通过，原两次环境漏配失败独立保留。随后普通source23游戏保存测试实测发现：产品MoveStock后立即checkpoint，原操作记录缺既有游戏时钟而冷开失败。source25仅沿原Receipt.timeline补本次实际V2搬移的真实progress，核暂存相对原World已更新；不改存档结构/Action/池或放松加载规则。原advisor实际几行窄复核无阻断。当前完整供水Store4项/18world/30独立冷进程通过，含8个断写/回复丢失场景和重复结算；source23失败世界/源码/程序保留。

当前仍1+0，无新提交/tag/PR/关闭。最新修复源码的共享/default-stack/资源兼容门待执行，不能把source21旧字节33组或source25专项替代完整关闭。原M0、各运行时预算、无关24WIP、原组件4文件及全部失败/恢复原件保持；下一轮先按872MB现有证据重核完整后继验证盘预算，使用原唯一SSD target/work。

## 最新实际证据：source26/27与资源计费

当前源26共享33组（core6/Store24/headless3）全部exit0；源27原6组默认栈原标准及新增V2 core/Store2组全部exit0，未设置RUST_MIN_STACK。源31逐一重新核对固定程序SHA、当前编译输入和24无关WIP，全部一致。测试组包含不同层级/重叠场景，不累加为独立业务数。source23普通存档失败、所有后继源码/程序与完整恢复原件保留。

resource30读取18个实际完成的原Store世界：World总owned196352B；按原assembly准入规则扣公开可分列模块、仍将私有Facility留在base的保守上界130144B<=128KiB；Source owned5984B、编码7186B；Engine3696608B+声明Source96KiB预留=3794912B<=4MiB。Store完整owned最大42850559B，不能作为共同8MiB已达标。正在核对既有normal池/角色归属与有限业务增量；没有扩大预算或忽略Store工作区。native/allocator/OS/stack峰值、normal64MiB联合、真实16GB、可持续速度、GPU/UI仍未验收。资源原测量28/29错误口径及失败保留，30只分列真实结果，不改原门。

仍1+0；#133/#92保持开放，无新提交/tag/PR/关闭；证据盘预算2GiB（执行前登记），M0及运行时门不变。

## source33实际正常角色联合验证

原advisor核清共同8MiB是诊断总额，不是Store或每角色上限。既有normal64MiB池授予SimCodec59MiB、HelperCompare6MiB、Model20MiB，按可用容量准入，未扩总额。source32新增有限正常角色玩法，首跑在pool启动前因Unixsocket路径过长失败；原件保留，仅缩短显式原SSD证据根，固定程序不变，source33原玩法+池内玩法两项均exit0。实际3秒生产/49L搬移/存档后，在真实Sim角色续产、重启、checkpoint、冷开和退出；5观察点failures0，最大观测请求峰56670767B、arena使用56655648B<=usable61865528B。准备角色完整Store最大52196039B，不能用unpooled42.85MB替代。3次实际角色授予/归还，池67108864B全部回收；并未运行长压或验收整机native/16GB/速度/M0。

完整证据仍work/water-treatment-runtime-20261025；短socket与完整世界原件在work/w33，不清理或遗失引用；合计1770732134B低于事前2GiB。当前原advisor只复核这个具体后继角色门，随后完成#133完整合同/发布前证据核对；保持1+0及Issue开放，无新提交/tag/PR。
