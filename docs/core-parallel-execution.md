# M0验收与0.1核心模型并行执行

2026-10-22：用户更新的目标已在任务goal中active。只做Rust模型/CLI，无UI、GPU、语音和AgentKit。先完成[集中回收](evidence/disk-reclamation-v105/STATUS.md)，再启动构建；该回收不等于性能验收。

## 当前代码依据和责任划分

M0 and the three bounded core lines are integrated through v0.5.135; exact per-version sources and execution evidence are recorded in [current status](m0-implementation-status.md). The original v106/v107 worktrees below describe the initial delegation, not today's unmerged work. The newest fixed-load physical observation remains v130, over the original normal threshold; v132 micro-hash timing is not whole-model performance. L0/L1/L2 repetitions, four speeds, faults and the real eight-hour run retain their original gates. The 32GiB host does not satisfy the real-16GB gate (`not_run`). No prior-source performance is transferred to new core features.

只读代码调查确认可复用：`city-core::transaction::Engine`单写事务/幂等journal/checkpoint、`assembly::World`账户/库存、`units::GameTime`及`calendar::Date`、`timeline::Timeline`工资/税债日程、容量Resource/Lease，以及`city-storage::Store` WAL。它们有固定上限和实验域约束；PE0固定四角色/固定日销量不能被当作动态市场，Region/Exchange有限跨城路线也不是道路交通图。

| 线 | 隔离分支和目录 | 独占修改范围 | 接口/依赖 | 实际完成条件 |
|---|---|---|---|---|
| A交通 | `codex/core-transport`，`/Users/frankqin/dev/MyCity-core-work/transport` | core transport模块、example、测试、该线文档 | 显式授权出入口/有向边/群体OD/容量；向其他线输出可达性、成本、未完成量、边界来源 | 正常、断路、拥堵、无路四场景及反例实测；确定性守恒、扩容和必要失效重算；算法限制和资源预算 |
| B工程 | `codex/core-construction`，同父目录construction | core construction模块、example、测试、该线文档 | 现有时间、事务、容量和账户库存；外部许可/参与方未办理必须等待 | 工料钱工时约束、停工取消改拆废物去向、验收前无运营能力；中断重试恢复不重复扣款/施工/退款；完整小场景和预算 |
| C经济 | `codex/core-economy`，同父目录economy | core town_economy模块、example、测试、该线文档 | 只读现有World/工资到账；交通/供给DTO先显式替身，提交回现有Engine | 动态需求价格购买力库存约束、28日和压力传播、钱物核对、亏损退出；期初资金依赖及未平衡参数如实报告 |

三线最初从v104起步，后续分支和来源按实际freeze及提交记录；三线均已有有限集成和实际证据，未完成门分别保留，不以派工或未运行的测试宣称通过。各分支允许自己的lib注册，但最终由root统一合并。共享transaction/Store/账务/持久化及主CLI由root协调单一负责人，禁止各线各造一份。隔离目录稀疏检出源代码/fixtures/.cargo，避免复制历史证据；旧设计从主目录只读。

## 已确认的真实依赖，不作为完成项掩盖

- 动态经济生产需要现有Engine中的有界原子recipe转换；现有MoveStock只能转移同一commodity且单一连续lot，不能免费生成产物或无条件混合不同lot。先实现只读计划器不等于已完成生产/实际销售；新增原语必须统一接入现有journal/持久化/回放，含资金、成本、失败原子性及恢复验证。
- 工程不能凭程序状态直接改World账务或能力，也不能将尚未实现的协商/法律程序自动批准；外部参与方结果需有绑定和明确等待。
- 模块独立测试通过不等于真实联合。最终逐项把交通和公用替身替换为实际输出，并核对交通断路→经营受限、工程占用→容量变化、工资→购买力等限定因果链，以及共同容量、时间、账务和恢复不变量。

## 构建、验收和回收调度

root持有同机重任务调度权。各agent先编码，不自行构建；只使用现有主目录target，由root顺序授权。切换来源记录准确源码摘要，固定验收exe独立保留，不能拿后编译的二进制冒充旧冻结来源。正式性能窗口禁止同时编译、重测试、盘点或清理；其他线可继续编码和只读审查。

阶段通过后才提交、唯一递增annotated tag、推送及验证远端；未通过保留反例和限制。各线交付实际Rust、可执行场景/测试、验证记录、预算。工程与经济共享接口集成单独验收，不修改冻结负载以求通过。

每阶段先记录新增大目录/预计峰值/下游原路径依赖和解除条件，终态先留证和回收决策再开下一轮。共享target不复制，不删除马上要重建的缓存；固定源/依赖/exe、业务/WAL、必要完整恢复和故障原件保留。无法确认用途者保留并记录解除事件。文件分配空间与df可用变化分别报告。

## 当前集成次序与验证门（2026-10-23更新）

