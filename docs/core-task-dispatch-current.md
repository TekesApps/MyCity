# MyCity 当前实际派工与交接

**2026-10-24 当前接管状态：**本聊天 root 是唯一 canonical 集成、共享核心修改、串行验证
与版本发布负责人；旧主会话已由用户停止，不再联系或等待其释放。当前有限集成发布基线为
v0.5.172；保留的 WIP 只按最新已验证阶段登记。下文各带日期/轮次的旧派工与失败记录是
历史证据，不恢复旧 writer 授权、不覆盖文末最新实际状态。最新已过 CPU 生命周期37项与
发布前 F/E26项、四个正向 fresh reader/两个拒绝反例见文末及各 STATUS。分项发布范围与限制见 release-association-20261024/STATUS.md。

2026-10-23初次核对主线main v0.5.140，root复用既有agents，没有重复启动同线。D随后验证并合入v0.5.144；M0、F、E的WIP仍留在各自原树。非本任务 `docs/evidence/m0-t13a-rare-control-01/` 保留。范围为Rust模型/CLI；没有UI/GPU/语音/AgentKit。本表是执行合同，不是全目标完成声明。

## 历史共享核心负责人（接管前）

`economy_scenarios` 是 Engine/World/assembly/Timeline/事务候选/Store提交及receipt decoder的唯一修改负责人。D与有限F已发布；该writer在 `/Users/frankqin/dev/MyCity-core-work/spatial-runtime-update-current` 交付E租约与周期租金后冻结核心，由root按明确白名单集成main验证。root维护集成与验证；其他agent不得同时修改这些共享结构，也不另造时钟、账本或持久化。下表保留各线原合同，最新小批次及验证状态见文末。

## 当前批次

| 任务/负责人 | 文件所有权 | 权威输入→输出 | 依赖与交接 | 完成条件 |
|---|---|---|---|---|
| M0 / equity_current | equity-current树的headless joint_driver.rs；独立fresh R07 control目录及说明 | 原采样的source/nonce/PIDbirth/footprint→既有Rust消费者的受限3槽reading→实际映射置换 | 保留冻结j3p控制器；root构建新exe后pin、advisor窄审、统一短测，再判断正式重跑条件 | 缺测撤销/TTL补回/物理退映射实测，原64MiB/负载不变，剩余超额不抹去；原分级/四速/故障/8h仍逐门实测 |
| D shared core / economy_scenarios | 上述共享核心；town_runtime/equity/model capacity | 实际现金、原成本池、工资债税及股权→真实出资/分红/资本返还和游标 | 已修复清算股东排序和统一尾差；root完成205—209最终同源实测；接口与持久字段冻结 | 可信来源、偿债优先、未知已付费用仍阻断利润；真实Store恢复/重放与资源预算通过 |
| D evidence / construction_finish | D树tests/equity_runtime.rs与examples/town_equity.rs | 实际Store命令与独立整数oracle→场景、反例、冷恢复证据 | 共享core只读；发现接口问题交唯一writer，root串行Cargo | 出资/税/分红/财政后支出/清算四账户真实守恒；lost-reply后的业务回执本身必须成功，不能只看重放相同 |
| F / joint_current_prep | spatial-runtime-update-current树独立spatial_transport_updates.rs、新测试/CLI；lib仅export | 验证Scene、原始profile bytes/hash和有效权限→版本候选/精确asset/按日proof解析 | 给共享writer精确Action、root字段和授权需求；共享提交窗口排在D后 | 同一Store open→close→reopen道路/门实际改变销量；历史proof原hash可解释；重复/冷恢复不重复激活；未支持模式拒绝 |
| E / scoped_index_calls | housing-current树housing.rs、独立tests/housing_runtime.rs和examples/town_housing.rs、接线文档；lib仅export | 源明确的期初存量住宅/实际交付、家庭账户与实际收入→容量/唯一住所/租金欠费/迁居等待 | 先有界状态/serde/copy/validate/金额计划；world/Action/Timeline月历到期由唯一writer分批接 | 实际租金和欠费入原账本、日历/恢复一体；同一Store入住/欠费/工资后支付/迁居等待；独立planner不算完成 |
| advisor / scoped_handle_advice | 只读具体争议及验收 | 实际源码/失败/测量证据→有限阻断和判定 | 当前F权限来源与R07协议；阶段最终证据才复核，不重复总体审查 | 对来源、守恒、权限、预算给可证伪结论，不因源码测试存在签通过 |
| root | 集成分支、证据/任务状态/发布；统一Cargo与测量 | 各线稳定小批次→联合世界、实测、完整证据与版本 | 一次一个共享writer；构建结束后才允许改固定源码；正式测量禁止本任务编译/重测试/盘点/清理 | 住所/收入租金消费→交通供给→企业工程→税/分红清算同一持久世界因果成立；未测不通过 |

## 测量、计算契约与交付

Cargo与重测试由root排程，统一 `/Volumes/HP P900/dev/MyCity/target`；work也在外置HP P900 USB/APFS SSD。正式性能运行期间全部本任务agent仅编码和轻量只读，不编译/重测/全盘盘点/清理，不停止其他项目进程。当前j3q-01已因pressure保护停止并在v140归档；没有修复/环境变化不重复长测。

各线分批交接源码路径、具体接口、反例、CLI/可执行场景、冷恢复原件、CPU/内存/盘增量和已知限制；补权威IO、关键事件、稳定区间、等价合批、非线性阈值及资源归属，保留参考事件路径。新动态粒度实验不替代原M0负载/门槛，真实16GB门保持not_run。大数量字段计入多份候选和恢复放大。

每批root实际验证后统一提交/合并、唯一递增annotated tag并原子推送核远端。阶段结束先查占用/后续依赖，再清理明确可重建文件；完整恢复原件和故障/验收证据不以hash替代。当前各线未验证部分均不得报告完成。

## v144发布后的增量验证（未发布候选）

D已经在main v0.5.144提交并核验远端。root随后复用同一target对候选逐批运行：
M0的213权限传递两测、214真实macOS读取校验一测通过，215 fresh主exe构建和216 embedded source核验通过。
221隔离短测实际失败：controller读取Applied回执路径错误，20秒未找到初始state；尚未进入provider物理生命周期。
真实state存在于exercise/layout/r07-state.json，clock交换和layout扫描已进行。修正路径后才具备另开唯一目录的条件，
不改旧失败收据，不将该结果记为模型退映射失败或性能通过。

E的217首次独立组件17项中15过2失败；修复首期日历边界与反例fixture后，218同类17项全部通过。
这仍只是housing组件，不是租金实际入账或Store恢复通过；共享Timeline原Claim接线未完成。
F的库在220/222已检查到后续test/example阶段，示例与联合场景仍在收口；所有新增恢复背板及持久篡改反例待实际测试。
对应不可变源码ZIP、日志、实际测试exe、失败目录与JSON汇总保留在外置work/construction-utilities-current-01。
root已接入单Store D+F联合CLI候选，当前仍未运行，不把独立场景合称联合通过。

## 已登记的GitHub交接（保持原派工）

