# Agent 城市建设工具协议

版本：设计草案 v0.4，2026-10-18。以下 API 为 MyCity 待实现协议，尚未注册到任何 harness，也不是 TekesAgentKit 已有方法。

v0.4.24工具查询／草案校验须沿 [覆盖矩阵](object-definition-coverage.md) 区分结构、契约、参数、表示、机制及实现状态，读取 [字段字典](capability-field-dictionary.md) 的类型／来源和 [物件契约](common-object-contracts.md) 的前后置。按 [粒度收敛](model-granularity-and-convergence.md) 返回实际模型、近似／覆盖／年龄、缺项和预算原因；精确工具不意味着逐设备／逐车仿真，未配置不按0、缺机制不因agent填字符串启用。

配套：[运行闭环](runtime-loop.md)、[性能预算](performance-budget.md)、[空间与精度契约](spatial-contract.md)、[市长与 City Team](city-team.md)、[评审总览](preparation-review.md)。

## 1. 目标与职责

v0.4.23 [完整案例](worked-examples-housing-retail-and-stalls.md) 展示工具应能追溯的分项：面积按site／楼面／使用权分开，设备额定与工况／实际电量分开，工程投入／维护／租金／押金／预付分开，独立情景与共同需求／实际收支分开。月末现金足够不代表月内可花尚未到账工资；摊位清场有钱也不代表已有接收点，提交须验证真实时间／容量与资金保护。

v0.4.22预演及执行按 [经济运行契约](physical-economy-runtime-contract.md)：输出独立情景／联合预测标识、共同需求／资金来源与覆盖，不能把各店独立满销相加。提交再核权利、现金／库存／接入；故障超占用显示actual／overcommitted，退款／版本变更／清场依原来源结清。新增128宿主／512功能／2048关联及16版本满额先拒新，不阻止旧事实更新；条数、字节与原合同／meter／债权共同准入，不因预留启用B类机制。

工具必须让 agent 知道：能在哪里建设、能建什么、精确占用多大范围、会改动哪些对象、需要多少资源、为何不合法，以及提交后实际发生了什么。

采用分层能力：市长表达目标，City Team 编排与解释，几何求解器生成可执行草案，规则引擎验证，模拟核心提交。agent 能查看和引用精确坐标，但不必逐点描绘道路或逐栋猜位置。

计划内的道路、地块、建筑、连接和地形调整必须展开成可检查的最终动作；“自动解决一切”的黑盒工具不能直接写世界状态。

## 2. 服务接入与权限

MyCity 内部以类型化命令／查询实现工具服务，再由消费侧适配层接到所选 harness 的公开工具接口。是否暴露 MCP 或其他公开扩展协议，在接入验证后确定，不在文档阶段绑定未验证传输。

TekesAgentKit 负责会话和执行状态，MyCity 拥有工具规则、世界状态与权限。身份及有效权限由宿主绑定的连接上下文注入；模型填写的 role、city_id 或 authorization_ref 不能自行扩大权限。

城市负责人可在项目授权内提交；部门分析默认只读和草拟，确需执行时获得明确子范围。数据库、存档文件、任意 Rust 代码与 shell 执行均不作为游戏建设接口。

## 3. 公共请求与响应

请求公共字段：schema_version、request_id、world_id、branch_id、arguments；地图／城市级请求另带 map_id／city_id；跨图请求带 involved_map_ids／city_ids，区域只读工具可省略单图；已建立的任务上下文另带 goal_id、goal_revision 和 team_task_id。snapshot_ref 在需要固定观察时提供，首次查询可由服务创建。持久化修改请求（含目标、监测、政策与世界）带 operation_id、有效权限依据和相应预期版本；草案修改使用独立的 plan_revision，不要求存在尚未创建的项目 ID。

request_id 用于追踪单次调用；operation_id 表示同一逻辑修改，跨重试保持不变。operation_id 的作用域为 world_id + branch_id，参数规范化后哈希校验，相同 ID 不同参数返回冲突。

schema 对未知字段、错误单位、NaN／Infinity、越界整数与混合坐标框架报错。schema_version 不支持时拒绝，不能猜测兼容。

响应公共字段：request_id、world_id、branch_id、status、snapshot_ref、simulation_tick、data、warnings、errors、next_cursor。响应另含 coverage 和 data_age；数据不足以满足精确查询时明确报告待验证。对象结果带 revision，坐标带 frame_id，距离和面积带明确单位。

status 区分 ok、rejected、pending、partial；partial 仅用于明确分页／批次结果或多阶段项目，不允许一次状态变更写入一半；持续工程半成品是合法状态。pending 必须提供 task_id 或 operation_id，可查询权威状态。

只读查询通过冻结 snapshot_ref 分页，禁止翻页混入不同时间的对象；快照过期返回 snapshot_expired。摘要包含总数、已返回数量和是否截断。

## 4. 首版工具目录

16 GB 档一次详细运行一张地图，已连接邻图使用摘要。能力描述返回地图尺寸、地图模式、区域时钟与剩余计算配额，见 [跨地图运行](cross-map-runtime.md)。

