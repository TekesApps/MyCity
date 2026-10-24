# #57 LS42 三类自然 need 持久模块交接

2026-10-25。固定基线 main v0.5.181 `ae32d7dc7db1c763a3b2d18534b9748bcd4d24b2`。本阶段只新增自然 need 持久模块，root负责原World/Engine/Timeline/Store/receipt的真实接线；模块候选和测试源码不是接线/履约验收。所有编译、fmt、rustc、脚本、测试执行、性能/压力/GPU均 **not_run**。无commit/push/tag/PR/发布/Issue回填或关闭，由当前总协调统一处理；旧主会话停止，不联系或唤醒。

## 树与全部原件

旧v151树已由协调方退役，不重建、不修改其归档。先核对实际repo AGENTS、HEAD、worktree清单，无可复用本线树；按本次授权从actual repo建标准独立Git树（不复制主repo WIP）：

- `/Users/frankqin/.codex/worktrees/issue57-service-needs/MyCity`
- branch `codex/issue57-service-needs`
- base `ae32d7dc7db1c763a3b2d18534b9748bcd4d24b2`
- `crates/city-core/src/service_needs.rs`
- `crates/city-core/tests/service_needs.rs`
- `tests/fixtures/issue-57-needs/literal.rs`
- 本文 `docs/handoffs/issue-57-service-needs.md`

仅上述四个新untracked原件。共享export/Cargo/World/Engine/Timeline/Store/decoder/版本/设计文档均未修改；LS42共享冻结正文root维护。没有写旧service_core，没有建第二公服池/钱包/钟/认证provider，也没有碰target/work或其他项目/进程/归档。

## root冻结规则及身份协议

已读实际AGENTS、LS42、原Housing/Scope/model_rows。固定noRegion单city `LocalDevelopmentNaturalNeedsV1`；root须确认原OpeningHousing.city_id为Some并精确相等，拒绝不支持Region/迁移。原Timeline稳定last_complete作为该profile本城Development时间；P5内部受控boundary不是已完成last_complete，due页不要求phaseNone。模块不读取Region.development_elapsed，不倒推混合历史。

Opening：version=1/profile、Scope(world/branch)、seed、city_id、按household_id严格递增的至多32原household注册。每行household_id/source_id/generation=1/people。source_id是原注册身份；generation=1仅本固定注册寿命版本，不称通用迁移代际。root须精确对照原Housing，无新户/分裂/换代自动注册。people=0不能接受entry或生成need。source_id不得重复，city0和空注册拒绝。

**唯一token协议（按root最终冻结，无另一套编码）**：SHA256(`b"MyCity/service-needs/household-token/v1\0"` || world:u64 LE || branch:u64 LE || household_id:u32 LE || source_id:u64 LE || generation:u32 LE)。摘要前8字节按BE解释u64。phi以checked u128(token)+u128(seed)+31*u128(kind)后mod P，kind为0医疗/P90、1警务/P180、2消防/P180。合法u64 seed不因u64中间和溢出而被拒绝。没有随机抽签/额外试次。

每类符合相位的游戏日08:00发生、次日08:00截止。首次发生严格晚于首次合法租赁接受entry（同秒08点不发生），不补发entry前历史。entry不是实际交通到达或通用入城。只由root成功HousingLease接受事务调用，entry含at和原Envelope.operation_id；失败不写。后续同家搬家/重租不改首次entry/phase/发生覆盖。Opening/new默认全部未entry，绝不从已占用状态猜过去。

## 精确API与持久布局

公开查询/结构：Opening::validate/registration_fingerprint；stable_token/phase；Runtime::new(opening,at)、opening/covered_through/pending/households/household/current/summary/latest_deadline、fingerprint/registration_fingerprint/validate/is_ready及owned/candidate/restore预算函数。

root-only crate-private：

