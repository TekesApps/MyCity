# T10-A：有限 profile 的可靠 CLI 会话

发布：v0.5.19；日期：2026-10-20。范围是 Rust 模型与本地实验 CLI，不含 UI、AgentKit、远程授权服务或新的全源调度器。本阶段依赖已实现的 T04 事务、T05 有限推进器、T06 持久化和 T09 查询，该v0.5.19边界保留；v0.5.21的T05-B接入见文末与[运行契约](m0-timeline-runtime.md)。

## 1. 入口与权限

```text
mycity-headless session WORLD RULES account ID
mycity-headless session WORLD RULES operator system
```

启动参数由可信本地宿主选择：`account ID`只获得该账户原有事务权限；`operator system`是显式本地实验管理员，不能将其暴露为玩家或 Agent 可选参数。输入 JSON 没有 Caller、法院授权或任意属性写入口。`RULES`必须等于当前二进制的源码指纹，且与存档 genesis 的 rules_identity 一致；旧源码世界不能借传入旧指纹交给新实现写入。没有实现旧版本迁移；已有只读 `query` 的诊断边界保持独立。

会话独占 Store 写锁；另一进程不能同时打开同一世界。stdin 每行一个请求，stdout 每行一个响应并立即 flush，stderr 只报告启动、输入流或输出失败。单行最多32768字节（含换行），过大即非零退出，不无限分配或读完超长流；JSON错误只拒本条。客户端也应边写边读，避免自身管道缓冲造成阻塞。请求的 `id`仅关联响应，持久幂等键仍是 Envelope.operation_id。

```json
{"id":1,"request":{"command":"status"}}
```

## 2. 控制与已提交边界

| 命令 | 含义与边界 |
|---|---|
| status | profile、已提交秒／revision／sequence／hash、区域快照 observed_at 与交互钟 age、来源对齐状态、源码／锁文件指纹；不是预测 |
| pause | 当前原子边界确认暂停，返回准确的已提交状态；没有后台线程继续推进 |
| resume | 允许后续显式 run，不自行造推进请求、不从现实离线时间补跑 |
| speed | day_wall_ms仅接受600000／60000／10000／1000，分别为每游戏日10分钟／1分钟／10秒／1秒；只改本会话节奏 |
| step | 暂停时提交一个明确的时钟 Envelope；不做墙钟等待，业务仍由原推进器裁决 |
| run | 运行状态下提交一个明确的时钟 Envelope，先等待目标墙钟时长再执行；一条至多一个推进事务 |
| submit | 提交非时钟业务；暂停仍允许处理既有责任。混入任何 Advance 的 submit 拒绝，不能旁路时钟门 |
| receipt | 按 operation_id读取热／冷原凭证，不执行或生成该操作；缺失为unknown，不推断成功／失败 |
| query | 复用七类快照分页，limit 1..64；游标过期拒绝，不混revision |

```json
{"id":2,"request":{"command":"speed","day_wall_ms":1000}}
```

```json
{"id":3,"request":{"command":"resume"}}
```

```json
{"id":4,"request":{"command":"run","envelope":{"world":1,"branch":1,"operation_id":100,"expected_revision":0,"actions":[{"kind":"region_advance","seconds":30}]}}}
```

```json
{"id":5,"request":{"command":"pause"}}
```

示例针对对应的区域实验期初，不应把示例revision套用到已运行世界。客户端观察上次返回状态后决定下一Envelope；本层不猜测／自动改写请求ID或revision。

支持 region（含 Exchange+Boot 联合）、独立 economy、独立 boot 三种现有权威推进器；每个时钟请求必须只有该 profile 的一种 Advance。Region／Economy每次最多3600游戏秒、独立Boot最多60秒，限制仍由模型验证，错误也保留原凭证。没有时钟的世界只能做非时钟业务和观察；不拼接被模型禁止的 Economy+Region。

`run`等待纳秒为 `seconds × day_wall_ms × 1000000 / 86400`，以u128计算再检查单条最多60秒墙钟；纳秒取整误差小于1纳秒，仅影响等待，不影响整数游戏时间或资源。不会积累追赶任务。响应报告目标等待、整个请求的实际墙钟、实际推进率及提交结果；实际墙钟含同步持久化，赶不上就慢下来。倍率对游戏时间的改变只能通过原事务，不删到期事实、不补资源。外部排队、首次打开和客户端处理时间不包含在单条 elapsed 中，整日实验另报整段进程墙钟。

