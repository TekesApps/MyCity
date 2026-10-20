# 后台计算连续目标：逐项完成审计

本表只审[用户连续目标与W0冻结标准](background-compute-acceptance.md)，不缩成“测试全绿”，也不扩成完整M0。W4正式运行、归档核验及最终独立审查已完成；Git发布以对应附注tag／远端提交为权威。源码、实际工件、核验和独立结论共同构成证据，发布状态最后单列。

| 要求 | 实际实现／权威位置 | 能证明该要求的实际证据 | 当前判定 |
|---|---|---|---|
| 开工前冻结阶段和失败标准 | W0协议、时钟、资产、矩阵与W1—W4依赖 | v0.5.25附注tag及冻结文档；后续没有削减1800秒／实际业务门 | 已完成 |
| 首个真实任务为已有空气CPU模型 | `core.air`连续State，货运实际在途源，固定分钟／任务切点 | W1完整连续场；W2数值及真实货运工件；当前Release中的continuous_windows与actual_freight测试 | 已完成，EI不是物理污染准确性 |
| worker独立、有界、只产纯候选 | `compute_worker`，输入／候选、有限JSONL与attempt控制 | W1同进程连续两任务、源／Cells／发布后握手故障；当前Release persistent_worker、bounded_cache测试 | 已完成有限worker，不是通用池 |
| 任务版本与固定读集 | `background`ticket／incarnation／generation、规则／source／input／场版本 | 当前Release stale_source、wrong_window、finite_forgery、atomic_rebind等反例 | 已完成 |
| Sim唯一业务写入 | `sim_host`独立进程，Store OS writer锁，worker无Store提交口 | 当前Release independent_sim_owns_lock及a_second_process_cannot_open；W3原始进程工件 | 已完成有限IPC，非安全沙箱 |
| 必要任务屏障及业务因果 | Timeline P0冻结、P4等待、P5不得提前 | 当前Release p4_requires_complete_candidate、pending_job、full_fact_inbox与normal_disk_exhaustion测试 | 已完成 |
| 大候选与业务同根持久化 | Sim内容寻址资产、小WAL、CURRENT和完整检查点 | 当前Release实际import7／accept11退出点，缺失／损坏资产先拒绝后GC，33任务引用轮换 | 已完成，非物理断电证明 |
| 重试封顶且原责任不丢 | 原子Rebind、最多3attempt、失败／取消保留pending | 当前Release three_real_cancelled_workers、cancelling_attempt；W3真实故障分支 | 已完成 |
| 陈旧／重复／失败请求不抢占最终票 | D控制命名空间、固定E接纳ID、成功才占最终票 | 当前Release internal_control_namespace、canonical_accept_rejections、failed_operation_flood测试 | 已完成 |
| 重启不会与旧worker竞争缓存 | 先登记owner、OS锁、同身份回收、每attempt清理及6MiB持久charge | 当前Release bounded_cache、worker_cache_charge、actual_sim_sigkill；W3孤儿恢复原始工件 | 已完成当前Unix入口；活动缓存不承诺可迁址 |
| 调度／经济／跨城真实联合 | W4期初＋策略、Timeline、日周工资、固定额营业税、合同债、实际借款、Boot／Exchange／Inbox | 当前完整Release七日模型及独立Sim快跑；正式工件已核验170点／173窗口／712票与实际库存现金 | 正式联合已核实，见W4原始工件与checks |
| worker与Sim在联合业务中实际失败并恢复 | W4 Cells故障；首日业务后Sim根提交／回复前故障；新客户端重开 | 正式原始transcript、两个不同job的fault、换代、原冷热票、业务不重复 | 正式已核实 |
| 至少1800秒持续推进而非补等 | 小时目标按实际墙钟分布，七日／每日检查点 | 正式report与原始单调时间、170检查点、7个daily_progress及≥1800秒采样 | 正式已核实 |
| 独立无故障CPU对照 | 宿主内Reference自行执行命令及空气，不读取故障分支候选 | 整个World／resources／leases／next_token／Timeline比较；仅归零revision，完整空气另比；最终持久字节再次核验 | 正式联合已核实，见W4原始工件与checks |
| 实际业务不是同实现自我印证 | 独立核对税现金／库存／工资／债务／费用／资源释放 | verifier实际读取CURRENT检查点，按冻结策略及期初复算，不能只看成功bool或相等摘要 | 开发反例已拒绝；正式持久工件已核实 |
| 全进程组资源、磁盘、延迟与限额 | Rust sampler同次ps帧、宿主内CPU对照、Sim／worker／孤儿及采样ps | 采样原文重算总和／峰值／最大间隔／漏采PID；report与最终角色收尾、启动前后vm_stat／swap | 正式已核实，采样限制保留；队列实际峰值未插桩 |
| 架构和最终运行两处独立审查 | advisor只读审W0／阶段实现，W4前审摘要覆盖及核验器 | 源码审查确认无阻断；最终须另读已完成的正式工件并记录结论 | 架构及最终实际工件均已独立审查，无阻断 |
| 可重跑而非不可核验口头结果 | frozen source、期初／策略、CLI、原始日志、世界／CPU工件、独立Python核验器 | W4按匹配tag build→joint-probe新目录1800→verify；归档manifest和各文件摘要 | 正式归档33文件逐hash核验及完整状态核验通过；新目录命令可重跑 |
| 每阶段验证后提交／递增附注tag／推送 | main阶段提交；W0 v0.5.25、W1 v0.5.26、W2 v0.5.27、W3 v0.5.28 | 当前远端main及各tag peeled commit已核对；W4须在实测／审查完成后单独发布 | W0—W3已发布；W4对应v0.5.29，实际发布以Git远端为权威 |
| 明确未测及目标边界 | W4运行专题限制，M0状态表不整体改通过 | 当前32GiB，真实16GB为not_run；无UI／渲染／GPU／AgentKit；完整M0不接受 | 持续保留 |

## 证据的范围

[W1检查](evidence/background-worker-w1/checks.json)、[W2检查](evidence/background-store-w2/checks.json)、[W3检查](evidence/background-sim-w3/checks.json)各自只对应其冻结tag；原核验器应在匹配源码运行，不强行改旧工件的source。当前[完整Release日志](evidence/background-joint-w4/release-tests.txt)在W4冻结源码下重新执行原测试与新增测试，共232项；历史阶段日志不能冒充W4持续运行。

[W4运行说明](background-joint-runtime.md)区分快跑、正式联合、采样和发布。开发核验器反例含短跑冒充耐久、伪造时长、缺比较点、不同时间峰值替换、共享容量偏差、损坏检查点、16GB假通过及借款假结清；这些仅证明核验器拒绝相应错误，不替代实际正式运行。

经济均衡、实际尾气／生态正确性、全城与8小时压力、16GB设备、跨平台、GPU、完整城市玩法仍是原型／后续验收，不因为此次后台闭环通过而改变。M0原60秒经济日严格倍率门也保持未达：本版回归约60.96秒，测试成功只证明业务与对照一致，不等于倍率目标通过。


最终独立结论及实际读取范围见[advisor最终审查](evidence/background-joint-w4/advisor-final-review.md)。正式工作1800.435030958秒，170点／173窗口、712原票、8层检查点及6419采样核验通过；观察RSS和峰49823744B，最大间隔0.417930666秒，2短命worker漏采。该限制不被“已完成”字样掩盖；本次闭环完成不改变16GB及完整M0未通过。各历史阶段源码／压缩工件与已发布tag的一致性另见[阶段完整性记录](evidence/background-joint-w4/phase-evidence-integrity.json)。