- `record_legal_entry(household_id, at, operation_id) -> Candidate`：需ready、无pending页、at==covered_through，绑定原接受点。首次设置entry/三类下次发生点，之后no-op保留原source。调用者无法提交任意entry证明。
- `prepare_due(at,max_households,operation_id) -> Candidate`：1..128户预算；0额度返回Budget，只未解/背压，不生成短缺。允许受控P5 at，不能跳过next_due；pending只能继续同at。
- `Candidate { expected fingerprint, next:Box<Runtime>, page }`与matches，root消费同prepared root并与effects/原票一次提交，候选不直接改共享权威。
- `mark_authenticated() -> Result`：仅root完成原Store恢复认证后调用；先validate，再置ready。`ready #[serde(skip)]`，new=true，deserialize=false；业务entry/due均未认证拒绝。validate本身允许未认证供恢复结构检查。root需验证实际entry原成功HousingLease原票、offer/hash/caller/at，空无entry可按root契约认证，不能仅看op非零。

Runtime持久Opening、enabled_at/covered_through、Option<DueRun>、同序最多32 HouseholdState。DueRun含at/next_household/subphase：先**所有旧deadline**，再所有occurrence；每次调用只处理一个subphase的至多max_households，最多96个Effect，不同页同原op可由at/subphase/户游标稳定定位。partial page与状态同Store发布，pending期间next_due返回同at；仅最后occurrence页清run并设置covered_through=at。root不得先前进last_complete。没有due时仍应在每个P5发布前调用，直接推进coverage、不跑两相due页；但结构校验/固定背板复制/流式hash仍有有界表成本，整个调用不是O(1)。

每户：首次Entry、三类Current、三类next_days、三类Summary、最新DeadlineResult、当前生成/截止原op引用、历史chain anchor/last_operation。Current为repr(C)四u32，明确16B：occurrence_day/deadline_day/delivery_ref/flags。flags仅0空、1未接纳等待、2已冻结未接纳零交付；delivery_ref本版必须0，反序列化后validate非零或不支持状态拒绝，不能回收真实在途任务。最新deadline另外保留到下次deadline，下一周期复用Current也不抹其原源/结果/op。

本版没有admit、dispatch、actual work、deliver或资源/钱款成功接口。完整覆盖下canonical确无接纳的等待到deadline，封一次ExpiredUnadmittedZeroDelivery；这证明本profile未接纳零交付，不证明“真实资源短缺/没有疾病事故”。没有处理完只pending，不封结果。deadline后的已闭合无任务槽可在后续周期发生点复用；必须由root先把前页结果与原票持久提交，不能将候选内已封当作已落盘。Future非零delivery_ref/任务状态需要新版本和原生命周期接口，本版拒绝，不能隐式回收。防御式reminder分支不授予新need/费；当前无admit且每期先处理deadline，正常轨迹不可能带任务跨周期，本版恢复拒绝非零reminder计数以避免伪造未来责任状态。

Effect携带household_id/source_id/generation/kind/occurrence_day/at/event；源键另含Opening world/branch，rule/profile版本不参与重领身份。每户chain按原entry或Effect+operation滚动，计数checked。这些摘要/锚点**不能恢复丢失的完整历史**；只有最新current、最新deadline及其原票引用精确可查，更旧明细必须读原Store保留来源，缺明细返回unknown，不能从计数重建出假细节。

## 校验与预算（未执行测量）

validate检查注册排序/身份、entry时间/原op、三类相位与strict entry、下一发生日、当前deadline与计数一致、最新deadline、unsupported delivery_ref/flag/reminder、覆盖与pending游标前后分区，防止跳过旧deadline或重复发生。所有日/周期推进/计数checked，日范围u32，时间乘加用u64；进入不可表示后续周期的地平线前报Overflow，不擦旧状态。没有额外全局receipt序列或事件调度器。

原LS42 Current目标16B/类确实仅cover Current：32×3×16=1536B。**额外**注册、首次entry、next_days、counts、最新deadline/op引用、chain和全局coverage/run都另计，不能声称原16B包已免费容纳。预先为全部注册户分配三类当前等待及来源元数据，不借2048已接纳服务槽。root已冻结此可选32户service runtime上限16KiB，全部单列计入原Engine4MiB/role8MiB/normal64MiB；旧World base128KiB gate不变，可选载荷另列。候选、Page effects、解码和恢复旧新对象另计并发峰；本文件不抬额度，不声称32768户或联合16GB通过。