这是同步、有界实验接口：排在 run 后的 pause 要等该等待与原子提交完成才确认，不能抢占睡眠／落盘，也不能取消已提交业务；单次run等待上限60秒，实际IO延迟另计。它不声称拥有最终玩家输入的低延迟控制，后续异步宿主需缩小推进量并在同一原子边界检查暂停。当前只能显式提交有限步，尚无自动连续运行至目标日期或全源队列。

重开一律暂停，默认1游戏日／分钟，不恢复运行态，不按现实经过时间补跑。精确重试不需要重新resume或等待，但仍经Store核对原actor／payload；同ID改内容或换actor不会再执行。

## 3. 回执、阻塞与未知

```json
{"id":6,"request":{"command":"receipt","operation_id":100}}
```

```json
{"id":7,"request":{"command":"query","section":"blocked","limit":16,"cursor":null}}
```

响应外层 `result` 表示协议处理完成，不等于业务成功：必须检查 `result.outcome.model` 的 Ok／Err，以及 `has_persistent_receipt`。recorded也可能是已持久记录的业务失败。`already_recorded`只表示该operation已有记录，最终是否精确重试必须看原payload／actor核对后的outcome。

协议拒绝有稳定 code（invalid_request、clock_command_shape、pause_state、unsupported_speed、wall_budget）；模型错误按原枚举序列化。存储背压有 storage_capacity 及实际用量，来源未对齐为source_sync_required，必要事实阻塞为fact_blocked；模型／存储执行拒绝后控制器暂停。查询 blocked／recovery 可以进一步定位来源、责任、容量和恢复状态，不会绕过原准入。

IO结果不确定或Store已被污染时，响应为 recovery_required／unknown，锁住后续控制和业务，要求关闭并重开；不能因为未收到stdout就认定没扣款。输出通道断开同样不能撤销已提交事实。重开后按原operation查询／重试确认，不凭新ID再次执行。普通成功／失败原票跨检查点保留，分页与receipt都不向内存加载无限历史。

## 4. 验收与资源边界

验收以[Release原始测试](evidence/m0-t10-session/tests.txt)、[Clippy](evidence/m0-t10-session/clippy.txt)、[格式](evidence/m0-t10-session/fmt.txt)及[来源](evidence/m0-t10-session/provenance.json)为准。测试包括真实CLI子进程、四档实际墙钟等待、区域到期切分对照、独立泵完成、经济一日真实节奏对照、冷凭证／重开幂等、越权／缺钱、跨scope／陈旧revision、过期分页、超大输入／时间、实际磁盘配额背压和来源未对齐。

四档短测只证明节奏选择不改变同一提交序列的业务结果，不是四档全城吞吐。经济一日按24个真实小时事务测整段墙钟，与不等待的相同事务序列核对完整state hash。原始 `T10_PACED_DAY` 记录是否达到严格60秒候选门；设置倍率和接近目标都不能冒充性能门通过。完整30日P-E0／P-E1仍沿用已有业务测试，本测试不新增长期经济平衡结论。

控制器只有一份当前请求／响应、一条≤32KiB输入及Store，没有按居民、建筑或地图复制；状态只导出≤8城快照元数据，不返回无界世界dump。查询仍≤64行，一次冷凭证读取最多一个已有有界检查点，档案链按已有小索引遍历。新增热记录字段为零，不新增持久队列、快照副本或业务档案。Rust对象布局／Vec容量不等于整机RSS，JSON临时页、冷检查点和I/O仍需实测联合峰值。从[全产品预算](product-runtime-budget-and-versions.md)的UI／工具0.40GiB域明确划8MiB给单会话解码／序列化临时页，本域余401.6MiB；原Store／冷检查点仍归原域，不能重复计量，也不能按地图复制8MiB。此为规划份额，尚非allocator硬门，原16GB预算不自动签收本层；真实16GB大城／GPU显示与保存联合仍未验。

## 5. 后续依赖

T10-A只交付有限 profile 的命令入口与实验控制。全源因果调度、动态同刻来源、阶段游标共同持久化、完整工资日历与区域双钟联合应在T05-B补齐；目标对象revision、建设授权／草案与预测处理状态仍未实现。完整T10／P-CMD、T12 CPU参考和T13规模／真实16GB验收保持未完成；T11 UI与AgentKit继续遵循用户本轮范围延期。


