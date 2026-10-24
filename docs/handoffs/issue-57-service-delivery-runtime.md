# #57 有限本城公服履约持久模块交接

2026-10-25。base main v0.5.183 `8295ad192b33ec56dc0001a24fdd4ce52f9f4898`。本轮在canonical `/Users/frankqin/dev/MyCity` 独占新增文件，不新建树，不复制/修改主线既有WIP。不启动旧已停止主会话。不改自然needs旧版或归档service_core。

四原件：`crates/city-core/src/service_delivery_runtime.rs`、`crates/city-core/tests/service_delivery_runtime.rs`、`tests/fixtures/issue-57-delivery/literal.rs`、本文。所有shared World/Engine/Action/Timeline/member/qualification/Store/decoder/lib/Cargo/版本/共享设计归root只读。读实际AGENTS/原needs/units及原资格、成员结构；当前冻结来自root本批消息及RUN36三类规则。原service_core只作历史候选边界，未复制其Interaction clock或occurrence非零门。

**全部编译、fmt、rustc、测试/脚本/可执行验证、基准/压力/GPU均not_run。** 无commit/push/tag/PR/Issue回填或关闭。只交模块源码与合成测试源码，不能宣称真实接线/履约通过；root统一共享接线、advisor审查、串行验证和Issue回填。

## 持久策略与唯一权威

新Opening.version=2、Policy=LocalDevelopmentExplicitHostActivityV1、Scope world/branch、固定city、原population_origin/member_version、至多3条Crew（每kind仅一队）。Clock仅Development；root须验证noRegion固定city profile及原Timeline完整稳定点/受控必要P5边界，不将Region混合时间或Interaction历史引入。原needs的新WithDeliveryV2 profile由root显式新增，旧V1/原token及checkpoint规则不可全局改写。本模块不生成needs、相位或随机；NeedKey含原scope/city/household/source/generation/kind/occurrence_day，day0合法，当前原注册generation=1。

固定Recipe：medical2人/3600共同机组秒/100000mWh+20000mL/3000分；police2人/3600秒/100000mWh+10000mL/2000分；fire4人/1800秒/1000000mWh+1000000mL/5000分。每队独立真实crew/vehicle/tools原resource id，fire必须实际工具；资格仍由root从原qState exact范围逐角色验证（medical+护理、police、fire及实际司机）。Crew成员范围长度不证明资格、工作合同、工资或雇佣关系。不得以工资、学历、venue Enter、静态资源容量或非零root编号代替认证。

Runtime含固定Rows<Task,4>、Opening、ready、单调delivery_id/ordinal/last_host_event/last_at和History；waiting只查原needs，不复制等待表。最多4个已接纳/等待deadline的历史任务槽；每原队最多一个未Closed任务。Assignment仅引用3原lease tokens、原member activity、位置/route版本、保护到点及原资金/投入/返程source，绝非第二池/钱包/成员账。Task保留NeedKey、累计费用目标/实际已耗输入/carry、实际事件时间、位置、取消/期限、原need/接纳/各物理阶段/取消/最近工时原操作引用、接纳root/revision和最近资格/水电源。History只保存终结计数/链锚/最近need，淘汰历史细节unknown，必须查原Store原件，不能用hash还原旧工作。

## root桥API（crate-private可信边界）

`prepare(&Runtime,&impl Authority,Command) -> Result<Outcome,Error>`；Outcome=Ready(Candidate)/Replay(Receipt)/Wait(Gate)。Command可保存为原Envelope动作内容，含delivery_id/原NeedKey/expected_revision/Action。Accept时delivery_id=0，由模块分配非零单调u32，其他动作必须定位同NeedKey同revision。Candidate含完整旧Runtime fingerprint、Source、next Box<Runtime>、Receipt/Effect。matches同时要求ready及整个旧state CAS；root还须核同Source.root/revision/clock，不能仅凭task revision提交。

Authority由原root实际adapter实现，不是外部公开provider：

- `prior()`从原Envelope.operation_id查原成功回执，不能按变更payload过滤旧票。完整Command相同才Replay，变更Conflict。root还必须比原Envelope body/hash/caller；Replay返回原结果，不再次apply。
- `source()`构造Source(scope/city/Development/root/revision/at/operation)。Source没有Deserialize，root必须取同一Prepared权威读根；不是把客户端DTO转成root。
- `verify(command,before)`返回具体Verified能力或Gate。Admission/HostActivity/WorkPrefix/Verified全crate-private且无Deserialize；没有Evidence布尔填真API。root验证实际来源后构造，非零字段仅结构门，没有生产认证adapter在本文件内。

