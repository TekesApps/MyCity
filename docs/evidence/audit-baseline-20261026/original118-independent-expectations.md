# #136 复用原 #118：独立输入与预期（非实现验收）

日期：2026-10-26，Asia/Shanghai。固定现基线 v0.5.195 / `31fb1730e5d68857fa461763c4d763df5d55f215`。当前总协调负责唯一共享代码和串行验证；本文件仅准备独立预期，不能当新版规则已经定稿或测试通过。

本轮唯一写入此文件，限64KiB。产品源码、测试、fixture、旧handoff及五份原件均只读；未创建树/会话/worker，未执行编译、测试、fmt、脚本、基准、GPU或联网Issue修改。未联系停止的旧主会话。没有阅读或依据尚未写的新容量实现反推结果。

## 1. 推导来源与证据边界

原交付完整保存于本目录下 `initial118-originals/`，由root预先核SHA：

- `crates/city-core/src/physical_capacity.rs`：纯候选的半开窗口、量纲、别名/组、Actual延续、未知容量和重试来源设计。
- `crates/city-core/tests/physical_capacity.rs`：原literal区间oracle和边界反例，仅作为既有需求依据，不把测试实现的结果当独立证明。
- `tests/fixtures/issue-118/shared-entrance-rail.json`：入口共享、轨道排他和Actual不因预测结束释放。
- `tests/fixtures/issue-118/pump-power-flow.json`：区间总量与kW峰值不可混同。
- `docs/handoffs/issue-118-physical-capacity.md`：原候选/真实原池/Store的明确缺口。

另只读核对实际 owning source：

| 来源 | 独立依据 | 本次观察SHA-256 |
| --- | --- | --- |
| `/Users/frankqin/dev/MyCity/crates/city-core/src/transaction.rs`，Resource:421、Lease:441、原容量动作:4038起、JournalRecord:294 | 原Lease是占用事实；原Resource提供聚合字段；原token带世界/分支；原事务/原票重试 | `7c43a78e7196aa9234b932cef93112d754790c8a2e47de27c4e861dd18d3a393` |
| `crates/city-core/src/timeline.rs`，DAY:16、Progress:292 | 原游戏秒与完成边界；工作中时刻不等完成时刻 | `a01e5f7b770dd39d07f525b3df48bf5b0be352542a9bea7003a1809008cf263d` |
| `crates/city-storage/src/checkpoint.rs`，receipt:239 | 查询原热/归档JournalRecord，不靠重新执行恢复 | `9a03cd678ce52829aa00029da487a3f353862ca3bc65cf25928ae1e649b26d4e` |
| `docs/physical-economy-runtime-contract.md`，§3及许可退出条款 | 重叠窗口接纳门、预留转Actual不双计、减容保事实、到期不等清场 | `6ac622e0eed731af73cb3227ee87b0f3e5e0b43bec4a37a2c4602b5be6f3e311` |
| `docs/m0-regional-fulfilment.md`，§2 | 旧原池人口/货物/Boot/水电槽的不同业务口径、全链保守持有 | `c20333a7ce700deb087bf26581edda5a9cad7b5a4a8ff2cec28863922b18451f` |

HEAD与实际观察字节分别记录；root并行WIP不能被称为纯HEAD来源。本文件不再修改或复制任何这些原件。

原候选含自有claims/receipts DTO；它们只是旧纯候选，不是可复制进持久状态的第二占用池或第二journal。其256资源/1024claims等原型限额也不增加现原池16 Resource/64 Lease。历史scalar readgate release五项通过、debug四个Store场景stack abort，以及后来的venue有限验证各有原范围，均不替代本#136窗口计数验收。

## 2. 先冻结事实与待定语义的界线

以下是需求层独立结果，非新API命名或新计数公式：

