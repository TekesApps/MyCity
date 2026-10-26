# v0.5.209：绑定原任务的共同执行请求（#102 有限增量）

本增量已真实接线并按影响范围验证；完整 #102 保持开放，不新增业务子任务验收或关闭。root 是共享核心唯一 writer；本轮无新派工、worker 池、调度器、授额器、持久完成字段或 PR。

## 已实现的最小业务闭环

`DescribeTaskExecution` 从原 Directory 当前选择生成版本化能力与请求。查询不写业务、不预留或归还资源；描述只表达原选择，不承诺业务准入；原票碰撞等业务门只在原 handler 判断。`RunBoundTask` 重新取得当前原路由、核对绑定后调用原 Air/Service/Supply/Timeline handler。调用者不能任选任务、改 target/page limit、绕过优先屏障或自动开启 adaptive 模式；旧 RunDue 行为保留。

摘要采用明确版本的类型化 JSON 编码与 SHA256 字节数组，绑定能力/策略/编码/源码版本、进程实例、原 worker busy/owner 状态、scope、Store sequence、完整 SourceBinding/Progress/Continuation、实际可选 TaskKey、原执行位置与 target/limit。零 revision、None 与同刻零宽时窗保留各自合法语义，不填造正版本或窗口。摘要不是业务完成或授权凭据。

执行前披露原 Sim Envelope 或 Air Job/接纳 operation，随后选中事件再次携带同一恢复身份。失败或未收到终态时查询原票/Job/owner 并恢复原结果，不能用新的 Describe+执行充当幂等重试。Client 在发送新绑定请求前保留 outstanding，发送失败仍需原 owner 实际退休。资源/Child/wait/返额均沿既有路径。

第一版只支持原可信策略的执行。请求 GPU、未实现细化或额外资源明确 unsupported/未解，不改成真实短缺，不替换策略或新建 grant。Air 原 wire4/control64KiB、候选传输8MiB与原业务 asset2MiB 分别声明；这些边界均不是内存授权。Sim 继承原角色，无伪造 child grant。成本、scratch、常驻与切换成本缺实际来源时保持 Unknown。

新文件 `city-headless/src/task_execution_protocol.rs` 为 sim_host 私有模块，复用原事实源；三个 Sim handler 与请求披露共享同一个原 Envelope 构造器。未接入 r03 旧原件中不适配的强制正版本、单一 work_unit、child-grant 假设或另一套退休状态机。

## 实际验证与证据边界

全部串行、debug/default stack，唯一外置 SSD target/work，最终代码固定在 source08：

|检查|实际父测试通过|范围|
|---|---:|---|
|新绑定请求|9|真实四类路由；查询只读/稳定；同刻 None；cap/策略/摘要/执行位置拒绝；真实无关转账使旧请求失效；成功后重复拒绝；未消费回复按原票恢复；新请求发送失败保留 owner；实际 normal Air 授权归还；原票碰撞仍由原handler拒绝且不推进|
|观察真实故障及原分类|5|不可用时钟依赖仍执行真实原事务并与独立直接参考相等；原 WalPayloadHalf/CurrentRenamed 写入切点保持未确认，恢复原票；另有3个实际冷进程，分开计数|
|原执行观察|3|原 Timeline 多页、零宽窗口、Supply；原票/checkpoint/参考/冷恢复|
|原 Directory|9|同源/同键 Service+Supply 单页，原 normal 角色，原 C 碰撞、准入、来源与恢复|
|Client 类型化接收|1|timeout、decode、identity 与断链区别保留|
|原 Air 控制/恢复|13|真实分页、局部调档、取消、自动Continue、不确定发送、broker/实际退休|

合计40个父测试通过；3个真实冷子进程另计。一个 ignored 父 helper 由真实冷子进程调用，不能把 ignored 本身计为通过。首次 client07 过滤器选中0项，原日志保留且不计通过，随后 client07b 用准确过滤器实际通过1项。详见 [机器结果](verification-summary.json)。

最终新绑定 Air 的原 normal grant61865984B，Rust 请求峰58291841B、观察时arena used56691944B，allocation failures0；1次workspace grant/1次return后原池67108864B全部可用。原 Directory normal 组另验现有 P5 同页行为。上述均不是全进程/native峰值、四路最坏峰值或代表城市容量。协议 metadata规划64KiB与观察64KiB均计入原诊断预算，capability描述16KiB/规范输入8KiB/观察12KiB保持有界，不是实测峰值或新增预算。

构建01/03/04/05/06失败与source02较早故障验证完整保留；分别为测试错误类型格式、文本抽取破坏一行、摘要String/byte数组类型、Store原票查询借用及测试mut声明问题。没有从这些构建运行新业务测试；只修适配器/测试，不放宽原门。source07的39父测试也保留；加载最新Simplicity规则后，仅删除新入口重复的格式/碰撞检查，增加真实原票碰撞反例，最终source08的40父测试通过。格式在输入边界检查一次，碰撞保留原handler职责；不扩大重构。最终 source08 全部相关输入在验证期间字节不变。

原 core/Store/资金/容量/教育/真实公服实现未改，不重新跑未受影响的全业务验收；这些仍只引用各原固定版本证据。特别是 #134 的有限牵引/旧存档兼容仍归 v206/207，不把本轮 headless 测试当 #134 全量重验。新绑定命令的真实 Busy/Blocked 场景没有单独额外实验，仍用同一个原 Directory/Client 优先屏障；不宣称新协议已覆盖全领域。

## 未完成与保留

完整102仍缺共同细化/授额接口、实际scratch/常驻/切换成本来源及更广真实模型覆盖；103兼容闭包/整组转换、104全局公平/多维预算/降速背压各自保持开放。现有Air分页/采样/局部自动Continue已在v164–167完成，未重复实现。此次业务/历史补关/缺陷关闭增量均0，源码交付积压18、待验证0保持；完整合同起点不可靠，验收周期Unknown。

完整初始/失败/最终源ZIP、固定可执行文件、生成输入、World/WAL/checkpoint/原票、命令、日志与逐文件索引保存在：
`/Volumes/HP P900/dev/MyCity/work/task-execution-protocol-20261026/`，短路径根 `work/tp102/` 同计1GiB事前保留预算；前期512MiB估算在保存更多固定程序前增为1GiB，未扩大运行时预算。恢复原件保留到后继完整验证与明确退役；hash不能替代原件。不清当前共享target缓存，不创建新tree/target/timer。24份无关WIP保持字节一致。

实际环境 Mac16,1/Apple M4/32GiB，macOS，HP P900 USB/APFS。真实16GB、native/长期/可持续速度、GPU、UI和原M0完整冻结验收均未执行/未宣称通过；原M0负载、默认栈和资源门不变。窗口为本有限实现验证起止，见机器结果，不当完整102开发耗时。