Admission须原need当前真实waiting、生成原票、原完整资格/合法劳动工资合同、成员全程排他、3原lease真实预留、足额资金保护、现场投入方案、合法目标接入、实际可执行路线/安全返还保护。缺任何一项返回Wait Unknown/相应门，不能先Ready再等provider。普通接纳08—min(合同到期,18点)-4h，需尚在原need截止前；准备300秒，保护费用不少于原上限、保护到点覆盖本有限service_end，实际安全返程能力由root验实，不从时间数字推出安全。

root接纳必须把Task/原need.delivery_ref/原member和资源租约/原资金保护及原票一次原子提交。本模块输出不提前占池；无provider不接受。每次工作仍按同Prepared root重验所需来源，不把Admission一次合格当永久资格。

## 实际阶段、输入与费用

Accepted→Dispatched→OnSite→WorkComplete→Returning→Reset→Closed。工作未满可合法开始返程以保存partial。Cancel只留标记/原操作，不释放资源或删除任务；未出发取消Close时按次费0，但必须root原保护释放/合法退款和责任处置同事务。已发任务不能直接Close。ResetComplete需要原Returned事件后600秒且真实整备事件；时钟到点只给necessary due，不能自动瞬移回站或伪造整备。

HostActivity明确ExplicitHostActivityV1 **真实游戏host事实输入**，不是交通provider验收。每次绑定原operation、该profile独立连续event_id、exact成员、routeversion、位置及occurred_at==Source.at；不支持回填历史host时点或任意跳号。Event id与Envelope op分离，root应在准备该事实时取last_host_event+1，同原票去重，不因重试另发事件。Depart是已经发生离站事实：准备至少300秒、在origin；即使cancel已标，真实Host仍记录Dispatched并保留返还责任，不经新接纳许可门。缺真实Host返回Wait，不能用计划/延时/venue Enter代替。Arrive只改onsite/work_cursor、不增工秒或现场费；必须正确target接入口。BeginReturn可记录可信途中实际返程位置，不由module选择路径；Returned需实际origin，ResetComplete需在origin整备结束。

WorkPrefix是root同Prepared认证的连续来源覆盖[from,through)，from必须等原work_cursor，through>from且<=Source.at。work_seconds为其中已经真实完成的**共同机组秒**，不是人数乘秒；不能超过区间秒或剩余任务量。BeginReturn冻结onsite_end；Returning/Reset仍可消费截至该点的历史认证前缀，不回拨实际位置/物理phase/Source.at。root须同时解析原member ledger共同机组在场区间、资格/合法时窗、electric/water实际同窗一次消费及来源，不能把机组全程准备/行驶工时算现场。任一覆盖unknown应Wait，不把预算耗尽认短缺。已确证区间可credit0且推进覆盖游标，不能用0跳过未知区间。真实返程/回站/整备不等待历史资料齐全；Close必须等待历史现场区间结清。

输入单位mWh/mL，按各类完整配方/必要秒复用原units::Carry逐认证秒累计，保留精确余数。root实际已耗至少覆盖本前缀所需整数消费，额外实际耗用同样累加不退回；不据此生成库存或对未认证区间凭空花钱。WorkPrefix里的来源与消费必须由root对应原Stock/电水provider/原receipt，不是本模块自行读回调认证。root准备失败不提交candidate输入、费用或游标；真实已发生的其他损耗仍由其原writer单独保留，不能以本工作未提交退回已耗水电。

累计费用目标：实际Depart20% + floor(60%上限×累计有效秒/必要秒) + 实际返程和整备完成20%，按原Carry保留分币余数；Effect给before/target/delta，不造余额。root只在原保护钱包中付本次差值，原need/delivery和同收费原票恰好一次，水电独立同源费用不双记。没有现场工作不收现场费，不免费重派；全部完成不超原表上限。Closure能力必须由root已经准备原成员/原资源释放及全部合法退款、费用/补偿/责任结清或合法移交，欠费/补偿未处理不得clear。physical reset完成不等财务Close；必要原债权仍由原权威维护。

## 截止、恢复与结束余量

SealDeadline必须同时具备root认证原actual来源完整覆盖deadline和task实际消费游标达到min(deadline,onsite_end)（截止前确有到场时）；单有covered_through不得复制旧0。未消费完整则DeadlinePending，仍保留原deadline必要责任；不允许以资料Unknown认0。未封前Work不能越过deadline，可以在当前晚Source.at消费历史前缀；晚到Arrive保留真实事实但截止前无现场区间，deadline arrival/work为0，先封后可继续late work，封后不允许改旧结果。封账不删在途/现场/返程，不释放；Closed但未封deadline也继续necessary并占历史槽。root needs V2负责跨自然周期同源reminder，不另量费；本模块从未自行发生自然need。