| 工具 | 类别 | 关键输入 | 关键输出 |
| --- | --- | --- | --- |
| city.matter_register | 筹备持久化 | 目标／范围／筹备授权预算／关联／预期版本／operation_id | matter_id、共享档案槽及筹备回执，不是工程接受 |
| city.matter_status | 只读 | matter_id／快照／分页 | 当前程序门、参与方、版本、资金缺口与各对象回执 |
| city.household_status | 只读 | 家庭／画像／快照／有效查询范围／分页 | 教育技能、就业收入、资金义务、住房消费与满意度原因 |
| city.housing_catalog | 只读 | 地区／资格／价格／快照／分页 | 房源、户型容量、质量、产权／居住权、可交付与锁定状态 |
| city.education_status | 只读 | 课程／地区／参加者范围／快照／分页 | 前置资格、教学容量、费用、工时与进度 |
| city.industry_catalog | 只读 | 行业／模板版本／分页 | 面积与FTE口径、岗位／资格、配方、交付类型、影响、来源及能力状态 |
| city.employment_status | 只读 | 城市／行业／快照／分页 | 可用劳动、招聘、雇佣、到岗／工时、工资支付与unresolved |
| city.industry_status | 只读 | 经营地点／行业／快照／分页 | 产出、库存、投入／成本瓶颈、上下游及环境交通证据 |
| city.election_status | 只读 | 城市／term_id／程序版本／分页 | 选举时间表、登记／选票总计、名单、席位、当选人与交接状态 |
| city.governance_status | 只读 | 城市／制度版本／分页 | 宪章、条例、拨款、生效程序与司法限制 |
| city.bill_propose | 草案 | 类型化条款／范围／财政影响／授权 | 法案草案与版本 |
| city.bill_evaluate | 评估 | 法案版本／制度与财政快照 | 能力、冲突、槽位、程序和资金影响 |
| city.bill_submit | 持久修改 | 冻结法案／授权／预期版本／operation_id | 程序与必要槽位预留，不等于通过或生效 |
| city.case_file | 程序修改 | 可信申请身份／事实／证据／请求／operation_id | 收件时刻、受理／排队／管辖回执 |
| city.case_status | 只读 | 有效查询身份／案件／分页 | 证据、阶段、停止令和裁决执行状态 |
| city.team_status | 只读 | 城市／名册版本 | 席位、兼任覆盖、工时、工资和模型费用配额 |
| city.team_catalog | 只读 | 专业／能力／分页 | 人员模板、知识包、薪酬和到岗条款 |
| city.team_change_evaluate | 预演 | 编制差异／合同／team_revision | 生效时间、工资预留、职能覆盖与任务交接 |
| city.team_change_apply | 世界修改 | 已评估变更／授权／operation_id／预期team_revision | 原子名册与资金预留变更／权限撤销回执 |
| city.team_knowledge_query | 只读 | 专业／来源／范围／分页 | 方法和案例引用、版本、证据状态与适用限制 |
| city.time_status | 只读 | 世界／分支 | 日历、game_time、目标与实际倍率、精细／聚合策略 |
| city.time_control | 持久修改 | 有效玩家授权、目标档位、预期版本、operation_id | 全世界同一时间策略变更，不跳过工程或只快进一城 |
| city.region_status | 只读 | 区域快照与地图过滤 | 地图模式、统一时钟、端口状态与预算 |
| city.exchange_query | 只读 | 类型、容量池、双方与时间窗 | 有效、预留、占用、自由容量与交付 |
| city.exchange_contract_propose | 草案 | 类型化条款、双方、资源与费用 | 合同草案及未满足项 |
| city.exchange_contract_evaluate | 预演 | 合同、区域快照与版本 | 供给、通道、预算校验，不占真实资源 |
| city.exchange_contract_apply | 持久修改 | 已评估合同、双方依据、operation_id | 原子创建合同和资源／资金预留 |
| city.exchange_contract_end | 持久修改 | 合同、条款、授权、operation_id | 终止、释放及在途义务处理 |
| city.exchange_deliveries | 只读 | 合同、批次、计量周期、游标 | 发货、派遣、交付和短缺 |
| city.exchange_settlements | 只读 | 合同、发票、账期、游标 | 配对分录、退款和欠款 |
| city.map_open | 表示切换 | map_id、有效授权、operation_id | 同一逻辑时刻加载并对齐，不增加详细槽位 |
| city.goal_propose | 目标草案 | 市长命令引用、目标、约束、默认值与未决项 | 目标草案与需确认项；只有宿主依据玩家事件／既有授权才能激活 |
| city.goal_update | 目标修改 | goal_id、预期版本、amends／supersedes 和玩家依据 | 新目标版本或待决定提案；不能自改权限、放宽硬约束 |
| city.goal_status | 只读 | goal_id | 目标／项目／监测状态、版本、完成证据与阻碍 |
| city.monitor_register | 监测配置 | 目标、metric_spec、窗口、阈值、告警与预算 | 持久化 monitor_ref，按游戏时钟调度 |
| city.monitor_update | 监测配置 | monitor_ref、预期版本、更新／停止参数 | 新监测版本；改变目标达标条件仍需有效目标修改 |
| city.events | 只读 | 目标／项目过滤、持久游标 | 有序事件、缺口／重同步提示，支持离线补读 |
| city.services_query | 只读 | 服务类型、区域、供方与快照 | 可用容量、响应成本、覆盖、报价与摘要精度 |
| city.service_contract_propose | 草案 | 供需双方、容量、时间窗、费用与优先级 | 协议草案与未满足条件 |
| city.service_contract_evaluate | 预演 | 草案、供方版本和快照 | 容量、路线、响应与预算校验，不占真实容量 |
| city.service_contract_apply | 持久修改 | 已校验草案、授权、版本、operation_id | 原子预留与协议生效回执 |
| city.service_contract_end | 持久修改 | 协议、终止条款、授权、operation_id | 到期／终止、预留释放及未完交付处理 |
| city.service_settlements | 只读 | 协议、交付、时间窗、游标 | 实际服务、费用、账单与对账状态 |
| city.capabilities | 只读 | 无或 schema_version | 支持动作、模板规则、精度、限制、预演模型与 endpoint 接入状态 |
| city.inspect | 只读 | 区域／对象／指标、快照 | 容量、网络、环境和财政摘要，全部来源版本 |
| city.resolve_region | 只读 | 对象关系、选区引用或多边形 | 冻结 region_ref、解释、面积、可操作范围和歧义 |
| city.query_objects | 只读 | 区域、类型、条件、游标 | 有界对象集合与稳定引用 |
| city.get_object | 只读 | 对象引用、详情字段、快照／分页 | 几何、资源需求与实获、用途容量、关系／影响、维护与状态来源 |
| city.catalog | 只读 | 模板类别与过滤条件 | 建筑、道路、管线及受支持植被模板的有类型能力与参数范围 |
| city.find_sites | 只读求解 | 用途、region_ref、硬约束、软偏好 | 候选地块及得分、排除原因、可行性 |
| city.create_plan | 草案 | matter_ref、目标、范围、预算和授权引用 | draft_id、plan_revision、冻结目标与约束 |
| city.propose_layout | 草案求解 | 草案、地块、道路端点、模板及布局规则 | 多个候选草案差异、最终几何、实际占地和未满足条件 |
| city.edit_plan | 草案修改 | 草案版本、增删改动作 | 新计划版本、变换明细、失效评估 |
| city.evaluate_plan | 预演 | 计划版本、快照、模型和时间窗 | 硬校验、预测、资源、依赖、评估凭据 |
| city.apply_plan | 修改世界 | 已评估计划、operation_id、有效授权 | 已接受或拒绝回执、项目／步骤状态 |
| city.operation_status | 只读 | operation_id | 未见记录、执行中、已完成或失败，已生效对象 |
| city.project_query | 只读 | 项目、阶段、快照 | 工程量、日历／有效工时、预计完工、费用、占道、污染与噪声 |
| city.project_control | 持久修改 | 项目版本、授权、暂停／恢复／优先级、operation_id | 阶段状态与现场保留，不重置进度或退款 |
| city.project_change_propose | 草案 | 工程、设计差异、剩余预算 | 冻结施工包的新版本、增量工期和费用 |
| city.cancel_task | 停止后续动作 | task_id、operation_id | 停止结果、不可撤回的已提交动作、释放预留 |
| city.observe | 只读 | 目标／项目、基线、时间窗 | 实际指标、差值、证据与未达成原因 |
| city.set_policy | 修改世界 | 类型化政策、范围、预期版本、授权、operation_id | 当前状态校验后的政策回执 |

