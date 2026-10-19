# T10-A：有限 profile 的可靠 CLI 会话

发布：v0.5.19；日期：2026-10-20。范围是 Rust 模型与本地实验 CLI，不含 UI、AgentKit、远程授权服务或新的全源调度器。本阶段依赖已实现的 T04 事务、T05 有限推进器、T06 持久化和 T09 查询，不替代尚缺的 T05 全源／阶段游标与日历整合。

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
