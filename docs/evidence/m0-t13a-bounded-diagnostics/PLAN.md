# T13A-3g：有界控制数据与诊断联合实测

测量前冻结：原 full 30 日（64 工资、2 万债项、16 周期义务、8 实际货单、720 小时、723 目标、724 worker 窗口、3011 原票）、8192 页与空气 60 秒数值步长不变。L1 沿用完整 361272 native + 1232426 stub、600 次扫描、0/300 秒快照；manifest 就绪120秒后启动完整业务。新进程真实冷装最新快照。原 normal 连续120秒准入、pressure/swap/disk保护不放宽。

共同诊断候选恰为8MiB：Sim2.5、worker1、L1诊断1、observer2、supervisor0.25、driver1、helper0.25 MiB。driver/supervisor为终身 requested-heap 峰加64KiB尾部检查，未设置heap日志也执行；不是malloc元数据/原生栈/全部RSS硬限。helper的256KiB是具名控制/路径/临时/黄金报告或哈希/退出尾部容量；40KiB解析与错误、部分路径临时量为保守静态预留，并非所有分配均由allocator硬池拦截。控制owner限额在分配前检查，主线程之外仅父管道watcher读取stdin；未来若并行解析，必须提升为全进程共同额度。128KiB watcher栈及其它原生栈归物理/常规内存，不能冒充256KiB包办整个helper。

v2 request/result使用固定Text、3槽artifact数组、8KiB控制上限、拒绝转义并用路径hex保留原UTF8路径。每次helper尝试独立目录，失败后不复用部分已发布工件。CURRENT/root/index链、完整Engine与空气比较保持；root额外作为原始工件发布，父进程验字节数和hash。64KiB golden/hash独占scratch，租约错误和退出均归还。空气报告以4KiB固定scratch流式产生原little-endian完整hash，不改变数值步或完整比较。

实测前先完成同来源全仓release offline测试、fmt与clippy；functional检查723条CPU目标和完整Engine逐字节对旧v42，CPU报告除来源与耗时保持相同，三helper模式和负例分开。构造image链只是功能反例；不能替代真实Store恢复。联合实测独立核验原票、完整期末、受控worker实际容量与触页、helper容量、退出记录、全角色RSS/footprint缺测、原始计时与保护结果。

本次仍有外部常规64MiB未填预留/实际grant余额缺口；不签完整联合压力、三轮稳态、四档、8小时或真实16GB。具名诊断份额也必须以实际来源和工件核验后才作有限结论，不以份额相加自动判通过。原失败保留。完整M0不宣称完成。