原有 city.plan_district 收敛为 create_plan + propose_layout + evaluate_plan；不保留绕过评估的独立建设通道。新增工具以明确 schema 版本发布。目标、授权及监测的语义以 [运行闭环](runtime-loop.md) 为准；工具上下文不能反过来成为自授权来源。

草案修改不改变正式物理世界，也不扣建设款；计算成本受调用预算限制。施工资金与空间预留在接受工程时发生，草案预估不是锁定承诺；筹备登记及其实际调查采购使用独立筹备授权／费用，不因此获得未来施工权。

## 5. 草案动作体系

| 动作类型 | 必须描述 | 验证重点 |
| --- | --- | --- |
| parcel.split／merge | 原地块引用、目标边界 | 权属、面积、无缝无重叠、已有建筑 |
| zone.assign | 区域、用途、密度及开发规则 | 合法用地、保护条件、不会瞬间生成私营建筑 |
| terrain.grade | 范围、目标坡面、挖填上限 | 完整改动范围、边坡、土方与水域 |
| road.build／upgrade | 端点、模板、曲线、连接模式 | 路权、交叉口、曲率、坡度、断联影响 |
| utility.build／upgrade | 类型、走廊、用途层、实际深度／z、地形版本、网络节点 | 容量、拓扑、服务、地下／地面接入及保守包络 |
| building.place | 模板版本、地块、位置、朝向、端口选择及共享空间引用 | 全用地、实体包络、门位／方向、用途兼容、权利、地基与必要接入 |
| connection.build | 明确建筑端口、网络锚点／侧／进出方向及连接走廊 | 分模式可达性、转向／过街／高差、实际容量、占地与权限 |
| object.demolish | 明确对象版本、依赖和安置规则 | 既有授权、人口／就业／管网后果 |
| land.clear | 范围、移除对象类型 | 林地与保护约束、清除成本 |

