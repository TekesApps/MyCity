# MyCity 当前实际派工与交接

2026-10-23初次核对主线main v0.5.140，root复用既有agents，没有重复启动同线。D随后验证并合入v0.5.144；M0、F、E的WIP仍留在各自原树。非本任务 `docs/evidence/m0-t13a-rare-control-01/` 保留。范围为Rust模型/CLI；没有UI/GPU/语音/AgentKit。本表是执行合同，不是全目标完成声明。

## 唯一共享核心负责人

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

External finite source handoff batch5 (#102 worker protocol/#120 opening loader/#97 vegetation)
remains unapplied in `/Volumes/HP P900/dev/MyCity/work/issue-102-120-97-source-handoff-20261024`.
Root checked all39 integrity-listed files,251018 bytes against full preserved originals.
Compilation/runtime/Store/resource verification remains not_run. Its v149-based private trees
are not integration-ready merely because the delivery hashes match; no shared source was
copied over current WIP, no measurement window is assigned, and no cleanup/release of their
original worktrees has been performed by this task.


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