## v0.5.21：T05-B分页接入

新增timeline查询和Progress／complete_boundary，status区分last_complete与工作时刻，显示持久续做票。Timeline启用后旧独立推进器拒绝；其分页Advance只走暂停step，run返回timeline_step_only，不声称新协调器已通过倍率性能门。完整协议和未实现边界见[T05-B契约](m0-timeline-runtime.md)，真实CLI处理中状态／只读查询／重开有本版测试。原上文v0.5.19的速度观测只属于该冻结版本。

## Operator 观测与原票读取边界（#49/#45，v176 有限接线）

这是 v173 分页修复之后的合同与 v176 有限实现；[固定来源与实际结果](evidence/operator-observation-20261024/STATUS.md)区分源码审查与 root 执行。独立 `query_observation` owner 只写真实 Store 查询适配器，root 独占 CLI/session 路由、caller 门、验证和发布。它不授予完整账户可见性，也不接入通用 Sim IPC 查询。

- 原独立 CLI `query WORLD RULES ...` 继续是可信本地 operator 诊断，保留原 Page 输出及 cursor 语义。新增 `observe-query` 与 session `observe_query` 使用显式 `mycity-operator-observation-v1` schema，内含同次原 Page；不静默替换旧 CLI 消费者的格式。
- session 的旧 `query` 和新 `observe_query` 在调用 Store 查询前统一检查可信 argv 选择的 `Caller::System`；账户返回稳定 `query_operator_required`。不能仅限制新路径而保留账户绕回 raw Query；JSON 仍不能声明 caller。
- 账户仍可查自己提交的成功、失败原票。依据原 `JournalRecord.actor` 与可信 Account ID 精确匹配，不能根据 actions 涉及哪一个钱包推断权限；System 保留完整诊断。hot 与 archived 使用同一过滤，先完成原 Store 校验，存档损坏/恢复错误照常传播。
- 账户侧缺失与不可见统一返回 `state: "unknown"`、无 record，含义是“没有可返回的本人原票”，不证明 operation 未发生。未知提交必须查询/重试原 Envelope，不能换 operation 重做。撤销回复、pause/reopen 与查询权限不撤销已发生事实。
- 观测适配器只移动原 Page 的所有权并追加有界元数据，不 clone 整页/World 或引入新缓存；序列化和输出缓冲仍须另计。原 sequence/state/snapshot hash/cursor、处理中边界、真实 last_complete 与 working/target 分开。通用行标签明确是 Store 返回的记录或诊断状态，不能把 live 存储/来源诊断一概叫作 committed 业务事实，也不能把计划当已发生交付。
- coverage、source/version、observed_at/age、事实/预测、阻断和条件 ETA 均须有对应来源；缺 provider 显式 typed Unknown。区域快照的原观测时点和 age 可解释；transport remaining_seconds 不能直接变成 ETA。没有新的预测、Goal/Monitor 或事项因果 provider，不在薄适配器里补造它们。

只读访问本身不推进时间、改随机、释放占用或收回义务。Store 内部诊断 API 不被改造成猜测式逐行权限过滤器。Status、其它控制/诊断与未来玩家可见性仍须各自明确，Query/Receipt 局部门不能冒称整个账户 session 已实现私有数据隔离。

验收须使用实际 session/CLI 进程：账户对所有旧/new query 拒绝且不返回行，System 正常分页；本人成功/失败、他人/System、缺失和 checkpoint archived 原票同规则；本人失响应后按原 Envelope 恢复；旧 CLI Page 格式、stale cursor 和 Store 错误保持。来源元数据不得拼接另一个 revision 的状态。仍使用原外置 target/work 串行排程，保留所有原始失败和恢复世界，不扩大原 M0/16GB 结论。

本次实际模块 5/5、新入口 3/3、原 session 16/16，共 24 项不同测试通过。模块实际测试限额、游标/失效、源状态错误与 working/complete 分离；新入口实际验证冷热原票和 actor 门。未专门向 session Receipt 路径注入损坏归档；保留该项待验证，不能用源码错误传播代替实际注入。Status 等其它入口的账户隔离和完整 #49 仍开放。
