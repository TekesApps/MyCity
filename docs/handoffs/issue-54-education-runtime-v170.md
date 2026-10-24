# #54 free offline runtime — frozen source candidate, author all not_run

2026-10-24。Owner仍为issue54-education；原tree/branch未reset/rebase，HEAD仍ae28b8c。
集成参考actualrepo main v170 db8f46d85b38eaf0ea16125329e195d0c079a503，以及root新增
`docs/building-unit-shared-facility-contract.md` v171教育实际活动接线边界。
root随后通知v172 `33f7ed988c9a0d1673a7cec48fc67e99837deb1c`有限发布共享member_activity，
该模块仅只读对齐，未复制至本旧tree、未改共享源码；root此前core230+旧教育纯14通过不能扩写为本runtime通过。
旧模块/测试/handoff完整副本已归档 `/Volumes/HP P900/dev/MyCity/work/issue54-pre-runtime-v170-20261024/`，
逐文件SHA与原件一致；旧handoff不覆盖。本轮只改原education.rs必要内部checkpoint/serde接口，
新增education_runtime.rs、独立tests和本handoff，不碰共享核心/导出/Cargo/venue/Store/Timeline/事务。

## 实施前冻结的可执行契约

1. Opening最多4个显式免费整组线下课程，引用HousingGenesisV1 population origin及精确学员/单教师区间，
   引用原venue use/offer和不可变游戏期初资格、免费合同来源。角色区间互斥且精确覆盖offer人数，
   不凭教师数量猜成员，不复制容量或人口。实际注册校验由root封闭边界提供，公开Opening不是权限。
2. tick0只登记零进度；实际激活必须completed Development秒覆盖原started..started+1。
   首窗没有教学时必须有显式ActivateNoInstruction事件覆盖原started到当前tick；保留started，
   不能延迟追认教学。正常Cohort::begin时间门不改。后续开始时点必须等于原cursor，空白窗显式NoInstruction。
3. 进行中备课/授课从Begin事件起持久化并取得统一共享成员活动账claim，不能等到End才查双占。
   Settle验证原claim、原token、未消费段的原venue在场覆盖/当前未退出/资格和System真实活动来源，备课不产学时；
   课时只来自明确Lesson完成窗。close=false原位推进settled_through、保原活动和排他；
   跨午夜按实际区间硬切最多两个日段，不释放/伪造restart，不自动Timeline Advance。
   每次正结算至多86400秒，教学每个日段仍须真实当日2h备课和原每日课时门；
   没有次日备课不得平均摊过去，root须在必要真实时点消费/停课，不跳过原源和日门。
   退出切断后续计课，StopForExit同原SafeExit事务产生共享claim关闭候选，但保留domain awaiting待核算行及原义务；
   不因未知历史长期锁住成员未来人时。私有模块不释放原Resource/Lease，容量/场所退出仍由root原writer提交。
4. root提供非反序列化的crate内部VerifiedOpening/VerifiedEvent/Snapshot/Restore/Compaction证据；
   source/event完整绑定原根、scope、revision、System host及已稳定完成Development tick。
   不是公开Fact DTO填true认证。活动共享账暂明确EducationOnly覆盖；未接劳动/交通不能宣称全域排他。
5. 固定32活动记录是进行中/近期表，不是终身16课上限；另有32条成功接纳事件的近期重放窗。
   Begin预留活动记录，Settle原位更新，close原位闭合，不因表满抹旧义务。
   教育输入流独立非零epoch、严格连续sequence，任意event_id不充当顺序。原root保证同原事件不能另分新seq；
   模块永久保存已消费sequence水位，重发旧seq换operation拒绝。新seq跳号留待处理，不判短缺。
   首门只接受受信host接纳的内部游戏流；不声称检测任意外部事实的语义重复，不能把同旧事实重新发新seq当恢复。
   未知提交必须先查原Envelope/receipt并原样重试，不能改body。失败请求不消耗成功接纳序号。
   完整责任路径的源序号和Cohort消费步按文末B1/B2修复公式保留；Stop变awaiting不释放历史责任额度。
   近期槽为pending保留Stop+最终核算两格、awaiting保留最终核算一格，非最终批次可经已认证压缩复用。
