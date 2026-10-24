# Issue #86 电力调度候选交接 — 2026-10-24

状态：**未提交、未编译、未执行验证、未接线；Issue 保持 open，本阶段无 PR。** 原 advisor 第八批单轮只读初审结论为 `finite-source-handoff acceptable`：未发现有限源码交接阻断，不代表执行通过或功能完成。本文件只细化本专属候选；不表示公共设计、EXEC 登记或原 writer 已接受。

## 原 advisor 审计回填（2026-10-24）

已读取原报告 `/Volumes/HP P900/dev/MyCity/work/takeover-audit-20261024/ADVISOR-BATCH8.md` 的 #86 结论。原 advisor 确认：有界源/储能 stock 去重、累计计量余数、SOC 不静默销毁、未解时保留完整前缀、备用与实际供给共受率/爬坡/库存限制，以及源 debit、储能、辅助、弃电、交付和损耗闭合，未发现阻断有限源码交接的明确缺陷。

真实源库存/SOC、跨段表数与比例保持、#85 网络重验、实际出口、原子提交和原票去重仍 pending；范围只是既有电能有限调度，没有真实燃料发电、启动或备用履约。未解不免除安全辅助和必要供给责任。全部执行验证仍 **not_run**；独立测试源码不等于通过，资源估计不等于预算或性能验收。本结论仅适用于 #86 有限源码交接，不外推为第八批整体最终审计或整批收口。

本次只更新本 handoff 与已有 #86 Issue 回填；模块和测试保持冻结，不执行编译/测试/fmt/脚本，不 commit/PR/清树，不联系旧主会话。后续由已接管的总协调统一处理。

## 来源与文件边界

- Issue：<https://github.com/TekesApps/MyCity/issues/86>，已读正文及空评论；U01 / REQ11、REQ28。读实际 repo AGENTS、`docs/product-utilities-environment-and-safety.md` §1/2、`docs/town-boot-electricity.md`、`docs/regional-probabilistic-decisions.md`、当前派工入口；#85 handoff 和 V1/V2 源码只读。
- 固定基线 main v0.5.158 / `ae28b8cda8041c7bd106a28cccdc3deab051b032`。
- 专属树 `/Users/frankqin/.codex/worktrees/issue86-electric-dispatch/MyCity`，分支 `codex/issue86-electric-dispatch`，common repo `/Users/frankqin/dev/MyCity/.git`。
- managed create_worktree 在项目入口因 invalid reference 失败；按授权从 actual repo 建立专属标准 Git worktree。不重复登记、不 reset/rebase、不复制 main WIP、不改项目配置。
- 本候选全部为 untracked 原件：`crates/city-core/src/electric_dispatch.rs`、`crates/city-core/tests/electric_dispatch.rs`、本 handoff。小合成 fixture 内嵌独立测试文件，无额外 fixture/依赖文件。收取必须包含 untracked，不能仅看 `git diff`。
- 所有共享根、清单、lock、Engine/World/Timeline/Store/receipt/Boot/air/IPC/worker/allocator、其他模块、共享设计和全局版本均未改。无 commit/push/PR/tag/发布/关闭 Issue；不停止进程、不占 target/work、不盘点或清理测量产物。

## 实际实现及精确边界

`POLICY=1`，确定义务与物量采用确定性精确算法，无概率模型或随机身份。复用 `units::Energy` (Wh)、现有 serde/SHA256 依赖和 #85 的原生 Wh/秒单位约定；不另造通用网络/权威能源池。`SourceState.used_wh` 是原源库存的候选累计 debit，不是可花费余额。真实库存仅原 writer 管理。

API：`Request::{validate,opening,restore,checkpoint_digest,propose,revalidate,validate_actual}`，`Battery::meter`；输入 `Provenance/Source/Battery/Ratio/Mode`，状态 `Checkpoint/SourceState/BatteryState`，输出 `Candidate/BatteryMeter`，原 writer 计量对照 `ActualBatch`，错误 `Unknown/Limit/Invalid/Overflow/Stale/Infeasible`。没有文件加载器/Deserialize、事务提交器或定时器。

有限源只接受**期初已存在或已交付的电能**以及明确来源证明，不计算煤、燃油、风、光、水等发电产出，不消费将来燃料产出。这里的 source rate 是有限已存在电能缓冲的可放出 envelope；真正机组转换、启动燃料/水/残余物和安全退出仍待 S07 原 owner。不得把本 DTO 当真实机组 profile，不能靠铭牌或虚构现实参数生成供给。#96、#36 与 #85 都是未验证依赖，不能当真实 provider。

源约束包含有限 opening Wh、上一原秒 gross rate、gross max、每原秒双向 ramp、不可安全停最低 gross rate、同资源本地 safety auxiliary、签约 reserve rate/持续秒数/合同引用。每秒先确定 `max(must_run,last-ramp,aux)`；上界同时扣备用 rate、备用爬坡空间，并从剩余电量保留 `reserve_rate*reserve_seconds`。备用只是能力保留，无交付/销售/收入，后续每秒不能侵占该保留量；reserve 资格/不可撤销预留及响应实际授权仍由原 writer 校验。这里保守要求备用可在一秒 ramp 内响应，不推论满足真实备用服务。

