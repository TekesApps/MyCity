# T05-B：真实来源、日历与持久阶段游标

发布v0.5.21，承接[A55架构](process-worker-and-render-architecture.md)。本阶段只做Rust权威模型与同步CLI，不实现worker池、监督IPC、UI、AgentKit或渲染。这里的“全源”指**当前已实现实验profile的全部到期来源**，不代表总体产品的交通、司法、公服等未来系统已实现。

## 1. 范围与一套时钟

`Timeline::Enable`在当前模型时刻启用协调器，此后拒绝旧Region／Economy／Boot独立Advance，防止两个执行器跨过未完成阶段。没有按墙钟追赶：活动城本地年龄随已推进的交互钟增加；关闭城仅履约、保留快照和实际增量，不补劳动／生产／城市发展。

来源直接读取业务记录而非预制“已处理事件”列表：Claims真实应付款、劳动应计／账期、Region周期义务和人口在途、Boot作业、Exchange报价／货物在途，以及Economy采购、劳动、工资、生产、交付、封日和新订单。边界取最近真实到期、目标及最多一小时的最小值；一事务最多处理limit个来源步骤／阶段转移。前序产生同刻到期项，由后续阶段立即发现。

现有Economy仍是独立四主体profile，不能和Region／新Payroll双重计算劳动；联合实验采用Region＋Boot＋Exchange＋独立家庭Payroll。放开Boot／Exchange恰好四账户的旧实验限制，账户仍由期初装配的64个总上限、合法来源、角色和合同引用控制，没有新增现金注入命令。

## 2. 同刻顺序与完整边界

| 阶段 | 当前真实作用与确定顺序 |
|---|---|
| P0 | 各Payroll先按旧活动状态赚取至本边界；推进本地／交互钟；Economy劳动到期；DayEnd利润税应计 |
| P1 | 到期报价失效；尚无司法案件执行器，不伪造法律来源 |
| P2 | Claims按ID→Payroll按ID→Region周期义务→Economy工资／月末税款→Boot完成及Economy原料到达／生产→Exchange货物交付及Economy消费交付；每条来源与实际钱物一起提交 |
| P3 | 人口实际到达、人口和运输租约一起变更 |
| P4 | Economy封日统计；只观察，不计税或付款 |
| P5 | 当前经济profile报价／下日开工；排空后标记边界完成 |

Progress持久保存target、at、last_complete、phase、lane、cursor和serial，receipt包含本页进度及实付。工作时刻可能已到at而部分付款尚未执行；查询统一带`complete_boundary`和Progress，不把半阶段结果称为完成城市。阶段冻结期间不允许插入新合同／随意付款改变后续现金顺序。允许对受信事实收件，并只允许真实已收到事实的Handoff或已完全支付事实的Archive腾位；来源frontier仍必须对齐才可继续时钟，不能跳过待收事实。

旧工资和生产由一事务拆成两页后，已付但尚未转入产成品的直接劳动列作待转入成本；检查点验证认可这一有凭证中间态。税拆成P0应计、P2付款、P4观察；旧30日最终账可与旧推进器逐字段对照。

## 3. 工资与日历

新Payroll是有限实验的劳动汇总行，不是逐人模拟。每天09:00—17:00、每天均工作为显式实验排班，仅活动home_city赚取；实际劳动秒×时薪／3600，整数分及有理余数持久保存。日结、周结、公历月结改变账期，不改变同一劳动所得。月锚31遇短月裁到月末，下一月保留原锚；按公历处理闰年、2100非闰和2400闰年。

ChangeCycle保留当前已承诺到期，生效后才使用新周期；StopWork仅停止新增劳动，旧债保留；Terminate立即开账并尝试实付，现金不足仍欠款，分以下余数不丢。Settle不会凭空补现金或删除未偿。此汇总Payroll目前无独立个税扣缴／劳动法／周末假期；旧Economy工资税仍实际运行，完整产品Payroll税表是后续装配门，不能据此签收全部P-TIME。

## 4. 有限续做票与三项阻断修复

Store在进入**一个最多一游戏小时的安全边界**前，用正常磁盘额度预留全部页的WAL／根写入字节与热回执槽；不是为任意远期target授予无限应急权。每页45120B保守额度，最多96页，即4331520B；页数由真实待处理来源和有限域表的保守步骤估算。limit固定，无法装入96页的细分页请求接纳前拒绝，密集来源默认256。事务账户dirty上限与装配的64账户一致，避免后页触及32账户后无法继续。