首版每种动作分别声明 implemented／preview_only／unsupported，不以“工具存在”代表所有动作可执行。公共设施优先支持直接建造；私人分区开发由模拟推进。公共住宅等直接开发必须有明确产权与预算规则。

依赖通过action_id引用。commissioning_group联合验证最终拓扑，但不瞬时建成；其动作按冻结施工包持续完成，组内施工可先后进行，最终再原子投运。require_operational表示最终联合验收门，不是接受项目即具备运行能力；跨组依赖无环。回执给出稳定项目／预分配资产ID映射，详见 [工程进度](construction-lifecycle.md)。

## 6. 硬约束、软偏好与求解

施工硬约束包括允许范围、禁止拆迁、保护地、累计预算、模板范围、最低净距及当前组必须满足的连接条件；未满足不能提交。实际入住等运营结果条件通过持续监测核验，不能与提交条件混用。未来服务依赖必须标为未投运或纳入require_operational最终投运组；施工临时供给须来自已存在或独立合法投运的设施，不能调用未来纸面供给。

软偏好包括更短道路、更少土方、朝向、街区整齐和未来扩建空间。返回评分分项和取舍，不用综合分掩盖硬约束失败。

find_sites 和 propose_layout 可返回 feasible、infeasible、unknown。unknown 表示搜索预算内未找到或数值无法判断，不能声称数学上不存在解。

“住宅容量两万人”可以验证模板和规划容量，“实际入住两万人”取决于就业、需求、时间和交通，只能作为后续观察目标。市长将入住数作为硬完成目标时，系统持续核验，不能把规划容量当作已完成。

不能验证的硬约束返回 unsupported_constraint。例如首版没有水文模型时，“下游水质不变”不能标绿通过；若市长接受保护带等替代约束，需形成新的目标版本。

## 7. 评估分层

1. schema 与引用：单位、范围、版本、模板、参数和动作依赖。
2. 硬几何与权限：完整设施及施工占地、保护区、跨区授权、碰撞、退界、坡度与网络连接。
3. 当前资源与建设：财政、空间占用、投入物、成本归属、工程阶段和服务容量。
4. 隔离预测：在指定快照与时间窗内运行交通、市场、人口和环境预演；声明 static／traffic／long_run 层级、时间预算、收敛与数据覆盖。首版最多一个完整预演任务，不无限并行复制城市。
5. 汇总：分别返回 hard_checks、operational_readiness、prediction_status、goal_assessment，而不合成一个失去含义的“通过”。预测缺失不能声称目标保证；若硬检查通过且市长已授权承担明确运营风险，可以按规则建设未投运资产。

评估凭据绑定 plan_revision、canonical_geometry_hash、construction_revision、模板／规则／授权版本和预测有效期。编辑草案使凭据失效。提交时仍重新检查当前财政、授权、占用和必要资源，评估凭据不保证未来一定可执行。

费用分别列出建设款、预留款、经常维护和预计收入；不得用乐观税收抵消确定的建设预算。展示模型时间窗与成本假设。

## 8. 提交、施工与竞态

apply_plan 接受已评估计划，在模拟提交边界原子校验并记录项目、预算和空间预留。首版可以采用全局 construction_revision 比较并交换，拒绝期间发生建设修改的旧方案。

建筑、道路、管线、场地处理和拆除采用持续工程，按阶段工作量、投入及真实尺度的游戏日历推进。原子性适用于接受项目、每次进度／扣款和最终投运，不把工程压成单步。用途／政策等非物理变更仍可在事务边界生效，不能借行政动作绕过实体施工。具体阶段及时间以 [工程进度](construction-lifecycle.md) 和 [游戏日历](game-time.md) 为准。

项目由多个持续阶段构成，每次进度变更原子提交并校验授权、依赖和实际投入。半成品是合法世界状态；阶段受阻保留已完成工作、已花费用和现场占用，不整体回滚。最终验收失败仍保留实物，不能凭投运失败退款重置。

取消仅按类别释放可释放的未用资金／远期预留，不能退回已花建设款、清空现场或消除实际后果。项目预留按游戏时钟管理，暂停不超时；受阻1日仅告警。已开工现场与安全缓冲持续占用，只有未开工远期预留按明确期限释放；材料和半成品不变成空地。恢复需重新评估。短期计算锁按现实超时回收，规则见 runtime-loop.md。

接受项目后，各原子组按当前状态与项目自身回执验证，不能把初次接受时的全局建设版本硬套到每一步，使项目被自己的前一步永久判过期。

若两个计划竞争同一土地或预算，只有先成功提交者取得预留，另一个得到冲突信息后重新规划。锁顺序与原子提交由服务负责，agent 不能靠调用顺序假设自己拥有地块。