6. 稳定前缀压缩是明确root操作：只有待退役的活动历史已闭合、进度已计入、对应共享claim已闭合/消费，
   且root持久Store receipt/checkpoint锚已覆盖该精确state与流水位时允许。
   保存四组完整cohort checkpoint、备课day/seconds、原started/身份/课程/状态/义务、日内量、收费余数、
   source epoch/consumed sequence、最后源及原receipt锚；才清空近期记录。
   进行中及已停止待核算行不退役：原始Begin源、原token、身份、开始、settled_through/cutoff和最后结算源完整保留于prefix种子；
   只有已成功应用的事件近期窗和已闭合活动历史转入原receipt锚。未闭合责任没有被合并/删除或伪造结束。
   不平均区间/跨日门，不扩活动表或共享预算门。重放只从已认证prefix种子处理最多32条近期命令。
   原Store仍保完整事件/receipt，压缩不删原件；旧详情未在近期表时必须查原receipt，缺失为unknown，不能补造。
   >32事件持续测试源码反复压缩并证明不重课时/费用；资本/真实Store重发验收属于root接线，非此纯fixture证据。
7. root唯一writer原子提交活动账、runtime候选、Cohort进度/游标/余数和原receipt，失败全部不发布。
   模块只产候选；不支付、不发工资/证书、不毕业认证。根/版本/身份/来源/占用变化提交时重验。

编译、测试、fmt/rustc、脚本、可执行/基准/GPU及真实Store冷恢复均author **not_run**；root串行验证。
不commit/push/PR/Issue回填、不清tree/target/work、不联系旧主线。以下是冻结交付契约，不预填advisor通过。

## 文件与最小接口变更

本轮实际改动：`crates/city-core/src/education.rs`仅增加Clock/Identity/Window/Course/Enrollment/Progress/Status等必要serde，
crate私有CohortCheckpoint及checkpoint/from_checkpoint校验，复用注册校验并将daily_seconds/seats改为crate只读可用。
原begin已观测首窗、课时/日历/收费/余数/状态/转学/义务及B1路径规则不改；Cohort本身未公开Deserialize或任意进度setter。
新增`crates/city-core/src/education_runtime.rs`、`crates/city-core/tests/education_runtime.rs`、本handoff。
原tests/education.rs和原docs/handoffs/issue-54-education.md保持字节不变。

## root接线API和精确来源责任

所有以下运行/恢复/权限边界均crate-private；公开Opening/Command/Origin是数据，不是生产权限：

- `Opening::{fingerprint,registration_origin}`：固定struct/array/enum的规范化JSON哈希。
  registration_origin=`SHA256("education-registration-origin-v1" + world/branch/scope LE64 + Opening fingerprint + population_origin)`，
  **不是当前Store root**，Opening内也无最终World hash字段，因此无初始化自引用。
  root初始化和恢复必须从原验证genesis/Housing及Scope独立重算，不仅检查非零。
- `VerifiedOpening::after_root_opening_validation(identity,input_hash,registration_origin,population_origin,total_people,offers,qualifications,free_contracts)`：
  root先验证真实游戏Opening来源。Runtime::new进一步检查身份/注册origin/人数、明确角色区间、原offer属性及精确注册行对应关系。
  source=1/version=1指既有HousingGenesisV1按原households顺序的稳定人口区间，population_origin复用原venue/Housing来源。
  enrollment.class=原offer.group_id，school=原venue_id，use_id和resource_id/窗口为原offer/Resource投影；不是新教室/座位余量。
  当前首门是新游戏started=0，不重置已有课程；未来非零入学/动态名册、收费/教师变更暂不支持。
  教师范围长度恰为1，学员范围长度为learners且与教师互斥，两区间无缺口精确覆盖offer总人群；teacher/staff/visitor/招生人数亦核。
  QualificationOrigin::ImmutableGameOpeningV1及FreeBasis::ImmutableGameOpeningEntitlementV1均须root解析独立合法注册来源，非非零ID即认证。
  Course.cents_per_hour必须0且合法free_contract_ref/source/course/roster完全匹配；不能把收费课程改0。
- `VerifiedEvent::after_system_host_validation(Command,Origin,completed_development_seconds)`：root先验证原System host真实内部命令、actor、权限、
  canonical epoch/成功接纳sequence/op/event绑定，以及原Timeline phase/target均None。Origin.clock明确Development，发生秒必须等于稳定完成tick；
  Origin.root为**提交前World hash**、revision为该原根revision，与registration_origin完全分离。不能拿InteractionSeconds或presence代替活动源。