源基础出力先满足不能安全停及辅助，剩余按 `(contract_priority,incremental_cost,stable id)` 补 export/充电请求。强制最低/缓慢停机引起的过剩记 `curtailed_wh`；弃电无钱字段，不算收入。安全本地辅助单独记账，**不得在 #85 再重复收同一辅助**。源/储能 stock_ref 跨集合禁止重复，防止同一库存包装两次。充电原输入由同秒真实有限源供应，不将同秒新充电作为放电源；全请求及历史同 5min 窗禁止混合充/放（含跨电池），是保守禁用相互转充，不声称最大化套利或最优出力。

`export_rate` 是 #85 网络已解释的**上游输入需求（含其独立线路损耗）**，不是下游已交付需求。`network_contract_ref/version` 和非零 plan digest 只标识，不认证线路可达、连接权、边容量或 losses。这里没有第二个 network solver；必须让原 #85 adapter 对匹配 world/branch/scope/root/version/clock/cursor 的固定源、上游输入、实际终端、边峰、损耗终点进行完整重验。当前 #85 候选不在固定基线且未导出，本候选与它**没有可编译直接依赖或真实接线**，只复用单位和分工契约。若 #85 无法消费本候选量，不得提交为真实供电。不能称本模块解决了线路瓶颈。

`ActualBatch` 对照每源 gross debit、各电池充入/放出累计差、local auxiliary、curtailment、区间和 `delivered+network_loss=export delta`。它只是**金额以外的数值校验**；非零 network digest 不证明网络计划有效。收费/成本/basis/权利/真实 meter/去重/电量出口仍走 S05/S07/S08 与 Boot 原分类出口，同一个 Prepared 原子提交。若真实可行网络只能交付另一组量，应回到最新可信来源重算，不可任意缩放 candidate 后自称已通过。

## 储能精确计量、退化与跨段来源

输入必须含 initial_soc_receipt、明确 nominal/current opening capacity/opening SOC、充/放效率、每等效满循环退化比例、充放硬率、认证的 lifetime charged/discharged meters、当前 5min mode window 与窗口充放历史。没有来源 receipt、效率分母、完整覆盖/age、同域身份则拒绝，不默认 SOC=满/空或免损。所有单位为整数 Wh；Wh/秒不是 W，不隐式把 W/kW/MW 填入。原 adapter 做显式精度和单位转换，当前无亚 Wh 分配。

累计 `stored=floor(charged*n_c/d_c)`；累计 `debit=ceil(discharged*d_d/n_d)`；SOC 从认证期初 SOC 加 stored 累计差、减 debit 累计差。charge/discharge loss 为对应 lifetime 计量与 stored/debit 的差；charge remainder、discharge ceil surplus 连同分母可核。每次按累计差消费，绝不对每次调用 delta 重舍入。

退化参考明确采用电池 SOC 侧充放 throughput=`stored+debit`，等效满循环 throughput/(2*nominal)，容量损失 `floor(throughput*fade_num/(2*fade_den))`，新段仅扣期初累计 fade 之后的差。本实现选择线性损失额定容量（不是逐循环复利），参数须 profile 明确，不发明现实电池参数。退化余数分母为 `2*fade_den`。SOC 不得超过退化后可用容量，超出就该秒未解，**不静默销毁电量**。没实现溢出电量安全泄放/处置或更换电池。

跨段必须从原已提交状态保留 lifetime 表数、余数所需固定 ratios、当前 SOC/可用容量、源剩余库存、上一 gross rate、同窗模式历史及全部未解责任。`opening` 本身不授权新期初；由原 writer 认证新段所有 opening 字段，并禁止同原 stock 重发额度、丢失历史或更换 efficiency/fade ratio 后重解释旧表数。修改 ratio 的迁移/换包协议未实现，必须拒绝接线直到原 owner 明确迁移。测试展示 literal meters 的人工认证交接，不是自动迁移或 Store 恢复。

## 原 clock、事件、切段、等价与义务

沿原 S05 的 game clock/Timeline，不创 clock_kind。`clock_ref` 必须绑定真实 owner 时钟；每游戏 300s 边界与法律/合同/需求/供给/天气/设备/拓扑硬变化触发原任务。稳定段 <=300s，不跨 5min 窗；非整窗结束必须有 hard_change_ref，provider 必须证明边界前稳定且全部时窗覆盖/age 有效，不能平均硬峰。整数秒是本参考表示域，亚秒变化未实现，禁止舍入跨越已发生硬事件。运行中提前事件不能覆盖原 Request；writer 必须封存实际 cursor、保留已发生状态，并原子废止旧未来候选后建立新段。**本模块没有自动封存/撤销全局候选的能力**。

