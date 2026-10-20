# W2：空气任务、Sim接纳与同根恢复

发布v0.5.27。承接[W0冻结验收](background-compute-acceptance.md)和[W1纯worker](background-worker-runtime.md)。本阶段实际接入Core与Store，并通过CLI调度一个独立worker；Sim职责目前仍在CLI进程中，独立Sim／监督进程和1800秒联合运行留给W3／W4，不能用本页通过替代它们。

## 1. 有限运行范围和因果

`Core::Background`是Engine的可选权威领域，初始只允许当前区域实验profile在交互钟0启用。区域空气网格128×128、格距32m、原点(0,0)，三张1024m地图必须位于4096m实验范围内；它不是完整产品的全区域环境实现。启用时拒绝不支持的地图范围和超过实验率上限的货运规格。

P0在旧时钟结束、真实货运状态改变前捕获[t0,T)活动。只有实际在途、未暂停的Freight shipment产生源，截止到实际到达时刻；电／水合同不冒充货车。源位于发货仓库的明确空间锚点，实验率为每Q每秒1000 microEI，数量存为毫Q，所以rate_micro等于该货运记录的毫Q数量。最多4条源，每条不超过1000000 microEI/s；EI是游戏指数，不是排放质量、油耗或真实健康标准。

任务保存job ID、接受时base_revision、场版本、固定起止、完整输入和起点检查点摘要；其后Lease／Restart／安全收件交接可以改变世界revision，不能把base_revision等于最新revision作为接纳条件。输入、场版本及必要时间窗口仍须相符。场、累计积分、累计诊断、任务消费与版本增加属于同一Engine事务。

P4必须等场完成到T，P5与下一时间窗口才可继续。先前P0／P2的应计工资、债务、实际交付和付款保留；空气重试不重复这些业务。只有重算候选，没有worker直接付款或写世界。零时长的同刻事件不新建空气任务，不凭空添加数值切点。

## 2. 代际、纯结果校验和观察

Restart增加writer incarnation并使正在运行的旧尝试失效；Lease持久增加generation与attempt。每job最多三次Lease，Restart不能清零。Fail只停止尝试，任务仍阻塞P4；InvalidInput不再Lease，Transient／Cancelled在剩余次数内可重试。耗尽保留任务与失败状态，不静默删除义务。

Sim导入先检查当前job／generation／incarnation／场版本、完整输入与时间、候选摘要及完整性，并从固定起点以CPU重算、比较整个State。成功事务准备完成后才调用Store持久钩子；钩子失败时场和任务消费都不发布。相同operation ID取回原回执，不能再运行一次业务提交。

CLI的`background-probe`把Core任务转换成W1 Ticket，worker回复必须与整份预期Ticket及PID一致。Store再次校验权威任务；这是不同于只相信worker回复摘要的第二层。拒绝错窗口、旧job／代际、错场版本、未完成、有限但被篡改的场及无权限接纳。Core检查点恢复也不能擦掉待办任务、修改其场版本或把它与Timeline拆开。

观察接口是Core的background／pending／field及Store的background_reserve／capacity_status，加上CLI JSON报告；没有UI。未完成时P4状态与旧场时刻可观察，不应把旧场当成T时刻已完成观察。完整P-E0与Region仍互斥；本阶段实际货运链只证明当前profile的钱物／运费和场能组合，不宣称全行业经济通过。

## 3. 大资产、小WAL及GC

域候选文件不超过2MiB。Sim只接受有界字节，复制到自己管理的顶层`air-result-HASH.air`，不保存worker路径。发布顺序：校验→临时文件→sync文件→hard-link摘要名→删除临时名／sync目录→Root登记staged引用。已有内容也重新核验和同步，坏文件不覆盖。Root登记仅管理物理资产和预留，不改变业务sequence或世界revision。

Accept的WAL只存job／generation／32字节候选摘要，仍受32KiB帧门。成功重放必须加载并核验Sim资产，再由Core复算校验；不能读取worker目录或用新算结果替换已提交字节。失败接纳在运行时以无attachment的确定拒绝路径形成回执，不建立未被模型接纳的候选依赖。

当前WAL至多32个不同的成功结果引用。下一个边界接纳前达到此门时，先正常检查点：完整AirState进入Engine检查点，旧WAL退役，才释放旧结果资产。冷回执的历史hash不要求永久保存整个旧候选；原回执仍保留。中段检查点、Restart或Fail不释放Root仍登记的staged资产。

