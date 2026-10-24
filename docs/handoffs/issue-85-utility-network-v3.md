# #85 V3 有理固定路由计量：有限源码候选交接

日期：2026-10-25。当前阶段为原 owner 协调、复用原 worker 实施，root 后续统一审查/串行运行验证。没有修改共享核心、发布或回填 Issue；本文件不代表公共设计已采纳。所有编译、测试、格式化、脚本、程序、基准/GPU运行验证均 **not_run**。

## 固定身份与原件

- 独立树：`/Users/frankqin/.codex/worktrees/issue85-utility-network/MyCity`。
- 原分支：`codex/issue85-utility-network`；固定 HEAD：`31dd72a904bbc987e3fbae59801a90e5f13eaf93`（v147）。未 rebase/reset/重建。
- common repo：`/Users/frankqin/dev/MyCity/.git`。root 当前 v183 / `8295ad1` 是共享实施背景，不是本树基线。
- 新增前五原件按原相对路径完整保留于 `/tmp/mycity-issue85-v3-prior-20261025.DnXHWi/exact/`，另保留前批原树和 `/tmp/mycity-batch4-module-delivery-20261023/issue85-prior-originals/`，不以 hash 代原件。
- V1/V2 主源码、V2子模块、旧测试、旧 fixture/旧 handoff 全部冻结不改。V2 的公平 carry/4096下降试探性能仍未运行验证，V3 不删除或追改其结论。

## 有限策略与输入契约

本门仅单一资源/1 source、至多4个完整 hard consumer、4条有向边的固定路由。同一个共享 source/edge 只计算一次汇总需求和容量，不给每 consumer 独立复制 sourceBudget。合法优先级由原 owner 提供；不新增全网分级器、产业、时钟或容量池。整体硬请求集不可行时不伪造部分完成；未解回原必要任务屏障。

V3 独立 policy/input/output 版本，尺度显式为 **milli-native**：`1000 mWh=1 Wh`、`1000 mL=1 L`。与原 `World.Quantity` 在原 profile 明确的电/水 milli-native 尺度下一一对应；与 V1/V2 原生整数 Wh/L 不兼容。旧 candidate/checkpoint 禁止静默导入或通过改标签迁移。source/clock/scale/quality/profile/拓扑/法定优先/开闭变化都切段、版本拒旧；真正迁移须 root 保留原 meter、余数、责任和原票，不填零重建过去。

稳定 uniform `Rate { numerator, denominator_seconds }` 表示实际连续物理量/秒。端点、source/edge cap与硬峰也用明确有理速率；连续 rate 与整数 meter 刻度分开，不能以结算 floor/ceil 改变功率或流量。

| 来源参数 | 精确 V3 rate（milli-native/秒） | 总量 |
| --- | --- | --- |
| 警务1h电 | `100000/3600 = 250/9 mWh/s` | 100Wh |
| 警务1h水 | `10000/3600 = 25/9 mL/s` | 10L |
| 医疗1h电/水 | `100000/3600 mWh/s`、`20000/3600 mL/s` | 100Wh / 20L |
| 消防1800s电/水 | 各自 `1000000/1800` mWh/s、mL/s | 1000Wh / 1000L |
| root实际24kW机组输出上界 | `400000/60 = 20000/3 mWh/s` | 60s内400Wh，不是 opening_wh 储能 |

上述需求源为现行 `docs/minimum-operating-parameters.md` 的有限任务参数；本模块不自行认证真实设备 duty/峰/现场条件。需要不同 duty 或 peak 必须由可信 owner 显式提供/换版，不能把100Wh集中100s或把小率取整为0/1 Wh/s。

root `ContinuousGensetV1` 提供原 Fuel 1Q与冷却60L/60s，转换为电400Wh、本征Heat600Wh、WasteWater60L，aux显式0，未接收电转明确Heat终点；无负荷总热峰60kW。**本模块不实施或认证这套生成器。** 它只消费 root 认证的同窗电输出边界；fuel/冷却/费用/basis、WIP及实际热/废水 sink 全由 root 同 Prepared 提交。不能拿原初电力 Stock、机组铭牌或未完成任务来填实际供给。

