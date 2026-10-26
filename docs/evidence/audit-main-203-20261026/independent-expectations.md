# #57 → #134 首条真实本城police链独立验收准备

2026-10-26。固定v0.5.202 / `6e4b20feb1df9c00d0a28e58ab9c123d44cdb2c7`。已读父目录 `ownership-and-acceptance.json`；当前root共享1+独立准备1，不扩线。本文件与同目录 `independent-inputs.json` 是唯二输出，合计远小于16MiB；**未编辑canonical任何代码/文档、未建树、未编译/运行测试/脚本/fmt/基准/GPU、未提交/PR/Issue写入**。未重新审计已关闭#149。不联系旧停止主会话。

本文提供独立条件输入、手算oracle和拒绝/恢复判据，不是root输入已登记、代码已接线或验收通过声明。JSON缺源用null/unknown，不用Fake、布尔或非零ID代认证；它是有限约束骨架，**不是可以直接提交的合法Opening**。所有提议参数须由root冻结并通过原Opening/原票/权威注册；缺任一必要来源，正确结果是unknown/不接纳。

## 1. 原组件和实际可复用接点

| 实读实际路径/API | 可复用事实 | 对本police链的实际缺口 |
| --- | --- | --- |
| `service_delivery_runtime.rs`：Opening v2/Policy、NeedKey、模块级prepare候选、Authority::prior/source/verify、Source/Admission/HostActivity/WorkPrefix、next_due及unsettled getter | Task<=4；Development；kind1与occurrence_day0可表达；400/1200/400费、carry、历史现场、截止/收尾和恢复ready门已有规则 | 当前lib未导出此module，World/Action/Timeline未见delivery消费者；当前Policy是ExplicitHostActivityV1，不能冒称逐边真实交通。#134需显式新policy/实际producer接缝，不能只填Host真值 |
| `service_needs.rs` / `service_needs_authority.rs`：原HousingLease entry、P5 due_page、DueView、Store原票认证 | 原真实自然needs及来源/相位/截止查询，可引用原household/source/generation/kind/occurrence | 实读仍V1当前delivery_ref=0/无admit消费者；#134需显式WithDelivery profile/链接/响应与期限结果，旧V1 wire/token/恢复保持 |
| `qualification_authority::{check_job,check_job_at_boundary,next_source_change}` / Engine::check_job_qualification | 原qState exact范围分页、Police与Driver位、真实root/revision/时窗变化；资格源到期在integer valid_until+1切段 | 没有本crew的真实member范围/Police2人/实际driver资格注册。学历/工资/venue不替代资格；未知不能当Eligible |
| `member_authority::{initialize,validate_with_facility}` / `member_activity::Ledger::{prepare_begin,prepare_settle}` | 唯一Housing人口命名空间/原epoch；半开区间互斥与原原子候选；其他活动和教学/设施劳动不能重叠 | 当前member_authority明确拒绝ServiceWork且注明无消费者。#134必须把真实Task和原成员行一一核对；不能绕过validate或另开成员表 |
| `transaction::Action::{ReserveCapacity,OccupyCapacity,ReleaseCapacity}`、原Resource/Lease/CapacityToken | 原唯一Count池和token；reservations+occupied<=capacity，预留转占用不二扣 | task crew/vehicle/tools与每条drive边Count的真实映射未注册。原time_capacity/venue/supply管理资源有专用门，不能用通用Action旁路；原lane资源不是RoadEdge.capacity_people |
| 原 `Protect/PayProtected/ReleaseProtection/Transfer` 和Timeline Payroll | 原钱/保护/原票及计费carry可以按同事务引用，非新钱包 | 没有police工资/雇佣/member-payee合同；`facility_work::Role`当前仅Electric/Water，不是现成Police工资provider。#134须明确原劳动/工资绑定，不重开有限已验收#132也不把其角色改名即获权 |
| `transport::Opening::{validate,input_fingerprint,assign}`、RoadEdge/Entrance；Store `town_transport.rs`及`spatial_transport_updates.rs` | 已授权有向图的有限结构/来源及版本绑定路径；原daily群体交通分配输入/不可变证明可参考 | assign/PathReceipt仅群体分配，未见一辆police车的逐边位置/offset/能量原Timeline消费者。`capacity_people`是人数容量，不可换算车辆槽；现有读图方式与本profile准入仍须root冻结 |
| `assembly::Commodity::Electricity`、`assembly::SourceKind::InventoryCapital`、`assembly::OpeningStock`、`assembly/typed_opening.rs` MilliWh | **原电力Stock合法存在**。可构造显式有owner/quantity/basis/唯一InventoryCapital来源的有限开局库存。assembly检查source种类/金额/不重用/全部使用 | 当前运行哪一笔电量可用、车辆能耗/所有权/用途/成本和补能来源unknown。不能从continuous_supply未来发电凭空credit，不能另造电量池 |
| `continuous_supply::{due_view,cursor,source_root,validate_prepared}`、V2 `product_quality` / Engine::water_product_quality、water_treatment::LotQuality/SourceRef | 真实已生产Stock、化学成分/来源和原来源范围可以只读复用；原供给/设施工资已有有限消费者 | 当前无面向本police外部载荷的完整消费口，producer额定值/未来产出不是可支用量；处理后的化学Judgment不是police用水许可。用途/purpose_ref/接入口/覆盖unknown则不接纳 |