- A有向图/OD模型在v107实测，Store市场adapter在v119及v122有限实测；v134另在128节点／512边／128入口／512 OD固定最大输入上完成normal、部分道路关闭、拥堵和全网无路探针。它每次全量确定性重算并用指纹拒绝旧输入；增量缓存未实现。最大输入的单次CLI和Assignment owned capacity已实测，但分配峰值、完整地图吞吐及16GB/M0预算仍未测。详见[交通模型与上限预算](core-transport.md)及[v134收据](evidence/core-transport-capacity-134/STATUS.md)。
- B投运、拆除、废物接收及关闭的Engine/Store接入已在v122限定实测；v126接入真实Boot电水与材料核算，v128验证共享材料完整生命周期，v133三日联合继续复用。v135另完成定额现金补偿的原子付款及持久恢复验证，原有无合同的Compensation gate仍仅表示证据审批。旧版None兼容、新非空合同及两项损坏checkpoint拒绝分别实测；没有法律协商、估价、托管或自动退款机制。
- C在v119已以同一Engine/TownSettle/Timeline执行28日市场及七种现金、原料、无路、容量、能源价格、需求压力，另实测营业额税fixture和贷款债权；v131另跑28日显式换岗工资／购买力对照。v133把就业变化、真实固定OD道路proof、工程和有限Boot公用接入同一Store，但仅为三日联合。现有28日就业与28日税债／道路场景仍分开，运输/公用替身、资金自给、完整税法及法律关闭均未证毕；不要将独立结果合并称为28日完整联动。见[v119经济与交通记录](evidence/core-transport-tax-integration-119/STATUS.md)、[v131就业记录](evidence/core-employment-131/STATUS.md)、[v133联合记录](evidence/core-employment-lifelines-133/STATUS.md)。
- M0最新原满级物理诊断见v130：完整恢复窗口仍超normal 7,628,426B。v132受限整数编码只证明局部哈希改善，不能转为整机/快进通过。v134记录紧凑索引候选不合入：固定填充抵消局部字节减少，且完整旧L1恢复契约不同；不缩冻结负载或阈值来制造通过。v132同源新smoke/formal控制器已完成静态预检，但尚未启动；原三轮四速、8h、真实16GB门保持未完成。

当前root串行验证窗口为A最大输入探针、B现金补偿与恢复反例；共享Engine/World/Store修改由root统一协调，各agent使用隔离工作树。正式M0窗口单独排定，期间停止本项目编译、重测试和清理，其他线继续编码/只读；外部任务污染单列，不能称clean通过。不因等待16GB设备暂停独立功能开发。

[最新集中复查](evidence/disk-reclamation-live-20261022/STATUS.md)新增删除0B；190份完整恢复/故障原件重读SHA匹配，唯一target继续复用。旧清理不重复计入；新增大负载仍须预登记峰值和精确退役条件。

### 本轮新增的八小时接线阻断

只读追踪确认正式steady已从native observer继承R07 provider；但`joint-endurance`原来虽被允许观察，却未启用provider，也未识别其`scale-endurance`驱动及`layout-session`等待ack。root已补最小角色映射草稿，advisor完成一次窄审，尚待Cargo与实际生命周期验证。没有将layout、pool、模型worker纳入额度。验收须覆盖驱动退出后额度重新计算、旧layout退出、冷恢复新owner接管；缺有效ack的样本不能当覆盖完整。provider reading有source/nonce/birth/TTL约束，ack本身仅核source/State/活owner，不能混称两者认证。该接线修复不是8小时实测通过。

本轮合并功能终态见[v108实测](evidence/core-m0-integration-108/STATUS.md)：199项及严格检查通过；交通adapter持下一功能窗口，production实际核算继续编码。上文阶段顺序是实际执行记录，未完成门保持未完成。


## v0.5.114之后的执行窗口

交通resolver已在v0.5.114独立16项测试/六个pureplan CLI通过，企业退出planner在v0.5.113独立13项通过；二者尚非真实Engine联合。建设原分支保留WIP，由construction_finish单一writer接续实测完整投运/拆除/废物/关闭及恢复；默认2MiB测试栈失败与16MiB条件通过必须分开报告。root随后集成并使用当前主线验证，不混用冻结M0来源。

- construction_finish：独占共享接口与Cargo功能窗口，先完成建设阶段；之后才接[真实交通经营契约](core-town-transport-integration.md)。
- economy_scenarios：只改真实28日example/scenario测试/fixtures，在隔离economy分支编码，等待构建窗口；不可修改Engine/Store等共享writer文件。
- root：建设窗口结束后运行既有2MiB02诊断，保持原负载/阈值；性能运行期间其它线只编码/只读。外置work/target与190保护原件规则不变，诊断、恢复和正式性能分别报告。
- advisor：已完成真实接入的具体可信输入/跨天配额审查；没有新证据不再做同专题整体复核。最终M0验收审查仍保留。