重开先验证CURRENT、检查点／冷档案、全部当前WAL和staged依赖，再截断确认边界后的尾部、GC无引用结果及导入临时文件。已提交资产丢失／损坏时明确拒绝，在这之前不删尾、不删孤儿、不退回旧根。

## 4. 独立预留及内存归属

首次边界在正常配额中另行准入：10MiB资产工作份额＋16×45120B控制发布份额＝11207680B（10.6884765625MiB），并保留原Timeline续做票。资产份额中4MiB覆盖Sim最大2MiB候选的保守双名发布高峰，6MiB为W3单在途输入／输出缓存留位；**W2尚未实现整个worker缓存的统一配额和回收**。本版探针保留4份输入输出作有界诊断工件，与正式缓存管理分开，不能自动覆盖长跑。

staged登记扣一个发布槽，剩余资产预留减去已物化的文件长度；成功任务管理使用自己的发布槽，Accept释放剩余BG预留。失败请求、重复导入、无必要的反复Restart不能消费专属额度。热回执也保护BG槽、Timeline页和原64个通用管理槽，不能只给磁盘留账面数字。

P4强制断页另计至少一页Timeline预留，避免大limit误算为只需一页。通用安全交接已消耗管理槽时，BG完成必须按其自身已验证额度判断；不能再次要求完整64个通用槽，挤掉已有BG责任。配额缩小／正常区耗尽后，已接义务仍可使用已资助票，下一窗口重新准入失败则安全停下。

这是Store配额内的字节预留，不是OS磁盘预分配。ENOSPC／I/O不确定仍会要求重开，不能据fsync／进程退出宣称物理断电认证。达到32引用前也可能因档案累计或普通配额不足暂停；显式正常检查点可回收旧WAL资产，不依赖它来临时救已经接下的义务。

内存不另加一整份城市：Engine仍硬限4MiB，启用字段的内联开销约576B，BG自有堆／状态上限320KiB；已提交场沿用256KiB数组主体，最多4源。当前WAL引用表32×32B＝1KiB，计入Store owned。未启用BG的20000 claims探针本版Engine约3645856B，启用后的真实数字由本版探针报告，不拿owned代替进程RSS。事务克隆、序列化、worker进程和检查点高峰仍受A55共享预算约束，待W4实际采样；16GB门未过。

## 5. 实测和复跑

```sh
cargo fmt --all -- --check
cargo test --workspace --release --locked --offline -- --nocapture --test-threads=1
cargo clippy --workspace --all-targets --release --locked --offline -- -D warnings
cargo run --release --locked --offline -p mycity-headless -- background-probe /tmp/mycity-background-new-directory
python3 docs/evidence/background-store-w2/verify.py --rerun
```

本阶段定向实验已实测：

- 空源／实际货运与limit=1／256／8192均可停在P4、接纳、完成并重开；提交钩子失败保留旧状态。
- 7个导入边界和11个WAL／回复边界的真实进程退出，重开取回同一场与原票，累计／任务不重复。
- 错窗口／有限伪造／旧job／旧代际／错场版本拒绝；staged中段检查点与换代后重新核验。
- 正常配额耗尽后，必要Restart／Lease、导入、Accept及P4/P5完成；额外无必要Restart和下一窗口被拒绝。
- 失败洪泛后再处理满收件箱的实际接收／交接／已付款归档，专属结果与时钟仍能完成；外部欠费不凭空气计算自动消失。
- 33次任务接纳，当前WAL结果峰值32；新义务前换段／GC，冷原票仍可读。

同版完整gate与实际CLI工件见[阶段证据](evidence/background-store-w2/gates.json)、[探针](evidence/background-store-w2/probe.json)、[核验脚本](evidence/background-store-w2/verify.py)。它们不构成独立Sim、30分钟联合资源或16GB通过；W3—W4继续按原冻结标准推进。


本版完整Release共216项测试通过，fmt／clippy拒绝warnings通过，归档核验与实际CLI复跑通过。源码`268fa1419b3527b4cbd3303aa0f0068d1c0238cf4038e54d22735e0bbc0216bd`；CLI探针墙钟1.801132459秒、Engine owned 2948980B，属于该有限输入。独立连续f64参考的最大场误差2.613400784667874e-7 EI、最大时间均值误差6.338552675326052e-8 EI，通过既定0.001＋0.0001×参考值容差；不是物理标定。既有经济日60.864331167秒，严格60秒门仍未满足。本机32GiB，command-rusage只记录命令rusage，不是全进程组物理峰值，也不签收16GB门。