1. 同一物体的合法别名、模式、合同、consumer必须解析到同一原Resource/Lease事实；别名不能另开一份可卖容量。
2. 同一原Lease只处于Reserved或Actual一种状态，预留转入场不增加第二份人数；只有真实退出才能删除Actual对应的占用。
3. 半开窗口下，容量允许的相邻预约应可同时存在。**所有预约units算术和不一定等于同刻需求峰值**。
4. A实际未退出越过预测结束时，B不得强行入场；B已接纳预约仍保留为等待/未履行承诺，不能为了保持计数漂亮而消失。
5. due、闭城、许可撤销、取消attempt、求解额度耗尽或重启都不证明物理退出；不免除旧事实/责任。
6. 原票只兑现一次，元数据索引不再签发成功receipt。无票或未知不等可安全重做。

**仍待root与原advisor定稿，本文不指定**：

- 新窗口模式下 `Resource.reserved/occupied` 的精确聚合定义、是否只表达当前切片、峰值、其他既定口径，以及与旧无窗口计数的组合。
- A已经Actual且没有可信退出时，是否允许此后才请求的未来B预约：拒绝/保守等待/有条件预留的策略与对玩家承诺文本。
- 因真实超时占用使原未来承诺不可按时履行时的改期、优先级、赔付、费用、窗口失效后承诺状态与退出程序。
- 版本迁移、clock_kind/clock_version与单位注册的具体wire/API、元数据/工作区配额、错误码与原子提交实现。

这些待定项不能使上面的保事实结果变成可选；反之也不能用本文件擅自宣布一种新字段口径已获批准。

## 3. 共同输入前提：不能由非零ID代替来源

以下数据是独立oracle的有限游戏合同输入，名称为符号，不要求实现采用这些字段名或数字ID。只有root从原游戏注册/合同/产权/活动验证后才是合法输入：

- 同一世界W、分支B、原提交基线R（合法genesis R=0保持Known零），同一可信快照；两个业务consumer分别为教学通行与公服通行。
- 一个已实际存在且依法开放的共享入口 `G`；其唯一原池行记为 `R_GATE`。别名 `education.east`、`service.east`、`map2.same-east`均指G，而非创建新资源。R_GATE是符号，不能把既有Boot101/102或其它旧槽改名挪用。
- `R_GATE.capacity=2`，精确单位为“同时在入口内的原游戏人数”，不是班数、招生名额、人/秒或移动次数。A的1名学员+1名教师合计2人；B的1名员工+1名访客合计2人。招生额度另行校验，不加入占用人数。
- A/B群组须有实际原游戏人口来源与不相交成员覆盖，例如经root验证的原ordinal范围[0,2)、[2,4)；不同group_id不自动证明成员不同。本文件本身不注册人口，也不证明教师资格、出勤或交通抵达。
- 原时钟是该合同明确选定的游戏钟C（单位：原game-second），不是真实墙钟/渲染帧/倍率。所有窗口与事件使用同一已验证clock_kind、clock_version V。V是root冻结的真实版本；原候选的SimulationTick/version1不能自动等同现生产clock。Timeline serial、operation_id、revision不能代替时钟版本。
- 有界覆盖H=[0,40)，窗口均为半开区间，且可观测/准入证据覆盖所请求窗口。初始原完成时刻t=0。原Timeline工作phase/target未完成时不拼接半状态为完成快照。若C/V/H、单位、身份或权利无真实来源，结果为Unknown/拒绝新承诺，不填零或自签proof。
- 活动入/出事件有可信原游戏来源、世界/分支、目标、群组、准确发生时刻、原token及唯一原操作/事件身份；System仅是调用权限。Account、截止时间或预约记录不能自发证明Actual。迟到事件策略未定时不得把occurred_at静默改写成now。

## 4. 独立窗口oracle：先预约，再看真实事实

对**尚未实际进入**且权利有效的预约，只做以下区间算术：

`planned_demand(t) = 对每个 start <= t < end 的预约累加人数`。

这条独立数学表达式不是持久第二余额，不要求新实现也用同一算法。表格期望由有限literal区间和实际游戏事件得出；不要拿待测helper本身计算expected。

### E01：不同consumer、同入口、重叠拒绝

空原池；A申请2人[10,20)，成功接纳。B分别从另一个alias申请2人[15,25)。

