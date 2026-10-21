# 角色区域分配：测前方案

原验收fixture和业务输入／阈值不改。完整目标仍为原分级、三轮稳态、四档、正式28,800秒联合及独立审查，真实16GB无设备保持not_run。当前先验证新整Role实现，不能把组件通过当正式性能通过。

## 容量及安全机制

| 活动角色 | normal授权 | 同时存活 |
|---|---:|---|
| Sim | 59MiB | 原worker独立5MiB，合计64MiB |
| Reference／FaultReference／普通Compare | 20MiB | 不可与Sim重叠，不启动worker |
| 流式BeforeReopen／CompareSpeed／CompareEndurance | 6MiB | 不可与Sim／Reference重叠 |

这些是待实测硬门，不由历史峰值反推；不能因放不下而扩大。父broker真实扣页→子进程按绑定PID／kind／source认证→激活真实区域→构造整个角色及实际backing身份→Filled→允许worker。所有模型角色注册同一个单会话broker；之前Reference EXTRA通路已接入。额度只有父真实wait后返还并重新触页。故障注入构造OOM、持久提交后回复前OOM、错kind／旧capability、部分回填等必须实测。

角色grant = 实际arena原始块 + 激活时System活跃前缀请求量 + adapter静态头。启动历史峰加头须≤1MiB，这是激活准入门，不称启动期每次分配前硬门。激活前每次分配均初始化；激活后所有Rust分配从同一first-fit区域取得，无System fallback或扩容。启动前缀在逻辑free后物理隔离保留至进程退出，不能制造未持有信用。arena完整初始化并逐页读取；同时报告Heap实际usable和used/free，不能把尾部对齐损失算可用。

唯一`city-role-allocator/adapter`允许受审查unsafe。锁仅AtomicBool＋UnsafeCell，不分配、不格式化、不调用用户代码；不支持同线程重入或信号处理器分配。核心继续forbid。固定上游linked_list_allocator=0.10.6（默认features关闭），原依赖不升级；锁／上游Heap内部unsafe属于审查边界。本轮Miri/ASan未运行，不能由单测宣称全面内存安全。栈、System自身开销、系统库／映射和其他native不属于Rust区域上限，必须后续全进程同刻物理实测。

OOM不会产生“未提交”承诺。子退出→实际wait→返还→重开Store，用原操作ID查票／重试；禁止自动改新ID。申请过大的try_reserve失败和真正alloc_error abort分开测试，不能用注入Err冒充OOM。

## Diagnostic ownership correction before measurement

The common8MiB component contract remains unchanged. Sim2.5MiB/helper0.25MiB diagnostics are inside their full Rust role arena; supervisor external diagnostic manifest becomes3.25MiB (worker1 + layout0.75 + supervisor0.25 + driver1 + pool0.25), observer separately2MiB.

Correction to the original draft: this does NOT increase endurance L1 pressure by2.75MiB. layout-session always passes64MiB external charge and4MiB metadata; the supervisor diagnostic manifest does not control that pressure. Only the older joint-budget-probe diagnostic-charge path has that arithmetic difference. Audit three separate ledgers: original L1 budget and actual pressure length unchanged fromv70, actualnormal64MiB role pool, and external diagnostics plus observer. L1 metrics belong to the independent diagnostic ledger; the64MiB already transferred to the role pool cannot simultaneously fund them. No original records, numeric steps or thresholds shrink. Whole normal/common and physical-memory gates remain unaccepted.

## 执行顺序及磁盘

1. 独立分配器测试：对齐／zero／失败realloc原值、跨激活迁移、碎片合并、并发监视线程切换、启动超限、真实OOM子进程；保存开发失败及修正源。
2. 冻结全部源码／新lock／固定exe，执行全仓release离线、严格clippy／fmt／build。原完整旧world、新旧reader、近16MiB合法合成CP在59MiB真实区域及原full30日实际业务均须通过；原始压力与业务分开。
3. 正常压力连续120秒后，以同源码原full L1+L2执行600秒短诊断，保留完整两代及实际故障／恢复、730左右角色heap记录和同刻进程组观测。期间不编译、测试、清理。短诊断不签正式分级／稳态／四档／8h；独立advisor复核后，下一步直接按冻结标准推进正式门。

复用唯一target，无新隔离target。必要新目录：work/t13a-role-arena-business-01（当前全业务恢复输入）；work/t13a85-joint-01（完整当前联合恢复基线，真实通过并解除旧依赖后可退役旧版）；work/candidate-binaries/role-arena（验收exe／helper）；work/t13a-role-arena-inputs（合成边界压力副本，仅到验证／审查终态，生成器和证据保存后逐文件回收，原v81完整v1保留）。新联合必须同源码从零运行，不能伪装旧run为当前代码。新pressure不是每轮无条件永久留存；旧v70/v50原件仍在验证依赖内，未解除前不删。清理按进程／占用／硬链接／精确清单执行，分配空间与df分开报告。

Naming update: v0.5.84 published disk cleanup first. The planned joint directory is now t13a85-joint-01; t13a84-joint-01 was never created. t13a85-preflight holds the required actual120s readiness evidence.