服务先按 operation_id 查询既有结果，再检查新请求的前置条件；重复请求返回原回执，不因世界版本已变化而拒绝已成功的同一操作。

请求超时先查询 operation_status；not_found 仅表示本次查询时没有权威记录。再次提交时继续使用同一 operation_id，由服务进行去重，不能换新 ID 盲目重建。

## 9. 错误与可恢复反馈

错误响应至少含 code、action_id、object_ref、conflict_geometry_ref、actual、required、recoverable、suggested_next_tool。不要只返回“建设失败”。

| 错误码 | 含义／处理 |
| --- | --- |
| ambiguous_region | 指代或区域关系不唯一，列候选供绑定 |
| out_of_scope | 任一直接改动范围超出授权，返回越界几何 |
| protected_overlap | 侵入保护地，必须调整方案或取得明确新政策 |
| memory_budget | 求解／预演剩余内存不足，分段处理或排队，不静默超配 |
| stale_reference／stale_plan | 对象或建设版本变化，读取最新状态后重评估 |
| budget_exceeded | 累计支出加预留超出限额，不能分单绕过 |
| geometry_invalid／numerically_ambiguous | 自交、退化或无法稳健判断，修几何或分段 |
| setback_violation／slope_exceeded | 实际净距或坡度不符合游戏模板 |
| bootstrap_dependency_cycle | 运营依赖成环且没有真实启动供应，不能凭空出水发电 |
| disconnected_access／utility_capacity | 道路不达或水电容量不足 |
| unsupported_constraint／unsupported_action | 缺少能力，必须说明而非静默忽略 |
| idempotency_conflict | 相同 operation_id 携带不同请求内容 |
| precondition_failed | 当前资源、权限或依赖与评估不一致 |
| capacity_overcommitted／unverified_flow | 故障超占用或本期供给不可验证，保留义务并报告缺口，不虚增输出 |
| obligation_limit／storage_unavailable | 无收尾槽位、有限资金或可靠落盘空间，拒绝新增承诺或暂停世界 |
| limit_exceeded | 对象、顶点或计算预算超限，分阶段处理 |

## 10. 一次精确设施建设示例

命令：“在规划区南侧建一座小型水泵站，接入这条路和已有水电，不要碰树林，预算不超过两百万。”

流程：capabilities → resolve_region → inspect／catalog → find_sites → matter_register → create_plan → propose_layout／edit_plan → evaluate_plan → apply_plan → operation_status → observe。持久草案绑定matter_ref，缺筹备授权或档案额度时先停止，不把初查结果当作已批准工程。

计划示例采用米制对象坐标；ID 为示意，必须来自真实查询。以下是 edit_plan 的 arguments 示例，通用请求上下文由外层提供：

```json
{
  "draft_id": "draft-pump-01",
  "expected_plan_revision": 2,
  "add_actions": [
    {
      "action_id": "pump",
      "type": "building.place",
      "catalog_asset_ref": {
        "id": "U1",
        "revision": 1
      },
      "parcel_ref": {
        "id": "parcel-17",
        "revision": 4
      },
      "placement": {
        "frame_id": "local-metric-v1",
        "anchor": "site_center",
        "position_m": {
          "x": 1048.0,
          "y": 720.0
        },
        "foundation": {
          "mode": "fit_existing_terrain"
        },
        "heading_deg": 90.0,
        "snap_step_m": 0.25
      }
    },
    {
      "action_id": "access",
      "type": "connection.build",
      "from": {
        "action_id": "pump",
        "port_id": "maintenance_access"
      },
      "to": {
        "object_id": "road-node-28",
        "revision": 3
      },
      "network_kind": "road",
      "route": {
        "solver": "within_region",
        "region_ref": "region-pump-project-r1"
      }
    },
    {
      "action_id": "water",
      "type": "connection.build",
      "from": {
        "action_id": "pump",
        "port_id": "water_out"
      },
      "to": {
        "object_id": "water-node-12",
        "revision": 2
      },
      "network_kind": "water",
      "route": {
        "solver": "follow_action_corridor",
        "action_id": "access"
      }
    },
    {
      "action_id": "power",
      "type": "connection.build",
      "from": {
        "action_id": "pump",
        "port_id": "power_in"
      },
      "to": {
        "object_id": "power-node-8",
        "revision": 2
      },
      "network_kind": "electricity",
      "route": {
        "solver": "follow_action_corridor",
        "action_id": "access"
      }
    }
  ],
  "commissioning_groups": [
    {
      "group_id": "pump-service",
      "action_ids": [
        "pump",
        "access",
        "water",
        "power"
      ],
      "require_operational": true
    }
  ]
}
```

模板还需声明有效水源资格；若地块无法满足水源条件，即使位置和管线都合法也拒绝。初始描述没有指定新水源时，团队必须核对已有取水条件。

heading 为 90° 时，16×20 m 设施用地旋转后的轴向外包框为 20×16 m；中心 (1048,720) 对应示意范围 x=[1038,1058]、y=[712,728]，面积 320 m²。8×12 m 主体对应 12×8 m，但其模板内偏移和端口仍应按模板变换，不由 agent 猜测。

