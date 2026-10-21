# T13A：Sim具名固定编码／候选缓冲

v0.5.59候选。只做Rust模型与CLI，不展开UI、GPU或AgentKit。本阶段为原正常容量池增加实际SimCodec owner，不宣称总内存闭合。

## 实现及来源

[计划](PLAN.md)保留原64MiB池、原worker4MiB及冻结full负载。[来源](source.json)为e6dd2bad02f0816f78ebc5d2d0794baac07e0f9dd8c12bbf16282965cd01acab，149文件，binary f26244705a1fc53cf874eaca24884b11dfdb96eae8939629191a4c9ec48bc7c0。来自v0.5.57代码基线；实测期间另一线程仅新增文档提交v0.5.58/a16eafc，crates/fixtures及本次binary始终未变。

broker注册先释放4MiB，Sim实际分配/触页两个2MiB槽、绑定真实PID/instance/epoch并Filled后才允许worker。控制表固定4项，Sim使用56..60页、worker60..64页，双owner期间池56MiB+Sim4+worker4=64。Prepared转换期单列时间，不冒充全压力已填满。真实Child wait后先处理worker再归还Sim；部分补页失败不增加信用，Fatal退出。输入checkpoint_to直接写scoped槽，候选直接读入校验并在同一借用内导入，不先生成整份Vec再复制。诊断元数据加入原份额，上限未扩大。

最终[56项定向测试、clippy、fmt、build](checks.json)均实际终态0，非全仓测试。包括实际进程死亡/丢回复/冷恢复/部分补页故障与独立代次余额。初始命令、编译及旧断言失败和中间55项通过按来源[保留](failed-attempts.json)，不将旧结果冒充最终来源全仓通过。

## 原full L2实际30日

新连续120秒normal准入后，实际session62534终态0；无本项目测试、编译或其他基准同行，源码冻结。work/t13a58-unpaced-01中“58”是实验开始时的命名，不代表发布tag。

经营343.253854834秒，wrapper383.88470675秒。[业务核验](verification-unpaced.json)确认723目标、3011原票、完整Engine含metadata及冷重开等于原CPU参考；724原窗口未缩减，34周期查询/1周期保存及最终checkpoint均按实际发布/完成计时。仅无等待full分支局部速度目标通过，不替代P-E0或正式四档。

[独立owner核验](workspace-unpaced.json)确认2代Sim grants/fills/returns与724次worker grants/fills/returns分别平衡，1449+1份Sim受控槽报告、724个完整实际job；双owner时真实池58720256B，最后67108864B。两次Prepared至Filled为21942792/22343333ns，转换时间原样保留，不扣经营耗时。

操作者启动命令漏配顶层MYCITY_HEAP_LOG，缺speed-run退出heap记录。[原核验失败](verification-initial.stderr)及[原脚本](verify-initial.py)保留。最终核验明确731份heap及缺失1份，不将该缺项判通过；supervisor report中25048B峰只是另一个已得观测，不替代缺失记录。**full_heap_telemetry_gate=not_accepted_missing_supervisor_record**。

采样RSS峰162316288B，已取得footprint峰121276360B，1642次进程footprint缺测保留，不当绝对峰。采样全normal、swap净增0，基线已6791299072B，不能称无swap。[分层CPU诊断](latency-unpaced.json)含726份sealed生产者、49825692B原计时：Claims20及source执行4样本不足；quote capture/admit p95=24811810ns超过20ms，正式十类门仍失败，不签性能总验收。

[归档](t13a58-unpaced-01/manifest.json)绑定真实终态/票/业务/报告，verify、workspace_audit、latency_audit、collect与[静态完整性](static-verification.json)分别实际执行。脚本0只支持明确范围，不覆盖上述缺测及失败。

## 边界与后续

Engine/State冻结副本、status场checkpoint、导入解码、事务/serde/helper仍EXTRA；实际固定缓冲只是进一步接入，不从进程历史峰相减冒充完整64MiB。L1缺席；normal64/3.10GiB、common8、完整联合、正式四档/三轮稳态、真正8h均未签，32GiB机器不能替代真实16GB，16GB not_run，M0 not_accepted。

先将该有限固定owner收口，再接其余真实分配路径和原完整联合；后续实验必须补顶层MYCITY_HEAP_LOG并以完整实际进程记录核验，不能借本次缺项降低门槛。另有v0.5.58区域概率决策设计需按独立显式profile接入，不能静默改变本次冻结旧profile/参考。

构建复用原target，无新临时target或incremental。本轮保留源码、全部失败/成功原日志、源码/依赖快照、实际binary与验收工件；缓存检查见cache-check.json，未删除仍用于后续验证的release/deps缓存。此前已清理debug/incremental释放约191MiB的记录独立保留在v0.5.57。
