# T13A-2e：真实计算区间计时与完整诊断

仅Rust模型和CLI。本阶段添加有界、默认关闭的瞬时计时收集器，数据不进入Engine、Receipt或检查点；开始／结束的原始字节偏移把空气页与事务结果、worker ticket绑定。空气计算成功不自动等于最终接纳成功。quote、实际提交和worker生产分别报告；冷恢复replay尚未开启细粒度计时，空报告不是零成本。

业务计时按一次8192步交易聚合Claims／Payroll／Region，而非每笔付款充样本；保留扫描量、有效工作量、实付量和子类型。父级区间扣除空气页日志回调耗时，Instant含调度抢占，不冒充线程CPU时钟。Prepare含子区间、Region P0含Capture、Admit含接纳空气复算，不能相加。持久化观察点另外记录WAL编码校验、容量准入、write、fsync、Root编码和原子发布、CURRENT编码及发布；这些含固定观察回调开销，不与20／40ms纯计算门混用。

worker须在发送Complete前持久化报告；崩溃attempt可能只有部分原始数据，明确保留为不完整。共享磁盘额度按64KiB提前预留，raw总128MiB、诊断日志总256MiB；进程死亡不退回已预留额度。producer的常驻诊断检测阈值Sim 5MiB、worker 1MiB；本次预留共6MiB向observer收费，L1不同行，尚不签联合运行预算。原负载、720小时目标、8192步页、空气60秒步长及全部原票保持。

实际完整诊断与门状态见下文。8小时、三轮稳态、四档速度和真实16GB均未通过；完整M0未验收。

边界解释：`durable_receipt:false`只表示未确认，不证明未提交；CURRENT发布后的I/O错误必须重新打开并查原票。失败尝试不进入已提交业务批次分位。共享额度约束raw／transactions流，report.json及目录元数据另按完整目录实际占用核对。5MiB／1MiB是记录点分配后的越限检查，不是分配前硬上限；报告JSON临时分配尚未完整收费，故不签联合热内存门。L0各经济分支独立目录，预算及样本按分支报告，不以相加样本掩盖不同失败输入。


首轮满级full-01在408个已核对目标后由旧观测器停止：观测器peak+tracer为2117703B，超2097152B门20551B；6MiB预收费后也相同超额。192.59498575秒后模型被SIGTERM，不能称完成30日。完整失败目录已归档，不续跑旧世界、不覆盖失败、不改变冻结负载或预算。新observe_v2以scandir单次统计字节／latency／文件数，消除递归glob与ps整表行副本，释放进程表和编码临时量；保留历史peak语义，不reset_peak。独立辅助测试用同份666行ps、724worker／3620文件目录重复40次，字节计数／解析相等，peak+tracer259259B；边界4096／4097、symlink和深度拒绝均测试。这是观测器替身辅助验证，绝不算城市业务或正式性能通过。


## 实际完整结果

Rust source `b72de1cbea8681b387106ee9f5d11fa9a146aa889caee6c0bbfdcaa4d36314f7`，Release binary `f684913b34866ce1be7563562fe767cb790789f367c2610cdf1263c3e6bebafb`。full-01与full-02二进制／业务输入相同，仅观测器修复；不是缩负载重试。full-02实际30日、723目标、724窗口、3011永久原票、2个Sim及724个worker完整报告，Sim经营264.2754425秒，含CPU参考／保存／冷开等外层完整运行298.126895166秒。冷ready0.0822125秒只是短诊断，不代替三轮／故障门。每个目标记录实际hash，完整实际与CPU Engine分别与v0.5.36对应图像全字段相等；两份最终World图像、全部3011原票索引／链也独立核对。

P-E0与6个P-E1分支整组CLI实际62.29692325秒完成并通过既有业务断言，7份producer报告；不把它当成含Sim IPC的实际经营档30日≤1800秒门。P-E1按输入case分层，不合并短case填满样本。

原始nearest-rank诊断如下（毫秒；Instant含抢占，不是线程CPU钟）：

| producer／执行类别 | 有效样本 | p95 | p99 | 最大 | 当前解释 |
|---|---:|---:|---:|---:|---|
| Sim Claims | 20 | 0.071918 | 0.077031 | 0.077031 | 不足200 |
| Sim Payroll | 724 | 0.004502 | 0.008335 | 22.298374 | 本次分位在20／40内，非正式重复门 |
| Sim Region | 736 | 6.989042 | 11.835709 | 107.563332 | 含P0 Capture，长尾保留 |
| Sim Capture／Admit | 1448 | 11.325614 | 23.027122 | 474.957253 | 长尾待定位，不能称无卡顿 |
| Sim admission air cell | 691200 | 0.017458 | 0.018208 | 21.041084 | 1024格页，计算尝试 |
| worker air cell | 2764800 | 0.001000 | 0.015000 | 7.262834 | 256格页，计算尝试 |
| Sim／worker air source | 各4 | 各自见raw报告 | 各自见raw报告 | 各自见raw报告 | 不足200，零源转换未充数 |

P-E0有721笔成功模型交易；P-E1 low-cash及supply各720笔，debt／prepay／refund／suspend仅4／27／28／49，不足仍保留。quote与未采集replay不填execution门。[独立核验](verify.py)逐文件校验、raw区间闭合及精确分位；[checks](checks.json)保留每个case和角色的全部统计。[I/O诊断](io-summary.json)将持久边界拆开：full-02 WAL fsync累计15.100秒、Root原子发布同步30.965秒、CURRENT写入rename15.740秒／目录sync14.715秒，编码及应用另外列出。不是以这些墙钟替代纯计算门，也不把父子区间相加。

full-02观察RSS峰110067712B，footprint缺测1248；observer热数据572909B，与6MiB预收费合计6864365B。完整实验目录逻辑峰104341264B，swap净增0。共享raw／transaction日志预留分别72417280／57802752B，实际目录另含report和元数据；不把预留当实际磁盘。L0观察RSS峰39223296B、footprint缺测252、observer热数据542642B，但经济分支诊断尚未向observer做统一预收费，联合内存门未通过。L1未同行、报告临时分配和外层CLI诊断收费未全部闭合，因此即使full-02收费低于8MiB，也不签最终联合预算。外层仅负责串行启动的Python launcher仍不计入组，边界继续保留。

## 验证与后续

全量258项Release测试在sourceA `9a213062bcb45b2cebe65cffa1099eedeeedf09ebd802f2eefe1a2f37b5a15f6`完成。随后仅去除三个不必要的drop(trace)、增加诊断提交确认元数据；sourceB实际跑8项受影响定向测试、全目标Clippy及fmt通过，不改写成测量源码已全量258项通过。初次编译及Clippy失败日志、sourceA快照、两版来源和最终定向命令均保留；实测全量业务等价性使用sourceB。详见[测试来源](workspace-test-provenance.json)、[最终定向命令](targeted-execution.json)及[advisor独立审查](advisor-review.md)。

下一依赖：Claims及空气源真实同负载独立重复、短P-E1 case样本；定位474.96ms长尾；补IPC／排队完整分层和四档实测，接L1同行、联合预算及三轮正式稳态，再完成原定8小时／96保存及故障恢复。无200样本、未跑长时和真实16GB均不算通过；完整M0 not_accepted。