精细参考是原游戏每秒一个不可分 atom，固定 rates/modes/profile 逐秒计算；并非另一个调度器。提案可到指定 cursor，按完整 atom 发布前缀。work cost=每 atom 的 `1+sources+batteries` 抽象单位，含未完成不可行 atom；每调用≤4096。验证/序列化/hash/sort的常数开销不等于这些 work 单位，未测 CPU 时间。额耗尽、不足源/爬坡/SOC、无可行完整秒均只 `unresolved`，不输出真实短缺证据；某秒失败的全部试探/电池修改不发布。必要屏障不能越过未完成 cursor。

固定完整 Request、相同可信 base、足够 atom 额度和相同目标时，整数秒顺序决定唯一状态；任意 query 切点计算的 next 相同，各累计 meter 差望远镜相消。查询次数、worker 数、现实四速不进入公式。行重排会改变 fingerprint/状态索引，必须重建经认证映射；不支持静默换序续算。容量变化等必须新段，不能宣称跨未认证版本自然等价。候选没有概率或随机游标；cursor 是已完成原秒，request fingerprint+policy 绑定轨迹。

`restore` 校验独立可信 digest、fingerprint、policy/cursor/条数、源/储能基本算术。digest 必须从原 Store/root/操作票独立认证，不能把 DTO 自带 hash 回填为可信证据。`propose` 内自 hash 仅结构检查，不是认证；只能给可信 base 调用。`revalidate` 重算完整 candidate，旧 base/版本/篡改量拒绝。重复同 base 查询有同候选，不是全局幂等；lost reply 必须先返回原票，不能再次 debit/付款。

已发生供电、已接受合同、必要安全辅助/尾停在暂停/闭城、未知来源和额度耗尽时仍归原责任路径。原区域交互钟履约不被这里恢复成闭城内生发电；没有 provider 就保留 unresolved，不能清零 accepted obligation。原 writer 必须认证 protected_obligation_ref 的完整集合，本模块单一引用不替代合同责任账本。这里没有自动部分安全交付：如某秒可行性失败，本秒不发布任何新量，安全责任交回原必要任务处理，不表示允许中断安全设备。

## 测试源码及未验证范围

测试文件内有独立 literal oracle 与小合成 fixture，没有读取生产数据；**全部 not_run**：

- 10秒：100Wh gross=80 export+20 local aux；备用同源留量/硬率/爬坡，不增加供给。
- 10Wh充电经0.9得到9Wh SOC；19Wh放电经0.95扣20Wh；7Wh放电的 ceil surplus=12/19，分段/可信 digest 恢复后相同。
- 20000Wh throughput、1/5000每满循环得到2Wh容量损失；SOC超容量拒绝，无免费销毁。
- 零SOC/零期初库存、ramp-down强制过剩、合同/成本/稳定ID、reserve共池、0/少额度与完整前缀。
- 错 world域/branch、过期不覆盖、未知SOC来源、重复stock、未知比例、跨5min边界、溢出。
- candidate改量、旧base重放、可信digest不符、network版本变化，实际量/线路损耗闭合及未知network digest。
- 硬变化转交累计SOC/余数/模式历史；旧checkpoint不能自动用于新fingerprint，同窗由放转充拒绝；硬率不足不以平均满足。

当前只存在测试源码，**不证明能编译或通过**。无真实 provider、原事务原票、失败 Prepared 回滚、lost-reply幂等、磁盘冷开、网络线路联合重验、真实燃料发电启动、备用履约收入、真实设备/CPU/GPU/吞吐/精度误差/四速或整城电网验证。需原 writer 接入后补上述真实集成/恢复测试。候选政策是有限保守参考，无优化策略/误差实测；不能自动选取更快近似策略或扩大预算。

## 最小共享接线请求（待总协调决定，尚未采纳）

1. 原 owner 导出 `pub mod electric_dispatch`；本树不改 lib/Cargo。当前独立 integration test 以 path inclusion 读取本文件并复用 crate 的 units。
2. S05 provider 提供全域 provenance、实际源库存/电池表数/模式历史与必要义务、绑定 #85 严格同域网络输入。#96/#36不可凭未验证 DTO 当真实来源。
3. 原 Timeline 注册每5min/硬变化的既有必要任务/屏障，按 cursor 推进。中途硬事件封存/撤销旧未来候选必须原子完成；源/储能跨段 ratio变更在明确迁移前拒绝。
4. 原 S05/S07/S08/Boot writer 在一次 Prepared 重读源和权利，重验电能候选、#85网络、actual量与金额/basis，原子提交 debit/损耗终点/电池计量/退化/义务/费用/状态和原票。绝不新增第二库存或账本。
5. 接线后在合法串行窗口验证实际输入、cold recovery、lost reply、硬事件旧候选、退化/效率尾差/安全责任，再谈联合性能验收。

## 拟预算、持久化与保留（估计，非验收）

