# T13A-3a：并发接入诊断失败留证与同快照恢复修复

本阶段没有签满级联合性能门。source A实际运行完整L1记录＋真实full业务：600次每秒扫描、0／300秒两份快照；实际30日723目标／724 worker窗口／3011原票，完整业务和空气与CPU参考一致，业务存档冷开与最早原票重试无副作用。Sim推进259.796907041秒，含IPC与持久化；不是受控提速比较。L1扫描最大ready age0.014944秒，保存开始最大迟到0.007956秒；两次持久完成14.482036125／24.903494291秒。第二次快照期间只读见证记录CURRENT/root合法链序号1813→1815及1个完整目标，证明实际提交重叠；见证在原采样树外，不纳入原性能门。

原600秒组最终因 `layout_restore_work_budget` 失败：创建允许外部模型／IPC预留64MiB，恢复器还把它当“诊断预留≤8MiB”。原始model_failed、stderr、测量和source A均保留。未把成功子项拼成整体通过，也没有重跑择优。

修复统一construct／restore外部组件预留上限64MiB；诊断热数据8MiB不变；恢复继续检查padding＋native＋stub＋COW＋metadata＋external＝原3.10GiB，超额不额外借预算。按advisor要求，恢复的external／metadata／COW检查前移到tables／native加载前。构造端>64MiB与usizeMAX在分配前拒绝；恢复反例故意没有任何大文件甚至没有files表，仍先返回work_budget。最终来源4项layout测试、all-target release clippy和fmt通过。

来源严格分开：

- source A `4af3deeb…24b902a`：600秒原并发诊断，最终失败；source.zip/source.json。
- reader B `ff52b253…5c69de5`：第一次统一上限后，实际装载原第2代快照3,378,784,402B，7.609301625秒；source-reader-b.zip/json。
- 最终 C `2fc75313…a857aff`：检查前移和反例后，同一原快照新进程完整装载，7.341501792秒，第300次变更→301次扫描；source-final.zip/json。不是重启机器／清系统缓存，也不是源码C通过600秒联合。

source-support.zip保留economy_short集成测试引用的历史World文件；它在编译指纹清单外，与source包及已发布Git仓库共同复现。两份原快照58文件6,757,568,804B逐字节hash独立复核；3011原票完整链和原CURRENT→root→Engine均核验；2415帧RSS成员求和一致。大型压力文件仍在ignored work，工件包保留index/hash及路径，不提交数GiB替身数据。

L1实际native361272／stub1232426每扫描不减。native28200样本p95/p99＝25792/42625ns，stub100800样本44542/60125ns；这两个类别本次分位达20/40ms，不签其余8类。真实业务与布局stub分别报告，非57000人完整经济。

原组32GB M4采样RSS峰3,465,199,616B，磁盘逻辑峰6,887,874,743B，swap净增0；footprint缺测2488项。原observer收费7,970,682B没有另加L1 Metrics706,120B；保守相加8,676,802B超过8MiB，且Python临时owned、树外witness还不完整，故诊断／联合预算未签。不能把observer自己的保护未触发当全组预算通过。

64MiB只是外部组件预留，外部实际owned峰未闭合，未使用预留也没有触页补齐，所以压力低于满预算上界。该输入不准直接用于正式三轮或8h；本次没有用减少padding制造满级通过。后续须补未借额度占位／借还及阶段内存账，并把L1 Metrics与所有观测成本统一入账；共享单调钟直接写事件，避免依赖树外见证。再推进四档／正式稳态和8h；原474.96ms长尾仍保留。真实16GB not_run，完整M0 not_accepted。