- `Snapshot::after_root_source_validation(event,opening_hash,population_origin,venue,activities,LedgerCoverage::EducationOnlyV1,stopped)`：
  root验证原Resource/Lease/venue/token bijection、当前在场覆盖、成员/资格和统一活动账后封装，非公开true Fact。
  开始只检查当前在场点，不伪造未来一秒观测；普通结算仅消费[settled_through,当前tick)，退出/换token/短覆盖拒绝整窗计课。
  SafeExit停止和延迟核算走文末独立合同，不把仍待核算历史伪造为仍在共享账active。
  0秒close只释放本活动claim，零课时/费用，不替代SafeExit，原venue占用/其它义务仍由root保留。
- `Runtime::plan(snapshot)->Candidate`：纯候选，含期望runtime hash、Origin及ActivityDelta。
  `apply_candidate`须只作用于原事务prepared图像，与root共享活动账同边界发布；CAS不符拒绝，编码失败完整回滚组件行。
  它本身不写文件、不构成WAL提交。root还必须比较原World root/revision和权限，防跨组件提交期间来源变化。
- `ActivityDelta::{Acquire{teacher,learners},Settle{teacher,learners,from,through,close}}`：不是第二活动账。
  root将两角色范围映射到唯一member_activity::Ledger::prepare_begin的一条compound活动；
  ClaimKey是教育stream源引用，**不假定等于共享ledger ordinal/activity id**。
  通过原begin operation/source匹配原ledger Active.id；`Runtime::pending_claims`提供固定4槽只读投影，
  共享ledger prepare_settle原位结算(close=false)/关闭(close=true)，与本候选一笔原子提交。
  Snapshot.activities是root原共享账的已认证角色投影（含start/settled_through/source），不复制钱包或资源池。
- `progress/pending/awaiting_settlement/stopped_activity/consumed_sequence/next_sequence/last_source/opening/prefix_anchor`只读；
  `recent_event(epoch,seq)`只返回真实保留的命令/源，压缩后缺详情返回Unknown，由root查原receipt，不从累计值推造历史。

固定名册/资格在Opening验证，此首门没有资格授予、毕业认证、工资、支付/退款或假付款receipt。
root实际Opening/World/事务/Timeline/Store/WAL/receipt/共享活动账接线尚未在本tree实现，不能将crate内部fixture边界当真实provider。
共享member_activity目前覆盖已接教育/公服等域以root实际版本为准；本候选明确EducationOnly，未接劳动/交通不能称全域人时验证。

## 压缩、恢复与责任保留

`compact_prefix(VerifiedCompaction,原共享账投影)`只在root已持久化可解析receipt/checkpoint锚后作用于prepared图像。
锚包含store_ref/root/epoch/through，绑定完整压缩前runtime hash，不得仅手填数字。
ReceiptAnchor.root语义为root定义的版本化receipt_digest编码，**不是旧World root**；store_ref为最后已接纳教育/关联停止原operation_id，
模块检查与last_source.operation_id一致。root须Store::receipt/lookup_archived解析成功System原Action及epoch/seq和当前CAS，
保留committed receipt segments/承接锚；不要求另存每个旧World。仅非零hash不构成生产压缩/恢复授权。
原Store保留或依法承接完整原事件/receipt/锚；回收必须重新核恢复依赖，hash不代原件。
所有已完成源事件的消费水位单调不减，已完全核算closed行可以退役；进行中及cutoff后awaiting行完整保留进prefix恢复种子，
不丢开始/原token/消费位置/尚未核算的责任。pending_claims只投影真实仍需共享active排他的行，不投影已停止awaiting。
prefix保存完整cohort进度/日内量/费用余数/起点/义务和备课当日累计，不按均值重算。
进行中与cutoff后待核算二者恢复有独立状态门；待核算原历史仍在，而未来成员排他已于cutoff由root关闭。

serde反序列化Runtime后ready=false，不能plan。`restore(VerifiedRestore,共享账投影)`要求root已校验原Store hash/receipt及genesis注册origin；
检查epoch/源/水位/关闭预留/原prefix活动与checkpoint，重放最多32条真实近期命令，再逐字段比较最终states/records/水位及pending共享claim。
压缩前历史已转原Store锚，模块不重建或认证已缺失的历史。连续途中恢复保同一activity Begin身份及settled_through，不重发资本/费用。
恢复/重放只是模块实现；没有root真实checkpoint/receipt来源就不能构造生产VerifiedRestore。