| 原游戏时间段 | 计划需求literal | 容量 | 期望 |
| --- | ---: | ---: | --- |
| [10,15) | 2 | 2 | A可用 |
| [15,20) | 4 | 2 | 新B不接纳 |
| [20,25) | B若被拒就没有新增承诺 | 2 | 不凭这段空闲倒过来批准完整重叠窗 |

B失败须无部分lease、无孤立token索引、无原池扣量/钱/权利副作用；A原lease、窗口、consumer、责任和receipt原样。B换alias、consumer标签或group_id都不能重新卖G。若B改用一个确实不同、原注册可验证的入口才可能独立，但空间近似/字符串不同不能证明是另一物体。

### E02：不重叠可预留，不把全窗口总和当同刻峰值

清洁独立场景，二者均在t=0、任何Actual之前申请：A=2人[10,20)，B=2人[20,30)。

| 时间段/边界 | literal峰值 |
| --- | ---: |
| [0,10) | 0 |
| [10,20) | 2 |
| t=20及[20,30) | 2（A预约右边界不含20，B左边界含20） |
| [30,40) | 0（仅预约窗口算术；不涉及Actual自动退出） |

准入期望：A、B均可预留；两个各2人的原lease/业务合同可同时保留，预约总人数4不等于峰值4。两张票仍指同一个R_GATE/容量2。**不指定此时原Resource.reserved必须是2还是4或其它approved口径**；这正是root新计数语义须定稿的部分。不可为让旧聚合校验通过而造两个Resource、降低Lease.units、丢一张票或将B不记原池。

邻接不等于免清场：若原合同有正转换/清场时间，必须把它计入受占窗口，改成真正不重叠后再要求双预约成功。本例明确清场额外时间为0（或已含在A的退出边界），不是默认所有入口无需清场。

### E03：前者越窗，后者等待且原承诺不消失

沿E02已经成功的两个预约，依次施加独立原游戏事实：

| 步骤 | 原事实 | 应保留/拒绝的结果 |
| --- | --- | --- |
| t=10 | 可信A整组真实进入，绑定A原token，2人 | A从Reserved转换Actual，一次；B仍Reserved，无双计、无新第三lease |
| t=20 | 没有A真实退出；只有A预测/预约结束 | A仍Actual 2人；不可由窗口结束删除原lease |
| t=20，B尝试进入 | 原已接受B尚在其[20,30)内 | B等待/阻止Actual；不是自动取消B，也不能把B送进来挤掉A |
| t=22 | 仍无A退出，业务查询/切图/冷恢复 | A Actual、B未履行预约及两方责任仍存在；不能显示“占用0/预约完成” |
| t=23 | 可信A真实退出事件，准确occurred_at=23且当前边界23 | 只释放A原token对应2人一次，保留A完成事实/原票；B原预约不重新开单、不刷新钱/权利 |
| t=23 | 可信B在此时真实进入（仍在原窗内） | B可原地转换Actual 2人，不因前次等待而成为Missing；这是23发生的新事实，非把20的迟到事件改时间 |
| t=30/31 | B无真实退出，窗口/时限已过 | B仍Actual；到期不是退场 |
| t=32 | 可信B真实退出 | 只释放B对应2人一次，保留其旧责/费用/完成原票 |

B等待的具体错误码、原期服务缩短后的履约/应计、改期/赔偿由root合同决定；本例只要求不造Actual、不删旧承诺、不假称整段服务已交付。等待不是“证明城市无资源”或资源永久短缺。这里不要求把未来保证与在场占用做哪一种聚合；事实结果必须保持。

### E04：Actual与未来申请的未定门

独立场景，A已经Actual，再申请未来B且只有预测退出时间。此时“接受为条件预约”还是“保守拒绝/等待”，root与原advisor尚定稿。本文不从旧候选Actual延伸到u64::MAX的实现决定答案，也不把预测退出当证明。无论采用哪种政策：A不消失；B没有已发生事实不得被造为Actual；如果已接纳B则须有可持久责任与等待/退出规则；不得用未知证据签保证。

## 5. 取消、身份、版本、原单位的独立反例

