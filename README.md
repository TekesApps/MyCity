# MyCity

仓库发布版本：`v0.5.224`。已开始Rust模型与headless CLI，当前不做UI。

v0.5.224 验证Town消费后原发电批次真实补货：准时/迟到、WIP/原料费用/同库存basis及冷恢复/原Sim预算门通过；生产代码/原输入未改，不称连续电厂或长期自给。完整#21/#117/#115开放。[结果与边界](docs/evidence/finite-city-refill-20261029/STATUS.md)。

v0.5.217 完成[#155实际岗位工时与公服成员共存](docs/evidence/payroll-members-runtime-20261028/STATUS.md)，修复#156正常存档恢复入口。共享/默认栈与原normal角色有限门通过；旧回收长例本次超时不计通过，复用范围单列。Issue关闭按后续回执，完整城市/16GB/M0长期仍未验收。

v0.5.216 接通[#21原Boot输出→实际Police消费](docs/evidence/finite-city-runtime-20261028/BRIDGE.md)，与Town同源用电、原票/中断/冷恢复、5任务回收及原normal资源门通过；账户Payroll成员时窗仍未接，完整城市/21/115开放。

v0.5.215 接通[#21本城有限Boot profile的共同输入与冷恢复](docs/evidence/finite-city-runtime-20261028/STATUS.md)：旧profile不变，同一存档实际公服到返、两日日结与WAL/checkpoint原票重试通过；实际公服供给桥和完整业务资源门仍未完成，#21开放。

v0.5.214 仅同步[#151及三项具体回归的实际关闭回执](docs/evidence/asset-capital-runtime-20261028/closure.json)与当前执行状态；游戏实现/验证见v213。父117仍开放，原完整M0和真实16GB产品容量不变。下一只固定已有业务的有限城市共同输入，不新增模型批次。

v0.5.213 完成[#151自有工程资本与原日税有限闭环](docs/evidence/asset-capital-runtime-20261028/STATUS.md)：真实工料投运、折旧、停用与实际关闭成本、原票及保存恢复，旧18单元/128KiB与4096B收据门、默认栈和原normal角色通过。#117完整资产领域仍开放；Issue关闭按实际回执同步，原M0/真实16GB产品容量未扩张。

v0.5.212 接入[原Sim角色与Air owner资源来源观测](docs/evidence/task-resource-facts-20261028/STATUS.md)：当前owner、历史执行及全角色arena分开，48项同源限定检查通过，原授权/实际wait归还不变。完整#102/103/104仍开放，不增加业务关闭或M0/16GB结论。

v0.5.211 仅同步[游戏v210发布源码的准确指纹补验](docs/evidence/bound-air-pages-20261028/STATUS.md)：458个构建输入与固定程序指纹一致，3项限定补验通过，原23项按版本复用；不新增业务或改变游戏代码。

v0.5.210 接入[共同绑定的Air精确分页请求](docs/evidence/bound-air-pages-20261028/STATUS.md)：显式等价页计划复用原控制/票据/owner，初始实际quota有独立执行摘要。26项按版本对应的检查通过；完整#102/103/104仍开放。

v0.5.209 接入[绑定原任务的共同能力/执行请求](docs/evidence/task-execution-protocol-20261026/STATUS.md)：40父测试与3实际冷恢复子进程通过，仍沿原handler、原票和资源owner。完整#102/103/104开放；未增加业务验收或M0/16GB结论。

v0.5.208 wires [common actual Air/Sim execution reports](docs/evidence/task-execution-report-20261026/STATUS.md) on the existing RunDue/Store path: 28 current-source checks pass, original role grant/return unchanged. Full #102/103/104 remain open; no new business acceptance/closure. Prior #134 remains accepted on v206/207.

v0.5.206 完成[#134有限实际公服履约及牵引恢复](docs/evidence/service-recovery-20261026/STATUS.md)：全体原车内驾驶资格到期后真实到车/牵引返站，完整v205存档兼容、缺事实v203候选明确拒绝且保留原件。18核心、5新增Store父/13冷子进程、原履约Store、22共享默认组及原normal角色通过；[#134已实际关闭](docs/evidence/service-recovery-20261026/closure.json)，v207仅同步回执。父57/74及真实16GB/M0长期验收继续开放。

v0.5.204 为误标签，实际指向v203旧提交，不含本次接线；已发布标签保留不移动，当前实际增量见v205及其证据。

v0.5.203 固定[最新源码审计检查点](AUDIT_HANDOFF.md)：#134真实到返接线与最新未完成收入桥可供审计，完整合同仍开放；旧source06单条正常链通过不覆盖当前源码。24份无关WIP提供完整补充包，原件保留；本次不编译测试、不新增验收或关闭。

v0.5.202 同步[#149实际验收关闭](docs/evidence/timeline-target-emergency-20261026/closure.json)及父118/关联136台账；v201游戏与验证字节不变。累计新验收业务5、历史补关0、缺陷3；18待集成/0待验证不变。本tag不新增业务，原M0/16GB等门保持原状态。

v0.5.201 验证[#149完成边界取消与实际退出](docs/evidence/timeline-target-emergency-20261026/STATUS.md)：4父专项、9组共享、原8+新1默认栈和真实normal角色均通过，原票／中断／冷恢复保留原义务和占用。v200生产候选字节未变，仅修测试配额预留；Issue关闭另按实际回执。原M0／16GB／native／长期范围仍开放。

v0.5.200 固定[最新非UI审计检查点](AUDIT_HANDOFF.md)：包含#149实际失败复现、当前修复候选及专属测试；候选尚未编译／运行，不作为功能验收。24份无关WIP以完整补充包提供，原件保留；本次关闭0项，原M0标准不变。

v0.5.199 同步v198已实际验收的#137关闭与#150默认栈缺陷关闭，父120保持开放；[回执](docs/evidence/typed-opening-provider-20261026/closure.json)。累计新验收业务5、历史补关0、缺陷2，积压18待集成/0待验证。本tag仅台账，不重复计业务；下一优先实测#149，不把其源码推导当已复现。

v0.5.198 接通[#137有限typed开局](docs/evidence/typed-opening-provider-20261026/STATUS.md)：同scope/domain/version引用、明确单位/Boot离散钟、原resources整表与同槽发布/冷恢复；旧v1兼容。修复#150公共分派默认栈回归，原6+新2默认门、当前CLI和真实normal角色均通过。父120/真实16GB/M0仍开放；Issue实际关闭与统计另按回读同步，不把本tag当验收。无PR，24WIP及失败完整原件保留。

v0.5.197 发布[#136原池时窗内核完整有限验收](docs/evidence/canonical-capacity-20261026/STATUS.md)与实际关闭台账：实现仍为v196，当前受影响6组、默认栈8门、真实normal角色联合业务/保存重启/原票全部通过。父118及真实16GB/M0/其它consumer继续开放；新验收业务累计4，历史补关0，缺陷1，积压19待集成/0待验证。原v196审计快照保持不变，无PR。

v0.5.196 固定[当前非UI审计基线](AUDIT_HANDOFF.md)：#136代码与直接相关共存/满表/保存恢复检查已入库，完整共享/默认栈/资源验收尚待，Issue仍开放。附24份未集成WIP完整审计包；本tag关闭0项，累计统计和差项见审计入口。

v0.5.195 同步已由v0.5.194完整实际验收的#135关闭与父清单。本窗口新验收业务3（132/133/135）、历史补关0、缺陷1（148），20待集成/0待验证；本tag仅台账，不重复计业务。

v0.5.194 接通[通用任务目录与原消费者](docs/evidence/task-registry-directory-20261026/STATUS.md)：真实版本/覆盖/dirty进入原RunDue，原Timeline已接受责任可续办，同P5多来源只提交一次。最新9项、受影响原6组、原纯13及正常借款/区域付款保存恢复通过；当前角色授额不变。已回填关闭#135，父101/真实16GB/M0/全局性能仍开放；无PR。

v0.5.193 同步已在v0.5.192真实验收、发布并关闭的#133有限取水—处理—原库存出水业务及父清单/统计。本窗口新验收业务2（132/133）、历史补登记0、缺陷1（148），21项待集成/0待验证；本tag仅关闭台账，不重复计业务。[完整证据](docs/evidence/water-treatment-runtime-20261025/STATUS.md)。父92/M0/native/16GB/速度及UI/GPU仍未完成。

v0.5.190 接通[原发电机设施操作、实际工时与工资](docs/evidence/facility-operation-20261025/STATUS.md)：同源18组共享回归、10组受影响回归、6组原默认栈验证和独立资金保护补验通过，原票/WAL/checkpoint/冷恢复有保留证据；修复原18单元住房前置16B越界，未改128KiB/96B断言或M0门。有限范围归#132/#148，完整#55/#86/#92、公服履约、真实16GB/native与完整M0仍开放。后续实际在制和差项见[执行清单](docs/execution-current.md)。

v0.5.189 整理[业务Issue执行与验收台账](docs/issue-execution-ledger.md)：保留领域父任务，按独立业务能力回填历史实际验收和剩余接线，UI延期、缺陷与真实16GB门单列。仅文档/Issue整理；当前设施工时工资WIP尚缺最终联合回归，不随本tag作为游戏功能发布。

v0.5.188 unifies original source-lot cost arithmetic and verifies [allocation-cycle prefix costs](docs/evidence/continuous-supply-cost-20261025/STATUS.md):3 arithmetic tests and9 original continuous-supply regressions pass. V2 withdrawal/joint treatment/Store integration remains pending. The previous24-test Idle component gate remains limited to synthetic RootVerifier; full #92 stays open.

v0.5.185 adds [original supply RunDue and OpeningInput recovery](docs/evidence/continuous-supply-due-20261025/STATUS.md):27 different release parents, one additional default-debug execution and six independent Store recovery children pass. Existing task priorities, original receipts and accepted targets survive hard supply stops; full issues and M0 remain open.

v0.5.184 接通[固定机组与两泵的原账连续供给](docs/evidence/continuous-service-supply-20261025/STATUS.md)：87项不同release测试、另1次默认debug执行与10个独立恢复子进程通过；分数投入、有限接收、一次启动费和非零时点停启恢复均有实际证据。#57仅组件验证，卫生水与真实服务履约、完整#85/#86和M0仍未完成。

v0.5.183 补齐[P5 必要续页调度与开局恢复联合门](docs/evidence/service-due-opening-20261025/STATUS.md)：29项不同release测试、另1次默认debug执行通过；C操作碰撞保留原责任，原票恢复不多推进一页，OpeningInput同槽及WAL/checkpoint恢复有实际证据。完整#57/#101/#120与M0仍未完成。

v0.5.182 接通[真实自然需要与期限](docs/evidence/service-natural-needs-20261025/STATUS.md)：原 HousingLease 同笔登记合法租赁入场，原 Timeline/P5 分页与 Store 原票恢复；300项不同release测试、另18次默认debug执行及48个独立恢复子进程通过。完整#57派遣、现场投入、收费和返程仍待接线，原M0门不改。

v0.5.181 接通[独立登记的唯一成员活动账](docs/evidence/independent-member-authority-20261025/STATUS.md)：教育借用原账，Store核开局身份并保留动态游标；285项不同release测试、另15次默认debug执行及32次独立恢复成功。完整#57的自然需求和出勤履约仍待接线，原M0门不改。

v0.5.180 接通[无教育依赖的原始基础资格登记](docs/evidence/baseline-qualification-20261025/STATUS.md)：显式协议3复用原资格状态、Housing人口身份与Store认证恢复，274项不同release测试及6次默认debug执行通过，14次独立恢复成功。旧协议2保持原口径；完整#55/#57、真实招聘与M0仍开放。

v0.5.179 补齐 v178 的[依赖链回归](docs/evidence/dependency-chain-regression-20261025/STATUS.md)与[E住房回归](docs/evidence/qualification-e-regression-20261025/STATUS.md)：#118→#101→#120 共24项主测试、4次独立恢复通过；住房13项与两次CLI执行通过。仅发布实际证据，不改游戏代码、M0标准或完整Issue状态，WIP与恢复原件保留。

v0.5.178 接入[原资格状态、考试完成与只读就业门](docs/evidence/qualification-authority-20261025/STATUS.md)：紧凑 Action 与原票 Context、授予/撤销及冷热恢复，381 项不同 release 测试、12 次默认 debug 验证通过。修复新发现的 #124 栈溢出，原 #122/#123 保持独立；完整 #54/#55、真实招聘、16GB 与 M0 仍开放。

v0.5.177 接入[有限免费线下教育的原权威与恢复](docs/evidence/education-authority-20261024/STATUS.md)：Opening/World、唯一成员活动账及原 Store 同笔提交，退出保留未知历史，压缩认证原票；341 项不同 release 测试和 9 次默认 debug 验证通过。修复冷开重复大投影导致的新栈问题，并独立核算恢复工作区；完整 #54、16GB 与 M0 仍开放。

v0.5.176 接入[本地操作员观测与原票读取权限](docs/evidence/operator-observation-20261024/STATUS.md)，24 项不同测试通过；原 CLI Page 格式保留，账户不能绕过操作员查询门，自己的成功/失败原票仍可冷热恢复。完整 #49、账户隔离、真实 16GB 与 M0 继续开放。v175 的[有限 E 住房包](docs/evidence/housing-no-candidate-wait-20261024/STATUS.md)已收口，父 #4 继续开放。

v0.5.173 修复 [Store 分页临时投影](docs/evidence/query-page-projection-20261024/STATUS.md)，6 项限定串行回归通过；完整 #49 仍未完成。

v0.5.172 新增[内部成员活动互斥组件](docs/evidence/member-activity-authority-20261024/STATUS.md)，并修正 venue 扩展后的解码参考镜像；244 项串行测试通过，真实教学及 World/Store 接线尚未完成。

v0.5.171 校正近期状态登记：#122/#123 已在 v0.5.159 修复并关闭，失败原件继续保留；冻结下一步教育实际活动接线边界，尚未宣称实现通过。

v0.5.170 补齐[场所开局同槽发布与恢复证据](docs/evidence/opening-venue-store-20261024/STATUS.md)：17 项串行测试及四次新进程恢复通过；共享 Store 实现未改，期初资本不重发、原目录不覆写，完整 #120 和 M0 仍开放。

v0.5.169 接入[原必要任务的真实选择](docs/evidence/required-dispatch-20261024/STATUS.md)：86 项串行测试通过，RunDue 在 P4 选择原空气待办，保留取消、重绑和到期责任；不自动推进时钟，完整多模型调度和 M0 仍未完成。

v0.5.168 接入[受管场所原池容量与恢复](docs/evidence/venue-authority-20261024/STATUS.md)：123 项串行测试通过，实际占用、退出、重放和旧格式槽复用有有限证据；新预算分区、失败原件与未完成 provider 均明确保留，完整 #118 和 M0 仍开放。

v0.5.167 补齐[共享 broker 准入与恢复验证](docs/evidence/air-shared-broker-20261024/STATUS.md)：114 项串行测试通过，确认单 Sim 准入隔离、取消归还与冷恢复；多 Worker 并行、全游戏预算及 M0 仍未完成。

v0.5.165 接入[实际页执行采样](docs/evidence/air-page-sampling-20261024/STATUS.md)：91 项串行测试及 12 次旧新 worker 对比通过；采样与暂停等待分开，自动总预算仍未激活。

v0.5.164 接入[精确分页与生产暂停/继续](docs/evidence/air-page-control-20261024/STATUS.md)：313 项串行测试通过，保留原任务、授额及提交边界；完整 #103、自动总预算与 M0 仍未完成。

v0.5.159 固定[当前有限集成基线](docs/evidence/release-association-20261024/STATUS.md)：默认栈恢复修复、同槽期初输入、容量只读投影、F/E 持久化、实际 Sim 调度观察及 CPU Worker 生命周期已有分项实测。未激活的教育源码仍为候选；动态总预算控制、生产 provider、完整 M0 与真实16GB均未完成。旧失败和无关 WIP 保留。

v0.5.154 verifies [creditor-first residual settlement and bounded parallel work](docs/evidence/core-parallel-154/STATUS.md), adds DoorPose candidates and bounded pool counters. Persistent geometry activation and Move remain incomplete. v0.5.153 adds [actual-paid-wage windows](docs/evidence/core-housing-income-153/STATUS.md), verified by canonical combined tests and a fixed CLI cold recovery. Move and full M0 remain incomplete. v0.5.152 fixes [bounded native snapshot recovery](docs/evidence/m0-t13a-native-347/STATUS.md): 29 targeted regressions and a full3.37GB original-snapshot cold recovery pass. The preceding600s native run remains failed; normal memory,8h and real16GB gates remain unmet.

v0.5.151 adds [bounded Sim-open lifecycle receipts](docs/evidence/m0-t13a-sim-open-diagnostic/STATUS.md), verified with eight focused tests and a complete720-hour/723-target real-business CLI run plus cold recovery. It measures owned capacity, not native peak or a memory saving; original M0 gates remain unmet.

v0.5.150 verifies [finite housing lease/recurrence and a persistent D/F/E scenario](docs/evidence/core-housing-runtime-150/STATUS.md): 525 distinct tests, final strict/fmt, actual CLI, real rent/arrears, cold recovery and historical transport proofs. Move, housing route binding and original M0/real16GB gates remain incomplete.

v0.5.149 verifies [dividend-to-liquidation persistence and the bounded IPC reader](docs/evidence/core-dividend-exit-reader-149/STATUS.md): actual capital return/recovery, three reader tests and eight Sim/worker persistent scenarios on main. No physical-memory saving or original M0 performance pass is claimed.

v0.5.148 verifies the [finite geometry candidate component](docs/evidence/geometry-candidate-148/STATUS.md): 17 release tests, strict/fmt, two actual CLI runs and a reproducible independent planar reference. It does not activate geometry in Store; E recurrence and original M0 performance gates remain incomplete.

v0.5.147完成[有限地面 WALK 道路/出入口开闭与 D+F 同存档联合运行](docs/evidence/core-spatial-runtime-147/STATUS.md)：同源237项Rust测试、strict/fmt、实际CLI、独立进程恢复和真实v144旧存档副本验证通过。几何改造、E接线、原M0性能门及真实16GB仍未完成或未测。
v0.5.144完成[有限实缴股权、可信税费、分红和清算闭环](docs/evidence/core-equity-runtime-144/STATUS.md)：最终同源212项Rust测试、strict/fmt、实际CLI及独立检查点核账通过，政府实收分红已再次支出。原M0失败和16GB未测保持；F/E运行时接入继续推进。[实际agent分工与共享核心负责人](docs/core-task-dispatch-current.md)已固定，当前不展开UI、GPU、语音或AgentKit。

v0.5.140归档[固定 full 联合正式尝试的提前失败](docs/evidence/m0-t13a-joint-formal-01/STATUS.md)：实际 1,226 秒后系统压力触发原保护，8h 未完成；normal footprint 仍超门且存在外部编译污染。原件保留，真实16GB仍未测，不宣布M0通过。

v0.5.139完成[有限空间步行接入真实Store交易](docs/evidence/core-spatial-traffic-139/STATUS.md)：实际Scene转换、OD/proof和销量因果，开放40/封路0/单向门0；冷恢复及精确重放通过。同源6测试、strict/fmt、独立账物/proof及进程资源观察通过；仅三节点微型场景，同Store运行时空间改造和其他交通方式仍未实现，不称F完整或M0通过。

v0.5.138完成[有限真实企业清算与28日Store场景](docs/evidence/core-business-liquidation-138/STATUS.md)：库存销售后实付工资/债并关闭、现金不足保留债务，WAL/checkpoint冷恢复与重放通过。最终同源170测试、严格检查、五CLI及三组独立检查点账物核对通过；此前455完整回归分来源列示。动态税、股权残值、住宅及空间交通联动尚未完成，原M0/8h/16GB门保持未完成。

v0.5.137记录[M0实际smoke、旧格式冷恢复与基线交接](docs/evidence/m0-t13a-smoke-handoff-137/STATUS.md)：600次布局扫描及完整业务/恢复完成；外部编译干扰、缺测及快进超预算明确保留，clean/normal未通过。仅精确回收旧两代4份合成载荷，分配空间约5.914GiB；当前及旧兼容恢复原件完整保留。8h未启动，真实16GB仍not_run；C—F隔离开发不算本发布完成。

v0.5.136修复[M0观察器来源键绑定](docs/evidence/m0-t13a-observer-source-136/STATUS.md)：原j3n通过准入后因工具字段错误零采样退出，失败原件保留。新控制器统一严格来源校验，18项Python测试、静态预检及真实冻结Rust故障退出路径验证通过；未运行新的smoke或8h，不改原负载/阈值。C线后续正在隔离实现真实企业清算与关闭，现有亏损停运不改称退出。

v0.5.135完成[建设定额补偿与恢复验证](docs/evidence/core-construction-compensation-135/STATUS.md)：owner申报、收款方接受、实际转账、工程支出和门状态在同一事务提交；失败不扣款，重放不复付，后续拆除复审不再次付款。最终同源299项测试、七个既有联合CLI及严格检查通过，修复两项损坏检查点校验缺口。未实现估价、协商、托管或自动退款；原M0性能、8h及真实16GB门仍未完成。

v0.5.134: [Fixed maximum transport probe](docs/evidence/core-transport-capacity-134/STATUS.md) passed all four cases and deterministic repeats at 128 nodes, 512 edges, 128 entrances, 512 OD groups and 127 hops. Path-index memory estimate corrected. Failed fixture attempt and withheld scoped-index candidate retained; observations are not clean-performance, M0 or 16GB acceptance. Construction compensation remains under isolated validation.

v0.5.133完成[显式换岗与真实设施联合验证](docs/evidence/core-employment-lifelines-133/STATUS.md)：相同开局、相同2/2/10道路容量下，日工资80/80/80与80/8/8经实际到户价4分产生销量20/20/20与20/2/2。七模式、独立账物/工资/proof核对、冷开重放及重复运行通过；零运费错误断言的失败保留。只改CLI，未实现内生求职或持续经营；M0原normal/四速/8h及真实16GB门仍未通过。

v0.5.132完成[受限整数规范编码优化与来源核对](docs/evidence/canonical-integer-132/STATUS.md)：保留原数值语义、完整校验和恢复格式；137项core、93项headless、21项storage库测试、20项Store联调及最终CLI/严格检查通过。局部真实Engine哈希中位耗时降低约4.87%，旧候选完整30日功能通过但仍约188秒，不能宣称快进改善或性能通过。最终集成来源独立绑定，错误来源收据和缺输入失败保留；原normal、三轮四速、8h与真实16GB门不变。

v0.5.131完成[显式就业变化与28日购买力对照](docs/evidence/core-employment-131/STATUS.md)：默认换岗使原企业产量200→100、销量112→100；隔离新工资差异后成交87/43。旧欠薪继续由旧雇主补发；六组各28日和第1/27日边界、独立18行账物/工资核对及严格检查通过。仅新增CLI，任职显式输入，交通/供给仍为profile替身，不代表内生就业、持续自融资或持久恢复验收。原M0 normal、分级四速、8h与真实16GB门不变。

v0.5.130记录[原满级内存与真实Engine耗时](docs/evidence/m0-t13a-native-timing-130/STATUS.md)：两代/冷恢复通过，同相位3336228080B仍超normal 7628426B，未观察到降幅；短测没有外部编译采样，但有2个进程内存缺测，不签性能。隔离typed探针、135项core测试与严格检查通过，诊断源码未合入；下一步仅验证advisor建议的受限整数编码优化。三轮四速、8h和真实16GB门保持未完成。

v0.5.129完成[三日真实供给、施工、交通与经营联调](docs/evidence/core-three-day-supply-129/STATUS.md)：显式停工/恢复、零有效工时拒绝、实际水电费用与材料成本、验收次日扩容及冷恢复/精确重放均跑通。正常与扣留组容量分别2/2/10与2/2/2，实际销量20/20/60与20/20/20；五个最终CLI场景及严格检查通过。当前单批库存生产、固定OD与显式班次限制保留，未证明自动断供处理、持续盈利或性能通过；原M0 normal、三轮四速、8h及真实16GB门不变。

v0.5.128完成[有界分段计时与共享材料完整生命周期](docs/evidence/core-timing-material-128/STATUS.md)：最终共享raw7两条生命周期及13项Store测试、93项headless测试与严格检查通过；新来源完整30日/720小时/723目标/724窗口功能、完整Engine对照及冷恢复通过。实际计时显示工作窗口父区间111.85秒，Status业务/完整摘要合计50.36秒；11次外部编译采样及118个进程内存缺测使clean性能不接受。原normal、8h和真实16GB门不变，独立联合场景继续推进。

v0.5.127完成[满级30日两档实际速度观察](docs/evidence/m0-t13a-full-speed-127/STATUS.md)：两档完整业务、状态对照与冷恢复通过；1秒/日188.76秒未达30秒预算，10秒/日299.95秒观测满足300秒及逐日预算。两档都有外部编译采样，另有进程内存缺测，clean验收不接受。准入控制器失败保留并修复，原normal、三轮四速、8h与真实16GB门不变。

v0.5.126新增真实施工Boot电/水履约与累计材料去向：按累计有效worker工时差计量，实际库存、费用、施工进度同一事务提交；材料sink使用真实库存lot basis，不再重复支付现金，清场和拆除不会重置累计sink。容量资源上限调整到16以覆盖实际9资源生命周期（原8不够），既有4MiB Engine／64MiB role界限不变。Run19的11个Store联调测试通过，覆盖施工、Town及跨城Exchange依次争用同一Boot电力库存，以及WAL/checkpoint/重试；路由source77仍是测试替身，不是真实路网联调。详见[施工Boot供给与材料核算](docs/construction-boot-material-accounting.md)。

验证边界：run15全workspace/all-target命令历时946.53秒后exit 101，唯一失败是storage metadata仍按旧资源数8断言；run16修正后storage library 21/21通过，run21在新增测试后严格Clippy通过，最终CLI与冷恢复通过。这不是整个workspace命令exit 0。M0既有物理/8小时失败维持，实际16GB未运行；旧来源性能不转移给本候选，64MiB整体角色准入也未因此通过。详见[本阶段原始收据与边界](docs/evidence/core-construction-utilities-126/STATUS.md)。

v0.5.125完成[合并pool进程的原满级诊断](docs/evidence/m0-t13a-combined-pool-125/STATUS.md)：同源正常对照、两代/冷恢复、单次半帧和三类进程/恢复失败实测完成，修复混合root错误R07归因。完整恢复窗口少1,327,440B但仍超原normal 7,628,426B，不签物理门、8h或16GB。旧两组8个纯合成载荷精确退役，最新完整恢复输入保留；该v125阶段记录当时建设材料/水电联调仍在继续，当前候选结果另列于上方，不因功能通过而签联合性能。

v0.5.124完成[分级速度入口与观察器时钟握手修复](docs/evidence/m0-t13a-tier-speed-124/STATUS.md)：三档各完整30日实际业务、冷恢复和独立全状态比较通过，03原时钟超时失败保留。尚非三轮/四速矩阵或联合性能通过，采样缺测及L1缺席明确记录；原normal、8h与真实16GB门不变。建设真实水电和减少重复进程开销的候选继续隔离实施，尚未实测。

v0.5.123完成[原满级R07的四角色native归因诊断](docs/evidence/m0-t13a-full-r07-native-attribution/STATUS.md)：03同源满映射窗口获得4份VMmap及两代/冷恢复；01/02控制脚本错误和原日志保留。辅助进程另计，VM分类不抵预算；02四份过时合成文件精确回收约5.922GiB，03完整原件保留。物理超额、速度、8h和真实16GB仍未通过，下一步只验证减少真实重复进程开销的候选。

v0.5.122完成[工程、交通、经营与有限Boot供电的阶段整合](docs/evidence/core-road-town-integration-122/STATUS.md)：真实CLI、28日经济压力矩阵、旧writer兼容、core/storage完整回归及全workspace严格检查通过；新增源码、失败历史和外置SSD环境逐项留证。M0停机后业务与57/58代布局快照恢复通过，但原formal01仍失败，正常物理预算、分级速度、8小时及真实16GB门仍未通过；不宣称完整M0或完整0.1完成。

v0.5.121归档[formal01提前终止证据](docs/evidence/m0-t13a-joint-smoke-control-03/formal01-terminal/README.md)：正式8小时尝试因原60秒系统压力守卫失败，未完成8小时；保留外部编译污染、缺测、完整恢复输入及原负载。2h／4h恢复已执行，6h／8h及真实16GB未通过，完整M0仍不接受。该次仅发布失败证据；0.1限定整合的后续结果见v0.5.122。

v0.5.120完成[工资退出原语与Store恢复实测](docs/evidence/core-payroll-exit-integration-120/STATUS.md)：16项Timeline、3项Store退出、18项storage lib、110项core lib及实际CLI/严格检查通过；保留故障复现，尚非企业清算。另归档[原负载607.95秒联合短测](docs/evidence/m0-t13a-joint-smoke-control-03/STATUS.md)及[旧压力载荷精确回收](docs/evidence/disk-reclamation-smoke02-handoff/STATUS.md)；未将短测、外部编译污染或脚本纯测试称为性能通过。原8h正式运行、速度/分级门及真实16GB仍未通过。

v0.5.119完成[交通实际履约与有限营业额税主线集成](docs/evidence/core-transport-tax-integration-119/STATUS.md)：主线110项core lib、4项建设恢复、四交通CLI、11经营配置及strict/fmt通过；候选349项独立记录。日历绑定proof实际影响销量与运费，税费复用Timeline并保留欠额，首轮压力fixture错误保留。工程动态扩容、真实企业退出、增量heap/16GB及M0仍未完成。

v0.5.118完成[联合耐久观测器离线修复](docs/evidence/m0-t13a-joint-smoke-control-03/STATUS.md)：16项纯测试、16支持文件身份和完整2441帧无损往返通过，修正固定业务日志外推及写盘前容量守卫。原256MiB/4Hz和安全阈值不变；新smoke/formal尚未启动，物理性能/16GB/M0仍未通过。核心交通—经济集成继续独立验证。

v0.5.117记录[原负载完整联合smoke](docs/evidence/m0-t13a-joint-smoke-control-02/STATUS.md)：外置SSD上611.85秒、完整720h业务与最新布局冷恢复完成；44帧采集不完整及115帧外部编译保留，日志准入拒绝，正式8h未启动。修复首轮socket路径启动错误，不修改冻结Rust、负载或预算；交通—经济Store真实集成仍在隔离分支验证，16GB/M0不宣称完成。

v0.5.116完成[真实28日经营场景主线集成](docs/evidence/core-market-scenarios-integration-116/STATUS.md)：七场景+参数覆盖、release构建/strict/fmt实测通过，工资/贷款/库存basis/现金可对账。家庭基础现金仍减少6160分，交通/公用替身、未建模税及真实退出不冒称完成；M0转完整联合smoke准备，16GB/8h未测。

v0.5.115完成[主线有限运行时集成](docs/evidence/core-runtime-integration-115/STATUS.md)：49项release测试、strict/fmt及完整建设/拆除/废物/关闭CLI实测通过，修正最终现金展示。固定2MiB诊断4捕获/2冷恢复通过但normal仍超8,415,170B，未设main默认；精确回收本轮54文件约6.285GiB，190原件保留。真实交通经营联调、16GB/8h仍未完成。

v0.5.114集成[已验证交通市场resolver](docs/evidence/core-market-access-integration-114/STATUS.md)：最终源码16项测试、strict Clippy及6个pureplan CLI实测通过；保留首轮授权分类失败。动态报价复用真实OD分配，尚未接Engine落账，不签三线联合；M0原容量归因进一步核实，未找到可安全删减的backing。

v0.5.113集成[有限企业退出计划器与M0预算归因](docs/evidence/core-business-exit-integration-113/STATUS.md)：13项实际测试、strict Clippy通过；尚未执行Engine退出结算。保留2MiB控制器失败、两代冷恢复和54文件精确回收；原normal满额及native超额已从raw重算，候选第二轮仅准备完成。交通与建设继续独立推进，真实16GB/8h未测。

v0.5.112完成[原负载控制器重试](docs/evidence/m0-t13a-extents-joint-retry-02/STATUS.md)：20项辅助测试、4次实际捕获与两代完整冷恢复通过，但90完整帧peak3,337,031,256B仍超原normal8,431,602B，非normal压力/外部Cargo下不签性能。保留retry01实际启动失败；本轮54份已无依赖载荷/归档重复组件终态回收约6.285GiB，190原件保留，work约25.44GiB。2MiB候选与建设拆除/经营退出继续推进，16GB/8h未测。

v0.5.111在外置work/target上完成[28日持久化与经济反例集成验证](docs/evidence/core-store28-integration-111/STATUS.md)：主分支core107项、Store3项、严格检查与fmt通过；每7日冷开、收据连续及工资/产出余数对齐。高购买力恢复fixture与原经济基准分开，真实交通/公用联调及长期平衡未证明；失败来源保留。迁移完成不代表M0规模性能通过。

v0.5.110完成[work完整外置迁移](docs/evidence/work-migration-20261022/STATUS.md)：全部普通文件逐SHA校验，190恢复/故障原件保留，三组真实新进程冷恢复通过后才删除内置盘副本；原路径符号链接保持，验收exe及旧新inode衔接另核验。后续性能明确使用外置HP P900 USB/APFS SSD，预检按work所在卷检查空间；迁移不等于M0性能通过。

v0.5.109完成[有限核心集成验证](docs/evidence/core-m0-integration-109/STATUS.md)：221项测试、严格检查和7项CLI通过，交通市场提案与实际28日Engine结算分开报告，家庭仍依靠初始现金，未宣称长期经济平衡或完整联合。完成[历史重复组件回收](docs/evidence/disk-reclamation-focused-20261022/STATUS.md)约382.6MiB及[共享target外置迁移](docs/evidence/target-migration-20261022/STATUS.md)，原路径链接保留、106项迁移回归两次通过，第二次无编译。M0诊断控制器失败及物理超额原文保留；8h/真实16GB仍未测。

v0.5.108完成[有限核心模型合并功能验证](docs/evidence/core-m0-integration-108/STATUS.md)：199项测试、严格Clippy、工程与六个经济CLI通过；合入实际施工/恢复兼容、未落账动态经济planner及价格修复、R07 extent和八小时角色接线。生产28日、拆除废物、真实交通联调及M0正式性能仍待完成，真实16GB/8h未测。新R07候选exe和源码已固定，原负载阈值与失败证据保留；本次未启动压力。

v0.5.107合入[有限群体交通模型](docs/evidence/core-transport-validation/STATUS.md)：13项测试、严格Clippy与正常/断路/拥堵/无路四个固定CLI实测通过。显式入口授权、城市边界来源、容量流量、最终成本及失效指纹可核对；结构预算尚非全城/16GB实测。工程原子层和经济计划器仍在隔离线实测/接入，不签三线联合或完整0.1；M0仍按v106来源记录未通过门。

v0.5.106完成[独立映射pool与地址诊断功能验证](docs/evidence/m0-t13a-pool-mapped-proof/STATUS.md)：同源全仓、严格检查、7项实际借还/故障测试、原L1两代诊断及新旧冷恢复通过；整组物理仍超原normal，不签性能，8h/真实16GB仍未测。新诊断54份无依赖合成载荷/重复组件终态回收6.285GiB，另于v105历史清理统计；190恢复/故障原件保留，work剩25.337GiB。0.1三线已隔离编码，独立验证与联合分别报告。

v0.5.105完成[用户指定集中清理](docs/evidence/disk-reclamation-v105/STATUS.md)：154份无依赖压力/已归档重复组件精确退役，分配空间减少7.032GiB；work剩25.322GiB，共享target2.080GiB复用。190完整恢复/故障原件及WIP保留，df变化独立报告。当前性能门不变；新增0.1交通、工程、经济并行线采用隔离分支，未实测不算完成。

v0.5.104完成[R07独立背板与恢复功能](docs/evidence/m0-t13a-r07-report-error/STATUS.md)：同源全仓/严格检查、两代真实快照、TTL实际补回、新PID与旧格式冷恢复、错误CLI拒绝通过；原数量和预算不变，整组物理仍超限，不签性能。终态8份旧合成载荷退役11.827GiB，190份恢复/故障原件保留；正式三轮/四速/8h/真实16GB仍未完成。

v0.5.103完成[历史验证产物集中回收](docs/evidence/disk-reclamation-v103/STATUS.md)：精确退役7份已无依赖的合成prefix/已归档重复组件，减少588.648MiB分配空间；work剩25.272GiB，190份完整恢复/故障原件及模型WIP不变。共享target保留复用，不改变性能验收状态。

v0.5.102完成[原生整组观测器接入](docs/evidence/m0-t13a-native-observer-errno/STATUS.md)：同源全仓/严格检查、实际失主/锁竞争及60秒周期验证完成；原规模L1短测85/88帧完整，物理仍超限约9.31MiB，置换0，不算性能通过。终态回收约11.88GiB，本轮退役快照不可直接恢复；正式三轮/四速/8h/真实16GB仍未通过。

v0.5.101完成[历史验证产物集中回收](docs/evidence/disk-reclamation-v101/STATUS.md)：精确退役已无依赖的旧暖场压力、已归档重复组件和旧静态库缓存；恢复原件及模型WIP保留。仅磁盘治理，不改变性能验收状态。

v0.5.100完成[原生单PID采样后端](docs/evidence/m0-t13a-native-reader/STATUS.md)：独立SDK布局/240活体夹取及4项身份/退出请求、五类实际协议拒绝通过，同源全workspace/严格检查通过。候选进程footprint约3.10MB，不代表完整observer；正式观测器尚未接入，置换额度0，原normal失败/三轮/四速/8h/真实16GB仍未通过。清理28份旧库缓存约104.69MiB，复用共享target。

v0.5.99完成[独立运行时VM地址诊断](docs/evidence/m0-t13a-runtime-vm/STATUS.md)：原满级L1单次smoke、10份vmmap与实际完整冷恢复通过；arena地址全部落实际MALLOC_LARGE。诊断工具有扰动，组件可置换额度仍为0，不改变上轮normal失败。原3+12秒smoke不是600秒；未取得末尾快照组，三轮/四速/8h/真实16GB仍未通过。本轮两份无后续恢复依赖合成载荷按精确清单退役。

v0.5.98完成[原300+1800秒L1单轮实测](docs/evidence/m0-t13a-typed-workspace/FORMAL-STATUS.md)：2101扫描、8代恢复/读回、当前两代完整冷恢复通过；两类L1页计算观察分位满足原阈值。完整帧footprint仍超normal约18.49MiB，物理门失败，三轮/四速/8h/真实16GB未通过。旧基线4份合成载荷退役5.920GiB，work剩24.806GiB；下一步独立VM归因，不能按超额直接削压力。

v0.5.97完成[常驻解码工作区与原快照格式兼容的实现验证](docs/evidence/m0-t13a-typed-workspace/STATUS.md)：同源全仓回归、严格检查及三份完整旧原件冷恢复通过；advisor认可冻结规范输入的4MiB外部算术准入。64MiB typed＋64MiB并发COW保持原128MiB工作预留；原参数多代正式快照、物理预算、8h与真实16GB尚未通过。

v0.5.96更正进程记录表述：曾短暂观察到1.95工具链Cargo，无本项目work/target文件占用，未停止该进程。v0.5.95完成[集中回收后的当前复查](docs/evidence/disk-reclamation-current-recheck/STATUS.md)：work24.40GiB、共享target1.34GiB，本次无新增安全退役项、释放0B；完整恢复及故障原件保留，历史回收不重复计入。模型WIP及性能验收状态不变。

v0.5.94完成[旧故障载荷交接与回收](docs/evidence/disk-reclamation-fault-handoff/STATUS.md)：最新原件实际冷恢复及逐字节等价核验后退役9个旧合成文件，分配空间约13.70GiB，work剩24.42GiB。报告延后实验仍失败；常驻typed解码基础尚为WIP，正式窗口/8h/真实16GB未完成。

v0.5.93完成[实际COW页回收与重复快照失败定位](docs/evidence/m0-t13a-snapshot-backing/STATUS.md)：同源全仓通过，原正式L1约310.29秒在第二次快照的第64页解码还款申请失败；保留原负载/现场，下一步优先消除报告跨还款逃逸。正式三轮、8h与真实16GB未完成。

v0.5.92完成[历史产物集中盘点](docs/evidence/disk-reclamation-latest-request/STATUS.md)：work约25.49GiB，共享target约1.34GiB；本轮未确认新的安全退役项，删除0B。当前原L1 formal第二次快照实际分配失败，完整基线与故障原件保留，继续修复后再验收。

v0.5.91完成[固定L1区域与真实因果校准](docs/evidence/m0-t13a-causal-clock/STATUS.md)：同源全仓、原L1完整短测与新旧冷恢复通过，取得28帧同相位实测；normal仍超约18.57MiB。短测27合成载荷已退役，下一步原300+1800秒L1，正式三轮／8h／真实16GB仍未完成。

v0.5.90完成[历史验证产物集中清理](docs/evidence/disk-reclamation-current-stage/STATUS.md)：精确退役102份终态合成载荷与旧重复组件，减少3.715GiB文件分配空间，work剩19.181GiB；完整恢复输入、失败现场及模型未提交工作保留。共享target复用，未新增模型验收结论。

v0.5.89完成[实际边界计数与原生观测](docs/evidence/m0-t13a-native-boundaries/STATUS.md)：冻结全仓回归、原L1短测及冷恢复成立；同相位物理驻留仍高于3.10GiB，下一步固定工作区复用实验。短测27份合成载荷已回收，未签正式稳态／8h或真实16GB。

v0.5.88完成[统一稳态控制器与实际正式L2首轮](docs/evidence/m0-t13a-steady-controller/STATUS.md)：冻结全仓回归和原300＋1800秒完整30日／独立Engine对照成立；稀有样本、完整资源与同边界泄漏未签，三轮／四速／8h及真实16GB尚未完成。

v0.5.87完成[集中回收与恢复基线交接](docs/evidence/disk-reclamation-stage-followup/STATUS.md)：精确删除6份退役合成压力／预留文件，减少8.880GiB文件分配空间；work剩17.657GiB，唯一target继续复用。新formal两代、旧格式兼容原件、故障与业务证据完整保留；模型未提交工作不混入发布。

v0.5.86完成[历史重复组件回收](docs/evidence/disk-reclamation-v70-components/STATUS.md)，保留完整恢复输入、业务与验收证据；共享target继续复用。稳态控制器开发仍在进行，本次不发布未验证代码。

v0.5.85完成[整角色Rust区域与联合短诊断](docs/evidence/m0-t13a-role-arena/STATUS.md)：原full30日、真实64MiB池授权、完整旧存档／近16MiB边界及606秒L1＋L2联合恢复通过；正式整体内存、分位、稳态、四速和8小时仍未签，真实16GB未测。下一步是[统一正式测量控制器](docs/evidence/m0-t13a-role-arena/NEXT.md)。

v0.5.84完成[历史产物集中回收](docs/evidence/disk-reclamation-2026-10-22/STATUS.md)：退役529份旧重复合成组件，扣除完整无损归档后净减少1.604GiB文件分配空间；完整恢复输入和Rust未提交工作保留，性能验收状态不变。

v0.5.83完成[固定词法容量与完整业务兼容](docs/evidence/m0-t13a-fixed-lexical/STATUS.md)：legacy／v2收据正文共用1MiB固定扫描栈；冻结同源全仓、近16MiB实际Store新旧reader及原full30日真实业务通过，解析压力单列。整Role授权、normal64/common8及原联合性能／8h仍未通过；临时压力副本已回收96.85MiB分配空间，真实16GB保持not_run。

v0.5.82记录[历史验证产物集中复查](docs/evidence/disk-reclamation-current-request/STATUS.md)：work约15.013GiB；现存完整恢复输入及证据仍有依赖，本次删除0B。当前未提交Rust实现不随本次文档发布。

v0.5.81完成[Meta／legacy保留工作区接入与兼容回归](docs/evidence/m0-t13a-metadata-workspace/STATUS.md)：原文件／Engine／收据／控制缓存预建并用于实际Store／Sim，修正旧有效大收据读取边界；同源全仓、旧完整world、新旧reader和原full30日真实业务通过。逃逸解析scratch、完整Role授权及normal64/common8、正式性能／8h仍未通过，真实16GB为not_run。

v0.5.78完成[历史验证重复组件回收](docs/evidence/disk-reclamation-duplicate-components/STATUS.md)：扣除无损证据归档后净减少约2.64GiB文件分配空间；完整恢复输入保留，未完成的模型工作不计为通过。

v0.5.80完成[历史产物集中复查](docs/evidence/disk-reclamation-final-recheck/STATUS.md)：work实测14.907GiB，现存三份完整压力恢复输入及业务／故障证据继续保留；本次新增删除0B，未重复计历史回收，未运行模型验收。

v0.5.79完成[模型与空间预建工作区构造接入基础](docs/evidence/m0-t13a-role-workspace/STATUS.md)：双root及准备区、候选、codec、原257×257灰度源和Large输出背板、固定几何背板接入实际创建／冷开；同源全仓、旧完整存档与原full30日业务已实测。Meta／legacy暂存、整角色broker、normal64/common8、正式性能与8h仍未验收，真实16GB保持not_run。上段回收记录属于v0.5.78，不重复计入本次成果。

**软件结构先读[总体架构](docs/SOFTWARE_ARCHITECTURE.md)：当前代码、主体与事件、单writer、多进程目标、数据文件和预算。**

**从[完整产品蓝图](docs/PRODUCT_BLUEPRINT.md)开始。** 总体覆盖市长／City Team、市民生活、企业经营和城市运行，实施分版本；原23类首期和90类目录不作为总体完成证明。

- [四视角覆盖与61条需求追踪](docs/product-coverage-and-requirements.md)：全部能力的完成条件／证据／主动边界。
- [属性驱动的概率机制通则](docs/regional-probabilistic-decisions.md)：跨系统识别统计倾向，组合区域／经济／气候／居民／企业等相关属性，统一概率语义、抽样及恢复契约；仅设计，尚未实现／调参／性能验证。
- [玩家、空间、建设与治理](docs/product-governance-space-and-player.md)：精确选择、文字／语音、事项、协商补偿、议会司法与财政。
- [市民、企业与复合空间](docs/product-citizens-enterprises-and-spaces.md)：生活就业、住房共享、公司持股、复合建筑／商租摊位和访客经济。
- [全模式交通](docs/product-transport-and-logistics.md)：公交停车、电车／地铁／客货铁路、轮渡港口、三档机场与航空公司。
- [树种、地理适生与空气作用](docs/tree-species-climate-and-air.md)：经纬度／气候／场地筛选，树种差异与有界环境作用；设计已补，运行未接。
- [能源公用、环境与安全](docs/product-utilities-environment-and-safety.md)：燃气／供热、各种电源、供排水／固废、通信、天气绿化与公服事故。
- [设施／设备构造](docs/product-facility-and-equipment-profiles.md)：可计算的成本、工程、资源、维护、退出及自定义约束。
- [运行、16GB增量预算与版本路线](docs/product-runtime-budget-and-versions.md)：从M0到1.0的依赖和验收，全部机制同世界准入。
- [城市快照、全存档市场与交互增量](docs/regional-snapshots-and-interaction-ledger.md)：地方发展与区域履约分钟、近远迁移、物理网络约束；有界Rust实验不代表全部市场已实现。
- [T07空间、地图与wgpu缓冲布局](docs/m0-spatial-model-and-gpu-layout.md)：原生灰度导入、厘米几何、曲线／门／接缝反例，CPU缓冲打包；无UI／GPU后端通过结论。
- [空间资产与存档根](docs/m0-spatial-storage.md)：不可变地形／几何与资金／区域状态共同恢复，损坏依赖拒载；不等于运行时建设。
- [T08有限经济闭环](docs/m0-economy-runtime.md)：劳动、采购交付、消费、成本、工资税债及退出；P-E0／P-E1实跑，长期平衡、16GB仍未验。
- [T08启动互锁P-BOOT](docs/m0-boot-runtime.md)：真实有限电池／水箱、泵与机组、同池容量、故障残余物与持久恢复；有限实验不代表真实公用网或煤机生命周期通过。
- [T09区域实际履约](docs/m0-regional-fulfilment.md)：三城货物／电水、共池容量、实际费用与损耗、关图／断链／恢复；完整P-CAP及真实车辆／公用物理仍未验。
- [检查点与重试档案](docs/m0-checkpoint-and-retry.md)：完整运行状态、分段WAL、冷凭证与损坏拒载；保留既有责任，不靠清历史绕过容量。
- [T05-B全源与持久日历](docs/m0-timeline-runtime.md)：已实现profile的真实到期、工资账期、P0—P5与有限续做票；逐页恢复实测，完整产品日历与16GB仍未验。
- [T13-A模型规模与性能](docs/m0-model-scale-performance.md)：已冻结L0真实包／L1完整布局压力／L2引擎上限、三轮稳态及实际8小时门；已完成CPU参考、完整L1布局短诊断及[三档真实Sim／worker／v2存档30日](docs/evidence/m0-t13a-tiered/STATUS.md)及[真实计算区间计时与完整诊断](docs/evidence/m0-t13a-cpu-regions/STATUS.md)及[稀有类别真实执行样本补齐](docs/evidence/m0-t13a-rare/STATUS.md)；另有[完整L2无等待30日速度实验](docs/evidence/m0-t13a-speeds/STATUS.md)与[实际session节奏修复](docs/evidence/m0-t13a-session-pacing/STATUS.md)，[P-E0实际session无等待30日](docs/evidence/m0-t13a-pe0-speeds/STATUS.md)已核验；[修复后P-E0实际60档](docs/evidence/m0-t13a-queued-pacing/STATUS.md)完整30日已核验通过（旧失败留证）；另有[固定L1恢复约束与旧组件兼容](docs/evidence/m0-t13a-bounded-restore/STATUS.md)及[Sim固定编码／候选owner与完整30日功能核验](docs/evidence/m0-t13a-sim-workspace/STATUS.md)（缺监督者heap记录、正式分位失败均明确保留）；[原完整L1＋L2短联合重测](docs/evidence/m0-t13a-joint-recheck/STATUS.md)已完成；[接纳流式摘要及同负载重测](docs/evidence/m0-t13a-stream-digest/STATUS.md)已有新证据；[worker固定空气模型背板](docs/evidence/m0-t13a-air-workspace/STATUS.md)将typed1MiB与IO4MiB合并为5MiB实际借还，性能分位失败与其余预算缺口仍保留；正式分位／full四档、完整联合预算、联合稳态及8小时门仍未通过。
- [可恢复后台计算验收](docs/background-compute-acceptance.md)：冻结W0—W4、独立worker／Sim唯一写入／联合故障实验与全进程测量；W1—W4有限闭环已实测；[1800秒联合运行与限制](docs/background-joint-runtime.md)及[逐项完成审计](docs/background-compute-completion-audit.md)有原始工件和最终独立结论，16GB及完整M0仍未通过。
- [W2任务与同根接纳](docs/background-store-runtime.md)：实际货运输入、P4屏障、Sim资产／小WAL、代际及独立预留；独立Sim见W3，有限长跑及进程组观察见W4。
- [W3独立Sim与恢复](docs/background-sim-runtime.md)：原子换代、固定接纳原票、持久worker身份及OS缓存栅栏、6MiB磁盘charge和真实进程故障；[W4联合运行](docs/background-joint-runtime.md)已完成1800.435秒及独立核验／审查，采样限制明确。
- [W1独立空气worker](docs/background-worker-runtime.md)：连续窗口、同进程复用、有界文件协议与阶段握手杀进程恢复；未接世界／Sim持久任务。
- [T12-A空气CPU参考](docs/m0-air-cpu-reference.md)：单通道EI、源／tile分页、数值位恢复及独立f64复算；此为数值基线，有限城市接纳见W2；GPU及worker池仍未实现。
- [进程、worker与GPU渲染架构](docs/process-worker-and-render-architecture.md)：App监督／Sim唯一writer、可重算任务与渲染部署、进程组16GB预算；worker纯计算见W1，独立Sim见W3，渲染尚未实现。
- [T10有限CLI会话](docs/m0-cli-session.md)：可信宿主身份、暂停／单步／实际倍率、冷回执与快照分页；T05-B分页用step，UI及16GB仍未验。
- [M0本轮连续交付：验收方法与结果](docs/m0-continuity-acceptance.md)
- [M0：事实收件、有限背压、责任退出与查询](docs/m0-fact-inbox-and-emergency-exit.md)
- [M0具体实施计划](docs/m0-implementation-plan.md)：任务依赖、跨方向风险、经济原型与真实16GB基础验收，计划与实际状态分别登记。
- [M0实现状态与证据](docs/m0-implementation-status.md)：按当前模型范围记录每阶段实测、未实现／未测项及运行方法。
- [两种独立检查与修正](docs/product-design-final-checks.md)：漏整块功能与跨系统协同分别检查，原型验证另列。

总体方案D_full已交付；当前按M0实施模型。完整场景装配、真实Agent／语音、经济平衡、性能和可玩性仍需原型；16GB预算不是实测通过记录。

[原首期实施切片](docs/FIRST_RELEASE_BASELINE.md)／[历史资料](DOCUMENT_HISTORY.md)保留为来源，不覆盖新总体目标。仓库：[TekesApps/MyCity](https://github.com/TekesApps/MyCity)，公开、[MIT License](LICENSE)。每次文档提交递增附注tag并推送，见[协作规范](AGENTS.md)。

最新有限阶段：[规范整数编码、ARM SHA分派与原负载联合实测](docs/evidence/m0-t13a-hotpath/STATUS.md)。最终同源全仓Release测试通过；原30日联合负载的完整业务、旧hash与恢复核验一致，执行和报价观察分位进入20/40ms门。正式性能协议、完整内存预算、8h和真实16GB仍未通过；不宣称M0完成。

最新实测：[730实例的真实full30日与驱动费用](docs/evidence/m0-t13a-runtime-ledger/STATUS.md)。724个worker、3011原票和真实Store冷开完整对照通过；业务279.96秒，驱动整堆峰加尾部600,825B。共同诊断／64MiB／L1联合／四档／稳态／8h及真实16GB仍未通过。

本阶段v0.5.47：[64MiB实体余额与724次真实worker借还](docs/evidence/m0-t13a-normal-pool/STATUS.md)。同源303项回归通过，修复accepted socket分片误拒；无L1的原full30日、完整CPU／原票／候选hash和731正常实例核验通过。原联合压力失败及首个功能失败保留；共同内存预算、四档、正式稳态、8h及真实16GB仍未签收。

本阶段v0.5.48：[CPU broker实际持有worker及有限恢复](docs/evidence/m0-t13a-broker/STATUS.md)。312项同源回归、原full30日／724次真实借还／完整Engine核验通过；旧启动超时与测试失败保留。实际8h及联合性能仍未签，下一步实现原长时控制器并实跑。

[Validation artifact retention](docs/validation-artifact-retention.md) / [2026-10-21 cleanup](docs/evidence/disk-reclamation-2026-10-21/STATUS.md)

v0.5.66: [Fixed end-of-run helper workspace and original full-load joint diagnostic](docs/evidence/m0-t13a-helper-compare/STATUS.md). Full model ownership, formal performance, 8h and real16GB gates remain open.

v0.5.67: [Typed full-model capacity plan](docs/evidence/m0-t13a-model-capacity/STATUS.md). The proposed backing fits64MiB; current runtime ownership and performance gates remain unaccepted.

[v0.5.68历史验证产物后续回收](docs/evidence/disk-reclamation-2026-10-21-followup/STATUS.md)：释放11.34GiB分配空间，保留完整恢复依赖、真实业务与当前模型WIP；不代表新模型／性能通过。

[v0.5.69固定核心表与事务准备租约](docs/evidence/m0-t13a-model-workspace/STATUS.md)：同源全仓测试、展开恢复与完整旧存档字节兼容已实测；完整模型预算及性能验收继续推进。

[v0.5.70后台模型常驻背板及原完整联合诊断](docs/evidence/m0-t13a-background-resident/STATUS.md)：完整原负载实测605.69秒及旧存档字节兼容；normal64／正式性能／8h仍待完成。

[v0.5.71一般模型有界解码](docs/evidence/m0-t13a-general-decode/STATUS.md)：可信容量直读、旧句柄代次及完整旧存档字节兼容实测；继续Store流式与整图池接入，未新增性能／8h签收。

[v0.5.72历史验证产物回收](docs/evidence/disk-reclamation-2026-10-21-reserves/STATUS.md)：精确退役旧布局预留页，保护完整恢复点与验收证据；不新增模型或性能通过声明。

[v0.5.73 Store流式检查点与回执](docs/evidence/m0-t13a-store-stream/STATUS.md)：完整输入与两遍输出绑定、逐条回执核验、检查点展开保护；旧完整存档与实际新检查点往返通过，整图池及正式性能／8h仍待验收。

- [WAL有界规范编码与真实存档兼容](docs/evidence/m0-t13a-wal-canonical/STATUS.md)：同源回归与实际重开通过，整图池和正式性能仍未签。
- [历史产物清理及当前保留审计](docs/evidence/disk-reclamation-current-audit/STATUS.md)：逐轮清单、完整恢复输入、分配空间与df分别记账。

- [借用业务／状态比较及完整旧字节oracle](docs/evidence/m0-t13a-comparison-codec/STATUS.md)：同源回归和完整旧world验证通过，实际池与正式性能仍未签。

- [角色恢复槽及实际Store／Reference／Compare接入](docs/evidence/m0-t13a-role-slots/STATUS.md)：388条同源harness记录及原full30日业务装配通过，实际池、正式性能和8h仍未签。

- [固定空气候选及实际导入／重放](docs/evidence/m0-t13a-air-candidate-slots/STATUS.md)：392条同源harness记录及原full业务装配通过，角色包授权与正式性能仍未签。