工具生成并返回连接走廊、最终坐标、高程和完整占地后，才能评估／提交。上面的候选本身不是有效施工承诺。

提交 arguments 示例：

```json
{
  "draft_id": "draft-pump-01",
  "expected_plan_revision": 3,
  "evaluation_ref": "eval-pump-01-r3",
  "expected_construction_revision": 41,
  "authorization_ref": "auth-pump-budget-r1",
  "operation_id": "build-pump-01"
}
```

此示例接受后建立持续工程，工期及阶段来自U1的版本化模板；泵站与接入先后施工，完成后作为同一组原子验收投运，最终需要当前足够供电、水源和合法维护入口。运行示例创建目标并注册水电供给与服务能力 Monitor，完工后按实际窗口采样。

接受回执给出项目ID、预分配资产ID、冻结几何哈希、当前阶段预留及建设版本，不表示建筑已完工。重复apply_plan只返回同一项目，不重新开始工期；后续通过project_query观察阶段，最后核实供给。

## 11. 工具限制与成本预算

[事项流程](civic-project-workflow.md) 要求物理项目持久化草案／create_plan绑定有效matter_ref，使用128个筹备／实施／收尾共享档案槽；轻量只读查询和目标草案不因此成为工程。行政代理登记须有效筹备授权，私人自动开发由宿主绑定的合法业主／模拟身份进入同一准入，不要求市长逐笔批准私人施工，也不能让模型自报owner获得权限。登记不预留施工土地／资金，调查开支另按实际授权付款；apply_plan仍是正式接受工程的唯一既定接口。

[市民生活](citizen-households-and-living.md) 使用44 MiB现行区域预算，仍含于原0.35 GiB模拟域。家庭／住房／课程查询和市长政策工具不赋予私人财产权；居民自动交易、培训与迁居由可信模拟通道依据真实家庭身份执行。行政模型不能自报家庭ID代扣私人款，搜索16候选耗尽返回unresolved而非无房证明，产权／资金操作以持久回执原子清算。

[制度规则](governance-council-and-justice.md) 要求行政提交分别检查法律／拨款、玩家授权、资源与法院命令。评估凭据还绑定rule_revision、appropriation_revision和司法命令版本，提交／阶段执行时重新校验；city.set_policy仅能行使现有条例已委任的行政范围，需要立法的变更走法案程序。没有agent直接写入最终票数、判决或制度身份的接口；系统申诉／表决／裁判通过独立可信通道，不依赖被告市长授权。

团队配置语义见 [编制与知识](team-staffing-and-knowledge.md)。成员ID、专业标签或模型自报岗位不构成权限，宿主校验当前任务与有效授权。聘用、解聘和兼任按team_revision及工资／时间预留一致性提交；未知回执先查询。离任不撤销已提交工程或删除项目证据，人员增加不倍增调用预算。

原型建议：只读几何摘录纳入缓存配额。单页最多 100 个对象、单个同步草案最多 100 个动作、单一输入区域总顶点最多 4096；这些是待测预算，不是实际产品限制。超限通过分页、分阶段或异步求解处理，结果不得静默截断关键冲突。

每任务设模型调用数、token／费用和工具计算预算，长计算返回 pending 与进度。cancel 需贯穿模型运行和工具任务，已提交修改仍遵守操作回执语义。

每次会话先读取 capabilities；不将整个城市每栋建筑的坐标反复传给模型。精细几何以引用持有，摘要给面积、尺寸、净距和关键坐标，按需读取明细。

## 12. 验收矩阵

- 合法设施：在正确比例、完整授权与连通条件下精确落地，地图和权威几何一致。
- 越界设施：主体中心合法但附属用地／外接走廊越界，提交失败且无扣款。
- 模板限制：随意缩放建筑、无效朝向参数或未支持动作必须拒绝。
- 道路几何：弯曲道路的路权、交叉口和边坡全部参与校验。
- 草案隔离：固定输入快照哈希不变，预演不产生正式世界写入；正式城市若仍在推进，可发生正常变化，不能错误要求前后整城哈希相等。
- 原子性：一次进度／扣款提交失败不产生半条事务；此前合法形成的半成品保留，不当作错误清空。
- 项目分期：后续阶段失败，已完成步骤和未执行项准确可见。
- 并发：两部门争同一地块／预算，一个成功，另一个得到明确冲突。
- 去重：超时重连重复同一 operation_id 只施工和扣款一次。
- 目标变更：旧 goal_revision 的后续写入被拦截。
- 能力缺失：不支持水质或桥隧时不声称验证通过。
- 容量与实绩：规划住宅容量不当作实际入住，团队汇报引用真实快照。
- 持续观察：监测在 agent 离线时持续运行，暂停无新样本，达标需符合窗口和分母条件。
- 投运分组：组内持续施工，完工后联合验收启用；无启动条件保持未投运，不能把半年施工回滚。
- 真实接入：通过所选 endpoint 的公开工具通道验证全部调用链，再做真实模型自然语言验收。

## 13. 实现前必须形成的交付物

