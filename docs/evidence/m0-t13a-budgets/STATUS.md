# T13A-3b：诊断费用与完整Rust进程堆证据

限定阶段已实测、advisor独立审查通过；不签完整8MiB诊断、满压力联合、正式稳态、8小时或16GB。发布v0.5.41。冻结标准、720小时目标、8192页、空气60秒数值步长、723目标／724窗口／3011原票均不变。

## 来源与复跑

- 实际机器：Mac16,1／Apple M4／10核／32GiB，macOS 26.6.2 arm64；不是16GB机器。
- 两次尝试使用同一源码SHA256 `fad9b177641404fa734639a47ec82d34608abc988cb6a249bc115f7e7ee31cd8`，同一release二进制 `95067b6c99bd39597f494a952c8e616b4866d17ce868fe7ef1f9fc7e64f2d826`。
- [source.json](source.json)、source.zip保存实际源码；source-support.zip含旧短case测试所需参考文件。peak_alloc精确锁定0.3.0，原crate及MIT／计量副作用见[依赖说明](dependency-notes.md)。自有Rust仍禁止unsafe。
- 原始命令 `target/release/mycity-headless joint-budget-probe NEW_DIR`，由[observe_v3.py](observe_v3.py)在同一树采样。L1先启动、manifest就绪120秒后full业务启动；L1每秒全扫描、0／300秒保存，最后新进程冷装。
- [核验器](verify.py)接受原work根目录，验证完整账、原票链、时序、730退出记录、raw角色样本及RSS成员求和；[失败核验](failed_check.py)保留保护退出；[静态核验](static_check.py)核对源码／二进制、冻结fixture、两包所有字节与依赖checksum。
- 两包artifacts.zip保留617／3721项小文件及全部业务票据，manifest逐项具名hash。大布局载荷留在对应ignored work目录，未提交Git；large-files-retained.json记长度／hash，归档保留完整index。两代快照的所有物理载荷清单均与v0.5.40相同，未缩小压力输入。

## 改动与边界

直方图桶改为有溢出拒绝的u32，不改1微秒分桶及原始9B纳秒记录；原128MiB raw容量远小于单桶u32上限。Sim诊断cap从5降为3MiB，worker仍1MiB，均将256KiB临时量计入。L1不在内存保留完整快照文件树，只保留≤128条定宽摘要，完整事件继续流式落盘；其Metrics、事件writer、摘要及临时量须在1MiB内。96摘要测试仅验证边界，不是8小时证据。

Rust监督取代Python runner，控制自身requested堆峰加64KiB尾部≤1MiB；用同一父Instant观察启动、pending、根推进、完成与冷装。该时钟只表示父观察时刻。正常空闲worker改用EOF退出以取得尾部计量，超时或活动故障仍终止原worker。

全局peak_alloc记录每个Rust进程requested堆当前值与终身峰，不reset；不含malloc元数据／rounding、native、映射或栈。realloc采用新分配并拷贝，零分配实际清零，代价已在本次实测，不和旧原生allocator作速度优劣结论。退出记录在JSON序列化之前取样，另留64KiB尾部；被强杀缺记录就是未测，不记零。

## 两次真实尝试分开报告

| 尝试 | 结果 | 原始后果 |
|---|---|---|
| budget-01 | failed，205.316秒保护退出 | 系统非正常pressure连续60.247秒，保持原门；未杀其他应用，未降负载。110个完成worker取得报告／堆记录，其余被终止进程缺测。背景状态保留，不归因给单一应用 |
| budget-02 | 同来源并发诊断完成，611.623秒 | 重试前120秒13次系统检查全normal；L1完整600扫描／两快照、full30日、完整冷装和恢复通过；重试未覆盖首次失败 |

第二次实际full推进285.410秒，723目标／724窗口，3011原票，与独立CPU完整Engine一致。此为当前有限模型的30日轨迹，不是JPROD完整城市玩法，也不代表四档速度已通过。L1实际仍每秒读取361272条native及1232426条stub；stub是布局压力，不是已实现业务。最后同来源冷装9.407秒，从iteration300继续至301。快照pending期间树内见证确认权威序号1479→1480；其只是并发功能见证。

第二次730条正常堆退出记录：724 worker、2 Sim、1业务驱动、1 L1、1冷装、1监督。角色requested堆终身峰分别为worker 3,753,799B、Sim 29,442,298B、业务驱动33,953,564B、L1 3,259,054,615B、冷装3,258,607,128B、监督115,475B。它们发生于不同阶段，不能相加当同刻总内存。

采样2447帧，进程组RSS峰3,473,047,552B；footprint缺2512个进程样本，不能宣称完整峰。系统本轮swap净增0、非正常pressure时段0；磁盘逻辑峰6,863,951,369B。采样helper及短命进程的完整峰仍未证明。

## 费用和时间门：只签已有证据

[独立核验结果](verification.json)逐项保留，不以包通过覆盖缺测：

| 本次已计量诊断组件 | 含预留的费用上界B |
|---|---:|
| Sim | 2,446,932 |
| worker | 846,892 |
| L1 | 719,936 |
| observer | 617,755 |
| Rust监督及尾部 | 181,011 |
| 上述小计 | 4,812,526 |

业务驱动诊断份额尚未闭合，小计不是完整8MiB门。观察报告中的固定六MiB其他费用是准入声明，不能替代独立真实费用证明。外部64MiB真实所有者／未用占位借还尚未实现，L1少占位的余额未补回压力，不能签正式满压力、稳态或8h。

本次execution中工资724样本p95约0.004ms，区域736样本p95 6.445ms，后台采集／接纳1448样本p95 10.542ms、p99 12.929ms、最大47.820ms。分位上界来自1微秒桶独立核验；原始纳秒保留。Claims20、空气源4样本仍不足，本次不单独签稀有类。quote层后台采集／接纳2172样本p95 **20.930ms**超过20ms诊断目标，p99 23.775ms、最大130.018ms；不得由execution通过掩盖报价开销。历史474.96ms后台长尾也不由短跑销项。

## 回归和下一依赖

`cargo test --release --locked --offline --workspace` 已取得成功终态，完整日志见[tests-workspace.txt](tests-workspace.txt)。新增正常EOF／活动kill语义、两个并发堆计量进程、桶溢出、96摘要及无效恢复前置拒绝等反例通过；release clippy通过。早期失败编译日志保留，不列为成功证据。advisor实际重跑独立核验并审查原工件，见[review](advisor-review.md)。

下一步是[明确依赖](NEXT.md)：先修PID目录复用、业务驱动诊断份额，再完成64MiB实际所有者／阶段占位借还。仍需四档实际速度、冻结其余分支、三场景各三轮35分钟及8小时；真实16GB为not_run，M0为not_accepted。快进等价性／批处理尚需独立实测，不通过删小时目标或原票制造通过。
