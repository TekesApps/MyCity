# 建设施工：真实Boot供给与永久材料去向

本页说明0.1候选的有限模型契约。只有Rust模型和CLI；不扩展UI或AgentKit。实现、实际来源及结果以本阶段验收记录为准，以下设计描述本身不能替代测试。

## 依赖与边界

工程复用现有Project、Engine候选事务、Timeline、Boot库存/生产、Exchange守恒及Store/WAL/checkpoint。既有申请、同意、许可、有效工时、占地、验收、清场和废物接收仍执行原程序。新合同不能自动授予许可或运营容量。当前单城只有一个有界Project槽，不能将其称为全城任意多个并发工程已经可运行。

供给分为两类：普通建设材料从业主已经拥有的真实库存扣减；电、水必须由不可变期初合同绑定到Boot管理的输出库存及真实运营方。Boot其他治理库存不能通过普通材料路径消耗。工程主、材料owner与合同供应方均有实际账户，供应方不能等于付款业主；供应方可以同时是承包商，两种应收累加而不是互相覆盖。

这里只接有限库存与生产结果；管网接入、瞬时峰值流量、跨城优先级和完整多人用工市场没有因此自动完成。施工场地和既有生产资源容量仍受现有机制约束，不能把库存足够当作未来管网容量已经验收。

## 公用资源：按有效施工工时累计

每种供给固定：operator、stock_id、每3600有效worker-seconds的Quantity用量、价格Money及价格对应的quantity_unit。Quantity和Money沿用既有精确整数单位，不从动画或现实等待时间计费。金额分和物理单位的示例参数尚未校准为真实市场价格。

对累计有效工时S，需求Q(S)=ceil(rate×S/3600)，采购费C(S)=ceil(tariff×Q(S)/quantity_unit)。每次Work只取新旧累计值的差，使用checked i128运算。分成多张工单、暂停、重试不能产生免费小数消耗或重复向上取整。utility_worker_seconds与已消耗/已收费累计值在取消、清场及拆除中不重置；有效拆除工时继续使用同一计量合同。

Work必须位于Timeline已完成的权威边界，Boot.time须与之对齐，未完成的调度phase/target不能伪装成可施工时刻。速度倍率只改变现实等待，不能更改项目所需游戏日历与有效工时。

## 材料：Project是累计核算的唯一writer

新期初可显式启用material_accounting。固定最多8行，每行为stock_id、原required、累计consumed及累计实际basis。只允许Draft且pristine时启用；None保持旧JSON/执行行为，不给旧V2存档静默补开，也不从旧估算单价猜实际历史成本。

Engine仍按实际源批次origin_quantity/origin_basis和offset差计算本次材料basis，然后在同一成功Work确认中写入Project永久行。采用实际成本而非设计估价。失败候选不发布库存、现金、进度或累计行；原票重试返回原持久回执，新票才可在真实补给后重试。

累计行表示历史资源去向，不能再出售，不是额外可变库存或另一本账。普通阶段与当前materials消耗对应；未完工清场与original_consumed历史对应；已完工拆除清空运行材料槽后，永久行仍保留且必须等于原required满消耗。每行库存身份、业主、治理归属、尾部零行、唯一ID、非负/上限及总basis不超过项目spent均校验。无数量的basis不能凭空出现。

Exchange只读取这个Project候选，将自身固定raw graph 7..12的累计quantity/basis加到原守恒式的资源去向侧；私有stock15等不进入该raw等式，电水仍走Boot出口分类。普通材料不得绕过治理路径消费其他Exchange受管库存。没有第二份Exchange镜像累计账，因此无需同步两套材料sink。

## 费用与原子性

业主现金支付承包商施工人工报价和公用资源采购费；项目spent还计入已消费材料的实际库存basis。材料已经由业主持有，因此扣库存basis不是再付同额现金。供应方库存basis只用于Boot/Exchange原有守恒，不再次加到业主项目采购费用。

例如共享raw库存Work实际消耗125、basis50分，同时付人工3600分、电20分、水10分：业主现金减少3630分，项目spent增加3680分；供应库存自身的电水成本不会再重复加进项目。承包商收入不自动等于家庭工资，完整用工到家庭工资映射仍需明确联调，不能重复向家庭凭空记收入。

全部费用仍检查现金保护额、项目总预算和实际库存。缺任一资源、保护资金不足、持久化prepared失败时整个候选不发布；applied后丢回包由原receipt恢复，不能重复扣费、施工或增加sink。成本不保证盈利，供应方价格可以低于成本；参数经济平衡不在机制正确性验收中预签。

## 验证和增量预算