页结果、游标、实际账和剩余票在同一CURRENT发布。原票重试不再花额度；仅预验证成功、同边界且serial恰好递增的Advance可使用续做票，错误revision／target／身份不能借失败回执挤占槽。另保留64个有限控制槽；无效请求和其他写入都保护剩余续做字节／槽，不可绕过预留。票满时可安全拒新，不许无限接收失败回执。

必要纯阶段转移和零现金债权登记也可依票继续；正常区后来耗尽不会卡住已接纳边界。P5关闭释放剩余票，远期target保留但下段必须重新正常准入；无空间就停在完整边界，可CancelAdvance。手动检查点仅在普通空间足够且保留票额度时允许，完成续做**不依赖**在应急区执行检查点。开始新票前必要时做普通检查点腾热槽／WAL；中途不自动检查点。物理硬满盘／I/O不确定仍可能阻断，按既有unknown／重开协议处理，预留不虚构真实磁盘可用空间。

三阻断对应：满收件用合法Handoff腾位；税不再藏于P4；纯游标／零付款以有限票续到安全边界。没有放开任意付款，也没有删除欠费或历史回执。

## 5. 容量与CLI

Claims最多20000条、Payroll最多64条，共用Timeline heap≤2MiB；Engine owned上限仍4MiB。核心检查点输入上限4MiB，外层JSON检查点上限16MiB；档案仍128MiB，物理容量另外准入。普通事务Timeline副本另从运行时余量划2MiB，准入预演与真实prepare顺序执行；保存／恢复临时页归同一高峰作业。增量份额见[运行预算](product-runtime-budget-and-versions.md#t05-b动态来源份额v0521)，不按城复制Timeline。

CLI保持`session WORLD CURRENT_SOURCE operator system`，以submit启用协调器、登记义务／劳动，以暂停状态step提交Timeline Advance，查询新增timeline及欠费行；status显示完成时刻、工作时刻、处理中、续做票。每页Envelope必须带当前revision与唯一operation_id，同票重试使用完全相同原Envelope。长目标由调用端逐页续做，当前同步session没有自动后台跑至目标。`run`对Timeline返回`timeline_step_only`，不把分页CPU耗时伪装为真实游戏倍率达标。旧profile的paced run保留，不能绕过新时钟排他。

可重跑有限联合探针（输出目录须不存在）：

```sh
cargo build --release --locked --offline -p mycity-headless
target/release/mycity-headless timeline-probe /tmp/mycity-t05b-fresh
```

## 6. 实测与仍未通过

本阶段证据入口为[独立复算](evidence/m0-t05b/verified-summary.json)，含[Release测试](evidence/m0-t05b/tests.txt)、[CLI结果](evidence/m0-t05b/timeline.json)、[来源](evidence/m0-t05b/provenance.json)、[进程观测](evidence/m0-t05b/process.txt)和[校验脚本](evidence/m0-t05b/verify.py)。测试覆盖2万笔真实应付款、30日旧经济终值对照、同劳动不同账期／改期／停工／不足现金、月锚／闰世纪、处理中检查点／重开／原票、满收件、正常区耗尽续做和远期目标拒延伸，以及33次真实子进程退出。退出点覆盖纯游标页、零现金登记和最终边界，各11个WAL／根／响应位置；不是物理断电试验。

联合探针运行三种实际Store：2万笔支付、一个经济日、两天区域＋启动＋货运＋迁移＋Payroll；最终账由脚本独立复算，查询不得改hash。M4／32GiB短进程观测仅属这些有限记录，不能证明实际16GB、完整城市吞吐、worker／GPU、经济平衡、完整产业或全部M0通过。P-TIME在当前来源与固定排班子集有证据，工资税表／节假日／全产品来源保持未实现；下一阶段按依赖推进，图形和Agent仍不在本轮授权内。

## 7. 主体事件视图与后续架构整理

主体可关联待办和历史，但权威是业务状态、唯一事实及其主体引用；共享事件不按参与者复制执行。当前Claims／Payroll／到期业务表和receipt只是有限实现，不是已建好每个主体的独立incoming queue及无限history。连续劳动／流量按区间积分，事件与边界触发计算，不生成逐人逐秒日志。未来worker只计算冻结输入的候选，排程与提交仍归Sim，共享文件按版本发布且单writer。详见[A55主体澄清](process-worker-and-render-architecture.md#10-主体待办与历史的统一视角v0521澄清)；用户要求完成本阶段后另作总体软件架构梳理，当前不扩worker／通用事件API。

## 有限岗位群组实际劳动接线（#155实现中，尚未完整验收）

承接#21同存档有限城市，登记原Payroll与住房人口namespace内的精确成员群组，不从收款账户猜员工。新增显式`LocalDevelopmentActualJobGroupV1`开局登记（可选boxed `payroll_members`）；费率为**整个登记岗位群组每小时金额**，不暗乘人数。登记包含scope、人口origin/member_version、原PayrollSpec及成员范围、明确不可变游戏来源ID/版本；AddPayroll只接纳匹配的原合同。原无登记输入保持显式旧prototype范围，不补造其历史成员身份。招聘、通勤、资格发放、薪资协商/劳动法与完整劳动市场另归原Issue，不被这个来源声明替代。

P0只按此前完成边界已真实持有的`PayrollWork`活动覆盖计工秒；无活动的已知冲突区间推进覆盖但新增工秒/工资为0，不碰原accrued/billed/paid/remainder。P5在原公服/设施截至本边界的工作及释放之后结算/关闭岗位活动并决定下一段：先占有者优先、不抢占；已知成员冲突等待并在后续合法完成边界再试，不追认过去。缺来源、账异常或执行额度不足必须保留未完成覆盖、沿原页背压，不能当成零工作或实际劳动力短缺。9:00、17:00与合同结束加入既有边界切分；StopWork/Terminate/EndForExit在原完成边界停止未来劳动，保留所有既有欠薪和余数，付款仍沿原实际付款日收入。

状态放既有按需OperatingModels backing，只保存有界登记/每合同原票与活动身份、已协调边界；不扩World或每条Payroll内联结构，不加成员账、钱包、时钟或独立调度器。按原Payroll上限有界，不逐人常驻模拟。多个群组同刻开始须遵守原member Begin操作身份：每原Envelope最多一次Begin，P5每步协调一行；需要下次Begin则提前提交yield，保留target/at/P5和行进度，不提前last_complete，不伪造页工作量。Frame只保留本票transient begin_used/yield状态，恢复沿持久行状态与原continuation；原票重试不再执行。

已有Town执行路径使用实际worked_seconds和worker_output余数，可消费部分班次，不另改日产模型。设施旧账户级Payroll保护在未知成员时保留；双方有可信范围时沿同一成员账裁决实际冲突，不按同账户合并工人或钱包。Owned/候选深clone/成员候选/恢复重叠须计费；先跑原18单元128KiB、原票4096B、无登记旧业务、null/copy/同槽和默认栈，再扩新业务/恢复/normal。原M0不变。

有限验收：正常不重叠群组；公服跨9点占用导致岗位等待、退出后仅从新claim计工；反向岗位占有阻止重叠公服；两个以上群组同票yield/部分P5冷恢复；17点/合同结束释放；已欠工资、余数与实际付款收入；P0/P3/P5页保存/原票/写入中断；部分工时传到Town产量/税/库存；旧prototype与新登记输入分开报告。设计/候选不代表实现或验收通过。


#155真实Store反例补充：原core内存双群组可续，但Store按`ceil(steps/page_limit)`只预留一票，9点必须第二次Begin时失败`timeline_reservation_exhausted`（store09固定原件保留）。沿原continuation预算增加两种独立成本：已准入n组P5协调/Done/yield的保守步骤`2*n+1`，以及该已选boundary.at可能Begin且非Holding的组数减1的必要分票。现在忙也计入可能Begin上界，因为前序公服可能恰好释放；不按最终target推算。新票数经checked加法、原96页/收据/磁盘门事前预留；续页和冷恢复只消费原remaining_pages，不重新授额。原无登记模式新增成本为0，不改原M0负载/预算。新profile尚未发布，失败实验不静默补足旧预留。

成员序号只在真实Begin/Settle/Close才申请，纯Idle/Waiting覆盖不耗序号。状态和immutable登记摘要使用有长度限的流式hash，不分配Vec编码缓冲；工作区显式计prepared成员副本和临时事件候选两份Ledger，以及HashWriter/RestoreCheck/State。Store复用现有genesis登记前后比对，再沿原receipt完成ready；缺省/null/copy不能把已配置状态悄悄读作旧prototype。
