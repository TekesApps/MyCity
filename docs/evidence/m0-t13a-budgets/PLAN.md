# T13A-3b：修复诊断总账与取得完整Rust进程堆峰

测量前冻结：沿用3a的600秒L1、每秒完整361272 native＋1232426 stub扫描、0/300秒快照；L1完整manifest就绪120秒后启动full原30日，723目标/724worker窗口/3011原票不变。最新原快照必须真实新进程装载。外部64MiB预留及未填余额的压力缺口保留，本次仍不签满级joint-owned/三轮/8h/16GB。

计时仍原始9B(class u8+纳秒u64)、1微秒分桶、执行/报价/重放分层。桶计数改u32以减少诊断内存：每个raw128MiB至多14913080条，远小于u32上限；仍显式拒溢出，不截断/饱和、不削减样本。L1仅保留≤128条定宽快照摘要，完整文件明细仍在events/index；96摘要内存测试不冒充8h实测。

命名诊断份额：Sim3MiB（包含256KiB临时预留）、worker1MiB（含256KiB）、L1 Metrics/事件/定宽摘要/临时量1MiB、observer2MiB、Rust监督及验证命令记录1MiB。非observer六MiB写入共同manifest；各已实现检查/实测与仍待证明边界分别报告，不以相加声明自动证明8MiB。

Rust监督替代Python runner；typed64KiB报告解析、64KiB日志与所有子进程在同一观察树。父Instant记录启动/就绪/请求/观察完成，pending快照前后CURRENT/root哈希一致且权威序号推进的见证在树内。父时刻是观察时刻，不伪装成子进程真实开始时刻或对齐多个局部elapsed。

新增锁定MIT peak_alloc=0.3.0，workspace仍forbid unsafe，自有代码无unsafe。记录每个Rust进程终身requested-heap峰，不reset。外部allocator的realloc采用alloc+copy，零分配也实际清零；额外开销算在本次计时，不和原生allocator旧版直接作提速对比。统计不含malloc元数据、栈、映射与foreign；RSS/footprint独立。本阶段只是取得实测内存依据，不能将它称为全部物理内存。

预期730条正常进程记录：监督1、L1 1、业务驱动1、Sim2、worker724、L1冷装1；按pid/实例/角色核验，不以继承环境替代收到记录。原主动中断stop保持kill；完成worker正常换窗改EOF优雅关闭，等1秒后仍可kill。缺记录/失败/强杀如实失败或缺测，不计零。最终记录序列化尾部另预留64KiB，不称覆盖进程完全退出的精确峰。

advisor已要求先减实际直方图而非仅降cap、worker必须正常取得尾部记录、监督1MiB须实测拒超限、进程尾部预留，以及借还必须证明实际压力。构造/恢复64MiB规则不改变；外部额度按组件上界贷款仍会留空洞，下一步须用本次实测峰与受控分配实现占位/借还，未做到不得给满级输入贴通过。