raw费用以源码const公式供root串行打印实际sizeof：

- `Runtime::owned_bytes() = size_of::<Runtime>() + 32*(size_of::<Household>() + size_of::<HouseholdState>())`，含两个Rows固定背板，Runtime内ready和Opening头已在sizeof内。
- `Runtime::candidate_bytes() = size_of::<Candidate>() + owned_bytes + 96*size_of::<Effect>()`。这是候选额外resident/Effect背板，root实际峰还要加当前旧runtime、Envelope/receipt/World准备区。
- `restore_workspace_bytes() = owned_bytes + MAX_SERIALIZED_BYTES(65536)`，保守包括新对象及完整编码缓冲；root旧对象、serde/native栈、Store证据读/验票临时区和原runtime仍另计。流式HashWriter只保留Sha256和字节计数，避免to_vec复制状态。
- Opening/state/Effect Rows全部固定容量，无增长Vec/无限事件/逐人agent/缓存/IPC/GPU。每due调用最多32户×3类，两个phase至多64户扫描；next_due/validate/hash含完整有界表扫描，非免费CPU。
- root restore须在解析前守64KiB字节、depth<=12、registered/state<=32行、fixed kind arrays3、Effect页<=96；模块本身没有对外bytes parser，不能把serde后validate当解析前门。原Store/decoder/export及预算接线归root。

## 测试源码与未验证项

小fixture `tests/fixtures/issue-57-needs/literal.rs`为手写相位算术oracle：token/seed全0时[0,31,62]，两者u64::MAX时[30,61,92]；不是成员/entry/公服proof。测试源码覆盖16B布局、u128边界、无entry/no need、无due推进coverage后真实entry、08同秒严格后续、重租不重置、三类生成/截止一次、分页与恢复、未认证不可推进、原op/CAS、错覆盖/跳due/0额度/超页、非零delivery_ref拒绝、世界分支/原注册身份影响token、最新deadline跨周期保留、地平线溢出和非法注册。Synthetic mark_authenticated仅模拟root验票完成，不等真实Store恢复通过。

未来最小命令（**只登记，未执行**）：

```sh
CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --manifest-path /Users/frankqin/.codex/worktrees/issue57-service-needs/MyCity/Cargo.toml -p mycity-core --test service_needs
```

唯一target应仍为原路径指向HP P900，work仍用原HP P900路径。合法窗口由root核挂载/路径并串行验证，不新建target，不清任何证据。root还须验证真实Housing成功/失败/原票retry与首次entry同事务、启用后Region/迁移拒绝、P5分页commit+恢复、不越必要due与same-second顺序、World/export/decoder/预算、原Store entry原票认证及资源恢复门。本阶段供给/完整qualification调度、实际交通/正确门、crew/vehicle/tools、临时水电/实际工时、资金/费用/退款/补偿、实际到返、D/满意度等均未接/未验收，不宣称完整#57或医疗消防profile完成。所有实际验证not_run。

## 冻结交付与root共享接线状态

四原件已交root，模块/测试/fixture冻结。root报告在canonical共享文件中已写可选runtime、state-v13、原HousingLease成功同事务entry、原Timeline boundary/P5屏障、Store原genesis注册比较及逐户原archive成功HousingLease验票认证，并准备真实生命周期/partialP5/WAL+CP冷恢复/181日三类周期/失败retry/同秒entry/旧输入disabled/错误注册等测试源码。这是root的源码接线状态通知，尚未执行测试，本实施树未改或独立复核这些共享改动，不提前报告通过。后续由原advisor审合并diff，root统一串行验证及Issue回填；本模块交付没有提交/push/PR/tag/发布/重复回填。最终预算按上节16KiB可选runtime门，所有执行验证继续not_run。