| 项 | 有界计划与未测点 |
| --- | --- |
| CPU对象 | sources≤16、batteries≤8、段≤300原秒、每调用work≤4096；每秒排序≤16；完整最大段可分多次求解，不声称ms时延 |
| CPU工作区 | 克隆单一有界checkpoint、rates/order Vec、校验set/JSON/hash；拟额外≤64KiB，分配次数/RSS/stack实际未测，不扩大normal64MiB/共同8MiB |
| 持久state | checkpoint源16×16=256B、电池8×16=128B，加policy/fingerprint/cursor和3计量，raw scalar约460B，序列化/Store envelope另计；输入profile、来源、mode历史及原票必须原owner保存。拟每scope当前profile+checkpoint≤16KiB，非实测 |
| IPC/cache | 没有IPC/worker或cache；未来若经IPC传递需原owner将同时输入/输出/编码副本列入共同预算。禁止缓存候选当库存 |
| 恢复双份 | 当前无Store schema/双缓冲文件；未来原加载器bounded decode，读取旧/新profile+state双份拟≤32KiB/scope，真实Store/WAL/receipt峰unknown |
| GPU | 无GPU、零上传副本，不申请GPU缓存 |
| 磁盘 | 本批仅三份UTF-8源码/测试/handoff，目标总<64KiB；无target/work/log/压测目录。基线checkout不是增量源码预算，未扫描全树大小 |
| 保留/回收 | 完整专属树与所有untracked留给总协调/原advisor；测量结束、writer停写、主线明确释放依赖、原件逐文件持久归档核验且重新核占用后，才由总协调删本批tree。不自行清tree/branch/sharedrepo/旧批/target/work，hash不能代替源码原件 |

只做轻量读源、git/status/hash和限定文件编辑；编译/运行/测试/formatter/测试脚本/benchmark/压力/GPU/正式测量/产物盘点清理：**not_run**。原M0负载/normal64MiB/共同8MiB/四速/8h/真实16GB门不变。

未来最小命令，**仅供原协调在测量自然边界确认HP P900挂载及唯一target/work后排程，不执行**：

```sh
cd /Users/frankqin/.codex/worktrees/issue86-electric-dispatch/MyCity
CARGO_TARGET_DIR="/Volumes/HP P900/dev/MyCity/target" cargo test --locked --offline -p mycity-core --test electric_dispatch
```

本测试不写work；以后原Store恢复测试仍必须用原 `/Volumes/HP P900/dev/MyCity/work` 下获授权范围，不另建本地work/target。此命令之前先由原writer检查未导出模块及编译边界；不以这条未来命令的存在宣称测试运行。

# 2026-10-25 新实现门：ContinuousGensetV1（未审计、全部执行 not_run）

此节为当前增量，前文 2026-10-24 advisor 的 `finite-source-handoff acceptable` **只覆盖旧 opening_wh 有限电能候选，不覆盖本新增连续转换**。旧 `electric_dispatch.rs` 和旧 `tests/electric_dispatch.rs` 全文保持冻结，旧功能/测试不改。原树 HEAD 仍为 `ae28b8cda8041c7bd106a28cccdc3deab051b032`，没有 rebase/reset/复制 main WIP；root 告知的 v0.5.183 / `8295ad1` 是其当前集成上下文，不冒称本私有树基线。重新读 actual repo AGENTS 和 #86 正文，未重新分配共享文件 owner。

## 新接口方案与实际交付文件

仅新增：

- `crates/city-core/src/electric_dispatch/continuous_genset.rs`：单机组固定物理 profile、可 serde 的有界状态/候选/计量，纯 CPU 无提交。
- `crates/city-core/tests/electric_dispatch_continuous_genset.rs`：独立 synthetic fixture、literal oracle、反例/拆窗/冷 serde 恢复测试源码。
- 更新本专属 handoff。没有新增依赖、共享根/清单/lock/units/World/Stock/Action/Timeline/Store/版本/设计/Issue 改动；本门共享设计及 Issue 由 root 单一 writer 同步。

旧 parent 文件未加新 `pub mod`：其冻结 integration test 用 path inclusion 且只引入 `units`，新增子模块需要现有 `arena::Scope`；擅自把 child 导出进 parent 会改变旧 path test 的编译上下文。因此新专属测试直接 path 引入 child 并 `use mycity_core::{arena, units}`。**root 集成时可以在 lib 用显式 path 单独导出 child，或在 parent 导出 child 并让旧 integration test 复用 crate 导出/提供 arena 别名；本会话不修改冻结旧测试或共享 lib。** 新门还没有可从 crate 根导入的入口，不宣称已接线。

公共候选 API：