在自然工作边界对齐[总览 #2](https://github.com/TekesApps/MyCity/issues/2)：
M0 [#20](https://github.com/TekesApps/MyCity/issues/20)，D [#10](https://github.com/TekesApps/MyCity/issues/10)，
F [#3](https://github.com/TekesApps/MyCity/issues/3)/[#6](https://github.com/TekesApps/MyCity/issues/6)，
E [#4](https://github.com/TekesApps/MyCity/issues/4)，联合场景 [#21](https://github.com/TekesApps/MyCity/issues/21)。
这是现有工作登记，不重开已完成D里程碑、不新增重复agent。其他模型及校准issue不自动取得实现或测量窗口；
参考设计PR #1尚未合并main，不把设计验收作为实现通过。main当前发布仍为v144，其他分支可能消耗后续tag，
下次发布须重新核对唯一递增版本。

226布局诊断实测旧世界Engine owned=5,305,672B，超4,194,304B；JournalRecord=3,864B×1,024，
占3,956,736B。新增Action宽度在每条16个槽中放大，是主因；空间active/spare为0，不能靠Box一个136B头解决。
诊断测试退出0仅说明取得数据，不是构造/业务预算通过。225核心171项127过44失败，存储测试未执行。
writer开始分离紧凑驻留日志与完整持久命令，原容量/重放/来源/检查点约束不降低。
227/short02已读到初始回执但controller采样API不匹配而失败；清理回执确认监督进程-2退出、没有本任务残留。
M0作者先做实际采样API契约预检，再冻结下一版控制目录；不盲重跑或宣称物理通过。

## 当前验证边界（232—235）

232 同源 release 实测：core 173/173、storage lib 27/27。Action 恢复为120B，原128B上限未放宽；
紧凑日志2200B×1024，完整持久回执3352B，Engine诊断owned 3,605,136B；这不是全进程或共同8MiB门通过。
233/235 的F Store测试均4/5；1025次实际提交跨过两次512自动归档，冷启动可读原成功/失败完整回执，
Store owned约41.79MB，小于自身64MiB。剩余场景已越过交易断言，失败在篡改反例读取root检查点格式；仍须修复并重跑，不能报F通过。
234 housing组件19/19实测通过，包括同一原Claim跨月应计、保留最早欠期、部分工资支付和溢出拒绝。
这仍不包含Engine/Timeline/Store实际接线；E不能据此完成。
M0 short03预检因外部编译器正常拒绝，model未启动，退出0不算性能通过。原normal残差仍未解决，8小时与真实16GB保持not_run。
所有失败/成功原件和实际exe保留外置 work/construction-utilities-current-01；没有恢复原件被hash替代或删除。

GitHub进一步拆分入口为 [#23](https://github.com/TekesApps/MyCity/issues/23)。当前既有工作映射：
F有限runtime [#25](https://github.com/TekesApps/MyCity/issues/25)、E住房租金 [#26](https://github.com/TekesApps/MyCity/issues/26)、
M0 normal/长跑 #112/#113、同Store联合 #115。索引与issue覆盖不表示实现完成，不自动开启新任务或第二测量线。

## v147有限阶段发布

主线255 strict、263 fmt、256组合237测试及258/259实际CLI通过；260独立进程恢复联合世界一致，
262新代码打开真实v144 run209完整副本，现金/分红一致且旧原件未改。Advisor接受此有限发布。
范围与资源边界见[完整证据](evidence/core-spatial-runtime-147/STATUS.md)。不是完整F或M0通过。
E新offer组件253的22项实际通过，writer已在原F树开始真实租约/原Timeline租金接线；不是Store验收。
F domain另在原spatial-transport-current树补几何候选，尚未导出/实测/授权发布。M0 reader栈候选仍未测。

外部授权任务#31/#27/#96的源码补丁交接已完整复制到外置work/issue-31-27-96-source-handoff-20261023，
其原工作树/源码保留；未apply/构建/测试，不打断当前F/E/M0窗口，也不视为功能完成。

## Active batch ownership after v0.5.147

- Shared core: economy_scenarios alone edits Engine/World/Timeline/Store/decoder in spatial-runtime-update-current. D and finite F are published; current implementation batch is E monthly rent recurrence on the existing Claim and calendar. Other agents do not write core.
- M0: equity_current provides the minimal two-file reader patch and actual 292 eight-case results. Root integrates onto current main and revalidates. Requested 512KiB stack has no measured physical saving; original normal/common-8MiB/endurance/real-16GB gates remain unmet.
- D: construction_finish owns independent tests/CLI for an additional dividend-to-exit joint scenario; existing v144 functionality remains authoritative. No new ledger or shared core ownership.
- F: joint_current_prep owns geometry_revision module/tests/example and handoff, root owns lib export, integration and evidence. This batch creates exact Scene/profile candidates with separate parent-based revision increments; it does not activate Store geometry.
- E: scoped_index_calls owns independent recurrence oracle/scenario preparation in housing-current. Input is first-period actual evidence 277-279; expected second-period output retains cumulative paid and oldest arrears due date, bills once, pays outstanding only. Core API freeze precedes test integration.
- Advisor handles only specific contract disputes and final fresh evidence. Root schedules every Cargo/heavy test and publishes verified small batches, with Issue #25/#26/#112/#115 implementation and unverified results.

No formal physical measurement window is active. Shared external target is reused. Source/executable pins, original recovery worlds and failure evidence remain retained. Existing D+finite-F same-world evidence does not prove the pending housing joint chain or complete M0.

## Verified finite E integration, 2026-10-24

Stage v0.5.150 integrates the frozen lease/recurrence implementation and an actual31-day
D/F/E Store scenario. Authoritative details and full verification provenance are in
[evidence/core-housing-runtime-150/STATUS.md](evidence/core-housing-runtime-150/STATUS.md).
525 distinct core/storage tests are covered across321/322/323;330 reruns the7 final affected
cases.329 fmt,333 workspace strict and331/332 actual fixed CLI pass. The original failed
attempts stay recorded; no assertion constant was changed to erase the v106 durable identity.

The shared writer remains economy_scenarios. Its next migration helper stays only in Ftree,
unexported and untested; no Move/EndLease/escrow/residential route binding is published.
Root alone integrates, schedules Cargo/native windows, records issue results and publishes.
M0 owned wrapper+transcript diagnostic patch is frozen in equity-current, not yet integrated
or physically measured. It must persist bounded source/identity/bracket evidence before a
quiet full-business diagnostic window; it provides no memory credit by itself. Original
normal/common8MiB/8h/real16GB gates remain unmet or not_run.

## Next bounded batches after v150

- Root main is frozen during all wrapped builds/tests/probes. Agents implement in their owned
  private trees and hand off exact files/interfaces; even a new untracked test changes the
  `crates` source manifest. No canonical edits without a root integration window.
- M0: the two-file bounded Sim-open receipt candidate is integrated for validation only.
  Actual results and accounting limits are in [diagnostic evidence](evidence/m0-t13a-sim-open-diagnostic/STATUS.md).
  Root owns all Cargo/native scheduling. No memory saving or original-gate pass is claimed.
- E core: economy_scenarios alone implements the next atomic Move/Wait slice in Ftree; tests
  remain with scoped_index_calls in housing-current until the precise API is frozen. Old Rent
  Claim identity remains; processed old debt requires explicit actual SettleClaim, not an
  assumed automatic future P2. Paid greater than earned rejects moving until refund support.
  A genuine payroll observation window is required; missing evidence persists a wait.
  The genesis assessment source remains an explicit fixture boundary, not a computed city
  satisfaction producer. This slice remains uncompiled/unverified.
- F: joint_current_prep prepares original Scene/profile fixtures, independent expected causal
  results and exact Store geometry seam in its existing private tree. It may not concurrently
  edit shared core while E is being wired. Existing geometry component and runtime profile
  tests do not prove Store geometry activation or housing route binding.
- D's published actual capital/dividend/exit mechanism stays authoritative. Unsupported housing
  earnings and obligations do not silently become distributable profit. The combined31day
  scenario is evidence for its stated causal chain, not completion of all lines.

Original finite source handoff batch5 (#102 worker protocol/#120 opening loader/#97 vegetation)
is retained in `/Volumes/HP P900/dev/MyCity/work/issue-102-120-97-source-handoff-20261024`.
Root originally checked39 integrity-listed files/251018 bytes; that v149 handoff was source-only,
not compilation/Store/resource acceptance. Later bounded102 ownership/observation and120
same-slot/local-file increments have separate current evidence below; #97 vegetation remains
unapplied and execution not_run. Complete originals remain recoverable independently of the
old checkout. Current102/120 private trees are retained for continuing issue work; obsolete
other trees are released only after a fresh complete-original/identity/occupancy audit.


## v152 finite native reader repair and queued E validation

347 ran fullL1/real business concurrently but failed at final cold layout restore; raw memory
still exceeds original normal. Native ABI16 fields outgrew the reader13 bound. The repaired
bounded parser is independent of snapshot R07 version and retains full native shape checks.
359's29 regressions,360 strict and362 original3.37GB cold recovery pass; native provenance
adds1.2MB within the unchanged64MiB workspace. Evidence: [native347 and repair](evidence/m0-t13a-native-347/STATUS.md).

Income-only code is isolated from Move WIP on branch codex/housing-paid-wage-window-v151.
Four actual Store cases and its real CLI pass in363/366; full recovery artifacts are retained,
and component8456B is observed. Broader regression is running; this code is not in v152.
No Move/geometry activation acceptance is implied. Future complex Issue ownership follows
updated AGENTS independent child-session workflow; root retains exclusive Cargo/publication
and shared-core integration scheduling. Existing frozen batches and unrelated WIP remain intact.


## Active parallel execution, 2026-10-24 (after v152 local commit)

Root checked all existing agent states, canonical main, registered branches/worktrees and
uncommitted files before dispatch. Existing agents were revived; no duplicate worker or
new clock/ledger/storage was introduced. Unrelated rare-control evidence and old Move WIP
remain untouched. Cargo/native processes were absent before the integration window.

| Slice / Issue | Owner and file boundary | Input -> output / dependency / handoff | Actual completion criterion |
|---|---|---|---|
| M0 / #112 #113 | equity_current, private headless candidate only | Original347 native failure and memory ownership -> specific bounded fix and rerun prerequisites; core read-only | Root runs original frozen load after relevant fix; retain all failed evidence, normal/common8MiB/8h/real16GB remain unmet |
| D / #10 #115 | construction_finish, new equity_creditor_residual test/example only in equity-runtime-current | Published equity/actual loan/exit APIs -> same-Store creditor-first residual and recovery oracle | Actual execution, exact cash and sourced debt, capital vs surplus, replay and cold restore; no new ledger |
| E core / #26 | economy_scenarios, sole Engine/World/Timeline/Store author | Frozen actual-paid-wage window -> next small atomic Move/Wait slice; old mixed WIP isolated | Unique residence/capacity, original debt identity and earned principal, real transfers and persisted wait; tested by root |
| E evidence / #26 | scoped_index_calls, housing-current new tests/examples only | Exact core API and existing numerical oracle -> executable Move/Wait counterexamples | Actual Store/CLI, fail-atomic and cold replay; source-only oracle is not completion |
| F / #25 #115 | joint_current_prep, spatial-transport-current tests/fixtures/handoff only | Current main scene/profile API -> first persistent geometry activation contract | Same-world road/entrance change alters arrival/trade; historical proof and recovery remain valid. Shared core queued after E lease handoff |
| Advisor | scoped_handle_advice, read-only | New363/366/367/368 evidence -> finite income-window acceptance | Concrete authority/window/recovery/resource blockers only, no repetitive general review |
| Integration | root, canonical integration/evidence/release | Exact8-file income whitelist on top of reader fix -> combined source tests and pinned CLI | Actual same-source combined validation, issue result backfill, unique tag and verified remote push |

The income-only private source passed367 core/storage regression and368 strict with source
unchanged; this does not substitute canonical integration. Root copied exactly6 core files
and2 test/CLI files after verifying canonical destination bytes equal their151 baseline.
The three152 reader repair files were not replaced. Runs369-373 are root's serialized
canonical verification queue using the existing external shared target. No formal performance
window is active; all agents still avoid Cargo/heavy tests until root grants a window.


## v153 actual integration result

Income-only8-file batch is actually integrated on top of152.369fmt/370five Store cases/
371core201+headless102+map5/372strict/373build/374fixedCLI pass with source unchanged.
Independent literal income and retained joint checkpoint/profile checks pass; initial opening-
cash expectation correction is recorded. Advisor accepts this finite release. Full evidence:
[evidence/core-housing-income-153](evidence/core-housing-income-153/STATUS.md).

Agents continued independently: M0 found6,334,090B noneligible overhead not yet attributable to
a safely reclaimable allocation and supplied a bounded pool-event diagnostic candidate only;
no memory fix/gate pass. D is writing actual creditor/residual Store cases. F supplied DoorPose
candidate code/tests, not yet compiled or activated. E shared writer has started real Move/Wait
and explicit city binding in the isolated paid-wage tree, with legacy missing bindings denied
Move; test writer waits for the exact API. All are private WIP, not v153 acceptance. Root still
alone owns verification/publication, and all original load/cap/failure evidence remains.


## v154 actual parallel batch

Final source398 passed148 selected library/integration tests,391 strict and390fmt.
Fixed actual394 D CLI,395 F candidate CLI and397 standalone pool CLI passed, with source
unchanged. Advisor accepted only these bounded claims. Exact pins, original failures,
independent literal checks, resources and retention are [recorded here](evidence/core-parallel-154/STATUS.md).
D creditor-priority residual is actually verified, not a new ledger. DoorPose is validated
candidate code, not same-Store geometry activation or actual sales change. Pool counters
are diagnostic, not normal-memory savings or original M0 acceptance.

Shared Engine/World/Timeline/Store ownership remains economy_scenarios. Its E Move/Wait and
explicit-city core batch is frozen in housing-paid-wage-window-v151, with full manifest/patch
preserved; test writer supplied2 new Store/CLI files in housing-current. All Move behavior
is still not_run, pending root exact integration after154. Root must preserve153 income
and152 reader changes, reject old whole-file rollback and verify actual prepayment/debt/
city/occupancy/cash/Wait/cold invariants before release. No new duplicate workers were started.
External source-only handoff batch7 (#104/#57/#98) remains unapplied; no original source
worktrees or recovery evidence were released or removed by this stage.


## Next E integration in progress after154 (not released)

Root integrated exactly16 manifest-listed core/decoder/mechanical housing fixture/new Store
case/CLI files onto154. The initial handoff path guard rejected a wrong decoder location
before any copy;399 therefore checked unchanged154 and is NOT Move evidence. Correct400
integration uses checkpoint/receipt_decode.rs and is pinned by move-integration-400.json.
Actual400 workspace all-target check failed with source unchanged: new bounded row Record
implementations/imports and GameTime arithmetic need correction. The sole shared writer is
fixing those concrete errors in its private tree; source400 and full logs are retained.
No Move, wait, city compatibility, old-claim recovery or new resource acceptance is claimed.
Root does not overwrite D/F/M0 or unrelated rare-control WIP and has not tagged this candidate.


401 corrected compilation and402fmt pass;403 core202 tests pass, with actual size_of9472B
opening rows and3072B move payload (Box/header/restore peak separate).404 Store cases1pass/
1fail: city-less legacy can cold-open and rejects Move; first persisted Wait cannot cold-open
because checkpoint_world_budget. Full failure world/executable retained. Advisor identified
World128KiB owned+scratch guard, distinct from Engine4MiB; do not raise either cap. Candidate
publication must enforce the same World guard as restore, and MoveRuntime Box header must
be accounted. Test owner verified input counts are under existing limits (2units,3cities,
1assessment/1candidate); do not shrink fixture to erase this failure. Shared writer is doing
exact capacity attribution/real layout correction; no new E stage/tag is accepted yet.


## Active next batches after154 / current E verification

Root preserved all canonical WIP and reused the six existing owners (no duplicate starts).
Shared-core ownership stays economy_scenarios; root alone runs Cargo, native validation and
publication using the same external target. No formal performance window is currently active.

- E: scoped_index_calls owns only housing_move_store_runtime.rs and town_housing_move.rs in
  housing-current. Correct the412 oracle using actual earned wages while retaining city-switch
  coverage; do not change core/caps to fit the old expected amount. Advisor resolved this concrete
  question against R54: ordinary dormant-city labor freezes, already earned21000 remains payable.
  Root integrates exact frozen files, reruns Store/CLI/broad regression and preserves failed worlds.
- F: economy_scenarios starts the next private Store activation increment; joint_current_prep owns
  F test/example inputs and independently expected traffic/trade changes. DoorPose candidate pass
  alone remains insufficient. Handoff must retain profile/geometry versions and old proof identity.
- D/joint: construction_finish prepares a bounded existing-API joint test/example connecting housing
  income/cash to consumption, actual transport/trade and trusted distributable returns. Missing
  seams are explicit dependencies for the shared writer, not new ledgers or clocks.
- M0: equity_current investigates safely attributable noneligible startup overhead and proposes
  a reproducible bounded experiment/fix. Counters alone are not savings; no same-load long rerun
  without a related fix/environment change. Raw347, all original thresholds and not_run gates stay.
- Advisor: only concrete disputes/acceptance/final fresh evidence. No generic repeat review.

Each line hands off exact source hashes, actual executable scenarios and counterexamples,
complete recovery originals, resource deltas and limits. Root can release small passing batches;
none of this marks unfinished lines or combined performance accepted.


## Concrete integration dependency: housing/cash and transport demand

The existing joint housing/equity/spatial Store scenario has real rent, wages, road closure/
restoration, delivery, sales, trusted inventory basis and tax consequences. It does not yet bind
HousingMove's new unit to trip origin: Opening.demands/RepeatFixedDailyDemand and immutable
transport OD are fixed. Actual cash can limit a sale without changing issued trip demand.
The next D/joint small batch therefore tests real wage cash → actual sales → margin/tax →
shareholder payout → fiscal use while reporting unchanged transport demand. It must not be
called the entire housing → transport causal chain.

A separate pending core seam must bind an authoritative dwelling entrance/household group to
OD origin, issue a new bounded demand/profile generation after an authorized change, and keep
old service-day proof identity and exactly-once consumption. Do not let test-only reassignment,
a caller quote or a planner mutate authoritative traffic. Scene geometry activation is the
current F priority; the housing OD bridge remains explicit, neither silently dropped nor
expanded into an unreviewed clock/ledger replacement. Overall A—F completion remains open.


## Verified finite E batch155

Canonical415 broad540 passed before the final historical-restore correction;421 selected45,
then final425core204/426Move4,423fmt/424strict/427build and actual fixed428CLI all passed.
429 fresh-process complete-copy checkpoint recovery and430 final reader on original404 full
working copy passed; independent checkpoint cash/Claim/state digest agrees. Advisor accepts
this finite release. [Evidence and limitations](evidence/core-housing-move-155/STATUS.md) include
2-unit lifetime-offer restriction, separate3-unit re-let, near-budget rejection, static scores,
missing housing→OD and all unpassed M0/full/16GB gates. Raw recovery originals remain complete.

F writer is implementing explicit immutable adapter basis and descriptor-backed geometry grants
in fresh spatial-geometry-v154; test owner has written genesis/old-close/geometry-cost candidates,
not yet Cargo/Store tested. D joint owner has delivered a private wage-cash→real spatial trade→
tax/dividend/fiscal candidate with fixed OD, not yet tested. M0 owner delivered a private real
Model lifecycle diagnostic with original347 event fixture, not a memory fix/test pass. Root
continues integration as small bounded batches using shared target; no parallel Cargo or old
whole-file rollback. Twenty identical terminal399-430 source ZIPs share full hardlink content;
selected allocated space reduced107.32MiB, separate APFS availability observation retained.


## Live batches after155 (2026-10-24)

The existing agents/branches were checked and reused; no duplicate workers and no unrelated
WIP overwrite. Root is the sole Cargo/measurement/release scheduler; economy_scenarios remains
the only Engine/World/Timeline/Store/decoder author. All evidence and shared target use external
HP P900 USB/APFS SSD. No formal measurement is currently running.

| Batch / owner | Exclusive files | Handoff / dependencies | Actual completion boundary |
|---|---|---|---|
| M0 / equity_current | Private normal_pool.rs diagnostic and bounded native observer, original fixture, offline attribution note | Root supplies CLI entry, pins fresh source/exe and schedules isolated diagnostic; original347/normal caps unchanged | Source-only diagnostic/observer; no physical fix, no long rerun, real16GB not_run |
| D joint / construction_finish | New household_wage_spatial support/test/CLI and independent oracle | Existing155 core read-only; root integrates exactly3 scenario files and executes/reviews recovery | Final436fmt/437strict/438 five Store checks/439build/440fixedCLI/441–442 fresh full-copy opens actually pass; independent oracle pending |
| F shared / economy_scenarios | Private spatial-geometry-v154 core, catalog, basis, Store activation | Geometry-only context patch over155; cannot roll back final E historical rent/Default changes | New stage/grant/Apply implementation remains uncompiled; root verifies before any release |
| F scenario / joint_current_prep | Same private tree tests/examples only | Actual Store API from shared writer; same-world geometry→trade→history→recovery, then CLI | Frozen test a0bb952d… is source-only/not_run; test owner may not modify catalog |
| E / scoped_index_calls | Existing housing-current test/CLI files only | Published155 is authoritative; keep private WIP and do not recopy old helper fixes | Finite Move/Wait/re-let/cold evidence accepted; lifetime offer cap and missing dwelling→OD remain |
| Advisor / scoped_handle_advice | Read-only concrete plan/results | New failures or final fixed evidence only | Finite D review accepts stated evidence; no oracle signature before it runs |
| Root | Canonical integration, evidence, joint interfaces, version/Issue receipts | Exact manifests, protected WIP, one Cargo window, complete originals | Continues small passing releases; no all-lines/M0 completion claim |

D fixed439 source5c1ddbdd… and executable0cda6575… bind the actual440 run. Its two-world
control preserves the same fixed spatial OD and differs by actual480c supplemental wages.
Household cash affects purchases and tax/dividends; no origin reassignment is invented. See
[evidence/core-wage-spatial-156](evidence/core-wage-spatial-156/STATUS.md) as results finalize.

Root completed the pending156 independent v7 checkpoint/cash/equity/tax/proof oracle with
exit0. This closes that finite joint scenario; M0/F/housing→OD dependencies stay open.


## Root M0 diagnostic increment157 / continuing parallel handoffs

Root v156 isolated worktree retains canonical F WIP while integrating only Model lifecycle
CLI, bounded observer and retained347 fixture. Actual447 normal_pool16 tests,449fmt,450strict,
451release/embedded source identity and456real helper run complete. CLI/observer0 is finite:
35filled native samples only, four events but incomplete native four phases. Original347
residual and original normal/common8MiB/speed/8h failures stay; real16GBnot_run. See
[evidence](evidence/m0-model-lifecycle-native-157/STATUS.md), including445/446/448/454 failures.

F actual443/444compile failures are retained in canonical uncommitted geometry evidence.
The sole writer fixes constructor/serde/optional-witness and independent asset/Scene/transport
revision bindings; advisor resolved that concrete token dispute. F is not accepted. D owner
froze new existing-API28-day scenario/test/CLI source only (not_run), without replacing the
previous actual28-day cases or claiming ongoing self-finance. E owner delivered a minimal
2-unit/3-immutable-offer compatibility/capacity handoff only, with no ignored test or core
mutation; dynamic offer generations remain later. No duplicate core writer or Cargo window.


## Takeover parallel queue — 2026-10-24

This section supersedes the historical owner assignments above. The old main chat is
stopped. Current coordinator `01a166fc-e444-7602-ad22-3a8551dda487` owns canonical
integration, shared core changes, serial compilation/testing and release scheduling.
Canonical HEAD is v0.5.158 / `ae28b8cda8041c7bd106a28cccdc3deab051b032` with protected
uncommitted F/E/M0 WIP; a HEAD alone is not the current source identity. Each adapter must
record the actual read-set hashes. Previous candidate source acceptance is not integration
acceptance. Existing F/E finite results and M0 entry/clock diagnostics retain their stated
limits; neither establishes M0 performance or real16GB acceptance.

| Priority / Issue | Owner and independent write scope | Entry condition / completion gate |
|---|---|---|
| P0 #120 | Chat `01a16b67-c377-7b22-b44c-065d4f43d504`; reuse original opening-loader tree, module/tests/fixtures/handoff | Real Opening/schema and bounded input adapter; assembler interoperability, raw-byte source identity and negative cases. Root alone chooses parsed-Opening seam and edits assembly/CLI/genesis. |
| P0 #101 | Existing chat `01a1682f-f3ce-7f61-833c-a31d55a43b34`; original registry files plus new headless task_registry_adapter module/test | Adapt real Background Job inputs to bounded hints. Required positive-dt air responsibility, actual ACK and dispatch stay in existing owner. Missing durable snapshot identity remains explicit; root wires sim_host and exports. |
| P1 #118 | Existing chat `01a1682f-f0de-7241-b895-0bbfa1386c5b`; handoff interface increment only for now | Read real Resource/Lease/Engine/Store and propose minimal snapshot/clock/receipt contract. Root must settle missing shared semantics before adapter implementation; no invented authority or second capacity pool. |
| Coordinator #25/#26/#112 | Current main only; existing shared-core WIP and frozen evidence | Keep geometry/housing recovery results; continue bounded native observer diagnosis separately. Serial verification windows, no concurrent private Cargo/native runs. |
| Waiting #31/#32 | Original owners retained, no new assignment | Reliable geometry/rights/profile/quote providers and explicit versioned equipment meter-boundary decision before real adapters. Nonzero references are insufficient. |
| Waiting #102 → #103 → #104 | Original modules retained, no new assignment | Actual #101 source/due/context observations and validated protocol/compatibility chain before real resource grants. Cancellation never proves resources released. |
| Waiting #73/#74/#75/#57/#54 | Original modules retained, no duplicate assignment | Real capacity, actual travel/attendance/fulfilment and authoritative receipt adapters; plans do not prove delivery. #54 finite synthetic tests already passed, production providers remain pending. |
| Waiting #85/#86/#94/#96/#97/#98/#61/#117/#36 | Original modules retained, no duplicate assignment | Versioned real input/units/ownership adapters and cross-resource causal boundaries before joint integration; unknown and unresolved must not become actual shortage. |
| Later #105–#111/#113/#114 | Not dispatched | Correct integrated representative load and recovery baseline first; GPU/full-path, long-run and true16GB need separate qualified measurement windows and hardware. |

The first two source lanes may run concurrently; #118 interface investigation does not
write their files. All three read shared core only. Per lane incremental source/fixture
budget <=8MiB and handoff package <=16MiB are planning caps, not measured runtime budgets.
No new target/cache, reset/rebase, source-tree cleanup, publication or issue closure by the
issue owners. Complete initial and final originals must remain recoverable.

Handoff must include actual files/API, exact upstream read-set, fixed fixture versus real
provider distinctions, independent expected outcomes, remaining dependencies, resource
limits and requested serial commands. Current main schedules builds/tests using the existing
HP P900 target/work, then requests advisor review of concrete changes/results, returns
blockers to the same implementer and records implementation/validation/unverified/PR status
in the existing issue. A source-only handoff cannot close the whole issue. Cleanup follows
complete durable archive, stopped writers, process-use and recovery-dependency checks;
no waiting on or waking the stopped main chat. All unlisted issues remain unassigned by
this increment, not implicitly complete or newly owned.

### Current queue validation increment

#101 original advisor accepted the finite real-Job adapter source. Root's canonical serial
core registry tests passed13/13 and headless adapter tests passed8/8; see
`work/parallel-issue101-validation-20261024/`. Only the core module export was wired; actual
Sim required/dispatch/ACK/Store paths and durable-token/recovery/resource gates remain pending.
Warnings and synthetic real-domain input limits are recorded in Issue#101.

#120 compiled in both initial and boxed revisions, but each aborted in its first default-stack
test with stack overflow; zero tests confirmed passed. Both source revisions, fixed executables
and failed logs are retained under `work/parallel-issue120-validation-20261024/`. The same owner
is diagnosing the concrete assembler/adapter stack boundary; no global stack or product budget
increase has been made. Source review acceptance is not an execution pass.

#118 P1 interface evidence is delivered; no physical adapter has been implemented. Store's
existing receipt lookup should be reused. Missing units/physical aliases/future windows/rights
prevent enabling the old full physical candidate constructor from bare scalar rows.

### Approved #118 read-only implementation boundary

Following the original advisor's finite P1 review, root approved the same #118 owner to
implement `city-storage/src/capacity_read.rs` and its dedicated tests/fixtures/handoff. Root
added only `World::scope_revision()` to return existing scalar metadata without allocating
a WorldView. Shared Store/receipt/transaction internals and module exports remain root-owned.
The adapter may only inspect the current original scalar pool and sequential original
receipt lookup within one exclusively managed Store instance, with guarded stamp checks.
It must preserve partial/absent clock, unknown physical dimensions/alias/window/rights,
original errors and full receipt outcomes. No physical provider, detached authentication,
new ledger, capacity mutation or automatic retry is authorized. Actual implementation and
serial Store tests remain pending; original candidate-only tests cannot establish this gate.

### #120 finite retained-path result

Root fixed the concrete town Opening decode stack failure with bounded profile scratch;
[finite evidence](evidence/opening-loader-retained-20261024/STATUS.md) records opening6,
wire4, correctly selected runtime2, exit13 and utility-binding6 passing tests. Two broader
equity/utility default-stack failures also reproduce with the original town file under the
same WIP and remain unresolved. No full regression/Store/publication/M0/16GB acceptance.
Scratch backing max42,272bytes plus1,872-byte header, allocator/stack overlap and native peaks
remain separately accountable on all profile-decode callers. #120 remains open; same-slot
Store publication and recovery integration are the next gate, not a second initialization.

The two independent default-stack failures are tracked separately as #122 (equity) and #123 (utility); they do not alter frozen M0 gates or become implicit passes after the finite #120 fix. No duplicate developer was assigned.

### #118 read gate actual result

Original advisor accepted frozen retention/budget corrections; root's existing release
validation configuration passed5/5 real-Store read tests. [Evidence](evidence/capacity-read-20261024/STATUS.md)
retains four release worlds and all debug failures. Default-debug Store creation still aborts;
release success does not resolve it. No physical provider/admission/resource/M0 pass follows.
This finite read gate permits next #101 real Sim observation/dispatch wiring, then #120 same-slot
Store initialization/publication/recovery. Shared control and commit paths stay root-owned.

### #101 actual Sim hook result

[Real-process evidence](evidence/task-registry-dispatch-20261024/STATUS.md): original advisor
accepted source; fixed release CLI/test with matching source identity passed2/2. Required
positive-dt zero-source execution, cancel/retry and original cold receipt replay are observed.
This is advisory wiring only; #102/#103/#104 control and formal budgets remain pending.
Next sequential verification is #120 same retained slot Store publication/recovery; original
Ticket/commit authority, M0 frozen gates and protected WIP remain unchanged.

### #120 same-slot Store actual result

[Finite Store evidence](evidence/opening-input-store-20261024/STATUS.md): serial release build
and six parent tests passed; 12 actual fresh-process recovery probes each passed. Original
advisor reviewed concrete results and found no new blocker. Original prepared slot, raw bytes,
genesis/root identity, actual 137-cent WAL/checkpoint recovery and CURRENT cutpoints are covered.
No production authority, physical power-loss, resource-peak, complete #120 or M0 pass follows.
#118 read gate, #101 advisory Sim hook and this #120 finite gate are now validated in dependency
order. #122/#123 and #118 default-debug stack failures remain open; preserve WIP and all evidence.

### Post-#120 next real integration ownership

#102 owner chat `01a16bad-d5a1-77e0-9990-fcb8c473cadf` reuses the original issue102
worktree and archived v149 source; it owns only protocol schema, a new private headless
adapter, dedicated tests/fixture and handoff. Root alone modifies compute_worker/sim_host/
normal_pool/broker and module exports. No private builds/tests or duplicate allocator.
Original advisor accepted keeping actual wait/retirement capability private to the Child
owner, with the original pool/broker as sole return authority. Core scalar validation cannot
prove OS wait. Cancel/report/idle never retire resources; spawned-but-attach-ACK-lost still
requires wait, and reaped-but-pool-communication-failed retains retry capability. Broker
Child ownership is not duplicated in Sim. GPU/Hybrid remain disabled without device fences.
#103 waits for this actual lifecycle path; #104 follows qualified strategy transitions and
real owner costs. One existing CPU air worker is the first integration target, not14 workers
or a proven global CPU/GPU grant controller. Full issues remain open.

Root short diagnostics for #122/#123 preserve exact assertions and default stack. Test-only
stage markers and fixed binaries are archived under `work/stack-faults-122-123-20261024/`;
no M0 load or formal measurement is run. Initial markers narrow both failures to the opening/
Engine construction region, not yet to WorldView or a proven shared root cause. All diagnostic
markers must be removed byte-for-byte after localization; no unrelated WIP may be reverted.

#102 first actual-owner adapter source review found two activation blockers: error paths
can drop the sole Child/Grant owner, and legacy2MiB report bounds do not fit the existing
8MiB air checkpoint ceiling. Original implementer is repairing its private API/handoff;
root must retain failed owners until successful wait/Reap and preserve original workload.
No activation or measured_report is allowed with fabricated work/scratch counters.

[Stack short-diagnostic evidence](evidence/stack-faults-122-123-20261024/STATUS.md) records
original Opening decode aborts and the failed private-scratch candidate: #122 then reaches
first transaction execute before aborting. All candidate/marker source edits were reverted
to exact pre-diagnostic originals, leaving protected WIP intact. #122/#123 remain open.

### #122/#123 finite repair accepted; next #102 shared window

[Final stack evidence](evidence/stack-faults-122-123-20261024/STATUS.md) records original
default failures repaired without changing stack/workload,209 release core and4 profile
passes,9 debug utility and6 retained-loader passes, Store6parents/12 cold children and
5 debug capacity reads. Original advisor accepted finite source/results; historical failed
full command206/1/1 remains recorded plus corrected F-fixture exact1/1. Publication/version
association is still pending coordinated handling of protected WIP; full issues are not
closed by candidate-only evidence.

#102 revision03 also passed source-only narrow review: boot/session binding preserves
u128 boot high bits. Root's shared hook must retain a pending Prepare request/key BEFORE
RPC so lost replies can retry the same capability, retain partial Worker/Grant on every
post-spawn failure, and keep original Reap/broker the only credit return. No alternate
prepare/key or fabricated work counters can count as recovery. Real shared lifecycle/pool
fault tests remain not_run. Root schedules this next; private modules remain frozen.

### #102 pending Prepare shared increment

Root added `normal_pool::PendingGrant`: it owns the original Ready, full-width boot,
parent instance and key before RPC. A failed resolve retains the same request; a successful
resolve transfers the original Grant once. Original advisor's concrete source review found
no blocker for this finite increment. Serial `cargo check -p mycity-headless --lib --locked
--offline` passed; this is compilation, not lifecycle or memory acceptance. The first command
used an incorrect package name and failed before compilation; its log is retained.

Real service cutpoints distinguish successful Prepare/Attach/Reap followed by a lost reply
from an explicit Reap rejection before action. The dedicated real RPC fixture is owned by
the existing #102 implementer; root alone integrates and runs it. Logs/originals live under
`work/issue102-integration-20261024/` (256MiB evidence planning cap); short retained socket
roots under the same external work avoid Unix path limits. No secret capability is logged.
Old `Grant::prepare/from_env`, Worker partial-spawn errors and Sim retirement containers
are still unmigrated. Passing this API gate cannot close #102 or activate #103/#104.

[Actual finite RPC results](evidence/worker-pending-rpc-20261024/STATUS.md): serial release
build and4/4 real service/RPC tests passed on fixed source/executable. Four final original
pool reports balance one grant/one return; independent phase checks distinguish server
mutation from local acknowledgement. Original source/executable/logs retained. Next root
window uniformly migrates actual spawn callers to persistent WorkerSlot, including broker
Started-before-accept retention and unconfirmed Start/session exit. No lossy temporary
String wrapper may drop a live owner. M0 load/standards and unrelated WIP unchanged.

### #102 actual CPU lifecycle gate passed

[Stage08 actual results](evidence/worker-pending-rpc-20261024/STATUS.md#stage08-persistent-actual-workersimbroker-ownership)
supersede the preceding unmigrated-caller status: root migrated all actual spawn callers to
persistent Worker/start methods, retained original Prepare/Child/Grant/Lease before later
failure points, and retired before Rebind. Structured Start-unconfirmed state now ends Sim
after the original result; original supervisor actual wait/revoke remains sole recovery.
The original advisor's concrete blocker was fixed and verified with no second command.

Serialized frozen source08:9 RPC/Worker/unwind +2 new broker +10 compute +6 pool protocol
+8 existing broker +2 #101 dispatch =37 selected tests passed. One native mapped observer
test explicitly not_run; formal M0/pressure/GPU/native16GiB acceptance unchanged. Source,
fixed executables, worlds, receipts and failure logs retained under existing HP work.
Advisor accepted concrete results without new blockers. Remaining #102 work is r03 actual
identity/report wiring and truthful measurement/compatibility, not replaying this finished
CPU retirement audit. #103/#104 are not activated. Coherent protected-WIP version association
is next; no full issue closure or release claimed yet.

[Current-source publication prerequisite](evidence/release-association-20261024/STATUS.md):
F8+unit8 and E4+2+4 passed26/26, four fresh reader processes reproduced business/state on
verified full copies of old recovery originals, and two corrupt copies retained their exact
rejections. Advisor accepted this finite gate. Historical F443/444 failures remain historical;
they are not current compile failures. Publication still requires a per-hunk scope and exact
version/read-set association; no blind whole-WIP commit and no M0/16GiB claim.

## v0.5.159 finite integration association

Current root owns integration and publication. Runtime per-file/hunk groups and limitations are
recorded in [the release manifest](evidence/release-association-20261024/publication-manifest.json).
The original registry module13 and adapter8 tests also passed on the same current source.
#54 remains inactive; #101 advisory observation is not a scheduler; #118 read model is not admission.
#102 r03 reports and supervisor registration retention remain next, before #103/#104 activation.
M0 Python WIP is deliberately retained outside this commit; no original frozen gate changes.


## v0.5.161 Client/Model owner caller migration

Root integrated the original #102 worker regression source and migrated actual Sim/Model/helper
entry scopes to one persistent owner. [Finite results](evidence/worker-client-slot-20261024/STATUS.md):
76 focused tests plus real Model lifecycle/Sim probe parent entries passed; original advisor
accepted the finite gate. Terminal unresolved responsibility and deliberate dead/unbalanced
pool cases are not recovery. All24 unrelated WIP files and original evidence remain intact.
Next: bind r03 report identity to actual wire/owner and retain Unknown measurements; resolve
v1 2MiB versus actual air8MiB capability explicitly before #103/#104 activation. #101 advisory
and #118 read model are not new scheduling/admission authorities; full issues stay open.


## v0.5.162 actual CPU observation / selected opening files

#102 original owner supplied v2 CPU observation; root wired canonical Worker/Sim status and
validated full Ticket, actual direct/unpooled/broker evidence without grants or fabricated metrics.
#120 original owner supplied local regular-file provider; root wired init-opening into original
Store factory and actual fresh-reader/recreation rejection gates. [65-test finite evidence](evidence/worker-observation-opening-cli-20261024/STATUS.md).
Neither full issue closes: common in-Sim task/report and actual measurements/103104 remain;
120 still lacks full CSV/profile/typed mappings and migration/external rights evidence. #54
waits authoritative school/attendance/capacity inputs; plans/fixtures cannot produce those facts.
24 unrelated WIP unchanged, original stopped chat never contacted; root remains sole integrator.


### Obsolete batch5–7 checkout release

After fresh Git identity, all tracked/untracked/ignored status, exact full-original byte/hash,
frozen ownership and empty occupancy checks, root removed only the seven standard worktrees
for97/103/61/28/104/57/98 with Git. Their28 exclusive files remain in the original HP P900
full-source packages; branches/common repo/target/work evidence stay intact.102/120 trees
are retained for continuing work. [Release receipt](evidence/worker-observation-opening-cli-20261024/worktree-release.json).
This changes checkout retention only; unapplied source and not_run business gates remain so.


## v0.5.163 actual child CPU accounting (#102 finite increment)

[Fixed-source CPU-clock evidence](evidence/worker-cpu-clock-20261024/STATUS.md) records explicit
air wire2, actual child ProcessCPUTime and independent owner send-start/receive elapsed.
74 selected tests pass serially; preserve the source05 new-test12/1/1 failure separately.
Only validated current owned wire binds the application instance; samples are cumulative,
partial kill is nonfinal, missing metrics never block necessary business, and original actual
wait/retirement remains authoritative. Work/scratch/queue/native/total resource costs stay unknown.
Full102/103/104 and M0 remain open. Next gate is equivalent page slicing plus actual safe-boundary
yield/continue, work counts and latency/cost calibration, not a fictitious alternate model.


## v0.5.164 actual equivalent air page control (#103 finite increment)

[Exact page-control evidence](evidence/air-page-control-20261024/STATUS.md): independent issue
child supplies pure bounded air_turns, root integrates actual loop counts, explicit wire3 and
original Sim/Worker owner.313 serial tests pass, plus one preserved-v163/current alternating-
quota checkpoint comparison. Production RunRequiredSliced permits pause/query/continue/cancel;
wrong/uncertain control ends the sliced command after actual retirement and preserved failure,
never auto-starts another attempt. Pause keeps original grant and required Job. Core pages
count Sources/Cells/integration/commits separately; no fabricated generic work-cost unit.
This single exact strategy does not activate old approximate dependency/catalog candidates or
an automatic total budget controller. Full103/104 remain open; original M0 gates unchanged.


## v0.5.165 actual air page sampling (#104 finite input gate)

[Actual sampling evidence](evidence/air-page-sampling-20261024/STATUS.md) records same-child
page wall/CPU brackets, separate ordinary/segment-integration groups and local parent
response/pause intervals. Explicit wire4 reuses actual exact-page control. Continue clears
old summary before new turn/quota observations; final summary freezes across Published/Complete.
Invalid/missing metrics become unavailable, never business rejection, free cost or admission.
91 serial tests pass;12 additional paired v164/new actual worker runs retain byte-identical
full checkpoints. All raw scheduling noise and failed first type-annotation build are retained.
No automatic controller is activated. Next is explicit bounded automatic Continue for an
already-granted task, varying pages only, with conservative Unknown/zero handling, minimum
hold/hysteresis and reported irreducible segment-page overrun. It cannot increase admission,
concurrency or assume freed resident memory. Whole-game cost/16GB/GPU/M0/full104 remain open.


## v0.5.166 actual automatic exact-page control (#104 finite increment)

[Actual automatic Continue evidence](evidence/air-auto-continue-20261024/STATUS.md) records
production Client-side AdaptiveSession borrowing the original owner. A boundary returns one
pending decision; the next step attempts it once. Pages1/2/4/8/16 only, source/cell unchanged;
real positive page wall samples, two-turn trend/hold and conservative Unknown/zero fallback.
Query responses precede Continue, cancel/drop/initial and Continue uncertain writes retain
original responsibilities. Typed timeout alone permits repoll; auxiliary final sample failure
is unavailable observation, never business rejection.111 serial tests pass, including10 actual
control cases and byte-identical688908B fixed/automatic checkpoint. Recorded target change is
a test stimulus, not calibrated fixed-target capacity. Original advisor accepts finite scope.
Full104/whole-game multidimensional budget, speed/backpressure/fairness,16GB/GPU/M0 stay open.
Next budget work must attach measured shared process/resident costs and existing allocator
constraints; it cannot turn this per-task wall preference into global CPU/memory admission.


### v0.5.167 shared broker admission/recovery (#104 finite gate)

[Actual evidence](evidence/air-shared-broker-20261024/STATUS.md):114 serial tests pass,
including13 actual controls. A paused task retains lease/work; a second Sim gets exact busy
rejection without credit changes, then runs after original retirement. Pooled cancel and
uncertain Continue retain necessary Jobs through cold Store reopen. Only actually retired
original credits return; original64MiB/8MiB stays fixed. No simultaneous Sim admission added.
Next concurrency design retains one Sim unique writer and bounded independent Worker slots,
original total admission, per-slot retirement and serial candidate validation. Full104/16GB/
GPU/M0 and earlier debug stack faults remain open. This gate is not whole-game acceptance.


### #118 managed venue authority gate — active after v0.5.167

Original #118 owner implements the dedicated bounded venue_capacity pure rules/test source
in an isolated v167 tree; root alone owns shared Opening/World/Engine/Store/restore/transaction
integration. [Existing facility contract](building-unit-shared-facility-contract.md) now states
the first managed people-present unit, canonical resource/rights, conservative whole-group
reservation/use, actual-exit responsibility and no generic/internal-effect bypass. Existing
capacity_read remains observation; no physical admission/proof is promoted from Unknown.
Next actual gate is original-pool Store atomicity and cold replay, then54/57 adapters. Two old
debug stack failures remain separate; all new code awaits root serial fixed-source validation.


### v0.5.168 actual original-pool venue gate (#118)

[Finite source/results](evidence/venue-authority-20261024/STATUS.md):123 distinct tests pass
on one fixed source/executable set; original failed checkpoint recovery and legacy optional
slot clearing now pass. Keep all earlier compiler/business/runner failures separately.
Original advisor accepts finite gate, not full118 or native/M0/16GB. `managed_people_v1`
explicitly separates the new51040B venue profile from unchanged base128KiB, while charging
full World/Engine/preparation and retaining original4MiB/8MiB/64MiB gates. This is not old
joint128KiB acceptance. System raw activity remains host input, not transport/teaching proof.
Root remains sole shared writer and serial verifier; original118 pure owner is frozen.
Next: real101 due-task dispatch binding, then120 managed-venue same-slot publication/recovery;
54/57 must consume only proved actual presence and retain missing upstream facts as Unknown.
Two default-debug stack faults #122/#123 remain separate;24 unrelated WIP unchanged.


### Active after v168: #101 selection and #120 venue opening bridge

Original101 owner alone writes required_dispatch.rs/task_registry_due.rs/private handoff in
codex/issue101-due-v168. Root alone adds Sim RunDue, pre-Rebind identity validation, export
and Worker active-request observation; idle retained Child differs from an active request.
Real pending uses progress.at/end at P1–P4 and keeps original retirement/Ticket/Accept/receipt;
no second queue/clock or autoAdvance. Source still not_run until owner freeze/advisor/root
serial validation. Invalid hint/cache never deletes mandatory work; true hard/clock/binding
failure backpressures with the obligation intact.

Original120 worker independently writes opening_venue_store.rs/its small fixture/handoff in
codex/issue120-venue-v168. This validates the new managed_people_v1 path through original
OpeningInput/workspace/Store, not a repeat claim for already-tested E0/Boot/Town. No shared
Store changes by that owner; root reviews actual source, validates after101 serially and
records any real integration gap. No code/fixture from private trees is yet an acceptance.
Both keep full originals, explicit external evidence roots and no compile/test/fmt/push/PR
by owners. Root retains sole Cargo/measurement/release window; no timers or old-main contact.


### v0.5.169 actual required-dispatch gate (#101)

[Finite evidence](evidence/required-dispatch-20261024/STATUS.md):8 groups/86 distinct tests
pass;7 actual due-worker Tickets bind fixed SOURCE. P4 selection does not autoAdvance;
idle child/cold running differ from active execution, original retirement/Rebind/Accept
and hard backpressure preserve obligations. Original advisor accepts finite source/results.
Full multi-domain101/global budget/16GB/M0 remains open;24 unrelated WIP unchanged.
120's independent freeze05 source fixes were narrowly accepted but still not_run; root now
queues its new venue same-slot Store/fresh-reader gate after this verified source phase.


### v0.5.170 managed-venue same-slot Store evidence (#120)

[Actual finite gate](evidence/opening-venue-store-20261024/STATUS.md):17 distinct main tests
pass; new parent starts4 independent successful fresh helpers. Existing Store implementation
already supports this bounded optional profile: no shared game/Store code changed this phase.
Same caller slot/raw genesis/counters/receipt replay/checkpoint and post-CURRENT lost-response
recovery,137-cent spent-capital preservation and no-overwrite all pass. Original source06
compile/run09 oracle failures are preserved; independent source assertions follow original
charter field semantics, fixture unchanged. Original advisor accepts actual finite gate.
Full120/CSV/typed profiles/migrations/production provider/native16GB/M0 remain open, as do
separate #122/#123 debug faults.24 unrelated WIP unchanged. Root retains sole shared writer,
serial verifier and release ownership; source-only54/57 still require truthful actual adapters.


### v0.5.171 live Issue reconciliation and next actual education boundary

Live GitHub GET confirms #122/#123 are **closed**, repaired and published in v0.5.159
(commit1bee5b4c9ec49ebd41e0b555fb8bcbe7e8efaca5). Later entries that carried “open”
or implied an unresolved defect were stale status propagation, not new failures. Historical
failed runs/originals remain immutable; no duplicate defect work or new execution is needed.
Recent venue result summary metadata is corrected, without altering the actual123/86/17
run results. Old chronological “pending/open” entries are historical, not current ownership.

Next #54 original owner resumes dedicated education_runtime/test/fixture/handoff source;
root alone owns shared member-activity interval authority, Opening/World/transaction/Store/
restore/receipt/export/budget. [Adopted design boundary](building-unit-shared-facility-contract.md#教育实际活动接线边界v171设计未实现)
requires exact role members and game-opening qualification provenance, actual preparation/
instruction distinct from venue presence, original completed Development tick and cross-day
cuts. Original advisor recommended this actual vertical path;14 prior pure synthetic tests
are not production integration. No new timers, duplicate owner or second capacity pool.
#57 outbound service remains a separate lifecycle: venue Enter cannot mean departure,
arrival, actual work and safe return. Its original candidate is not yet canonical or tested.
Full54/57 and actual shared adapters remain pending; existing24 WIP and M0 gates unchanged.


### v0.5.172 internal shared member activity and current-schema reference repair

[Actual evidence](evidence/member-activity-authority-20261024/STATUS.md): root-private32-active
ledger now implements atomic two-range exclusion, ongoing responsibility, day-boundary
nonclosing settlement and admitted-stream replay watermarks. Every active reserves a close
ordinal even atMAX; rejected new work does not consume it. No capacity or wallet is duplicated.
Logical resident3192B + candidate3352B are separately charged components, not native peak or
an automatic grant. World/Store/domain progress integration remains pending under root.

Source04 check/build0; full core230pass/1ignored and education14pass=244 distinct. Member10
is included. First source02 core228/2fail/1ignored remains retained: current direct-derive
mirrors omitted the v168 venue extension. Test-only mirror fields now match current schema,
with a separately frozen15-field legacy World byte oracle, all6 legacy Opening fixtures and
new venue cases. No production decoder/assertion normalization or M0 change. Original
advisor accepts concrete fix and actual finite results. Original54 owner is actively building
its dedicated runtime in its original private tree, still author not_run; root does not copy
unfinished private source. Full54/57/118, native16GB and M0 remain open;24 WIP unchanged.
#107 read-only audit found no existing migration writer seam; no preflight-only substitute
was dispatched as executable migration. Keep original root/recovery evidence intact.

### v0.5.175–176 current finite acceptance and next shared bridge

#25/v174 and #26/v175 completed their original finite F/E packages after original advisor
scope audits and actual Store recovery gates; both issues are closed, parents/full product
remain open. [Housing Wait evidence](evidence/housing-no-candidate-wait-20261024/STATUS.md)
retains the new2/2 plus version-specific prior evidence. No full16GB/M0/32-household claim.

#49/v176 [operator observation and receipt gate](evidence/operator-observation-20261024/STATUS.md)
passes24 distinct tests (module5/newprocess3/oldsession16), preserving initial4pass/1fail.
Trusted System gates old/new session query; own successful/failed original receipts remain
visible to their actor in hot/archive paths; unknown is not permission to reissue business.
Full49 including general Goal/Monitor/SimIPC/providers and wider privacy remains open.

#54 original owner frozen03 is now accepted by the original advisor after B1/B2 repairs:
full pending/stop/history/final reserve, retained authenticated actual endpoint, Stop refusing
unsupported actual evidence, required strategy_version2 refusing implicit old-state upgrade.
Original owner execution remained not_run; root adopted and actually verified runtime16,
prior education14 and core230 (260 distinct component tests). Root then implemented the
actual Opening/World/member ledger/Timeline/Store bridge, with six real recovery parents.
These results do not convert synthetic witnesses into external provider certification.
No second writer, capacity pool or silent old-candidate migration is authorized.

Root source11 removes seven unnecessary full World projections in Store cold open, using
the same original scope/profile fields. This fixes a new education default-debug overflow;
original #122/#123 exacts pass separately and stay closed. Source05 release340 passes and
source11 new education debug6 passes are distinct evidence generations. Final source12
adds independent189104B typed restore admission before copying; public4MiB gate unchanged.
Workspace all-target check/build0, release341 distinct passes and default-debug9 executions
(oldexacts2/newbudget1/education6) pass. Full54 stays open; see the retained failures,
scope and budget limits in [education authority evidence](evidence/education-authority-20261024/STATUS.md).

Current dependency check found no safe additional real-provider parallel increment among
#57/#61/#98/#73 before that shared seam and authenticated inputs. Repeating synthetic DTO/
planner delivery would not advance real integration. Keep original owners and originals:
#57 needs need/depart/onsite/return facts; #61 replayable activity+place/field coverage;
#98 unit/versioned actual emission/noise sources; #73 plans are not actual passenger trips.


### v0.5.178 qualification authority, exact reads and #124

Root integrated original #54 completion and #55 read modules plus one qualification authority
into original Opening/World/Prepared/Store, preserving original source author snapshots and
single writer. Free retraining actual hours/calendar/exam/prerequisite gates, exact latest-exam
anchors, original receipt authentication, grant/revocation and bounded exact job-read paging
are now executable. No second wallet/resource pool or applicant-supplied authentication marker.
Full54/55 remain open: paid education, transfers/all-course and actual hiring/Payroll are pending.

[Actual evidence](evidence/qualification-authority-20261025/STATUS.md): final unmarked source17
passes381 different release parents and12 default-debug executions separately. Six final
qualification child probes pass; three original states match source09 release/source17
release/debug hashes and facts. Action120B satisfies the unchanged128B assertion. Earlier
source06 Action344B failure, source08 E0425, new #124 default-stack failure/diagnostics and
native crash originals remain intact. Removing two unnecessary whole WorldView scope
projections fixes this particular checkpoint-restore overflow without changing comparison,
publication, layout or budgets; original advisor accepts final result and #124 closure. No full
16GB/M0/native peak, universal stack safety or performance conclusion.24 protected WIP unchanged.

Next true business integration is #57's original public-service owner, subject to an explicit
local Development-clock profile, dedicated original Resource/Lease meaning, qualification and
member namespace independent of unrelated teaching. Need/depart/arrival/work/safe-return and
fee facts must remain distinct. Old service_core source has InteractionSeconds and is not yet
canonical/runtime-authoritative; plans and venue Enter cannot replace actual travel or service.
No duplicate owner, new clock/pool or repeat DTO-only milestone is assigned. Original target/work,
M0 frozen standards, full recovery originals and root shared-file ownership remain unchanged.