候选已实际测得Project大小1616B，低于原STATE_BUDGET_BYTES=2048，未扩大此预算。固定8行和2种公用资源按批次计算，不新增逐设备/逐住户常驻对象；这只是组件尺寸及模型预算，不是16GB物理验收。新增Boot分类计数、两个Engine候选及Store恢复的联合占用仍由完整来源回归与后续规模测量核对，原M0冻结来源不被新模型测试覆盖。

目前已有候选core正常/缺货/保护资金/同承包商供应方/旧None失败重放/恢复篡改反例，以及Store prepared失败、applied失回包、真实补给后新票、WAL/checkpoint和共享raw材料去向实测。新增实际半建清场和投运后拆除的Store联调已在定向阶段通过，包含独立运营/废物容量、真实参与方命令、丢回包和cold/checkpoint/原票重放；没有用手工修改phase代替这些流程。14轮后续 workspace 全目标回归出现一项独立的 metadata 资源数断言失败，因此不能将该轮整套命令称为通过；其余单项通过结果按下方证据分开记录。19轮另通过了施工、Town生产和跨城Exchange三者按顺序争用同一Boot电力库存的Store联调，具体范围及路由替身边界见下方。


## 实际资源目录容量冲突

Boot/Exchange固定6个容量资源，再加施工场地、运营服务和废物接收，共需9个独立资源。实测旧MAX_RESOURCES=8在Store装配前以initial_resource_limit拒绝，原失败保留。为支持真实组合，当前源将有界目录扩为16，未修改4MiB Engine、64MiB model、8MiB common或解析文件字节预算；原M0布局输入依然8条，未跟着放大或缩小冻结负载。

Resource记录实测16B，每份资源背板从128B变256B，单份增量128B；这不是整个进程的总增量，Engine、准备区、双槽及metadata都有持有者。实际16条资源Engine分配容量4,104,080B，双槽EngineWorkspace合计16,424,120B（其中每份retained preparation 1,516,368B），已包含各自背板，不应再重复相加。14轮的16条装配/完整checkpoint恢复、17条初始/恢复拒绝，以及重复ID、未知资源和超容量反例实际通过；同轮8项Store公用设施生命周期测试全部通过。MAX_LEASES=64未扩大。

这些数值不是allocator lifetime峰或同步进程footprint；旧冻结exe的性能结果也不转移到这份新源。M0原normal、分级四速/稳态、8h及真实16GB门继续独立保留。通用capacity-plan的所有潜在同时角色仍需和实际授予路径区别，不能因为单个1616B组件或4MiB Engine合格就签整个64MiB共池。

### 当前验证收据（2026-10-23）

证据根：`/Volumes/HP P900/dev/MyCity/work/construction-utilities-current-01/`。以下结论对应各轮冻结的源码manifest与命令收据，不等于最终主线发布或性能验收。

