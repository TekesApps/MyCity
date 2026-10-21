# v0.5.97：常驻解码工作区与原快照格式兼容的实现验证

当前固定源码SHA为`757907bccc82ec1dc13c28f11324914c6def755366e50156a093fef9c7a8d588`，205文件；见[源码/可执行文件身份](source.json)、[冻结账](freeze.json)和完整源码zip。原acceptance、opening和Cargo.lock的SHA均保持。唯一共享target继续复用，没有新独立编译缓存。

## 本轮已实现与实际证据

- 原128MiB工作预留拆为常驻64MiB已初始化typed解码区及64MiB实际并发COW页。解码区包括Native、四类最大表背板、inline头和5,338,736B实际尾部，总计67,108,864B；外层Arc/Mutex仍须记入metadata。未声称保持旧空闲状态128页可同时借用；保留解码时64页并发能力。
- 解码直接覆写固定背板，失败可原地址重试；无每代释放并重新分配64份1MiB页。空闲扫描实际typed数据与尾部；解码忙时try_lock显式报告64MiB借用，不阻塞扫描。额外typed扫描CPU工作与旧字节触页不同，必须重新实测，不能称工作量完全相同。
- 快照index借用原manifest/entries直接序列化，不再复制为整棵Value。冷恢复原生记录及stub读入预建存储；hash/EOF/逐记录相等通过后复制实际读入的stub前缀至原主表，避免临时整表候选和exact缩容。
- 完整冷恢复index改固定typed投影：固定256KiB原始缓冲、不增长；29个文件上限、7个padding、13项字段偏移、6项向量和固定Arena结构。保留标识在visit_str分配前限制128B；未知显示字段不保留，缺失来源仍报告null。
- [整合定点测试](integrated-unit.stdout)19项通过，相关core/headless all-target严格clippy退出0；尚未以此替代全仓及性能验证。
- [实际完整冷恢复](cold-results.json)：最新82首代4.256s、v50旧格式4.055s、formal第8代4.830s，均exit0。各读取3,378,784,402B，扫描361,272原生/1,232,426替身记录，64MiB工作区实际存在。输入原index保持不变；没有生成新的压力副本。执行时间为整条CLI命令，非30分钟稳态指标。

## 兼容与未关闭项

reader仅承诺既有冻结v1/v2生成器的规范布局profile；上述三份真实完整原件已实测。不宣称旧Value reader能接受的任意JSON都保持接受性：typed已知键重复会拒绝，未知字段IgnoredAny的数字处理与Value可能不同。文件名/哈希长度限额是该规范profile约束，不是通用存档格式承诺。records-only兼容命令仍是独立旧路径，不能混作完整恢复验证。

advisor指出原4MiB metadata须分别覆盖2MiB IO、结构控制、冷索引、parser/错误和路径/发布报告暂态。新64MiB只证明解码背板，不证明外层控制/DOM/原生栈都已纳入。固定投影已去掉冷索引DOM链；serde转义scratch仍按256KiB raw和锁定实现核算，不能写成≤128B。后续须完成“构造/发布/冷恢复/最终报告”同时存活分账，并保留原生2MiB线程栈的物理测量。不得扩quota、减少payload或扣typed尾部补账。

随后已完成当前规范profile的[外部算术](metadata-bound.json)及[独立准入](advisor-metadata-review.md)：IO2,097,152B，结构398,720B，阶段最大3,714,560B，距原4MiB余479,744B。final模板含128个摘要/10类双分位及metrics树重叠，主线程scan event/路径/entries/插入暂态逐项计入；public类型probe及Rust1.85.1 BTree源、输入文件和exe身份绑定。只适用该冻结构造/固定路径/规范输入，不是通用CLI硬cap、任意异常index失败峰值或物理内存证明。后续字段/路径/构造变化须重算，不把这份算术当性能通过。

[功能独立审查](advisor-functional-review.md)未发现新的执行/守恒/恢复/unsafe阻断，允许进入原formal；仅在全仓终态通过后发布有限功能子阶段。原formal脚本必须先核对全仓terminal、该准入、完整源/exe/锁文件及原冻结参数，再执行120秒实际内存压力准入。

该冻结源码的[全仓回归终态](regression-terminal.json)已成功：workspace713.986秒、全仓clippy7.478秒、fmt0.440秒、build0.084秒，均exit0；60个harness汇总、442条passed记录含nested，不称442个唯一测试。[汇总](functional-validation.json)绑定同源，源zip/exe不变。额外all-target构建54.19秒成功，复用同一target，用于缓存依赖核查，不是性能验收。原约18.57MiB物理预算超界没有被上述结果消除。

原参数多代正式负载、三场景三轮、四档速度、同源稀有类≥200、实际8小时尚未完成；真实16GB设备保持not_run，不宣称完整M0。

## 本阶段产物回收

新增仅固定源码zip、验收exe硬链接、小型报告/日志，冷恢复直接复用三份保护原件；无新pressure/reserve。原完整保护基线及最新pending继续保留，pending不是有效恢复点。全仓验证终态后按统一回收规则重新查占用和废弃中间产物；不删马上需重建的共享target，不在当前运行中清理。

[阶段回收结果](stage-retention.json)：新增删除0B，唯一target继续复用。当前workspace/all-target图存在不同有效profile；图外库仍有匹配rmeta，未证明其对clippy/CLI已废弃，先保留。没有独立target、没有新增压力副本；不将df变化算回收。本阶段为有限功能发布，下一原formal实验使用新目录并预先记录用途/峰值/解除条件，结束后再做精确回收决策。