源必须绑定原 source/stock/contract、world/branch/root、Development(city)、provider/source版本与真实覆盖窗。来源 ID 非零只是结构完整，不是认证；缺失或跨 clock/缺窗口时 unknown/拒绝，不判真实短缺。电源类型与储存水显式分离，不能把库存电伪装非储存网供。水还需原质量/用途/接入和实际剩余量。

## 精确累计与整数结算映射

物理积分使用约分和 checked u128 有界共同分母，不用浮点或无限增长的分母。公开/序列化 Amount 使用 `{whole:u64, rem:u64, den:u64}`，`Checkpoint.common_denominator` 也为u64；不对外持久化超u64的u128 numerator，避免经JSON Value中转丢整数精度。编译D必须包含 source实际电源 `20000/3 mWh/s` 的分母3，以及所有连续rate/硬峰/路径loss/aux所需有界LCM。root #86的Config冻结同一 `reception_denominator`，消费分数与机组接收/Heat分数使用同一精度域，不得把floor meter差当physical peak或把消费分数dump到Heat。

先计算 uniform 物理需求、逐路由损耗、source辅助与共享cap，再按相同固定率对时间解析积分；与同一稳定输入下逐秒精细参考等价。只有整个实际请求窗已完成且所有硬门满足，才能产生对应候选；预算不足或不支持精度不产生伪交付，也不能越过必要工作游标。

物理累计 meter 保存 exact whole/remainder/immutable denominator；每调用返回新旧累计差，恢复不归零。物理 source gross、terminal、loss、aux 必须精确守恒。整数 `Quantity` 结算投影与物理计量分别输出：若 source累计 ceil预扣，则明确保留 `ceil×D−actual_numerator` 的未转化/预扣余量；若 terminal floor记整刻度，则余下实际分数也必须保留。差额不能藏入loss或辅助，不能多发物量、金额或权益。每个整数投影与其 exact meter 的已知差小于1 milli-unit，物理守恒本身不允许该误差。

对于连续电，预扣余量是原结算核对/托管责任，不是新增可花费库存或跨窗电池；绝不能用它放宽下一窗实际 source rate、硬峰或真实生成窗口。root 必须区分 exact 物理转化与整数结算映射，保留未转化量的原归属，不能把预扣余量当已发电送到消费者或热 sink。真实物理供给始终由同窗生成原事实覆盖，不能借未来源补当前交付。

稳定输入下同可信 base、同完成 cursor 的整段/任意整数秒切分结果须相同；source/debit/meter/consumer身份不随查询或行排列改变。硬变化精确切段并完整封存，纯 API 不全局撤销旧候选或授权新 opening，root 原票/版本门负责原子封存与去重。

### 稳定合同与每次真实供给窗分离

稳定合同可定义最长3600s的uniform profile/合法版本/有限总上限，**不表示未来3600s已产生供给**。RootVerifier分别核稳定contract、原授权opening以及本次半开 `[from,until)` 的实际 supply witness。候选需携带原source-window引用/digest及本次被认证的exact gross量；该witness应由root原实际来源/Prepared产生，不是本模块自签证书。

例如机组按60s提供实际来源，先处理 `[0,60)`，下一次只有新 `[60,120)` 的真实witness成立才续同State；相同definition/source版本和稳定horizon不改变fingerprint，不能每60s重新opening、重置预扣余量。真正policy/source/clock/scale/quality/rights变更仍切段并拒自动迁移。旧 `[0,60)` witness不能覆盖之后的窗。缺witness是unknown/未解，不是fixed-route真实短缺；也不能提前填“未来一小时已经发电”。

root witness可认证已生成量中分给本计划的精确部分，但它不是新可花费供给池；其他消费者、实际散热以及必要投入必须在同一原Prepared核总量与源唯一消费。电source的原stock引用须由root解释为真实生产投入/来源绑定，不得自行解释成初始电库存。连续源预扣计量余数不创造下一秒物理电量，即使有正prepaid，当前source关闭、缺实际witness或cap不足时也不能继续工作。