这些是代码接口可复用性，不是声称本场景已装配来源。具体接线归当前root #134；#74（车队）、#73（旅程）、#118（canonical资源）、#55（资格）是已有领域来源/缺口归属，不能把其候选当真实履约。#133的既有水处理能力可按原边界引用，不扩大其已验收结论为police用途许可。

## 2. 第一条有限输入必须冻结的前置门

1. 显式noRegion、固定原Housing Some(city_id)、本城Development新版本，原Timeline stable last_complete及受控P5边界；禁止将#74 Interaction输入换标签。region存在/切换/迁移能力未支持则拒新配置，既有责任不删除。
2. 原自然police need必须真实存在且未被接受：原world/branch/household/source/generation1/kind1/occurrence_day；08点发生、次日08点deadline。oracle使用相对发生日day0，不断言当前seed/户的真实相位就是0。必须有严格更早的原合法entry，不能客户端填需要。
3. 2名原成员Police资格、实际司机Driver资格，原登记人口namespace/代际/Source覆盖；相容雇佣/工资合同和合法班次、休息、日周工时，原唯一成员账排他覆盖准备/去程/现场/返程/整备，不能只占现场。
4. 原独立crew/vehicle/tools及可执行可返路线；每段drive边显式引用原Resource Count。节点/边/入口/权限/version来自已获准图，正确household单元接入组和可用门不能由楼外位置猜。往返有向边是否共享同一物理冲突组也必须明示。
5. 原服务付款人、供方、员工payee及足额原钱包/保护；服务费、员工工资、实际去返能耗和现场输入分别有来源。保护不是付款，未来服务收入不是开局已到账工资资金，安全滞留/返还保障未知不可承诺。
6. 原finite库存或已经真实交付的metered供给：电量、车辆消耗计量与现场水电；水质、用途许可/使用方/入口/时窗/余量独立验证，不凭商品名、chemistry合格或量够就通过。缺源必须unknown，计算额度耗尽只是unresolved。

JSON所有actual ID/root/contract字段仍unknown；符号station/junction/door、out_A等只属于待冻结oracle，不能给真实编号后倒说早有注册。按原规则可构造有来源测试Opening，不代表这些运行事实已经存在。

## 3. 独立名义手算oracle（conditional，非当前事实）

拟定有限测试图：station→junction的out_A 120秒，junction→正确door的out_B 180秒；反向back_B 180秒、back_A 120秒。每段能量分别6000/9000/9000/6000mWh，即在本oracle段内50mWh/秒。车辆槽拟每物理lane Count1，out/back共享与否必须root实际注册；不从人数容量换算。图时间与能耗是待root冻结的测试参数，不由ETA生成实际事件。