| 编号 | 输入变体 | 独立期望 |
| --- | --- | --- |
| E05 | B还Reserved，原合法取消者取消；原操作重试 | 仅释放B原lease/预留权利一次；A不变。原账单/已发生准备成本不自动全退；同请求读原receipt，不双释放/返款 |
| E06 | A已Actual，普通Cancel或通用Release试图冒充退出 | 不得删除Actual；只能合法退出流程。取消attempt/未执行候选不影响已发生在场事实 |
| E07 | 错世界/分支、错actor、篡改人数/用途/alias目标/窗口却复用同operation | 原权限/operation冲突拒绝，不能通过更名重入；旧事实及原票不变。ID或hash非零不是授权 |
| E08 | 原窗口[10,20)变[20,10)、[10,10)，零量或人数加法溢出，window end溢出 | 拒绝新承诺且无部分写；时间/数量未知不是合法0，溢出不绕成小量或免费 |
| E09 | 同一窗口混Count人数与班数/招生名额/车次 | 不兼容量纲拒绝，不做无来源隐式换算；角色人数只算一次 |
| E10 | 请求钟版本V2或本地发展钟，但来源/窗口绑定V1交互钟 | Stale/Unknown/拒绝，不能只因u64数值相同就比较；版本切换不得丢Actual或原责任 |
| E11 | 窗口越出H；source覆盖缺失/过期；Timeline半阶段/未启用 | 不得宣称可行/安全/真实短缺；Unknown或拒新，已存在lease/必要事件保全。不得把last_complete+任意常数当未来覆盖证据 |
| E12 | 不同group_id却引用同一原人口partition成员 | 不能同时Actual；不因标签不同增加成员或忽略别的业务占用。实际人已存在时保留事实并停不安全新增，不通过删人“修复” |

### 原物理单位不能被Count万能化

原 `m0-regional-fulfilment.md` 的旧实验合同明确：201/202每名迁居者1槽、每Q货物向上折20槽；101/103是24kW、60秒包络且单批<=400Wh的电槽；102/104是10L/s、60秒包络且单批<=600L的水槽。它们不是新入口人数单位，也不等于把原Lease.units直接标成kW或L。物量仍走原库存/计量，槽不创造水电。

可独立准备的单位反例：同一合法80kW功率域，两consumer同刻65+20=85>80，即使日平均低也拒不相容新增；这只是功率峰值算术，只有root确实注册该单位/profile时才可进入产品验收。泵1200L区间总量与每秒流率不可互换；未有完整区间积分/余数/来源规则时Unknown/unsupported，不能把总量按重叠边界求和冒充峰值。Count首门不代表Rate/Total后端已完成。

## 6. 单一原池、兼容与恢复：检查对象而非另建账本

拟采用：原Lease保留token/actor/resource/units/occupied为唯一事实；有界token索引补充canonical解析、consumer、单位/版本、窗口和原合同/来源引用。索引不存第二个可独立消费的容量余额，不把旧candidate.claims或candidate.receipts持久化。表中expected是原事实与合同的逻辑投影，不是建议新增持久表。

| 编号 | 场景 | 独立期望 |
| --- | --- | --- |
| E13 | 无新窗口配置的旧存档/Boot/Exchange/Construction/原无窗口lease | 旧原票、单位、保守全链持有和关闭/故障清场规则不变；不能为了新入口模型重释旧槽、按新窗口到期释放或暗增容量。旧managed venue的整窗保守模式也不静默升级 |
| E14 | 同一模式/地图再读同token；两个consumer别名同G | 只有一份原Lease事实；索引是引用。同token不可被登记为两个独立使用额度；同物不同resource伪登记须拒绝 |
| E15 | 原lease存在、受管窗口索引丢失/错单位/consumer/版本；或索引有token但原lease缺失 | 完整性/Unknown并拒绝新承诺或安全停进度；不得从缺记录推断释放，也不得从索引补生一张原lease。迁移/legacy处理由root明确，不自动重建假窗口 |
| E16 | 原Reservation→Actual提交前写盘失败 | 可见根仍为完整旧状态；重启从原完整根恢复；不同时拥有“已扣Reserved但Actual未记”的半状态 |
| E17 | 原提交成功但调用者未收到返回；热receipt、checkpoint归档后重试 | 原原子结果一次。查询原完整JournalRecord/原payload/actor，不再分配token，不重复收费/交付/释放；目标已释放也不先Missing而跳过原票 |
| E18 | 反复checkpoint/reopen，分别落在E03的Reserved、A Actual/B等待、A已退出、B Actual、B已退出阶段 | 所有原token/状态/承诺/consumer/单位钟版本及剩余责任一致；冷恢复不重跑过去的进入/退出事实、不重发人口/容量 |
| E19 | 原拒绝结果、不存在的operation、错base、跨scope重试 | 原拒绝保持原拒绝；缺票不承诺安全重做；不能将store sequence/model revision/clock tick/operation_id相互替代 |
| E20 | 闭城/故障/撤许可/缩容发生在A Actual/B Reserved之后 | 阻新但保A和B旧责任；必要退出仍有路径。若当前Actual超新有效容量，报告超额/恢复义务而不截断counter/删lease。具体动态Action未实施时只能列待验，不能宣称已接通 |

