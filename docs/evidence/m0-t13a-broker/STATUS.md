# T13A-3i：broker实际持有worker与有限故障恢复

版本v0.5.48。本阶段完成Rust模型／CLI的正常托管与有限真实进程故障恢复；**不签完整T13-A、L1联合、正式性能或8小时**。不增加UI、GPU或AgentKit，不减少原工资／债项／交付／空气工作量。

## 进程与权限

启用normal pool时，Client持实际Sim Child和内存中的监督凭证，Sim只获不同的会话凭证。broker持实际worker Child；Sim经短路径0600 Unix socket直接读写既有控制帧，broker把fd移入worker stdin/stdout后丢弃自身副本，没有payload转发线程。只有Sim持Store writer，worker只产出可拒绝候选。

正常Stop及故障Revoke均由broker实际wait后补回4MiB。宿主实际wait Sim后才撤销，收到并确认撤销回执前禁止新会话；旧generation／lease／PID不得回收新worker。晚到旧ACK幂等返回，不新增错误代次事件。legacy直接Child与broker模式互斥。Start丢回复会busy，需要旧Sim退出、撤销／确认后重开；**不是Start原地重试成功**。broker被SIGKILL不执行Rust Drop、不保证自动回收、不接管旧ready，也不伪造归还。

物理页仍按1秒调度检查，周期日志改为30秒累计；全部借还／失败转换仍保留。每行硬限1536B，原总8MiB日志上限不变；[静态包络](PLAN.md)含768attempt余量／960周期记录／16会话和64例外，7,569,408B<8MiB。它不是实际8h结果，也没有少算业务或减少物理扫描。

## 最终来源与有限实测

[141文件源码／二进制绑定](source.json)：source `34c59abec56d3c01322e13244c426daf9b6a3a6cf80ed9e12e3d80a119c3b8cd`，binary `31119f70ecc9709b464ad0e1a0b826787511976be88e078582b49d93010b7fc9`。Rust1.85.1，release／locked／offline。最终[全仓312通过](tests-verification.json)，包括默认OS临时目录的并行broker场景；[clippy](clippy-final.txt)／[fmt](fmt-final.txt)／[build](build-final.txt)终态0。

[真实有限故障工件审计](fault-audit.json)覆盖12个池：11正常收尾，刻意kill broker的1个无归还／无正常报告。7类进程测试涵盖租约／Sources／Cells／候选发布／提交后丢回复的Sim kill及冷恢复、同Sim内杀worker重试、worker暂停后实际kill/wait、Bind丢回复、Revoke丢请求／回复及重复确认、worker已spawn但Sim尚未知ID时被杀、真实Sim spawn权限失败收尾，以及broker死亡失败关闭。原始[zip](fault-artifacts.zip)／[逐文件清单](fault-manifest.json)和[测试日志](fault-tests-final.txt)均绑定最终来源。身份隔离与最大宽度事件另有单元断言；不把它们当物理峰值或8h证明。

## 原full30日，同源、无L1

最近连续[120秒normal](readiness.json)准入后，完整[运行工件](t13a3m-functional-01/manifest.json)／[zip](t13a3m-functional-01/artifacts.zip)执行原720小时／723目标／724worker／3011原票。外部Python仅保持池stdin和实际子进程等待；入口script hash与真实Rust binary分别绑定，解释器／startup／native未闭合，不能签共同8MiB。

- [完整核验](full-verification.json)：完整Engine含metadata等于CPU，全部输入／候选hash及CPU编码与v42相同；731正常Rust heap实例，所有原票链与最旧原票重试无副作用。
- [池与会话核验](pool-verification.json)：724grant==Filled==return、拒绝0；两会话注册／Bind／Revoke／ACK有序，最终无活动额度。2572次实际余额扫描、13次周期日志，事件正文2,146,312B；最大release事件至Filled事件52.167041ms，内部prepare至fill最大134.213541ms，二者口径不同，不证明持续足额压力。池整堆峰＋64KiB退出尾部67,191,808B低于64MiB＋0.25MiB，仅作保守总量检查，不用两峰相减证明全部诊断。
- [运行绑定](functional-verification.json)：总408.605068s、业务371.977167s，实际Store冷ready0.085898s。这不是四档或联合条件下的速度结论。
- [逐实例采样覆盖](coverage.json)：1635帧，RSS观察峰165,838,848B、已得footprint峰145,164,280B，1710次进程footprint缺测；pressure全normal、swap净增0。helper PID91217无RSS帧，不能当0或由正常footer补出物理峰值。

## 未隐藏的失败与不同来源

| 来源／尝试 | 结果与处理 |
|---|---|
| cac433首版 [来源](attempt1-source/source.json)／[原full](t13a3j-functional-01/manifest.json) | 原full454.910411s正常完成；后来独立探针发现晚到旧ACK错记新gen。保留其成功与缺陷，不拼成最终来源 |
| 晚到ACK [修复前](late-ack-before-fix/result.json)／[修复后](late-ack-after-fix/result.json) | 旧generation1回执却新增generation2事件；现无新增事件。仅synthetic控制，无Sim／worker／业务，不当恢复替身 |
| 68605c [来源与失败日志](attempt2-source/source.json)／[启动超时](startup-timeout.json) | 首轮31lib通过、7broker在原10s就绪期限失败；原因未证明，原临时清理缺失Ready细节。相同来源后续7案例及默认单例通过，不宣称已查明原因；期限未放宽。最终测试panic会保留工件和last status |
| 68605c全仓 | broker7过，随后旧overbound测试发送第8193字节时BrokenPipe失败。服务端8KiB拒绝／关连接合法，测试已允许这种断连／丢回复，同时强制服务端control_bound事件、拒绝计数、零grant和64MiB。最终312通过；不是放宽协议 |
| 初始晚ACK探针 | 读ready文件时JSON尚未写完，仅修探针等待完整JSON；生产ready读取本就重试。初始工件保留，不算业务故障或通过 |

[静态归档核验](static-check.json)／[脚本](static_check.py)检查三个来源与两个完整业务归档、原冻结输入和每文件hash；旧source不能替代新source缺测。原上一阶段L1压力失败继续保留，无本阶段新L1通过结论。

## 仍待验收

| 项目 | 状态 |
|---|---|
| 本阶段正常托管及有限故障恢复 | passed，范围如上 |
| 启动期限／完整稳定性 | 首轮失败原因未证明，正式门未签 |
| 精确常规64MiB、共同8MiB | not_accepted；普通Engine／serde仍额外压力，未从填充余额扣减 |
| 原L1联合、三轮稳态、四档速度 | 未签收／not_run；无L1成功不能替代 |
| 实际8h、96次保存及原时点故障 | not_run；下一步实现原冻结长时控制器并实际运行，不拿有限故障替代 |
| 真实16GB | not_run，当前32GiB M4不能代替 |
| 完整M0 | not_accepted |

独立advisor实际重跑全部核验器，同意按本阶段有限实现发布、完整T13A继续未签，见[审查](advisor-review.md)。后续直接推进长时控制器，不再扩本阶段故事。