| 原时间（相对发生日秒） | 条件下已实际完成的区间 | 独立预期 |
| --- | --- | --- |
| 28800，08:00 | 接纳同原need，开始准备 | 原crew/vehicle/tools/member与必要保护已原子落实；服务费0，现场量0 |
| [28800,29100) | 准备300秒 | 占队/成员/车辆；不能记现场工时 |
| 29100 | 原Timeline实际离站/进入out_A | 服务费400分；edge A占1车槽；不能同时断言到达 |
| [29100,29220) | out_A 120秒 | 扣6000mWh；29160只能处于A边60秒offset，扣3000mWh，不能在junction/door |
| [29220,29400) | out_B 180秒 | A已退出才释放其lane槽；B占1；再扣9000mWh |
| 29400，08:10 | 原Timeline实际到正确door | 从need到场600秒；现场工秒仍0，服务费仍400 |
| [29400,33000) | 共同2人实际检查3600秒 | 只记3600共同秒，不是7200服务秒；100000mWh+10000mL，现场费1200分 |
| [33000,33180) | back_B | 消耗9000mWh，不增加现场工时 |
| [33180,33300) | back_A | 消耗6000mWh，33300才实际回站 |
| [33300,33900) | 整备600秒 | 全队/成员/车辆工具责任保留，不能用预计33300或已完工提前释放 |
| 33900，09:25 | 真实整备完成 | 再400分，累计2000；随后只有全部责任/历史工作/款项依法处理的原事务可Close/释放 |
| 115200，次日08:00 | 原截止封账 | 完整来源与消费游标齐备时冻结arrival29400/work3600一次；早Close也不能漏此必要结果 |

名义合计：每人出勤5100秒，两人10200 member-seconds；现场有效3600共同秒；去返30000mWh +现场100000mWh=130000mWh，水10000mL。准备/等待/整备的额外设备消耗若实际profile非0必须另计，本oracle暂仅声明这些段的drive消耗0，**不是全设施底座用电0**；不得把供方站房本来承担的用电藏入此服务配方或声称已经覆盖。

原Stock的条件算术（不是当前库存）：若root以原InventoryCapital合法注册200000mWh有限电量、授权20000mL水，则名义扣后分别70000mWh、10000mL；owner/basis/source/用途均未冻结，不可提交该Opening。不能把generator尚未生产的130000mWh当同等库存，不能把water_product_quality返回量再额外credit一份水。

工资独立：真实工资合同/费率当前unknown。为一个可手算**待批准测试合同**，若root明确2人各1200分/小时且5100秒全程有薪，则每人1700、合3400分；按准备/去程/现场/返程/整备分别200/200/2400/200/400（两人合计）。服务费2000不是工资3400，收入与工资成本不得互相抵销后少记；供方须有实际已保护工资资金，输入/车辆能耗成本另有真实lot basis/费用，当前金额unknown。两人真实payee分别1700，不凭同一crew虚构公共工资账户。中断等待若依法有薪，按真实延长时间增加，不从按次上限偷偷免掉。

现场分段carry：完整比例energy=100000/3600mWh/s，water=10000/3600mL/s，work fee=1200/3600分/s。第1秒整数27mWh余2800/3600、2mL余2800/3600、0分余1200/3600；补3599秒整数99973mWh、9998mL、1200分，合计配方且余0。任意分页/存读档不得重置分母/余数。600秒历史前缀为16666mWh余2400/3600、1666mL余2400/3600、200分现场费；不把600共同秒乘2再收费。

## 4. 关键拒绝、竞争和实际失败的独立判据

