# T13A：原负载长时控制器接入（未发布）

拟v0.5.49，当前发布仍v0.5.48。只做Rust模型/CLI。控制器smoke功能经实测和独立advisor核验通过；全仓回归仍运行，尚未提交本阶段。实际8h未运行、完整M0 not_accepted。

## 实现与来源

新增scale-endurance NEW_DIR formal|smoke。formal保留初始化后同一Instant：720小时h×40s、2880查询n×10s、96周期保存n×300s；不合并迟到，不随Sim重开重置。驱动在已有事务/worker结果边界续步。≥2h Cells杀worker、≥4h真实提交后杀Sim/原operation取原票重放、≥6h正常保存/停机/新进程冷开。周期保存与期末额外检查点分记。

独立FaultReference从原期初重跑指定job的一次Fail+Rebind，不复制actual元数据；CompareEndurance比较完整Engine字节。现有正常业务入口保持。控制器仍需外部Rust联合托管L1，本阶段Python runner仅作功能启动/等待；解释器/native/共同诊断未闭合。

[142文件源码](source.json) ce9efd12246a4870495c838c4047ffc274177316f956a73c47de5343b754e2b2；Rust1.85.1 release/locked/offline，binary429b53cd0b4214eda13016461fb4aa52d9b14fb8f506135bded965a6643f3908；冻结输入876ea9不变。[计划](PLAN.md)、[实现审查](advisor-plan-review.md)、[功能终审](advisor-final-review.md)。

## 已实跑的原Full smoke

[完整工件](t13a49-smoke01/manifest.json)/[zip](t13a49-smoke01/artifacts.zip)，工作目录work/t13a49-smoke01。实际观察器477秒级（478.170s）、runner477.589s、业务398.389688s；smoke仅墙钟缩短，仍720小时/723目标/2880查询/96保存/全部经济与空气工作量，没有L1。回归测试并行，禁止作为正式性能数值。原120秒normal准入工件保留，但不当正式新鲜联合准入。

[独立核验](verification.json)：job38在18.413505s杀Cells worker；job73在36.528254s杀已提交未回的Sim，原票重放无副作用；54.177922s开始计划冷开，54.660159s完成。96保存/2880查询全部实成，最大完成迟到分别0.614446/0.596782秒，仅smoke诊断。recovery_seconds从ProbeRequired发出算，含故障前计算，不能冒充纯恢复口径；正式期限待独立核验。

完整Engine等于独立FaultReference；相对正常参考仅世界revision+2和背景incarnation+1。3013连续原票链=原3011+2个明确控制事务，724逻辑任务全部输入/候选hash与v48相同。725真实grant/Filled/return，四broker会话收尾，拒绝0。

正常heap尾部732份：723worker+3Sim+4helper+driver+pool。被杀Sim99251、Cells worker99630、提交后尚存worker99969无正常尾部；后一个已有完成report。全部729生产者目录中727有report，缺的仅Cells worker和Sim。不能补造缺失尾部/物理峰。[首轮核验器错误及修正](verifier-correction.md)和原失败日志保留，业务没有重跑。

[采样覆盖](coverage.json)和原observer报告保留：同刻RSS观察峰170950656B、已得footprint峰125683656B，2003进程footprint缺测，helper11758/11764无RSS帧；pressure观察全normal/swap净增0。不是绝对生命周期峰，不证明16GB或联合预算。

## 验证与未完成

clippy全仓/全targets、release build终态0；source静态绑定通过。全仓cargo test --workspace --locked --offline句柄63357仍运行，不能称315全过，也尚未发布v0.5.49。smoke观察器60441与归档65797已终态0。下一次先核对63357真实状态；不要因日志暂时不变重启，源/fixture不变直到结束。

实际8h、L1联合、原三轮稳态、四档速度、正式计算/排队/恢复/保存门、精确常规64MiB、共同8MiB仍未签。真实16GB not_run（当前32GiB M4）。下一依赖见[NEXT](NEXT.md)：Rust联合托管和覆盖完整8h的L1时轴，再原冻结实跑；不扩有限案例。

同源[诊断账核验](diagnostic-audit.json)/[脚本](diagnostic_audit.py)已执行：封存Sim/worker组件容量与驱动整堆分别在原分配内；缺失实例不补0。helper模型堆与Sim模型堆仍为额外压力；池只有整堆上界，不能减两个历史峰签0.25MiB。正式Rust联合名义分配8MiB；保留1MiB Python runner再加L1的名义分配会成为8.75MiB，此加法本身不算联合实测。共同预算和normal3.10GiB继续未签。

同源原始计时[逐生产者/角色/封存状态核验](latency-audit.json)/[脚本](latency_audit.py)已执行：49,759,461B raw，精确nearest-rank，不混quote/replay/故障未封存样本。已封存execution的Capture/Admit n1302，p95=22.452289ms、p99=61.938375ms；Region n657，p99=50.114417ms，均有20/40ms越门。并行debug回归造成的干扰无法剔除，不宣称已证明具体因果瓶颈，也不把这些结果升级为正式性能通过；正式条件须重测/定位。L0和L1四类本轮缺测，Claims等稀有类别样本仍不足200，旧其他来源不能自动补齐。

计时按既有summary进一步拆分（不是新一轮业务）：Capture651样本p95=14.896583ms/p99=50.111416ms；Admit651样本p95=26.256852ms/p99=83.271832ms。Region的clock子类包含Capture，两个计时不可相加。当前Accept实现仍独立重算完整空气候选，不能为了达门删掉候选核验或漏算数值步骤；后续先在规定环境复测/定位，再做保持原结果的优化。