```rust
ContinuousGensetV1::opening(&Config, Instant) -> Result<State, Error>
ContinuousGensetV1::meters(&Config, &State) -> Result<Meters, Error>
ContinuousGensetV1::state_digest(&State) -> Result<[u8; 32], Error>
ContinuousGensetV1::restore(&Config, &[u8], trusted_digest) -> Result<State, Error>
ContinuousGensetV1::propose(&Config, &State, &AuthorityView, Command) -> Result<Candidate, Error>
ContinuousGensetV1::revalidate(&Config, &State, &current_view, &Candidate) -> Result<State, Error>
ContinuousGensetV1::confirm_actual_basis(&Candidate, &ActualBasis) -> Result<(), Error>
```

`Config` 固定 `Profile::ContinuousGensetV1`、VERSION=1、`arena::Scope`、city/clock、genset/source version、终身 `reception_denominator=D`、两个固定 InputLot、两个真实 sink 引用/有限量/峰与可选消费者身份。使用原 `units::{Quantity,Money}`；Quantity 此入口固定解释为 mQ / mL，输出计量字段明确为 mWh，不调用原 Energy(Wh) 做隐式小数换算。D须为3的正倍数且≤`MAX_RECEPTION_DENOMINATOR=3_600_000`，由root绑定#85 V3实际compiled共同分母；不直接依赖未验证私有模块。Config入场前拒绝不支持精度为`Precision`，不是短缺；不为换D清理余数。若来源/消费者/精度需变更，未实现seal/migrate协议时直接拒绝旧State接新Config。

`ClockBinding::{Development{city_id},World,Interaction}` 是本机组的可序列化**域绑定**，不是另建时钟；后两者在 Config/Instant 校验拒绝。这里未依赖私有旧基线不存在的 `task_registry`。root 必须从原真实 clock kind 显式匹配 Development 并原样带 city，不能把 Interaction seconds 或同数值 World time 填为 Development。Scope 的 world/branch 由现有原类型构造/反序列化验证；每个 view/reception/command 再核 scope/clock/city。

## 冻结物理合同与整数/分数计量

新 profile 并不等价于任何旧 Boot 配方/价格/库存域；aux=0 是本 profile 的显式常量，不推广到其他机型。固定额定每 60 **Development 游戏秒**：

- 原已存在 Fuel 1000mQ=1Q，燃料能量 1000Wh/Q。
- 原已存在冷却 Water 60000mL=60L。
- 电 400000mWh=400Wh，本征 Heat 600000mWh=600Wh，WasteWater 60000mL=60L。
- 电硬峰 24000W、本征热 36000W；未接收的电全部转指定 Heat，最坏热峰 60000W；废水峰 1000mL/s。

累计运行秒数 `t=State.active_seconds` 跨 start/stop/fault/restart/查询切窗永久保留，不以 run 重新计量：

```text
fuel_burned_thirds_mQ = 50*t                 # 单位 1/3 mQ，精确燃烧量
net_fuel_out_mQ = fuel_taken_mQ-fuel_returned_mQ
fuel_wip_thirds_mQ = 3*net_fuel_out_mQ-50*t
water_consumed_mL = wastewater_mL = 1000*t
water_wip_mL = water_taken_mL-water_returned_mL-1000*t
D = Config.reception_denominator            # 冻结共同分母，D % 3 == 0
E_exact = t*20000*(D/3)                    # 单位 1/D mWh，未floor的真实发电
R_exact = received_mWh*D+received_remainder
intrinsic_heat_mWh = 10000*t
dump_exact = E_exact-R_exact
heat_exact = intrinsic_heat_mWh*D+dump_exact
whole = floor(exact/D); remainder = exact mod D   # 只投影计量，不丢物量
```

每次 advance **先**要求原库存足够支付 `ceil(50*t_after/3)-net_fuel_out` 的正差，以及所需 Water 正差，进入 input WIP，再只转换精确燃烧量。不是把 ceil 入料都叫燃烧，也不把 0.01Q 视作 1Q 发电。库存数只在 root 原 Prepared 实际 take；这里的累计数是核算/候选，不是新可花费库存池。

完整能量闭合（同一 1/D mWh 尺度）：

```text
net_fuel_out_mQ * 1000 * D
 = fuel_wip_thirds_mQ * 1000 * (D/3)
 + R_exact + heat_exact
```

**发电分数已经且仅分到actual reception或Heat，不再另设物理输出WIP。** Meters发电whole/remainder只是同一真实输出的投影，不能既把该分数算“尚未发出”又计入接收/heat。State持久接收whole和remainder；heat/dump及其分数由相同Config/t/received精确派生，跨故障/重启保留。Fuel未转输入WIP仍为1/3mQ，Water净扣=Water WIP+已转换WasteWater，全部和cost分数分别明确。WasteWater必须由原sink实际接收，不仅增加无上限统计。

## 生命周期、真实来源与每 run 一次启动费

`opening` 只产生原未启动 State，不创造来源或授权。`Start {at,run_id}` 仅可从 Stopped/Faulted，run_id 严格递增且 `at>=old cursor`；每次成功 run_count+1，`Delta.start_fee=300` 分。只预装下一运行秒的现存输入（首秒通常17mQ Fuel +1000mL Water），不燃烧/发电。初启不额外虚构启停耗料或启动延时；预装仍受实际库存、付款、热/废水首秒无负荷容纳能力和硬峰门约束。任一门失败，返回 Err，传入 State 不变，无 fee 或 take 可提交。

