# v0.5.210：共同绑定的 Air 精确页计划（#102 有限增量）

共同入口现在能显式请求已有精确分页执行；完整102/103/104保持开放。本增量不是新模型精细化、新授额器或新增业务子任务验收。root唯一共享writer，独立测试准备交付后由root核对并收口；当前冻结0+0，无PR。

## 实现与边界

共同capability/选择类型编码升为2。`ExecuteExactPages{plan_version,quota}`仅适用于原Directory当前实际Air分支，仍进入原`required(quota Some)`、原PageControl/Continue与原票/worker owner。Describe基础摘要绑定原选择；接受事件和终态另以`mycity-accepted-task-execution-v1`类型编码绑定完整实际BoundTaskRequest，包含初始intent、plan及全部quota字段。初始执行摘要不随后续合法Continue重写；下一turn额度仍由原ticket/turn/instance核对。

格式/支持范围在输入边界检查一次，业务准入和原票碰撞留原handler。旧capability1、错plan/非法quota、非Air请求明确拒绝，不能静默换策略或另选任务。默认ExecuteOriginal保留；模型RequestRefinement、额外资源与GPU仍unsupported/未解。元数据请求不持久化，不迁移存档；遇未确认回复仍需原Job/票/owner恢复，不能重新Describe后盲发新attempt。

这是同一精确模型的执行粒度。quota不等于CPU纳秒或内存授权；原段尾整场积分可超过cell_limit，原work counters分别报告，未知成本不填零。没有新角色、资源池、调度器或返额路径。normal64MiB、Sim原59MiB和共同诊断8MiB及原M0标准均未扩大。

## 实际验证

26项父测试按固定版本对应，全部debug/default stack且串行：

|结果|固定来源|证明范围|
|---|---|---|
|3项新实际场景通过|source03最终程序|共同入口到真实分页暂停、Observe/ReceiptProof、不同合法quota的Continue及原非分页精确参考；取消/checkpoint/不同Sim冷恢复/显式续办；旧capability、plan、三个quota下界、非Air及来源过期拒绝|
|1项执行摘要测试通过|source02|相同选择摘要下，ExecuteOriginal与精确页计划、三个初始quota字段改变均改变执行摘要|
|9项原共同协议通过|source02|四条原路由、原票碰撞、拒绝/过期/重复/丢终态/不确定发送、同页公服与供水、原normal角色；增加实际自然零窗Sim分支的exact-pages拒绝|
|13项原Air控制/恢复通过|source02|原暂停、合法及错误/重复turn、取消、自动Continue、不确定发送、原broker/实际退休与恢复|

source02的完整26项通过后，原始证据检查发现新测试的重启会覆盖同名事件日志。最终source03仅改变该测试的分会话日志和取消态完整存档副本；**生产文件字节不变**。因此只重跑3项受影响场景，23项回归保留source02原程序/输入/结果，不冒充在source03重跑。提交时仅删除新测试EOF多余空行，语义不变，原编译源保留且不重测。构建来源身份因测试文件改变而变化，各原程序对应其原规则身份；[机器结果](verification-summary.json)及work内source02-to03-reuse.json记录边界。

原始事件另核实：非分页参考和分页候选都是非空对象且完全相等，不是两个null；取消前后两个完整会话日志对应不同Sim PID，并保留`cancelled-original/CURRENT.json`、完整checkpoint/WAL/空间输入。所有正常角色观测allocation failures0，原grant61865984B，已观测Rust请求峰58294434B；参考与分页两次workspace grant/return后池67108864B全部可用。这是有限Rust角色证据，不是native/进程峰值或城市规模承载证明。

首次source01为2通过/1失败：测试预期在原P4必要任务等待中转账成功，真实原Timeline正确记录busy拒绝。测试改为检查该真实拒绝推进Store序号，从而使旧绑定失效；没有放宽游戏门或认定转账发生。source01程序、失败World与日志完整保留。独立worker初稿也保留，首次执行前root修正了边界循环和状态对比，源码交付不当作通过。

## 未验与保留

完整102还缺真实模型细化/额外授额接口、scratch/常驻/切换成本来源和更广真实模型覆盖；103兼容闭包、104全局公平/多维预算/降速背压仍各自开放。core、Store和原业务算法未改，复用原固定版本业务/写入故障证据；本轮不重复完整产品或M0验收。真实16GB、native/进程峰值、长期/代表城市速度、GPU和UI均未测。

完整初始/worker初稿/失败/最终源码、固定程序、输入、World/WAL/checkpoint、取消态副本、事件/原票与逐文件索引位于外置HP P900：`work/bound-air-pages-20261028/`和短根`work/bp102/`。磁盘事前768MiB估算在第三份固定main/test和完整取消态副本保存前修订为1GiB；运行时预算不变。保留到后继完整验证及明确退役，hash不代原件。当前共享target继续使用；无新tree/target/timer，不清仍需恢复或后续验证的内容。

Mac16,1/32GiB，HP P900 USB/APFS唯一target/work。24份无关WIP字节一致；业务/历史补关/独立缺陷关闭增量均0，18源码包积压保持。有限阶段起止见机器结果，不把关闭窗口或tag数量当完整Issue开发耗时。
