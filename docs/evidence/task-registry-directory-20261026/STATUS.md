# #135 通用目录与原消费者实际接线验收

2026-10-26，root唯一共享writer，实际1+0；当前验收结果齐备，尚待阶段发布及Issue关闭回读。父#101的未来通用provider、策略与全局预算范围不由此关闭。直接集成，无PR。

## 实际行为与完整子合同

目录成为原Sim RunDue的真实入口，固定Air、Service、Supply、Timeline四provider；按provider/scope/clock隔离原source、需求和候选身份。原Timeline聚合继续P0–P5同步责任，无可选模型时已接受的目标也不能遗漏。多个同TaskKey来源共享原P5执行组，一次原Advance，不重复结算。不新增worker、权威队列、时钟、完成位、持久字段或授额器。

完整SourceBinding保留原Air Job、Service/Supply DueView和Timeline Progress；schema与内容版本分开，Store sequence只作原CAS。已处理覆盖不是缓存采样时间；当前来源没有认证缓存时cache age为Unknown、caches为空，继续必要工作。真实缓存命中/过期只有原纯组件规则证据，不冒充游戏provider命中。原P4屏障、P5预留宽度、下一边界准入、同刻零窗和原writer提交门保留。

|#135原合同|实际证据|
|---|---|
|休眠不丢必要到期/故障，Unknown不假完成|source07新增9项，实际无可选模型的原目标、原Air版本0→1→2、原Supply Start/Stop/无关交易和同刻零窗；source08原6组覆盖P4重试/取消/硬背压、服务/供给/V2必要责任|
|真实source版本/CAS/游标、重复dirty幂等|完整原绑定与稳定schema分别断言；观察不写Store，原提交后旧目录被拒绝；同Store/clock/完全相同TaskKey的Service/Supply仍分区，同P5页一次提交，与原直接执行参考相等|
|恢复重建，不补造事实或第二时钟/队列|真实Sim退出/重启、checkpoint、独立冷开及原票重试；未消费terminal后查原票继续；无原target时NoWork不创造未来推进|
|原consumer联合恢复及边界/预算拒绝|source07联合原角色路径；129条原Claims只作准入拒绝代理，失败原状态不变，再走原width32报价；source08真实V2及source10真实借款/区域付款均恢复、原票一次，与原参考相等|
|工作区/重叠与原资源边界|当前同源joint role2个、V2 role5个观察点，原59MiB角色分配失败0；64MiB池全额归还；完整Store和原selector工作区包含在角色观察内，32KiB仅metadata设计估计而非完整峰证明|

## 当前固定结果与正常游戏补证

- source07：最新9项全部通过。source08：受影响原task_registry_adapter、task_registry_dispatch、task_registry_due、service_needs_due、continuous_supply_due、water_treatment_due六组全部通过，均使用SHA `59ca94409913122fd3b188ecd7cddf0d3a280f926eda8eeb1efee17707d3655c` 固定headless。
- source06：原纯task_registry13项通过，只证明组件版本/覆盖/cache/limit规则。当前456编译输入逐文件与source07快照相等；core/Store/Cargo未修改，24无关WIP逐文件保持。
- source10：正常原借款500分、到期30秒；经目录实际续12页，中途退出重启、保存冷开，借款原票和每张C票重试不二次执行。原开局区域责任1000分同时支付，借款人/出借人最终201000/599000分，总现金3000000分；最终存档与原直接执行参考一致，无新未来target。这是实际业务，区别于Claims准入代理。
- source01客户端漏收中间事件导致失败：只增加原事件名白名单，未改帧/限额/协议身份。保留完整失败世界，复制件查原票/状态，未对不明结果盲发新票。source09借款预期漏计既有区域付款，原游戏正确执行；source10只修独立预期并增加原区域责任断言，未改业务以迁就测试。
- 原advisor最后只复核删除无写入间隔的重复collect及资源结论边界，无具体阻断。原handler提交/重绑定前仍核完整原上下文；public validate保留给延迟快照。advisor未重跑测试，不替代运行证据。

验证采用正常玩法、退出/存档/冷恢复、回报丢失及原操作重试；不通过篡改存档制造安全认证任务。源码09/10验证小程序及确切编译参数保留在完整原件，不新增产品调度机制。

## 资源、原件和限制

Mac16,1/32GiB、Rust1.85.1、HP P900 USB/APFS，原唯一target/work串行。joint role最大已观测requested56589064B，V2为56670760B，均在原59MiB授额内、失败0，退出后原67108864B全回。父测试持有另一个完整参考Store是测试工具开销，不能当游戏免费内存。CPU时间是带进程/IO的有限执行记录，不是持续推进速度承诺。

完整源码ZIP、固定程序、正常/失败日志、独立预期、输入、原票/WAL/checkpoint和世界原件保留：

- `/Volumes/HP P900/dev/MyCity/work/task-registry-directory-20261026/`
- 原角色短根 `/Volumes/HP P900/dev/MyCity/work/r35`、`r36`、`r37`、`r38`。

五根合计事前512MiB预算，本轮静态核对约513.6MB逻辑大小；索引不替代原件，不宣称APFS释放量。`verify_evidence.py`只读核固定源/程序/结果/资源/WIP/磁盘，不是新的游戏测试。当前不清理仍被引用原件/target。

全整机native/OS/stack峰、诊断8MiB与全局M0联合门、真实16GB、持续快进/长期增长、UI/render/GPU、未来认证缓存provider均未验。M0冻结负载/标准、normal64MiB、角色授额及默认栈不变。完整非UI目标仍持续；#134缺真实出行/到返来源，后续按具体依赖消化#136/#137，不新增源码模块批次。