`Advance` 仅 Running，`at>cursor`，每次最多3600原秒；一次完成全窗或返回 `Error::Unresolved(Constraint)`，**没有发布半窗或默默跳到 target 的行为**。root 可在有合法来源的明确切点另提短窗，不能把失败解释为真实短缺。每窗都使用同一累计 t、配置/来源版本、run；Delta.start_fee=0，运行60/3600秒或多次查询不会重收。

`Stop` / `Fault {fault_ref}` 必须精确位于已处理 cursor，不能用未来 stop/fault 跳过仍未解的过去发电/输入/安全责任。root Timeline 先按真实硬事件边界处理合法前缀，再把同 cursor 的状态转停机/故障。已烧燃料、本征热、既有接收/损耗和启动费不回退。

stop/fault 把 **whole unconverted WIP** 归回原 stock：Fuel 返回 floor(fuel_wip_thirds/3) mQ，Water 返回全部整数 mL；Fuel 剩余不足1mQ的 1/3 或2/3mQ及其 basis **继续在机组 WIP 中保留**，不能擅自四舍五入为1mQ返 Fuel、清零或变作已经烧掉。刚 start 就 stop/fault 可退原17mQ/1000mL全数及其 basis，但不退已经成立的300分启动合同。烧1秒后 stop，17mQ中50/3mQ已转换，余1/3mQ不够原库存 tick，当前无 Fuel 退款而持久保留分数。下一 run 复用此分数且再收一次300分。停机等待的 Development 时间不计 active_seconds，不补发电。

`AuthorityView` 由 root 原来源读，包含 Scope+Development、source version、read root/version、完整区间 coverage/observed/max_age、现存可用原料/到达时点/Materialization、payer实际余额、原 sink 剩余量。只接受 ExistingStock 且到达不晚于窗初；FutureProduction（即便数量看起来充足）拒绝。源、废水/热 sink 的4个 ID不得相同，不允许把自身本批输出伪装成启动 Water。非零ID/root、Materialization标签及 typed serde 都不认证来源；资格、质量、连接、产权、资源/付款和到达真相仍由 root 原 Authority 校验。

## 接收、sink、费用和 basis 的桥接门

`Reception` 绑定 Scope/Development、genset/source_version/run_id、consumer identity/version、原 read root、receipt_ref、精确from/until与 **lifetime cumulative actual received whole mWh+received_remainder_numerator<D**，root必须认证实际接收而不是预测需求。新接收精确量≤本窗新产出的精确量，不能把上窗已转Heat的电拿回出售。消费者为空时只能接收0；真正received电直接计原消费者实际消费，**不进入可花费电库存**。

actual_peak_w是原来源独立认证的窗内实际硬峰，必须≤24000W；精确接收delta在scaled单位校验 `received_exact_delta*3600 <= actual_peak_W*1000*dt*D`，**没有ceil/floor峰值容错**，更不能从whole跳数反推功率。若root不能认证峰及因果合法接收时窗，不能把聚合总量当已真实消费。100W连续实际接收每秒27+7/9mWh，分数不能错误dump为heat或被当成28mWh/s虚假峰。模块不认证连接/许可/服务质量，不声称#85联合电网已经通过。

Heat sink容量与free都为whole+remainder/D，容量比较在checked u128 scaled单位完成。Delta所有generated/received/dump/heat也是精确物理delta的whole+remainder；`Delta.whole_meters`另给累计whole投影的端点差，给root原整数tick计量桥对照。例：无负荷第二秒物理heat delta仍为16666+2/3mWh，但whole_meter_delta=16667，后者包含前一窗计量余数进位，不能误当真实额外发热/发电。若原Stock只存整数mWh，root需将累计heat的fraction作为原sink已接收分数保留，并核Stock whole投影；不能把每次物理delta的floor直接累加后丢进位，也不能把fraction再列为源输出WIP。全部持久字段为u64/原Money/i64，无u128 JSON值；中间checked u128。

未接收电转 Heat，与本征热一起扣原指定 Heat sink 的当窗剩余容量和 Config 固定 lifetime 有限 allowance；Water 同步扣 WasteWater sink。热至少60000W、废水至少1000mL/s的硬峰门始终保守保留，**包括有电消费者的窗口**；不因平均电量推断热已削峰。start 还要求首秒无负荷 sink 可容纳，防止已知无合法去向仍收启动费。每次 view 即便伪称又有无限free，也不能越过 Config lifetime allowance。输入/资金/sink不足只 unresolved，物理候选原 state 不改，义务仍 pending。