next_due返回原deadline或已实际回站后的reset_due；未知不能消除due，root必要P5需排空/背压，不让wallclock/取消advance抹掉必要责任。下一次Accept只可回收**旧state里已Closed且deadline已封**的记录，root须先有真实Store提交原票/need终结。root原canonical need还须拒绝全部旧完成源再admit，module有限latest/history不是全存档去重索引。

接纳时预留Task revision、域ordinal、Host event序号和closed计数的必需收尾余量。新admit/可选Work不能吃掉已接纳任务的Depart/Arrive/Return/Reset/Close/一次取消/一次deadline所需余量；另预留跨deadline两侧至多两个最终聚合历史前缀，直到其消费到onsite_end，未知不能回收此额。原provider须能按保留的真实来源汇总最终有界前缀；更多可选微步不能吃掉最终结清额。未Closed的每条任务还预留一次closed计数。u32 delivery_id耗尽只拒新admit，不影响旧Close。序号不当clock或事实认证。满表且无可回收终态只Budget背压，不删责任。没有无限历史/日志每微步复制机制。

ready跳过serde：new=true，deserialize=false，root通过原Opening/注册和原Store所有原接纳/阶段/工作/取消/关闭/截止来源恢复认证后调用mark_authenticated()->Result；未认证prepare拒绝，validate仅做结构。历史不再保留的细节unknown，root不得仅凭nonzero source/op调用mark。registration_fingerprint只hash Opening，fingerprint包含全部mutable状态和余数；Source与原根版本由root关联，不重抽随机（本策略无随机）。

## 有界预算与验证源码

raw owned=`size_of::<Runtime>() + 3*sizeof(Crew)+4*sizeof(Task)`，含固定Rows背板；candidate_bytes另计next完整state/Candidate、回收临时4Task背板、4个显式Task工作副本、Rows头与2个HashWriter。root需**另外**加当前旧Runtime及自身其他prepared candidates、原needs/member/capacity/fee候选、原票/IPC/native栈；不把候选包含当前state解释成免记旧state。restore_workspace=新owned+64KiB编码缓冲，原旧state/Store读/验票和解码native栈另计。实际sizeof/allocator/峰全部not_run，未自领新预算。

解析前root须守MAX_SERIALIZED_BYTES64KiB、depth16、crews<=3/tasks<=4及固定数组长度；module serde Rows不会增长，但validate不是解析前字节/深度门。本文件没有对外bytes parser。fingerprint流式hash不造整份JSON Vec；回收用有界Rows临时背板，所有CPU扫描<=4task或3crew，没有全城队列、GPU、独立worker/调度器/现金或库存表。

测试源码用Fake合成能力，与真实provider明确分开。literal oracle手写三类总工秒/输入/费用及police 1秒+3599秒余数（27+99973mWh、2+9998mL、0+1200分现场，dispatch400/reset400，总2000）。覆盖三类正常全阶段、未出发cancel零费、cancel后真实离站/安全返还、假Host/错成员route时点event、未知接纳保障、无inputs/错前缀、deadline半工/late不改、late到达、恢复ready/原票先查/changed-payload、candidate CAS、同队不双卖、满4槽不回收未封deadline、封后有限回收、最大ordinal/delivery id关闭余量及篡改状态拒绝。测试全部仅源码not_run，没有advisor通过预填。

未来root合法串行窗口最小命令（未执行）：

```sh
CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --manifest-path /Users/frankqin/dev/MyCity/Cargo.toml -p mycity-core --test service_delivery_runtime
```

唯一target/work继续原HP P900路径；root先核挂载/实际路径，不创建额外target，不盘点或清证据。正式验收还需root原needs V2 pin/deadline/reminder、实际原资源/成员/工资资金保护、electric/water理性量与Stock真实消费、ExplicitHost原票与正确门、P5必要due/closed-city/cancel/retry/Cold恢复/WAL原子性、源权限与不足/unknown分离、完整费用/退款/释放/预算峰等。无交通provider/全城复杂医疗消防/8h四速/16GB联合完成声明；root统一Issue回填，模块交付不关闭完整#57。

## 原advisor两项源门阻断的最小回修（待原advisor窄核）

root反馈原advisor指出：1 Returning后Work仅OnSite会丢实际返程前尚Unknown的现场区间；2 单看provider covered_through封deadline可先写假0，再晚补截止前工作却改不了原结果。原实施线在本四文件范围内修复，没有增加新实施线或共享接线。