若未知/预算满/事实冲突，返回Unknown/Limit等保持原状态；不判真实短缺，不取消已接受责任。
完整责任路径保留近期终结槽和源序号，不能只保证物理close后丢下无序号可核算的awaiting；详见B1/B2修复。
源epoch变更/重置不支持，额度不足先拒绝新承诺或非必要细分，不花已接责任保留额度。
现root实际低层global World revision/时钟/receipt耗尽等仍按原writer边界，不能用本局部预留宣称所有全局资源均可无限推进。

## 预算估计和保留（均未实测）

- 固定4cohort（每组至多一项进行中或已停止待核算活动），32进行中/近期活动行、32成功事件重放窗、4个pending/awaiting种子、4cohort prefix+live checkpoint；
  人口/offer/活动共享账为引用/投影，不按人口扩常驻agent，不扩大原M-life总量或normal64MiB/共同8MiB。
- Runtime全为有界inline字段，无增长Vec/String。拟owned≤64KiB、Candidate≤4KiB；这两项是待root执行的形状门，不是实测通过。
  选中行候选不会复制整个Opening；rootprepared/active/restore receiver的Runtime并存须分别计费，不能共享收费。
  恢复会另建一个同尺寸replay image，旧/新/重放可能同时存在，粗估组件3份≤192KiB，加candidate≤4KiB；
  原Opening解析副本、World/Engine/WAL缓冲、serde/SHA栈/native及父caller帧均须另计，不能称该数字是进程峰或栈上限。
- 指纹以Write流式SHA输出，无JSON输出Vec，单次编码硬门128KiB；最坏合法字段编码探针/ABI大小仍not_run，
  root在激活前须验证真实最大shape及原总门。apply候选检查前后编码，失败回滚并留原责任，可由root持久压缩后在合法窗口重试。
  该128KiB是本模块序列化限制，不授予共享Store/Engine/Opening额外预算。
- CPU每事件有界扫描4组/32活动/最多128条共享账投影；源hash最多128KiB，restore最多32次事件重放。
  原共享账投影超过128返回Limit；它是借用扫描门，非新增128活动池。CPU时间/性能/16GB/并存峰全部未测。
- GPU=0，模型调用=0，独立常驻cache=0，无新IPC/worker endpoint；候选为同进程root事务数据。
  disk每checkpoint本域编码拟≤128KiB，Root WAL原事件/receipt增长不由本模块删除，持续游戏增长/索引/锚承接必须root量化。
  两份状态/检查点原件编码至少分别收费，加原全world与receipt；不能用压缩hash替换仍需恢复的完整原件。
- 此轮仅小源码/测试/handoff与授权旧原件归档，未产生target/work运行载荷；无清理。root归档并核验所有文件、writer停写、解除恢复依赖后才决定退役tree。

## 专属测试源码、未实现和冻结

`tests/education_runtime.rs`按path包含两模块，仅依赖现有mycity_core::units/serde_json/sha2，不改lib或Cargo。
16项synthetic测试：tick0不能激活/在场不等于课时、备课零学时与真实1h literal；进行中教师-学员双占和序列化继续；
退出/换token不能按全窗计课；StopForExit未知前缀也切断共享排他并保awaiting，压缩恢复后仍保待核算，
后续只对1800秒独立实际覆盖计课，后缀零课时不表示成员未工作，旧seq重放拒绝；
跨午夜partial结算保原Begin/claim，带pending压缩和恢复，0秒关闭不重计；
40次一分钟课程共2400秒/83源事件，持续压缩后旧seq换op仍拒绝且旧详情Unknown；
clock/scope/epoch/序号/资源/成员反例；收费课程或缺合法免费/资格注册拒绝；prepare不变/stale CAS/修改原state hash拒绝；
近期30条留Stop与最终核算两槽；序号MAX单/双cohort同token经过Stop、压缩、恢复、历史批次、最终核算；
3天多的已认证备课按历史端点分批，当前提交tick不回拨，不把剩余真前缀叫NoInstruction；形状/编码未来门。
使用Vec仅fixture Host的共享账模拟，不是生产共享ledger或Store；MAX测试的近界prefix是明确的合成root锚，不能作为真实档案来源。