**basis 曲线支持域，必须由 root 核实：** InputLot 是本门新建独占、规范化初始 lot 的固定 `quantity Q / actual basis B`，按 `floor(B*x/Q)` 的原分摊曲线核对 take/refund；不支持把原非零offset、混合lot、补货或变更basis的原 stock 无声明地解释成归零新源。被 Boot/Town/Exchange/Construction 认领的 stock 禁用，Boot 6stock 独占守恒域不能扣。root 所拟新专属 stock 仍是原 World Stock，必须独占且正确初始化/维持其 origin curve；本模块不新建池。

按净已扣整数量计算 held_basis；Fuel converted_basis=`floor(B*fuel_burned_thirds/(3Q))`，Water converted_basis=`floor(B*consumed_ml/Q)`。WIP basis=held-converted，含尚未归入整数分成本的来源余数；对应 cost remainder/denominator 在 Meters 显式给出，不能拿300分启动费替代 Fuel/Water basis。take/refund 成本是同一原 curve 新旧净扣 prefix 差，跨查询/切段望远镜相消。conversion成本、启动费、WIP成本独立输出。

`confirm_actual_basis` 必须在 root 已执行本 Prepared 的实际 take/refund/fee 后比较其 Money 结果；不符返回 Basis，整批回滚。**如果归还原 Stock 时重置 origin 造成未来 take 曲线改变，或者实际原 Stock 原本不是该规范化曲线，本候选不能自动修正/强行通过，必须由 root 维护原 lot 身份或明确拒绝该接线。** 不允许通过修改实际 basis 来迎合 planner。当前不支持更换/混合/补货源版本；所有 WIP、carry、旧义务清晰迁移前，改 Config/source_version 会因 fingerprint 不同拒绝旧 State。

root已明确其canonical units.rs新`frozen_lot_basis_delta(origin_quantity,origin_basis,offset,take)`用于RootActual对照同一累计floor曲线，Boot冻结lot分支复用原规则而不改变nonfrozen路径，尚未编译。本旧树不导入/复制该新增helper；本模块独立数学候选仅与明确normalized原lot绑定。停机后的原lot/WIP basis完整保持，不能把可混pool当该源。

root 的一次 Prepared 应：读可信 committed Config/State/原票→绑定当前 AuthorityView→propose→认证实际消费/峰/合法接收与 sink→重读 current view并revalidate→实际原 Stock take/refund、启动付款与 sink/consumer记量→confirm_actual_basis→原子提交新的State、分数WIP、basis、费用/已转换成本、输出/接收/热/废水及原票。具体已有 Action/Timeline/Store API 由 root 唯一 writer 完成，模块不授予 CitySystem 权限。lost reply 先返原持久票，不能再调用Start或重复接受candidate。重复同base只得到同一个无副作用提案，不是系统幂等。

## 分段等价、恢复与失败边界

固定 profile/source/lot curve、相同真实 start/stop/fault 时点和 run_id、相同消费者 cumulative reception 轨迹、完整合法 coverage/peak/sink前缀下，t 以 active秒增加，所有输入、转换、输出、basis是同一累计函数；Delta按端点相减。因此1s/60s/3600s切窗到相同终点的 State/物理meter/carry/basis完全相同，不按查询次数收费或加入随机/事件游标。无概率抽样，原随机身份不改变。所有物理硬峰原样保留，未平均；原Timeline事件切点仍必须处理，不能跨 source/工况/资格/拓扑变化以旧Config求整窗。

State保留config digest/version、Development cursor、status、last run_id/run_count、lifetime active秒、累计take/return和actual received的whole/remainder。其余物理/费用/carry由固定来源和这些累计值精确派生，不需要独立易漂移的余数副本。序列化无Vec/String/无逐次历史增长；`restore`最多8192字节，严格未知字段拒绝、原Scope/Quantity/Money验证，并核独立可信state digest后重验物理边界。Config/原lot curve必须由原Store独立保留；DTO自带hash不能作认证。stop/fault不重新协商分母，不需要新sink准入，忽略无新增输出时与停机无关的freeHeat精度；沿原已接纳D保留全部余数/WIP。当前“冷serde恢复”仅纯测试源码，不是Store/WAL/断电原票恢复证据。

`revalidate` 对当前 view、base digest、command、全量candidate重算比对；改root/版本/起点/量/费用/状态拒绝。source clock即使seconds数值相等，World/Interaction或另city/branch拒绝。Fault/Stop后必须新run_id才能Start；Running再Start拒绝。start失败不改变原state；advance input/heat/waste不满足也不改变原state且不返回假短缺证据；未解不能解除原必要供给/安全辅助/到期责任。这里显式aux=0不取消任何其他系统已有安全义务。

## 独立测试源码（全部 not_run）

新专属测试内置固定合成lot与root receipts：Fuel 1分/mQ、水3分/L仅fixture成本，不是profile价格。测试覆盖：

