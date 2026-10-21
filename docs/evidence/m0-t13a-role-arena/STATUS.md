# 整角色Rust区域与原规模联合短诊断

仅Rust模型与CLI。源冻结f75a8a2f…，194文件；原fixture、完整期初、地形及性能标准不变。新增固定linked_list_allocator 0.10.6，原registry版本／checksum不变；完整上游crate归档保留。仅私有adapter允许unsafe，业务crate继续forbid。Miri／ASan未运行，不宣称全面内存安全证明。

## 实现及边界

Sim整个Rust分配域59MiB，与原worker5MiB共享normal64MiB；Reference／FaultReference20MiB、流式比较6MiB串行通过同一broker。父先扣页、子认证绑定后激活、实际主背板构造后Filled；激活后无System fallback／扩容。启动前缀和静态头计入grant，启动历史峰准入≤1MiB；已释放启动块物理隔离保留至退出。整块arena实际初始化／触页，后续分配继续使用同一块，不能拿地址／容量标签冒充真实背板。

运行时归还仍须实际Child wait。证据为冻结生产wait／try_wait调用链、真实故障子进程测试和逐身份协议轨迹；未独立记录内核wait时间戳。Filled证明主背板已经构造，后续诊断对象也从同一区域分配，不称全部生命周期对象预先存在。线程栈、原生库、映射及System自身开销在Rust区域外，不能把grant称为物理内存上限。

## 实测

|验证|结果|
|---|---|
|冻结全仓release|59组harness／423条通过记录，含嵌套进程；557.855秒|
|严格clippy／fmt／build|全部退出0，共用既有target|
|真实OOM／故障|构造OOM不Filled、不提前退款；持久提交后OOM以原操作票据恢复，保持一次副作用|
|旧完整Store|59MiB区域内恢复序号3013，完整2,507,418B Engine相等；243原件及副本保持|
|近16MiB合法CP|新旧实际Store读取序号2、完整1,882B Engine相等；唯一有效root，避免fallback假通过|
|原full30日真实业务|总337.220秒、经营298.426秒、冷ready0.187秒；720小时／723目标／724worker；完整Engine和全部目标原字节与v48一致|
|同源full L1＋L2短联合|观察606.617秒；600次L1扫描、两代完整快照及最新实际冷恢复；完整30日、2880查询、96保存、两项故障和计划冷重开|
|实际池轨迹|8角色、725worker生命周期；全部64MiB归还；被杀1Sim／2worker缺heap footer保留缺测|

真实业务和L1压力替身分开。L1不是完整交通／机场算法；保留原361272 native、1232426 stub、全部压力长度与原64MiB外部池预留。诊断manifest的6→3.25MiB不改变endurance的L1压力；此前“补回2.75MiB”说法已在PLAN纠正。布局、角色池、诊断三账单列，不能把同一64MiB同时用于角色和池外metrics。

完整[业务核验](business-audit.json)、[联合核验](joint-verify.stdout)、[角色背板核验](joint-workspace_audit.stdout)、[诊断账](joint-diagnostic_audit.stdout)、[覆盖率](joint-coverage.stdout)、[原始分位](joint-latency_audit.stdout)均可追溯。独立advisor实际重跑前三项只读审计且退出0，见[最终复核](advisor-final-review.md)。外层完整业务执行耗时337.741秒，与业务报告总时间区别保留。

## 资源和未通过项

32GiB M4主机；RSS采样峰3,498,655,744B，取得的footprint峰3,489,419,896B，2548次进程样本缺footprint；最大采样间隔0.526秒。不能称绝对生命周期物理峰值。swap起点3,367,698,432B、净增0，不称无swap；本轮非正常压力持续0秒。世界逻辑占用峰6,906,601,264B。

当前原始执行分层未见已具200样本层超20／40ms，但L0及稀有层不足，不能签十类正式分位。共同8MiB、常规3.10GiB整体预算、三场景各三轮稳态、四速及真正28,800秒仍未通过；真实16GB保持not_run，完整M0不签。观察器／诊断脚本通过只证明本次有限范围。

## 失败及回收

首次冻结全仓失败于broker测试三处旧4MiB池断言；修正为59MiB，内部codec4MiB断言保留。第一次源zip、日志和panic现场不改写，见failed-freeze-01及development-failures.md。

阶段终态后按[精确回收结果](retention/result.json)处理旧基线及临时输入。当前完整恢复基线改为v85两代，v50旧格式原件继续完整保留；v70退役正文后的旧布局和临时合成输入不能直接完整恢复。日志、收据、业务、index、故障、固定源／依赖／exe保留。唯一target继续复用，具体删除与分配／df口径见结果。

本阶段所删13个单链接文件删除前分配块合计6,373,318,656B（约5.936GiB）；不是文件系统净物理释放证明。回收后work14,798,016KiB、共享target1,398,104KiB。严格st_blocks保留一致性核验曾失败：19路径仅分配块减少，身份／长度／mtime／链接数不变；关键joint字节对删除前归档复核，三份完整布局重SHA均通过。失败脚本及差异保留。df／du删除前读数未持久化，结果为null，不事后重建。