工具 JSON Schema、单位及引用公共类型、模板配置格式、错误码枚举、几何回归样例、命令回放样例和 endpoint 工具桥接验证记录。

本文的 JSON 是可审阅协议示例；完整机器可验证 schema 仍需在实现准备阶段生成并与类型定义保持一致，不应把示例当作现成 API。

## 14. 公共服务采购

具体服务单位、有效容量、跨区响应和费用规则见 [共享公共服务](shared-public-services.md)。服务协议使用独立的容量预留，不占用建筑施工用地；账单与实际交付状态一起持久化。外部供方摘要有明确内存上限，不借此常驻邻图精细资源。

## 15. 跨地图统一协议

共同权威见 [跨地图运行](cross-map-runtime.md)。service_contract 工具是 exchange_contract 的类型适配，不另建合同、容量池和账本。商品自动撮合也登记到同一资源预留和交付机制，不要求每笔交易都调用 agent。

区域工具返回 source_mode、measured_tick、coverage 和 exchange_epoch，保留物理单位。摘要不足以证明逐栋结果时明确待验证。map_open 仅切换表示，不能增权、回滚邻城或重播收费。

## 16. 提交准入与有界查询

交换合同接受必须在同一事务校验双方权限、共享瓶颈、指定时段资源、资金和义务记录空间。服务入口与通用交换入口使用同一预留，不重复扣款。长期框架和当前已锁定窗口分别返回，不能把下次续购当作既成承诺。

退款响应区分托管释放、现金退款、服务抵扣和应收应付；付款方／所有者身份不可由摘要聚合抹去。负载、正文长度、分页、合同分段和路径明细受 performance-budget.md 限制；长结果以引用／游标返回，不一次把所有历史装入内存。

## 17. 日历与工程工具上下文

时间查询返回game_time、日历日期、目标／实际time_scale、simulation_mode和策略版本；原simulation_tick是逻辑提交／推进索引，不可在聚合模式下乘固定10秒推断日期。建议city.time_status／city.time_control在有效玩家授权下查询与调整全局速度，所有地图共用，不能只快进一项工程。

工程工具按construction-lifecycle.md返回阶段工作量、已发生费用、占道／排放／噪声及实际清场状态；schema示例中的commissioning_group仍表示最终联合投运，不表示同一步完工。

## v0.4.11查询与处理中状态

家庭查询读取每户必要状态，画像仅为统计；case_status及工程状态即使归档仍返回未清偿执行／尾款引用。所有快照查询提供simulation_time、processing_phase、完整／处理中标记及数据年龄；同刻到期分页未排空时可以返回最近完整快照，不把不同阶段数据拼成完整快照。写工具等待安全边界后重验版本／法律／现金，使用既有pending任务回执，不因环满拒绝已接受义务，见 [闭环契约](closed-loop-contract.md)。

## 统一对象能力与植被操作

[城市事物统一模型](city-object-model.md) 扩展现有get_object／catalog／query_objects／inspect，不新增重复的通用状态接口。返回值区分配置需求、实际交付、适用性和有单位／用途／时间窗的容量；字段无模型支持返回unsupported，非适用为not_applicable，不能都填0。每个关系／影响附已有权威来源，私有账户访问仍受权限限制。

植被种植、管理模式变更、清除及斑块／重要树表示移交采用有类型草案动作，经原计划评估／提交、权利／保护／资金／资源和几何校验；capabilities逐项声明是否实现，不因本文自动获得执行能力。新种植需准入共享4096状态和对应来源，日常已授权维护由可信服务调度依合同执行，不能以set_property直接改成熟度、管理质量或家户幸福。自然生长在登记范围内自动推进，不逐次请求市长批准；超出该范围的自主扩张首版unsupported。

画面树返回is_sample与真实patch_ref；登记重要树有独立object_ref。样本提升登记前须确定可移交的几何／覆盖作用并预留槽，原子扣除原斑块重复贡献；未知返回needs_detailed_validation。查询分页沿同一快照，不能生成一份全城每棵树的无限详情数组。

## 共享空间与复合用途