新增持久Task.onsite_end：有真实arrival时BeginReturn将其冻结为真实return_at；有return但无arrival则None。validate精确比对该关系、arrival<=end、work_cursor<=end，Closed不得仍有未结区间。getter `unsettled_onsite_through()`显式暴露必要历史责任。Work开放OnSite/Returning/Reset的已到场任务，through不能越过冻结end，source.at保持当前root时点；历史消费只更新工作/投入/费用/carry/原票，不将Returning/Reset改回WorkComplete，不回拨位置或Host游标。即使整备已实际结束，Verified Closure也不能掩盖尚Unknown的历史工作，返回HistoricalPending；本版本没有合法转移未结历史区间的API，必须原source结清，不是假装已经移交。root需保留所需原member历史区间、真实水电/Stock来源与资金保护，不能因记录返程就删掉证明，也不能把当前member settlement cursor反推为过去0工时。

SealDeadline先核canonical来源覆盖，再核实际work_cursor达到截止前现场目标min(deadline,onsite_end)。没有完整消费只DeadlinePending，不产生任何期限结果。历史可在Source.at=115200时消费through=30000，不回拨原Timeline；先结清截止前prefix才封一次。封后新的Work只能覆盖剩余合法区间，晚工作不能改变已封work_seconds。无/晚到场的截止前现场区间为空，仍需原canonical完整覆盖证明才封0。

新onsite_end参与原state fingerprint、serde/restore结构校验和原candidate budget sizeof；Task revision/域ordinal收尾余量额外预留最多两次pre/post-deadline最终聚合历史结算。next_due保留deadline/reset原必要边界；过去的onsite_end是历史结算目标，不是倒拨Timeline的授权。root须结合unsettled getter维护必要责任并让实际返程/整备事实继续，不用永久阻止真实返程来掩盖源门缺口。

新增测试源码：Unknown现场29400→实际返程30000→回站30600→整备31200，Close先HistoricalPending，随后Source.at保持31200补认证[29400,30000)600s，费用200分/实际16666mWh+1666mL，物理phase/位置不回拨；另在Source.at=115200尝试封账先DeadlinePending，补历史600s后封正确600而非0；pending checkpoint保留并拒超过cutoff/篡改；Returning中晚补完整现场工时仍保持Returning。序号边界oracle相应增加最终历史结算预留。全部测试仅源码 **not_run**，无advisor通过预填，待root安排原advisor窄复核。根共享source/费用/Stock/Store仍由root实施，本回修不宣称真实provider通过。

## 后续窄核：WorkComplete零工时覆盖阻断的最小回修（待窄确认）

原advisor确认前两处主体修复后指出：现场工时达到上限即WorkComplete，但该phase仍拒绝Work，故仍在现场等待至deadline时，合法零工时覆盖无法消费，不能封账，也不能用虚构返程解除。本次仅将WorkComplete加入Work可消费phase，既有完整from/through/源身份/成员/输入/onsite_end/截止切段和累计工时上限保持不变。由于WorkComplete已达到必要秒数，任何新增正工秒仍Invalid；认证零工时前缀可以推进coverage/carry原票但不增加现场费用/有效工时，不改变WorkComplete或位置、不生成返回事件。Unknown仍Wait，禁止把未知当0。收尾结算余量原本已包括WorkComplete潜在pre/post-deadline最终前缀，无新增免费额度。

新增测试源码：29400到场→33000已完成3600秒仍现场，直接封账先pending、Unknown零工时来源仍不推进；认证[33000,115200)零工时后仍WorkComplete/position20/no return、工作3600/费用1600，再封正确deadline3600。另在WorkComplete checkpoint恢复后校验ready门、拒绝新增1秒和未封前越deadline零工时，正确零前缀后再次checkpoint恢复再封，不造返程/不改已收费。所有构建/测试/fmt/rustc/脚本/程序仍 **not_run**，待原advisor窄确认；共享文件与fixture未改，原WIP保留，回修后再次冻结。


## Root v0.5.184 有限实际验证回填

原 owner 的历史 not_run 交付记录保留。本次 root 原 advisor 源审/窄核与串行结果见
[固定联合门证据](../evidence/continuous-service-supply-20261025/STATUS.md)。V3组件20、
连续机组组件24、#57派遣状态组件22通过；原账联合供给9及Store恢复也通过。
仅#57组件验证，不代表真实服务履约；仅固定原账供给，不代表旧V1/V2、开放网络、
SOC/爬坡或卫生认证已完成。完整Issue保持open，无PR；root统一版本/发布/回填。