本阶段没有任何编译、cargo test/check/build/clippy/fmt、rustc、脚本、运行/压力/GPU、真实Opening/venue/活动账/Store冷恢复/原事务失败注入执行；author全部not_run。
root后续须真实覆盖Accept/Enter后仍零课时、实际开始/结算、双占、重复原receipt、退出后拒计、失败原子性和占用责任保留，以及WAL/检查点冷恢复；
目前仅有接口与测试源码，不是这些真实门已通过，不是完整#54。

未来root串行最小模块测试请求（未执行）：
`CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --release --locked --offline -p mycity-core --test education_runtime -- --test-threads=1`
必须在root固定整合源码/原HP P900唯一target和work窗口；真实Store/receipt测试另由root接线与排程，不执行本tree新target或并行测量。
冻结后交原advisor单轮源码审查，未预填通过。不commit/push/PR/Issue回填、不清worktree/分支/target/work、不联系旧主线。

## 必要SafeExit接口收敛（停止未来排他与历史核算分离）

三时点严格分开：Record.settled_through=已消费课程cursor，ActualCoverage.window/Command.actual_through=独立实际活动覆盖终点，
Record.cutoff.exited_at=同原token的物理退出；结算事件Origin.occurred_at=当前稳定完成tick，不能回拨时钟。

1. root原SafeExit的**同一prepare/WAL/commit**调用封闭Command::StopForExit，来源operation/event/revision/退出tick精确对应原SafeExit，
   该cutoff-only协议严格要求StoppedCoverage.actual=None，Some整次Invalid拒绝，不能接收后忽略认证历史。
   Snapshot.stopped与候选场所退出来源/原token一致。Runtime只记录end/cutoff/停止原源，将pending转awaiting，
   不消费历史区间、不加课时或费用，发ActivityDelta::Settle(close=true,through=cutoff)供root关闭原共享claim。
   同一原Venue退出可确定顺序fanout多个cohort内部子命令，各有连续成功sequence，绑定同原op/event；
   op/event不按数字排序也不当任意外部事实去重器，原Envelope重复必须先走receipt，不再分配新sequence。
2. awaiting状态完整保原进度/余数/日内量/入学及义务与历史cursor；stopped_activity返回原Begin/cutoff/待核算游标。
   awaiting不出现在pending_claims、不被伪造为仍在root ledger active，故成员可参加退出后的其它已接域活动；
   本课程未知历史仍未抹掉，该课程续记暂等原writer核实，不把unknown按0抹账。
3. 后续显式SettleAfterExit必须针对这个已停止待核算行，指出原Begin、退出原op/time、actual_through和finalize。
   正课时前缀必须有独立ActualCoverage（原游戏Host实际活动来源、cohort/generation/原Begin/kind/完整窗口、原source op/event/revision/观测时点），
   并共同满足同token场所在场前缀、资格、当日备课、共享活动已接来源及原规则。**仅在场不补到退出时点**。
   actual_through不得超过cutoff；ActualCoverage缺失/短覆盖/错源保awaiting，不能为了close造历史。
   后缀由该显式命令和StoppedCoverage.no_instruction证明是**本课程未授课**，不推断成员整体空闲；
   仍可在另一个已接活动中工作/旅行。原NoInstruction推进后缀，保原累计和余数，不造课时/付款。
   finalize=false只消费已认证的历史批次，保awaiting和未消费责任，不消费任何NoInstruction后缀；
   finalize=true才在覆盖既有已认证实际终点后消费明确未授课后缀并清awaiting。
   两者activity_delta=None，因为root已在真实SafeExit关闭原shared claim；不重关、不释放容量。
4. 模块没有SafeExit文件I/O、真实capacity/ledger调用或解除原其它义务。root必须实际接上述同事务hook，
   并认证ActualCoverage/StoppedCoverage的原Host/venue来源，commit失败整笔不发布；仅复制这些DTO或调用者System身份不构成来源认证。
   root若尚未接StopForExit同事务，退出完整验收仍**阻断/未实现**，不能用后续SettleAfterExit替代。

首门外部任意事实语义去重、动态名册/教师/收费/课程、跨城时钟、完整劳动/交通人时、资格发布及完整#54均未实现。
本源码已提供必要停止/待核算候选与合成反例，但真实SafeExit同事务、来源认证、Store恢复及实际资源验收都尚未由作者执行或证明。