- **同车/同crew/同成员**：第一任务全程持原资源/成员排他；第二need仍保留waiting，不能新建无保障Accepted。不同车辆而同一成员与教学/设施劳动重叠也拒新任务；资格并不授予同时工作权。Root实际角色种类未支持ServiceWork时不能绕过现门“先接纳再修”。
- **同路槽**：另一路已接纳真实车在out_A的Count1占用时，本车在上游合法等待，不进入/不扣下一段完整能耗/不造到达。29220实际退出后同秒合法下一车可进入，半开边界不允许双占，也不多释放。原edge原物理Resource若被其他模式/任务引用，不能开副池绕冲突。
- **新派遣关闭**：未接纳/未实际出发的计划不得新承诺；已有真实离站/在途/返还不因当前ClosedToNew或取消advance而消失。关闭UI/新派遣标记不能阻止原Timeline必要安全推进。
- **途中闭边**：29160位于out_A中段时，close-to-new只禁止新进入；原ingress/退离权和安全通过规则明确时按原规则继续或安全退离。若是物理unsafe-in-transit，不能默认继续穿过；留实际edge/offset与占用/成员/资金责任，等待合法替代/返还。两种闭边不可用同一bool混为“已到达/已回站”。冻结graph版本与已接受责任不能被新快照抹掉。
- **能源不足**：接纳前若原可用电量不足完整130000mWh（以及profile其它必要消耗/保护），无合法保障不得接纳。途中原可供能量突然失效则只推进真实可供前缀，保留实际位置/责任，不透支未来发电、不瞬移完成剩余边。29160断供时不得写junction/door；已扣3000mWh不退。原返回备用量是同一真实Stock的保护归属，不再新增一份备用库存。
- **取消**：未离站合法取消按次费0，原租约/成员/资金在原事务依法释放；已出发取消不把车送回origin、不释放当前道路槽、不退已耗能/已实际工资。新返回路线须有真实授权与能量，不能因“取消成功”清任务。
- **原qualification/供给/水用途/工资来源缺失**：unknown/ineligible按真实门区分；不能借一个全局非零root、Fake provider或阈值猜成成功。只有求解预算耗尽则unresolved，不产生“真实缺水/无人”等断言。
- **费用与财务**：实际Depart400、现场认证前缀按carry、实际返程整备400；接受保护不收费，预计完成不收费。真实crew工资/输入成本按原合同和lot basis另记；已耗水电不退、未付/补偿原债务不删，不能用2000服务上限替代全部安全收尾资金。

## 5. 不得被简化的逆序工作和恢复判据

1. 正确到场29400，现场来源Unknown，30000实际返程、30600回站、31200整备：允许真实阶段推进，但冻结onsite_end30000，原[29400,30000)历史责任保留。当前Source时间31200或更晚认证600秒后，工作600/现场200分/相应实际水电才入账，physical phase仍Reset/位置仍station；Close之前无资料则HistoricalPending，不能把Unknown0化或造返程来逃工时。
2. Source覆盖到115200但现场消费游标29400，不许直接封0。须先消费截止前认证区间（在原onsite_end已冻结时目标min(deadline,end)），再封600；Source时钟不回拨。若仍现场满3600工秒于33000即WorkComplete，真实后续零工时前缀须能消费至deadline，再封3600，不得强制造返程。
3. 在out_A29160、A/B换边29220、在场第1秒29401、历史区间pending、回站未整备、Closed未deadline各保存/加载/进程中断冷恢复：继续同delivery/need、原edge+offset、车槽/crew/member token、能量/输入累计与carry、工资/费用目标、onsite_end、源版本和due。不能恢复为Waiting、重新发车、重新收400或删未封deadline。
4. 原Envelope完全同payload重试只读原结果，不再扣能/占槽/付费/消耗carry；同operation换route/need/member/金额或同host事实换新operation重用，都须原票/事实序号门拒绝。Stage state CAS与原source root/revision同时校验，不能混用新图旧资格。
5. 原Store checkpoint/WAL/完整archive原件恢复认证后才ready；半提交失败不允许只留下扣钱或只有arrival。读档之前未认证不能prepare。#149已有已验收target/必要责任语义直接复用，不重复审计或因调用CancelAdvance而假释放任务。

## 6. 待root冻结与实际验收入口

阻断输入清单：world/branch/city/实际原need；真实两名成员与driver/qState来源/合法工资合同和payees；vehicle/crew/tools/逐边Count原resource/token映射；已准图/入口/版本/逐段移动与能量profile；原电力Stock及InventoryCapital owner/basis/权限/可用额；water lot/质量/明确police用途许可/临时接入及实际余量；服务payer/供方/工资/输入/安全延迟保护资金；原needs WithDelivery新版本/成员消费者/World/Action/Timeline/Store/恢复桥。未冻结前，JSON usable_for_admission=false。

当前准备不激活任何shared文件。root按ownership记录先登记确切新接缝，再写原adapter及原正常角色Store验收：首次合法输入、拒绝/竞争/边界、实际逐边、原票一次、途中冷恢复、历史资料晚到、原资源/成员/费用守恒及预算峰。准备中的数字/结构不证明normal64MiB、共同8MiB、整机16GB、性能/四速/8h或完整#134通过；所有实际运行结果均not_run。完成即冻结本两个文件，root统一后续验收/Issue回填。