## 当前实施文件与验证边界

本次仅新增 `crates/city-core/src/utility_network/rational_v3.rs`、`crates/city-core/tests/utility_network_v3.rs`、`tests/fixtures/issue-85/v3.rs` 和本 handoff；独立测试用 path inclusion，不改旧模块入口或 crate lib/Cargo。root 接线前需自行导出新模块，不能把独立测试入口视为公共库已集成。

V3采用的有限上界：输入rate numerator≤10¹² milli-native，rate denominator≤3600秒（构造时约分，接线须使用合法规范值）；ratio各分量≤1000且非零retention∈(0,1]、辅助∈[0,1]；预先编译共同D≤10¹²，超过即Precision/error，不用临时无限LCM。窗口≤3600秒、1source/4consumer/4edge/每path≤4；MAX_WORK=256是有界解析步骤的调用准入，不是耗时/吞吐承诺。原Quantity映射上限为非负i64，所有中间乘加/比较/分母构造使用checked u128，超过整数/精度资格不会降成浮点或被称真实短缺。

root可在opening及真实生成之前读取V3编译D，冻结#86 Config.reception_denominator；getter只是结构/精度检查，不产生源认证。统一精度后，警务1秒物理电为 `{whole:27,rem:7,den:9}` mWh，terminal整刻度27但7/9不丢失；source ceil投影28但预扣余量2/9不得当额外真实供给。消费1秒水为 `{whole:2,rem:7,den:9}` mL。root真正送入#86 Reception的是exact物理量，不能用整刻度或ceil投影替代。

root唯一共享writer拥有 Engine/World/Stock/Units/资源lease/账/Timeline/Store/schema/receipt/版本、真实gen provider/消费adapter。#57原owner仅写service_needs，#86原owner仅做其原连续物理候选；本任务不改它们。后续原Prepared必须同批提交 fuel/冷却/实际供电/消费者水耗/热和废水实际sink/费用basis/meter余量/工作秒与原票。纯 candidate 非真实已交付，组件交接不解除provider/集成/恢复门。

未来仅 root 在既有 HP P900唯一target/work合法串行窗口运行验证；本树不创建target、缓存或测量目录，不启动验证、不抢Issue回填/关闭/PR。V3只做有界率与固定路由计量，不承诺全网吞吐、GPU或人口规模。

## 实际公共接口与接线责任

- `Contract::{validate, fingerprint, precision_denominator, authorize, opening, restore, propose, revalidate, seal_at_change}`：校验有限输入、固定身份和精度，建立/恢复累计状态，提出整窗候选，重算候选和精确封存。
- `Rate::new`、`Ratio::new`：构造规范有理数；`ExactAmount { whole:u64, rem:u64, den:u64 }` 表示物理数量，不是金额或费率。
- `State::{cursor, checkpoint, digest}`；checkpoint保存policy、合同fingerprint、原`genesis_ref`、绝对cursor、已过秒数sequence、固定D和规范排序物理meters。恢复的`expected_digest`必须来自root独立可信持久化，不能从同一待恢复输入自算后充当认证。
- `RootVerifier::verifies(&Contract)`认证整个合同及其中的源、路由、质量、用途、rights与sink；`verifies_opening(&Contract, genesis_ref)`只读认证原opening，恢复及所有消费已有State的入口都重新检查，明确撤销后拒绝继续；`verify_supply(&Contract, from, until, physical_gross) -> Option<SupplyWitness>`认证本次真实来源。纯模块不访问原Stock、provider或全局原票池。
- `SupplyWitness`携带`source_window_ref/source_ref/from_second/until_second/physical_gross/receipt_digest`。root负责原票唯一消费与跨计划总量；非零ID与digest形状不等于真实认证。
- `Candidate`携带`base_digest/from_second/until_second/next/consumer_deltas/source_delta/source_witness/edge_deltas/work_used`。消费者的`physical`与`credited_milli_delta`分列，源的`physical_output/physical_auxiliary/physical_gross`与`source_meter_ceil_delta_milli`分列；各自前后分数/预扣余量显式输出。源ceil投影不是fuel debit。
- `Proposal`分别提供`candidate/evidence/pending_reason/work_used`；`UnresolvedReason::{WorkBudget, MissingSupplyWitness}`不产生实际短缺结论。容量证据只适用于已认证输入的这个固定源/路由；精度、溢出与无效认证返回error。