实际新增SafeExit反例源码精确区间：先原课程BeginLesson于7201，SafeExit于10001（无已核实课时），
StopForExit关原claim但保awaiting，压缩/反序列化继续后新工作从10001开始；
到10801显式核算，缺ActualCoverage先Unknown且不占未来人时，再提供7201..9001实际覆盖，
只加1800秒，9001..10801仅本课程NoInstruction，原工作不删除；同seq重放拒绝，费用仍0。
错误停止token时当前runtime/原active均未变，root整笔原事务失败必须保原占用和责任。

最终冻结范围恰为四个改动文件：education.rs、education_runtime.rs、tests/education_runtime.rs、本新handoff。
原tests/education.rs及旧handoff仍保原字节，旧原件归档完整保留。实际SHA随交付消息回报，避免本文件自引用hash。
原advisor尚未审本冻结runtime，不作无阻断/执行通过声明；root下一步只读审真实冻结源，再唯一串行编译/实际接线验证。

## author-frozen-01 B1/B2修复，待同advisor窄复核

本次仅修原advisor两项阻断；完整旧四文件保留在
`/Volumes/HP P900/dev/MyCity/work/education-authority-integration-20261024/author-frozen-01`，未覆盖。
原版本的“每pending一个close额度”和“停后一次清awaiting”均不足，不能作为最终契约。
本次education.rs字节不变，只更新education_runtime.rs、专属runtime测试及本handoff；四文件重新冻结。

**B1：保留完整Stop/核算路径，而非只数active。** 固定批次上界B=86400秒、现有GameTime编码上界T=i64::MAX。
每个责任的已消费课程cursor为c：

- pending：`d=ceil((T-c)/B)`，保留`d+2`个成功源序号（Stop、最多d个满批次、单独最终后缀），
  Cohort内部消费序号保留`2*d+1`步（每批最多两个日段，加最终NoInstruction）。
- awaiting：退出cutoff为e，`d=ceil((e-c)/B)`，保留`d+1`个成功源序号及`2*d+1`个Cohort步。
- 没有pending/awaiting的状态没有此保留。四cohort逐项相加，候选后`MAX-consumed_sequence >= 总保留`；
  普通直接close解除对应责任；Stop只将pending保留转换成awaiting保留，不能释放未消费历史额度。
  其它cohort/非最终小批次同样检查全体保留，不能花别人尾部。restore在prefix种子及最终状态重验相同不变量。

这是有限整数额度安全计算，不是新游戏周期/时钟、未来实际人时、容量或CPU作业预留；没有循环遍历未来天数。
使用现有GameTime范围给任意合法延迟的上界，不假定root每日调度。开始时若剩余源序号不足最坏责任路径，立即Limit拒接；
所以“早期游戏在MAX-1接受Begin”现在明确被拒绝，不能为保旧合成断言继续接受会困死的活动。
满86400秒的合法历史批次至少将d减少1，故在仅剩保留额度时仍存在逐批完成路径；
不承诺无限任意细分可用，细分不减少保留又耗序号时Limit，不删事实也不推进失败源序号。

近期表不扩大：pending保留2槽（Stop+最终核算），awaiting保留1槽；中间历史批次不耗最终槽。
不够本次事件+剩余尾部槽时返回独立`NeedsCompaction`，而不是误判真实短缺或丢弃责任。
root须用已经成功提交的原教育/关联Stop receipt授予`VerifiedCompaction`，维护不消耗教育成功源序号；
压缩仍保完整pending/awaiting、历史cursor、known_actual_end、起点/日内量/余数及永久消费水位。
因此有限32槽可以逐批复用；不能要求把所有延迟批次同时放进32槽。
**若原成功receipt/锚被回收或不可认证，则压缩及继续消费明确阻断**，不可伪造锚、换epoch或丢历史；
真实FS/receipt持久性仍为root责任，不将此条件性保证称为实际Store通过。

**B2：当前提交tick与历史消费端点分开。** `SettleAfterExit`新增必填`finalize`：

- false：正长且至多86400秒的独立已认证历史前缀，按原午夜硬切推进到actual_through；
  当前Origin.occurred_at仍为当前稳定完成tick，可晚于历史终点。保awaiting，不消费NoInstruction，不改共享账。
- true：只在本段已达到此前/本次已认证的真实终点时，才处理明确NoInstruction后缀并清awaiting。
  Record.known_actual_end单调记录已认证ActualCoverage.window.end，不能缩小后换新证明把真实余段说成未授课。
  root可从stopped_activity读取消费cursor和known_actual_end，以满批次继续；只要还有真实未消费段就不能最终化。