按 [空间共享契约](spatial-contract.md#61-地块内的复合用途与共享空间)，catalog返回模板有界共享子区域、空间层、兼容用途、净空／接入及权限条件；get_object／query_objects返回site、实体占用、共享空间引用和同一对象的管理／使用关系。共享子区域内放置绿化仍须检查来源、4096状态及保护／维护责任，不能仅看土地格已有建筑就拒绝，也不能因shared标记忽略碰撞。

提案展开共享区的明确几何、对象／权利版本、关联建筑、适用用途和费用归属；evaluate_plan同时检查既有实体、阶段施工占用、消防／维护接入和实际共享容量，apply_plan按同一事务重查。相容计划不因site相交必然互斥，不相容的实体／资源竞争仍只有先提交者成功；同一共享对象的并发管理变化按版本冲突重评估。许可结束、关闭或拆除需反馈其他使用者与未结义务，无明确权限仅能预览。

## 地下／地面／空域能力

[垂直空间契约](vertical-space-and-airspace.md) 定义六个展示用途层；capabilities分别报告分层表示、受限地下几何、空间政策、地铁／铁路、航空运营及航空噪声能力。catalog／get_object返回实际z包络、深度／AGL参考系和地形版本，不能只返回一个layer_id就通过建设。范围／政策查询沿用resolve_region及冻结平面几何的垂直／时段条件引用，空间规则沿用bill工具及原256槽，无任意禁飞写接口。

evaluate_plan报告地基／竖井／出入口／施工范围、净空、管辖及适用法规；逐段验证地形和实际通过时间。地形变化使埋深、净空和规则依赖失效，需重评估而非自动搬动对象。跨层交叉不自动连接，未知／未支持返回needs_detailed_validation／unsupported；超过初轮深度／高度范围返回unsupported_range，不按无限合法空白处理。未实现航空时不允许以改layer、绝对海拔或区域标签回报已改航／降噪。

## 建筑入口与分用途接入

[接入专题](building-access-and-traffic.md) 扩展catalog的最多8个局部端口定义与get_object的实例口／连接／受阻原因。find_sites及propose_layout同时给建筑朝向和真实接入方案，evaluate_plan按步行、顾客、货运、必要维护／应急及管网分别校验，不回传一个笼统connected=true。门口到网络的走廊、时段、道路侧／高程、允许进出、共享瓶颈和权利来源可查。

接入锚点最多一个／端口，不能借连接工具无限扩张道路或从门口穿楼掉头。变更入口、道路、围墙或共享通道先检查相关使用方和阶段占用；apply_plan及投运门复核当前版本，封门后在途按真实状态等待／退出／改道。端口／连接身份和权威状态使用全世界65536×32 B表，不为每次查询另建一份；字段实际尺寸、走廊、队列／容量、索引和临时求解仍按各自配额联合计费。

## v0.4.18运行一致性校验

[空间运行契约](spatial-runtime-contract.md) 使对象本体、使用关系、端口、唯一容量组和来源分别可查。evaluate_plan核对宿主／设施与路网、8子区／32关系、8192全世界关系／占用组、65536口及原来源记录的联合准入；4桶或原OD／货运／服务不能表达占用时新增前返回unsupported／unresolved。

种植／后续建设检查最大增长包络，法规／权限／开放变化先失效执行凭据，工具报告硬受阻原因、valid_until及快照年龄，沿原status／事务重验而不新增任意写权限。分割、合并、删宿主或结束关系先原子移交全部占用和义务，失败保持原状；未知几何／规则不能以陈旧可达结果授权。

## v0.4.19：模板、能力与指标查询的统一契约

现有目录／模板查询和对象查询应投影 [Root契约](root-component-contract.md)、[四类模板](first-release-object-templates.md) 与 [双向表](property-system-metric-matrix.md)：返回稳定身份／代际、冻结模板版本、适用能力、字段单位、原权威来源及版本／时间／覆盖；查询不存在、未配置、不适用、未知、未支持和真实零值要分别解释，不把缺参数当免费或零消耗。必要profile未配置的模板不能进入可执行建设／投运，按现有校验错误返回template_unconfigured原因。

上述字段是待实现工具契约的语义扩展，不是新注册的AgentKit方法。工具沿现有有界输出／分页、权限、草案、预演及原子提交链；Root不新增一份实体注册表，schema与模板正文共用64×4 KiB热页。agent只能提出真实动作，不能通过写M指标直接改变P权威状态。每项可执行动作还需同时通过对象、门、空间关系、岗位／家庭、库存、合同／债权等联合配额。

## v0.4.20：目录范围与可执行资产分开查询

[建筑类型目录](building-types-and-technology-catalog.md) 的分类ID不是可执行asset_ref。目录返回规格／机制类型、首版／后续／远期范围和独立就绪／缺项说明；运行时仍用原status／reason语义及真实能力检查。agent可讨论后续风电、核电或车站概念，但无已配置且受支持的冻结资产时不得生成可施工承诺，预览unsupported不能伪造发电／客流结果。

小泵换大泵、扩建住宅、改用途或换燃料均沿原变更／工程流程，保留身份迁移、旧住户／库存／在途／合同／长期责任；不能直接替换template_ref完成工程。目录正文分页沿既有输出和64热页配额，不全量塞入agent上下文，不另建每实例目录状态表。

## v0.4.21：自定义方案的实物清单与经济解释

[复合设施契约](composition-and-cost-model.md) 要求自定义输入选择已支持用途／设备／规格，不允许任意成本、能耗或产出覆盖。输出展开功能／共享依赖、真实几何与峰值／接入约束、设备数量／W及区间水电量、工程量与分期费用、付款方、市场快照／租金／客流假设和缺项；估算／已承诺／实际交付与现金分别报告，不以“收益不错”替代合法性／容量。

沿原草案／预演／提交、profile冻结和真实工程；后续混合楼、商租、摊位许可、政务办公或轨道缺机制时明确unsupported。组合展开≤4功能单元、宿主8口／8子区及版本／原资源联合准入只是候选边界，不改变已有首版范围；时段摊位清场未完成不能自动释放。