- start预装17mQ/1000mL，零转换/零输出，WIP成本独立于300分；真实basis不匹配拒绝。
- literal1秒：burn50/3mQ，余1/3mQ；电6666+2/3mWh，本征热10000mWh，零接收总热16666+2/3mWh；Fuel converted16分/WIP1分，Water consumed3分；没有另加电输出WIP，全部按D=9精确能量闭合。
- literal60秒真实接收400000mWh：原1000mQ/60000mL，Heat600000mWh，废水60000mL，无电库存credit。
- literal3600秒无负荷：Fuel60000mQ、水3600000mL，电24000000mWh转热，加本征36000000mWh，Heat60000000mWh；1s/60s/3600s分窗完整State相等，整run只300分。
- 7秒serde冷恢复继续60秒相同，错可信hash/篡改/超长bytes拒绝；原State保留，不冒称Store恢复。
- 原料0/未来自产水/未到达/资金299分失败无副作用；10mQ不能启动，17mQ最多发首秒而不是整1Q周期。
- start立即stop/fault归还未用17mQ/1000mL及原basis；burn1秒后故障不退已烧量或fee，保留分数，读档→10秒restart→12秒时总active3秒，能量/余数不重置，第二run再300分。
- 热59999W/废水999mL/s拒绝，60000W+1000mL/s及真实容量可行；3600秒Heat容量差1mWh或废水差1mL拒绝，满后不隐式续容。
- 实际接收超本窗、旧已弃电再卖、超24kW/伪低峰、source/version/clock/scope/root/coverage/age、外来genset/run拒绝；旧base和改量candidate拒绝。
- 非整成本curve拆窗相同，成本held=converted+WIP；停止/fault必须精确cursor，不能跳过尚未处理区间；Water和自己的WasteWater重复ID拒绝。
- 100W消费者literal：1秒实际27+7/9mWh，dump6638+8/9mWh，Heat16638+8/9mWh，无误弃分数或虚假峰；1s/60s/3600s到整小时State相等，接收100000mWh、Heat59900000mWh。物理分数delta和whole_meter_delta进位分开；fractional sink容量差1/D拒绝。
- D超上限/不含源分母在opening前拒绝；接收分数serde恢复、stop保留分数/未烧Fuel，换D拒绝旧State，必要stop不依赖新增sink精度准入。

所有测试只是源码，未运行、不宣称编译或通过。本轮没有原 World/Stock/Action/Timeline/Store桥、真实来源/接收/资格/费用、真正库存扣量或sink接收证据；这些由root后续串行实施/验证。也没有本新增门的advisor结论，不能套用上一门审计。无PR/commit/push/tag/版本发布/关闭Issue/清tree/进程干预；保持open。

## 有界预算与保留

本门单机组、2输入lot、2sink、至多1有限消费者，advance≤3600游戏秒；公式 O(1)，不按秒循环、不新建调度器、无search/随机/worker/IPC/GPU。原Timeline确认事件/接受峰时窗的开销仍归root，不能把纯公式O(1)当整城吞吐或四速验收。

Config/State/Candidate/Delta/Meters全为固定字段及固定数组，无历史Vec增长；hash用小JSON临时缓冲。拟本门单调用额外CPU工作区≤64KiB、单机组Config+State序列化≤4KiB、恢复旧/新双份≤8KiB（不含原World/Store快照/receipt/sink/actual reception来源），全部估计未测，不新增获批额度、不扩大normal64MiB/共同8MiB或原M0联合门。root实例数、原票/WAL增长与实际Stock/sink副本需共同预算。无cache、GPU上传或新进程。restore输入硬限8192B，不替代原Store全局有界加载。

增量仅新模块/专属测试/handoff，目标新增源码<96KiB，无target/work/生成log/压力文件；完整旧源码/旧测试及新untracked原件保留本树。归档/依赖释放/停写/测量结束/重核占用之前不删tree，由接管root统一收尾。hash不代原件，不联系旧停主会话。

未来最小串行命令，仅交root在确认原HP P900唯一target/work且合法测量边界后排程，**本会话未执行**：

```sh
CARGO_TARGET_DIR="/Volumes/HP P900/dev/MyCity/target" cargo test --locked --offline -p mycity-core --test electric_dispatch_continuous_genset
```

旧 `electric_dispatch` 专属测试仍可由root另排，当前没有执行任何cargo/rustc/fmt、测试脚本/程序/benchmark/压力/实际Store恢复。旧审计不自动扩展新profile；新源码冻结后由root原advisor检查及唯一target串行实测。


## Root v0.5.184 有限实际验证回填

原 owner 的历史 not_run 交付记录保留。本次 root 原 advisor 源审/窄核与串行结果见
[固定联合门证据](../evidence/continuous-service-supply-20261025/STATUS.md)。V3组件20、
连续机组组件24、#57派遣状态组件22通过；原账联合供给9及Store恢复也通过。
仅#57组件验证，不代表真实服务履约；仅固定原账供给，不代表旧V1/V2、开放网络、
SOC/爬坡或卫生认证已完成。完整Issue保持open，无PR；root统一版本/发布/回填。