- 历史批次的last_settlement记录的是当前观测事件，其occurred_at可大于课程cursor；
  prefix恢复保留这一区分，按近期真实命令重放，不回拨Timeline、不重记已消费段。

新增长前缀literal反例：备课开始1，退出259213，当前提交tick259220；认证实际前缀1..259213。
先后消费到86401、172801、259201，均保持awaiting且不处理后缀，途中压缩/恢复；
最后消费到259213并仅将259213..259220作本课程NoInstruction。
第3日备课累计13秒、cohort序号9、学员实际学习0秒/费用0；缩小已知终点提前finalize及单次吞下>1日都拒绝。

单cohort MAX反例：在数值测试专用的晚GameTime（T-86400）开始、源MAX-3，Stop用MAX-2，压缩/恢复，
历史批次MAX-1保awaiting，最终核算MAX清责任；双cohort同token、同原退出op/event以两个连续源序号Stop，
再压缩/恢复、各批次/各最终核算正好结束于MAX，中间别的细分不能消耗保留。
这些是合成计数边界，不代表真实Timeline/日历/世界revision能达到这些值；原global counter等界限仍root独立负责。

仍无任意未知/非法历史的“强制通过”入口：缺实际来源、原日内硬门不满足或锚不可用仍保待核算，
人时已停止不会因历史未知重新被全局占住。不能将真实未消费前缀标NoInstruction以省批次。
前次B1/B2接口/计数及当时14项测试源码全部author **not_run**；本次追加后的16项同样未执行。
原advisor尚未窄复核本次修复，未预填B1/B2解除。无共享修改、无运行/脚本/格式化/发布/Issue回填/清理。

## author-frozen-02 剩余B2最小修复：cutoff-only Stop，runtime策略2

原advisor的`advisor-narrow-02.txt`结论：B1已解除，B2主路径正确，剩Stop接受actual=Some却未消费/保终点的阻断。
本次选择其最小策略：**StopForExit只接受actual=None**。Some无论字段是否看似有效，都整次Invalid，
不更新runtime、不消费成功序号、不产生关闭claim候选；不能把已经传入的实际来源忽略后保存。
ActualCoverage只能走既有SettleAfterExit认证和known_actual_end单调保留路径。
root不得为通过入口而把已取得的实际数据直接take/丢弃：须保原Host来源/待核算证据，并在后续明确核算命令中提供；
本模块没有新增来源I/O，真实来源持久性和同事务SafeExit/ledger仍为root责任。

**策略/兼容处理先于复用旧档：** Runtime新增必填strategy_version=2，无serde默认，指纹域改为education-runtime-v2。
初始化及恢复重放图像均用2；plan/restore拒绝其它版本。缺字段的frozen-01/02候选快照直接反序列化失败，
显式旧值也不能恢复；prefix待核算cutoff.actual=Some额外拒绝，不会在加载时清空该证据。
这也拒绝已经压缩掉旧Some细节的旧候选，不能默认为修复后合法历史。
Opening仍为v1，registration_origin算法/原纯Cohort规则不变；本运行策略不是发布tag，也未修改共享版本/根/导出。
不提供自动迁移：若root确有旧候选数据要保留，必须先按原Host/receipt证据完整重放或另行审定迁移；
证据不足则明确阻断，不能只补字段、重算hash当作合法升级。01/02旧冻结原件完整保留、未覆盖。

新增两项测试源码：Stop携带已认证外形的实际区间[1,100]时入口拒绝，hash/序号/原claim/状态全不变；
旧缺版本与显式版本1拒绝，不静默升级。
原多日认证测试加强为：先消费一批并记录真实终点259213，**先压缩/反序列化恢复**，
再尝试把终点缩成86401并finalize，仍Boundary且状态不变，之后真实剩余前缀仍可按批消费。
这是合成源码反例，16项均not_run；没有运行编译/cargo/rustc/fmt/脚本/Store验证。

本次仅修改专属education_runtime.rs、tests/education_runtime.rs和本handoff；education.rs保持前次字节。
共享核心只读，无提交/发布/Issue回填/清理；原advisor尚未对本次最小修复窄核，不预填解除结论。
实际文件已读回核对后重新冻结，SHA随交付消息给root；当前容量中断通知不作为完成或执行验证证据。
