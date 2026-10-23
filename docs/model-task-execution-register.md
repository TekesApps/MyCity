# 模型—任务—执行方清单与协调接口

设计修订 EXEC-1，2026-10-23。本轮仅文档，未修改游戏代码、未编译或运行测试，也未向主会话发消息。自行只读核对主仓库 main `93be71bb4a0fcd2f531af61f2d902b2a984e37ee`（v0.5.144）的相关已提交源码；文档在独立设计分支编写，不把主线新代码合入本分支。源码定位与核对范围见[来源记录](evidence/city-simulation-budget-design/execution-register-sources.md)。这次代码阅读不是重新执行各模型或签收性能。

本清单是[软件总体架构](SOFTWARE_ARCHITECTURE.md)的执行登记补充，策略、依赖和误差继承[BUDGET-3](model-granularity-and-convergence.md#budget-3模型策略与依赖兼容契约)，共同资源和服务目标继承[预算设计](city-scale-and-adaptive-budget.md#0-首要产品约束全游戏预算推进目标与业务承载)。不另建模型规则、世界时钟、worker调度器或新资源总额。

## 1. 模型、任务与进程分开登记

14类是设计中的逻辑模型族，不是14个已有完整求解器，也不是14个独立进程。一个模型可生成多个区域／主体群任务；相容任务可批执行，一个worker可先后执行不同模型任务。只有实现了对应输入／结果适配和隔离协议，才能把它迁至worker。短小精确业务及权威接纳留在Sim，不能仅为“架构整齐”全部跨进程。

| 角色 | 当前代码事实 | 本次接口职责与边界 |
| --- | --- | --- |
| App／可信宿主 supervisor | 当前headless宿主及有限监督路径；产品App/UI未实现 | 监督进程／重连／退出、请求目标倍率和显示阻断；不自行决定已发生事实、推进世界或确认phase |
| Sim | `sim_host.rs`持有Store并管理空气任务，Timeline调度已接入的到期源 | 任务资格／依赖、策略组合、计算额度和生效边界的最终协调；原writer验证／持久提交。协调职责待通用实现，不把Sim降成纯转发器 |
| CPU worker | `compute_worker.rs`目前是纯空气worker，`mycity-air-worker-v1`，没有Store写权 | 接固定输入算候选并回报进度／成本／细化需求；不能自改档、取消义务、扩资源或提交业务 |
| normal-pool broker | 当前进程角色／worker租约与真实Child回收机制，不是模型策略协调器 | 执行Sim准入后的有界进程／内存租约；真实wait／资源退役前不归还，不能以逻辑任务结束替代实际回收 |
| GPU Owner／Broker | 当前城市GPU计算路径未实现 | 未来承接有资格GPU候选，设备／队列／缓冲归统一owner，完成候选仍由Sim接纳；不按每模型创建Device |
| 测试supervisor／layout／oracle | `steady/supervisor.rs`等用于冻结联合负载与观察 | 不计作城市模型worker或生产业务吞吐；产品辅助进程和测试工具开销按既有规则分开计费 |

当前独立Sim入口和进程内CLI→Store→Engine入口共存。下文“Sim内”表示执行同一权威模型路径，不能据此声称所有CLI都已改为独立Sim协议。当前只接出一个空气计算worker；A55首轮最多2个普通CPU计算任务是未来共同预算下的候选上界，本轮不扩大。用户举例“同时五六个worker”不作为现行配置；增加并发须另行核算进程基础成本、工作页、尾延迟及版本化预算，不能由14类模型数量推导。

## 2. 已有实现的实际调度边界

- `timeline_driver.rs::boundary`取target、`last_complete+3600`及当前已声明的claim／payroll／旧Economy／Boot／Region／Exchange到期源的最小值；不是每小时运行全部14类模型。`schedule.rs`的通用静态due结构不能替代这个已接Store的动态来源调度。
- P0按实际活动区间应计并捕获空气输入；P2按既定lane处理实际到期与交付，P3处理人口到达；必要空气任务未完成时P4阻塞；P5排空才发布完整边界。周期不等于每帧／每个现实秒；墙钟预算窗只决定资源提案。
- `TownSettle`／`TownTransportSettle`在完整、无待续target且整游戏日边界显式调用，校验`settled_day+1`；交通proof与经营日／generation绑定。它们不是已经被Timeline自动发现的Town每日任务。旧Economy的固定30日profile则已进入Timeline；二者不能混作同一执行器或重复劳动／生产。
- `Construction::EngineCommand`、股权出资／分红、退出等由显式合法动作进入Engine/Store。具有时间锚不代表已经注册自动唤醒来源；未来要承担到期责任，须先接来源发现／预留／持久游标，不能只给它配置一个“worker周期”。

## 3. 14类模型的任务、激活、周期、依赖与执行位置

以下任务名是**设计登记ID**，不是新增可调用命令。状态栏说明有限代码或缺口；未实现部分只定义未来接口，不填成运行功能。各模型升降档按BUDGET-3本地策略目录，当前没有分级能力的入口先登记唯一现有可信策略，不生成虚构精细档。

| 模型族／任务拆分 | 当前代码／激活事实 | 更新周期或最早触发（游戏时间） | 必要依赖与输出 | 当前执行方 → 后续候选 |
| --- | --- | --- | --- | --- |
| 身份／空间／产权 `geo.check` | `assembly`、`city-map`、Engine合法性检查有实现；完整运行时空间／产权模型未闭合 | 创建／导入／显式修改／授权操作时检查；无依据不全图轮询 | typed ID/generation、厘米几何、许可／规则版本→合法性及精确接入；几何相交不自动授权 | Sim内精查／map适配；纯批筛选以后可CPU worker，权利提交仍Sim |
| 人口／家庭 `population.transfer`、`population.behavior` | `region`人口迁移／到达有限实现；通用户／出生／内生迁居求解未实现 | 已接受迁移的arrives_at由Timeline P3；未来行为按变更／资格评估窗，不默认逐人每秒 | 真实人数／源城、通路／容量和住所权益→实际转移；预测行为只产候选 | 当前Sim内；未来批量倾向CPU／GPU候选，不给每户进程 |
| 教育／劳动 `labor.accrue`、`labor.bill`、`education.evaluate` | `timeline::Payroll`有限排班、应计／账期和显式换岗；完整教育／求职尚缺 | P0区间应计、P2 next_due；现有Payroll显式09–17工作及日／周／公历月账期；教育周期待有效课程来源 | 活动城、合同／资格、实际工时和到岗来源→劳动／课时及应付；旧债不因停工消失 | 精确应计／付款Sim内；未来岗位／课程匹配纯候选可worker |
| 交通 `traffic.assign` | `transport::Opening::assign`有限纯计算，`traffic_adapter`有限Scene步行转换；Store固定日proof接经营 | 当前显式分配／settle_town_market调用，服务日／输入版本绑定；通用封路触发重算和缓存失效仍待接 | 有向图、门、真实需求／模式、容量／时窗→Assignment及未服务量／proof；已发生交通不能由评分代替 | 当前调用方CPU／Store流程；以后冻结图OD→CPU worker，GPU仅批评分候选 |
| 货运／仓储 `freight.fulfill` | `exchange`的Dispatch／Cancel／SetLink及有限在途／交付，库存lot及费用走Engine | 派单／断链显式动作，arrives_at和报价expires_at由Timeline；暂停在途按原剩余量处理 | 有限订单、lot／路线／容量、interaction钟→实际交付／损耗／费用或未完成 | 权威履约Sim内；路线批计算以后可worker，不能独立worker入库付款 |
| 公用网 `utility.fulfill` | `boot`有限水电作业，`town_utility::BootSupplyBinding`仅单位／费率映射，非独立供给solver | Start／Fault等动作；Boot运行job.due由Timeline P2；有限Boot作业SECONDS=60不是全城更新周期 | 真实库存／作业／共同容量／转换、计量条款→实供与成本；全资源公用网未完成 | 当前Sim内；未来子网纯分配可worker／GPU候选，实际投入和meter提交仍Sim |
| 市场／企业 `market.settle`、`market.predict` | `economy`旧固定profile；`town_economy`纯planner及`town_runtime`/Engine实际日结有限接入 | 旧Economy next_due已在Timeline；Town日结显式调用且不能跳过settled_day，非自动周期worker | 已赚劳动／实际可用电、库存／成本、交通proof／订单及账户→实际产销／利润／税或候选 | planner和结算当前CPU／Sim路径；未来纯需求／匹配候选可worker，不重写账本 |
| 钱包／税／债／股份 `finance.apply`、`equity.apply` | Engine/Store、Claims／Payroll／税、v144有限实际出资／利润分红与退出接入 | 真实交易／到期触发；分红／出资为显式授权动作，没有后台“每秒理财worker” | 已发生劳动／交付／成本、账户保护／税基／股权来源→逐主体精确原票／钱债；预测不入账 | Sim唯一权威执行，无降低已发生财务精度档；纯报表以后可worker |
| 公服／危害 `service.assign` | 完整通用服务队／床位／灾害执行器未实现；设计不能当已运行模块 | 未来请求／资格变化／占满／释放／告警期限；无请求且无占用责任可无任务 | 真实服务资源、资格／路线／优先级／占用→接纳／实际服务／未解；期限需注册来源 | 待实现；纯分配候选可CPU worker，真实占用／完成由Sim提交 |
| 建设／维护／设备 `construction.work`、`maintenance.assess` | `construction/mod.rs`有限工程状态、Work／工料／公用投入及补偿，Engine显式动作；通用维护尚缺 | 当前审查／开工／Work／养护／恢复显式按GameTime校验，非自动Timeline源；未来班次／投入耗尽／养护与故障due需接入 | 权利／工料／工时／水电／施工路效及法定门→实际进度／sink／费用；不能按截止日期直接完工 | 当前Sim内；未来工程方案／风险纯评估可worker，进度和义务仍Sim |
| 环境／自然 `air.window`、`nature.update` | `air`/`background`有限空气策略及独立worker已接；噪声／全自然生态不等同已实现 | enabled空气在P0正dt捕获必需窗口，P4前完成；内部到下一60s锚及源start/end截断；零新源仍可能衰减／积分，不能直接停算 | 已提交场／积分、Exchange活动源窗／版本、数值策略→完整候选后Sim校验接纳；自然斑块周期待实现 | 当前空气CPU独立worker，Sim验证／Store提交；GPU为未实现候选，不改变数值切点 |
| 需求／满意度／迁居反馈 `feedback.evaluate` | 通用LS42／属性概率／生活反馈和自适应分组仍属设计；Town需求planner有限计算不是完整闭环 | 未来因果输入变更／评估窗／冷却／阈值触发；相容不变时可复用 | 服务／交通／经营／环境的覆盖和误差资格→有来源的倾向／候选，不能直接写迁居或消费事实 | 待实现；批量CPU／GPU候选，Sim沿原合法动作接纳 |
| 治理／法律／事项 `law.deadline` | 通用司法／选举／事项到期执行器未实现；当前已有有限工程gate／权限检查，P1不是完整法律系统 | 未来规则生效／公示申诉／听证及法定到期；无新程序也须处理旧责任 | 现行规则、权利、证据／通知、法定历法→有效决定／义务；不以近似建议覆盖法律事实 | 当前有限检查Sim内；未来建议评分可worker，法定时点／权威判断仍Sim |
| 跨城／事件／保存 `world.advance`、`world.persist` | Region双钟／有限履约、Timeline、Store单writer／续做／WAL／检查点／原票实现 | Advance请求驱动最早边界，暂停不按现实离线追赶；显式保存／恢复及已有持久周期按入口，不假定所有5min自动保存已实现 | 全世界来源、水位、phase/lane/cursor、冻结读集／恢复资产→完整提交边界／存档；闭城已接受责任仍计入 | Sim/Store；宿主监督真实退出；未来压缩／纯索引可worker，根发布不外包 |

“无需参与”只在本次无需新计算时成立：静态输入和缓存仍有效、没有到期源／未完成业务、稳定区间已证明可积分，或该未实现能力明确未启用。不得把“没注册唤醒”“worker未启动”当成已履行责任。输出已过期而消费者需要它时，必须激活任务／等待／unknown；不能延用不合格结果。

## 4. 激活与执行生命周期

共用一份任务登记，不按模型建立独立时钟：`Dormant → Eligible/Dirty → Admitted → Running → CandidateReady → Validated → Committed`；WaitingDependency、Superseded、Failed／Unresolved及CancelledAttempt是显式状态，不代表取消业务义务。这是待实现的通用接口语义，不替换已有空气／Timeline持久协议。

1. **发现**：领域owner给出source due、变更失效或消费者覆盖需求。按clock_kind和最早边界进入Sim；周期可不同，没有共同“每模型每tick一次”。显式动作若未来改为自动责任，先实现有限来源发现、去重／预留及恢复，再开放承诺。
2. **判重／复用**：按world/branch、模型／scope、来源／读集版本、策略、数值区间和已提交水位合并兼容请求。有效缓存可直接满足消费者；缓存命中仍检查资格。共享上游同任务只收费一次，不把不同权利或责任合并。
3. **准入**：Sim选取兼容策略组合，冻结范围／due／必要或可选，预留CPU/GPU时间、内存、结果校验和恢复额度；原必要边界续做预留不被新软任务抢走。策略只在完整边界、新续做票前改变；正在处理的原绑定依旧。
4. **派发**：短小精确操作在Sim内，适合外包的纯任务派至已实现的执行适配器。任务逻辑激活不要求新进程；worker可按lease复用或空闲等待，保留进程的基础／缓存内存仍计费。停用空闲进程需确认无任务、实际wait及资源退役，不影响仍保留的业务来源。
5. **回报／接纳**：进度和成本不赋予writer权限。候选完整、读集／generation和领域约束合格后由原Sim/Store接纳；未解不能直接变短缺。完成不代表所有消费者都自动新鲜，依赖按提交版本失效／刷新。
6. **休眠／恢复**：登记下次due或明确失效订阅，保留义务、原票／游标、随机和策略绑定；重开不能凭墙钟重新抽样或补跑闭城内生发展。运行中的必要任务失联仍由持久意图重算，不能因worker未激活漏掉。

活动task数、running task数、实际worker进程数、空闲worker数分别观察。并发由总预算及因果可并行性决定，不能以任务数量决定。当前无法安全并行的事务仍串行；外包纯候选也不能用完成先后改变付款／交付排序。

## 5. 策略和资源协调接口（待实现的最小扩展）

复用既有Ticket／任务租约／控制帧，以下是接口字段**语义分组**，不是新建一套IPC、每个字段一份表或已实现API。Sim内任务也使用同一逻辑契约，不强迫短小计算经过IPC。

| 接口／方向 | 必需语义 | 当前支持／缺口 |
| --- | --- | --- |
| 任务登记：领域owner→Sim | 模型／任务ID、scope、来源与clock_kind、激活原因／最早due、必要性、完成前依赖及输出覆盖、当前策略资格 | 当前Timeline适配器读取有限来源；通用登记及动态依赖未实现 |
| 任务票：Sim→执行方 | 已有world/branch/job/incarnation/generation/attempt、读集和输入hash、区间；扩展策略／兼容组合版本、必要输出资格与有界工作／结果预算 | 空气Ticket含身份／base_revision/field/time/hash，Request含固定strategy；尚无通用策略组合票 |
| 状态／成本报告：执行方→Sim | 完成范围、remaining-work估计及置信／未知、实际CPU服务与wall/等待、工作页／缓存、误差和阈值风险、报告sequence／job绑定 | 空气Event已有stage/cursor/candidate hash/bytes；Sim计时包含等待和IO，不能当纯CPU服务；一般成本／误差报告待实现 |
| 细化／让额建议：执行方→Sim | 需要或可选的策略变化、业务原因／证据／涉及scope、依赖要求、预计收益及转换增量／截止条件；信息不足显式unknown | 未来请求，不是worker自授权；无效／重复／过期建议拒收，不增加义务或资金 |
| 策略／预算授予：Sim→执行方／broker | 同一兼容组合及生效边界、允许executor、CPU核毫秒／GPU时间与工作页额度、有效窗／租约、取消代际和越界动作 | 现有broker内存／角色租约只承担其原语义；不能假装已有CPU/GPU时间配额和档位让额 |
| 结果／回执：执行方→Sim→消费者 | 完整覆盖、实际成本、误差资格／版本、候选及hash；原writer验证提交后发布回执／依赖版本 | 空气候选与Store一次接纳已有有限路径；通用结果和资源结算需扩展；超时／耗尽仍unresolved |
| 退休／回收：Sim↔broker／GPU owner | 终态／取消代际、实际Child wait或设备资源退休、仍固定页和恢复引用、可回收余额 | 复用当前实际回收边界；逻辑降档不能提前领取物理释放信用 |

缺少成本或误差依据时不编造预算或资格。报告与建议按既有64KiB控制帧和有界队列／目录限制传输，细项放有界摘要或已有只读工件，不为每模型扩大各一套队列。策略／依赖描述符进既有任务元数据包络，新增字段实际字节、索引和日志须准入；装不下先收窄支持，不暗增总额。

旧协议／存档必须显式版本化扩展：旧worker未声明策略／预算能力时只能走原有固定策略，不给它发送无法识别字段后静默回退；若当前必要任务要求其不支持的资格，则不派发／安全阻塞。新消息不能授予worker Caller/System、phase ack或Store写权。恢复保留生效策略、读集、任务游标与原operation身份，实际耗时样本不可恢复时标unknown，而不是重做历史决策。

请求“交通局部升档”后，Sim先展开相关空间／配送／到岗等依赖，选择完整兼容组合及成本，再从突发余量和可选预测等让额。必要任务最低投入和旧义务保护，不能要求每个worker都降一档；很多模型可能根本未激活，精确事务也没有可降精度。若无可行预算组合，按原设计降实际推进速度／限制新承诺／安全背压。

## 6. 与既有实施及验收衔接

不重写已有小模型，先为现有入口登记唯一当前策略／激活来源／完整覆盖，再实现共享协调适配器。只在纯计算热点和隔离收益有证据时迁移worker。通用模型周期、活跃进程池和多档策略不是本文已交付功能；实际准入先复用单空气worker与Sim内任务，按C0—C3逐步扩展，GPU路径仍须C4。

[校准计划](simulation-budget-calibration-plan.md)补充的执行登记验收检查：静态有效模型不产生多余任务；相异周期／相同到期正确唤醒；源／依赖变化使缓存失效；显式入口不能误报自动周期；空气零新源仍保留必要衰减／积分；任务可以排队而非每模型开进程；空闲进程与被固定资源正确计费；升档请求／让额／兼容组合／必要义务／未解语义在恢复和预算不足时保持。

本轮到此交付清单和接口契约，后续实现／运行仍先协调所有权、固定版本、共享target及测量窗口。未联系主会话、未改其工作区/WIP／冻结标准，未运行任何测试。
