# v0.5.98：原参数L1单轮完成，normal物理门失败

固定source `757907bccc82ec1dc13c28f11324914c6def755366e50156a093fef9c7a8d588`，exe SHA `178c3a709ef3cc5d69e166bd2da60bcb43f0ae804fac27c53dae55514e9be2ac`。使用v0.5.97已通过的同源全仓/严格检查，本轮没有修改Rust或原负载/阈值。前后源码zip及exe实际核验一致。

## 实测结果

[正式汇总](formal-result.json)、[原始窗口审计](window_audit.json)、[资源窗口](resource_window_audit.json)、[同相位驻留](layout_residency_audit.json)及[advisor独立复核](advisor-formal-review.md)为本轮依据。原始文件在`work/typed-workspace-formal-l1-01/observed/`完整保留。

- 120.275秒实际正常内存压力准入后，原300秒暖场＋1800秒测量完成；观测2115.516秒、supervisor2114.863秒、进程exit0。额外时间包含构造/保存/退出/完整冷开，不倒扣后宣称经营提速。
- 原完整L1：每扫描361,272条native与1,232,426条stub，2101次已发布扫描全部完成。8代各29文件、3,378,784,402B完整恢复及压力读回通过；实际COW归还`[7,8,9,10,9,8,9,1]`，分配失败0。最后一代开始后按owner截止停止脏写，因此实际归还1份，不把计数补成10。完整扫描最大wall 202.304ms，最大ready迟到22.467ms，均保留，不与每页分位混淆。
- 窗内native页84,600样本，p95/p99为0.062208/0.091916ms；stub页302,400样本，0.075125/0.141791ms。原20/40ms观察阈值满足；只签**单轮两类L1页计算观察**，不是JPROD全功能或全部业务类别通过。
- 7200窗口帧中7114完整、86不完整（含OS进程枚举ESRCH、祖先未明），均保留unknown，不插值/补零。完整帧也确认RSS峰3,358,785,536B、footprint峰3,347,992,392B。normal为3,328,599,654B，分别超30,185,882B及19,392,738B（footprint约18.49MiB）：**failed observed**。不扣observer，不把16MiB同边界增长容差加到总预算。
- 诊断hot观察上界4,666,009B、swap净增长0、非normal压力连续0秒；世界峰10,198,927,146B低于20GiB。这些不构成normal/common8全部签收。
- 2038个同相位空闲gap共8074完整匹配帧，L1 requested/arena/隔离首尾delta均0。只证明本次L1空闲计数稳定，不签L2业务日边界泄漏或长期无增长。

## 恢复、审计适用范围与留存

最新第7/8代又由同源reader实际全冷载入，各读3,378,784,402B并继续扫描；4.708/4.616秒exit0，见[cold-current结果](cold-current-results.json)。这两份原件完整保留，hash不替代数据。当前运行第1..6代已按双代策略退役，不能直接完整恢复。

沿用的quiescent审计仅适用于L2 measurement transcript，对L1执行时报缺文件；[原stderr](quiescent_audit.stderr)与[首轮工具结果](postrun-results.json)保留，不算通过。L1没有该业务轨迹，入口现已按scenario跳过；独立layout驻留审计实际成功。没有重跑/改写模型以掩盖工具适用范围错误。

经advisor释放依赖，旧`steady-controller-formal-l2-01`第7/8代仅4份pressure/reserve按[精确清单](formal-retirement/deleted.json)退役，分配空间6,356,516,864B（5.920GiB），work剩24.806GiB。新两代58索引文件删除前后完整SHA核验，其他work文件身份/长度/mtime/链接数不变。旧业务/WAL、native/stub组件、索引、原始样本、固定源码/依赖/exe、v50和最新82故障原件保留；旧两代现在不能直接完整恢复，见[availability](formal-retirement/availability.json)。

原删除脚本在4文件删除及保护核验后，因后置检查观察到未归属的1.95 Cargo而退出1；lsof无本项目work/target占用，未停止该进程。[无删除finalizer](finalize-retirement.py)重新核对全部其他work元数据、58文件SHA和占用后完成收尾。原错误与脚本保留，不重跑删除步骤。df增加6,247,690,240B，独立于删除inode的分配块统计；APFS共享块/快照/其他任务影响不能归于本次回收。target始终复用，未删构建缓存制造成果。

## 下一步与未测门

先执行[NEXT](VM-NEXT.md)的独立VM诊断，归因真实运行时组件。已有冻结规则允许有事实的同用途置换，不是按本轮超额倒扣padding。当前完整三轮、L0/L2同源正式窗口、四速、稀有类、实际8小时尚未完成；32GiB Mac不代替真实16GiB设备，该门保持not_run，完整M0不通过。