**计数待定须显式暴露**：在E02中“两个原Reserved lease各2人、窗口相邻”是要求保留的事实；若继续把两个units全加成原当前reserved=4又以capacity2拒绝，可能无法满足新窗口功能。若改成峰值/切片，则须root定义混合旧/新租约、查询、事务不变量及恢复校验；不能用本文件或某个旧原型输出默许改变所有原池语义。本文不检查尚未写的实现以替它选择答案。

## 7. 容量、元数据、工作区满额：独立结果不依赖省略责任

- 原资源16行/lease64槽仍是当前原池基线；新token索引/别名/consumer/窗口条数及生命周期保留额度须root另定并显式计账，原型256/1024不能当授权。
- 分别构造原物理窗口容量不足、原lease槽满、token metadata满、alias/consumer登记满、原receipt/持久化空间不足、准备/恢复scratch不足的单一失败；新承诺不得留下半份lease或孤立索引。
- 用已存在A Actual、B Reserved/等待作为每个满额场景的固定旧责。拒新/未解后两者仍可查询；取消B及认证退出A的责任不能因“只剩新请求配额”而被忘记。
- 已经发生的必要游戏事件必须进入原可靠保全/停进度边界。不能因为当前计数/工作区求解未完成而把真实占用宣布不存在，也不能绕持久提交发布不可恢复的新结果。
- 额度耗尽、覆盖不足、计算未完成分别是Quota/Unknown/Unsolved或等价显式结果，不是“已证明真正物理短缺”；不得通过拆consumer/alias增加总额，不扩原normal64MiB/common8MiB/Engine上限。
- 终态索引何时归档/释放必须依原receipt/恢复/责任保留策略，不能按时间定期清空后让同旧event重新发容量。保留身份/来源不等于永久复制全部历史到热内存，具体持久索引由root定稿。

## 8. 给root的独立验收顺序与明确Unknown

建议由root依其既定串行窗口实施验收，不是本文件执行命令：先E01/E02的有限literal窗口算术与同池关系；再E03的真实事件序列；再取消/错scope/单位与钟版本；再逐阶段原Store热/冷恢复和原票一次；最后满额/工作区失败及旧模式回归。原输入、每步原Envelope/活动来源、前后根/原lease/索引、原receipt和失败恢复都须保留。不得只用字符串日志/receipt数量或合成true证明actual业务。

本轮准备完成时仍明确Unknown：

1. 新模式聚合字段定义和Actual对后续未来预约的策略（root/原advisor待定）。
2. 可用于该共享入口的真实canonical注册、合法单位profile、clock_kind/version、未来覆盖、owner/operator/许可及精确原人口partition绑定；本文符号输入不提供认证。
3. 改期/优先/清场时长/过期预约责任和退款应计；不能从容量门补造这些业务合同。
4. 新索引schema、配额、旧档迁移、原池事务/旁路封堵、System原活动事件权限及冷恢复实现是否满足上述预期。
5. 任何新实现的实际测试、ABI/序列化/stack/RSS/GPU/性能/磁盘峰值、M0/四速承载；本轮全部not_run。

交付后停写。没有新增实施线、没有宣称新版计数合同或产品验收通过。