| 轮次 | 实际命令范围 | 实际结果 | 限定 |
|---|---|---|---|
| `14-resource-lifecycle` | release 下运行 `resource_capacity` 与 `construction_utilities` 两个集成测试目标 | exit 0；容量测试 2/2、Store 测试 8/8 | 实测 Project 1616B、Engine 4,104,080B、双槽 Workspace 16,424,120B；这是该轮资源/分配容量观测，不是allocator峰值或整机内存测量 |
| `15-workspace-final` | release `cargo test --workspace --all-targets --no-fail-fast` | 全部目标执行结束但命令 exit 101 | 唯一报告失败为 `mycity-storage --lib` 的 `metadata_counts_full_receipts_and_all_retained_file_slots`；旧期望资源数为8，而当前目录已扩到16。不得将本轮称作 workspace 全套通过 |
| `16-metadata-policy` | release 下 `mycity-storage --lib` 定向测试 | exit 0，21/21 | 修正metadata容量政策后该crate library测试通过；这不是重新运行15轮的全workspace/all-targets命令 |
| `17-clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 | 严格Clippy通过；不替代全workspace测试，也不证明运行期性能 |
| `18-shared-supply` | release 下运行 `construction_utilities` 集成测试目标 | exit 101；8/11通过，3项共享供给场景失败 | 失败为测试装配问题：两项在日界后排程导致 `Timeline(Input)`，另一项在Timeline尚未启用时对snapshot `unwrap`。保留此轮失败证据，不当作模型失败或通过 |
| `19-shared-supply-clock` | 修正测试日历/fixture后，重跑同一 release 集成测试目标 | exit 0；11/11通过，source unchanged | 三种真实顺序的共享库存竞争均通过，包括丢回包、WAL冷开、checkpoint及原票重放；具体数量与路由边界见下节 |
| `20-final-bins` | locked/offline release 构建 `mycity-headless` 与 `construction_utilities` 两个binary | exit 0 | 最终CLI源码指纹为 `4d44b663c997d2cc6c8136bc9ecd17c3bcc009023cc3de15a9df9830060c9a09`；两个binary均记录了相同source fingerprint |
| `21-final-clippy` | 19轮11项测试加入后的 strict workspace/all-targets Clippy | exit 0 | `-D warnings`通过；该检查不替代workspace完整测试或性能验收 |
| `22-final-cli` | 固定最终 `construction_utilities` binary，执行真实Store冷开CLI | exit 0；约1.07s | 具体输出与非干净单进程资源观测见下节；Town/跨城竞争字段为 `not_tested` |
| `23-final-capacity` | `mycity-headless model-capacity-plan <dir>` | exit 2 | 误传额外目录参数，程序返回usage；保留原始失败收据，不视为容量数据 |
| `24-final-capacity` | `mycity-headless model-capacity-plan` | exit 0 | 正确无参调用；capacity plan 与前述预算数字一致，仍明确 `target_slots_fit=false`、`runtime_grant_accepted=false` |

容量比较器将当前候选与独立保存的v125基线并列：metadata为35,344,632→35,345,312B；core Engine为4,103,496→4,104,080B；generic potential为79,700,704→79,704,104B。该generic potential预算为67,108,864B，两版 `target_slots_fit=false` 且 `runtime_grant_accepted=false`。v125这里只是对照输入，不能把其任何设备验收继承到当前候选。当前仍未实测真实16GB物理设备、allocator生命周期峰、完整进程RSS或M0规模/耐久性能；这些门明确保持未测。

### 共享Boot库存的实际交易顺序

19轮在同一Store和同一实际Boot电力库存上，分别验证三个顺序；电力初始库存为100,000，Town每生产并销售1单位商品消耗1,000单位电。每步均检查供给库存、Boot本地出口分类、资金/原料守恒及永久材料sink，没有为任一参与者复制或补发一份库存。

| 顺序 | 实际结果 | 持久化与边界 |
|---|---|---|
| 施工 → Town | 施工先消耗1,000电，库存剩99,000；随后Town实际产销99单位并消耗余下99,000电 | Work在Applied后丢回包，经WAL冷开和原票重放不重复扣款/施工；Town结算后checkpoint冷开并重放不重复结算 |
| Town → 施工 | Town先消耗全部100,000电，产销100单位；施工Work失败，库存、资金、材料和施工进度保持原值，仅写持久失败回执 | 对失败原票重放仍为失败；WAL与checkpoint冷开后状态一致，失败操作不因恢复而重新解释 |
| 跨城Exchange → 施工 → Town | Exchange先实际派送40,000电，库存剩60,000；施工再消耗1,000，余59,000；Town因此只产销59单位并消耗最后59,000 | 跨城Applied丢回包冷开重放、Work成功重放、Town checkpoint冷开重放都保持一次性；Town生产只走本fixture的 `route_source=77` 路由替身，非真实道路分配或全路网交通联调。Exchange使用fixture内真实电力报价和已配置边3/4 |

这些测试证明该固定fixture中的工程、Town和Exchange共享一个库存池，并且先后顺序改变时可用量与交易结果相应变化。它们没有证明全地图电网拓扑、距离/变压限制、真实道路分配或并发调度公平性；跨城Town路由仍是明确标记的测试替身。

### 最终CLI与资源观测

22轮使用20轮构建、source fingerprint为 `4d44b663c997d2cc6c8136bc9ecd17c3bcc009023cc3de15a9df9830060c9a09` 的最终 `construction_utilities` 可执行文件，实际Store场景冷开后状态hash一致。该小fixture记录worker 3,600秒、公用电量/用水各1,000、对应费用20/10，项目spent 4,630分（其中材料basis 1,000、人工3,600、公用费30），Project大小1,616B；电水实际出口分类均为1,000。CLI报告 `town_cross_city_competition=not_tested`，不覆盖19轮的三个独立竞争测试，也不表明其具备真实跨城道路履约。

同一CLI进程由macOS `/usr/bin/time -l` 观测约1.07秒，maximum resident set size 56,623,104B，peak memory footprint 52,707,712B。此轮运行时存在无关的TekesAgentKit Cargo活动，因此它只是单个小fixture进程的一次观测，不是隔离、干净的性能实验，更不能替代模型专属heap、完整进程并发峰或真实16GB设备验收。24轮无参capacity plan虽成功打印计划，但容量目标本身仍未被接受；实际物理/耐久门继续未测。

本阶段发布收据与精确来源见[独立证据页](evidence/core-construction-utilities-126/STATUS.md)。