源`physical_supply_rate`是自身aux之后、边损耗之前的出口率；源input cap/peak包含aux。边cap/peak明确按该边输入端计算。消费者独立contract、payer、meter与接入rights由root认证，不强制等于source合同。共享边/源汇总完整hard obligations；重复physical引用、stock别名及未使用的声明边拒绝。

## 测试源码与验收边界

新增测试源码共20项，覆盖警务/医疗/消防字面总量，机组`20000/3`精度，整段/多切点/3600个1秒推进与恢复等价，水1秒余数，损耗+aux精确守恒，共享边容量/峰值，输入排列稳定、rights/clock/scale/source/拓扑/需求/peak变更拒旧，checkpoint逻辑篡改与摘要匹配但累计历史越预算的拒绝，opening撤销后restore/propose/revalidate/seal拒绝，源累计总预算，计算额度不足及缺witness未解，旧票不能覆盖后窗，以及正预扣余量不能放宽后窗关闭/零容量。fixture verifier是合成认证，不是root真实provider或真实consumer运行证据。

两条非零损耗边各retention `2/3`、aux `1/5` 的字面例：terminal累计1 milli，source出口`9/4`，路损`5/4`，aux`9/20`，gross`27/10`，ceil3与未转化余量`3/10`分开保留。它验证物理算术约束，不认证真实设备或费用。

所有新增测试均仅为源码，编译/测试/格式化/性能/恢复程序/真实Prepared集成均`not_run`。root仍须完成：新模块导出与版本接线、真实可信verifier、#86共同D、同Prepared的输入/物理输出/消费/热废水sink/费用basis/工作秒/原票/余数原子提交、Store恢复及全局去重，以及授权串行窗口中的构建与真实测试。

## 本轮窄审与冻结记录

复用原worker与原advisor。advisor首次窄审指出恢复未认证genesis、匹配digest且表数自洽但越预算的历史仍可恢复两项阻断；原worker修复后，原advisor仅复核这两处delta与反例，结论为“两项阻断均已在源码层解除；本次窄复核未发现阻断V3有限源码候选交接的问题”。该结论不表示可编译或运行通过。

修复位置：`rational_v3.rs:503`重验恢复genesis、`:513–515`拒非起点不可行历史、`:604–617`统一检查已有State；`utility_network_v3.rs:242`构造预算1000但60秒累计`1666+6/9`且digest匹配的反例，`:263`覆盖撤销opening后四个入口拒绝。起点合法零计量可以建立必要屏障，不伪造历史消费。

最终只读Git记录：分支/HEAD保持上述固定身份；九个untracked文件为旧五件加本次新四件，没有tracked修改。旧五件与`exact/`原件逐一`git diff --no-index --stat`结果均无差异、退出0。新四件逐一`git diff --no-index --check /dev/null`未报告空白问题，新增文件的差异退出码为1。仅此静态检查，不能替代构建或测试。未提交、推送、创建PR或回填Issue；V3回填和运行门由root统一负责。


## Root v0.5.184 有限实际验证回填

原 owner 的历史 not_run 交付记录保留。本次 root 原 advisor 源审/窄核与串行结果见
[固定联合门证据](../evidence/continuous-service-supply-20261025/STATUS.md)。V3组件20、
连续机组组件24、#57派遣状态组件22通过；原账联合供给9及Store恢复也通过。
仅#57组件验证，不代表真实服务履约；仅固定原账供给，不代表旧V1/V2、开放网络、
SOC/爬坡或卫生认证已完成。完整Issue保持open，无PR；root统一版本/发布/回填。
