# 固定期末核验工作区与实际额度生命周期

> 当前恢复可用性更新（v0.5.70）：旧v66 joint-02两份layout压力已退役，对应布局快照不能直接完整恢复；business world及完整兼容副本保留。最新完整布局基线已换为t13a70-joint-01两代，v50旧格式依赖仍完整。详见[本阶段报告](../m0-t13a-background-resident/STATUS.md)；下文旧保留承诺仅代表当时状态。

> 当前可用性更新（v0.5.68）：v64 joint-01及v65 joint-01的四份压力载荷已退役，对应快照不能直接完整恢复；原始证据保留。v66 joint-02两代及v50旧格式依赖仍完整。详见[后续清理报告](../disk-reclamation-2026-10-21-followup/STATUS.md)。旧文中的完整保留承诺仅代表当时状态。

v0.5.66有限阶段，仍只做Rust模型/CLI。原冻结fixture、完整30日业务、全部CPU数值步、L1数量及原性能门不改。[测前方案](PLAN.md)和[advisor代码审查](advisor-code-review.md)说明边界；第二轮联合已终态且六项离线核验通过，[advisor最终独立审查](advisor-final-review.md)认可有限发布边界。

## 实现边界

BeforeReopen不再读整份image到Vec；CompareEndurance/CompareSpeed复用固定文件比较器，要求受信规范writer的完整原始字节相等。实际读取来自Owner两个2MiB固定背板，4MiB全部初始化，借用结束重填；连续分块读取的最大payload按每loan高水位记录，不能只记末块。CURRENT/root/index/image的长度/hash/序列绑定、实际文件身份及结束复核均保留。

完整相等后才引用独立参考报告的business_sha256，结果comparison_proof显式绑定参考模式、Engine与报告工件；不是helper独立重算业务hash。普通Compare允许metadata差异，保留原Engine路径；真实Store冷开继续由原控制器实际执行。非规范但语义相等文件不在新严格比较器兼容承诺内。

broker在创建helper前从原64MiB池释放4MiB，Register固定HelperCompare种类，Start明确拒绝此种类启动worker。Filled后才读模型；父实际wait后Revoke/ACK归还。spawn失败Cancel，Bind和后续错误先kill/wait；父SIGKILL后EOF只促子退出，池不凭EOF退款，保持fail-closed；不宣称自动孤儿回收已实现。显式MYCITY_HELPER_COMPARE_PROBE仅故障测试使用，测量控制器未启用。

固定helper在Owner分配前设置整堆约束，生命周期Rust heap峰加原64KiB退出尾部须不超过4MiB+256KiB，无峰值重置/相减。JsonReader在serde前限制token/嵌套，报告总长64KiB和严格EOF保留。allocator元数据、栈和原生库物理开销另计，不能声称物理内存恰为4MiB。

## 来源及测试

最终源码9deedcb11bce7d2d431d4fd8fabad6342fc17a5772cb3de1b7d60ab345aaab82，156文件；封存binary753704248cb853f874da9ad609de10ac9bacf1173ff8916725ee4019c47dcc05，[来源](source.json)/[源码](source.zip)绑定原验收fixture与未变观测器。[最终全仓Release](tests.json)10241终态0，350条harness通过记录含nested，不称去重测试数；clippy19040、fmt、全目标build均0。测量开始后无本任务构建、测试或其他benchmark并行。

反例覆盖固定槽空/分块/中尾差异/长短/IO错误/展开归还，报告长key/长已知值/深unknown/尾随JSON，helper不得启动worker、未spawn取消、分配前杀、Filled后实际SIGKILL、wait后仅退款一次及实际父SIGKILL不自动退款。之前失败与不同来源独立保存，见[失败说明](failure-notes.md)、pre-high-water/和pre-driver-reap/；不将旧全仓结果拼作最终来源。

## 联合实测进度（尚未最终签收）

原完整L1+L2工作负载93103已终态0、实际607.619664708秒，但最外层heap日志缺测，独立核验未通过，不能签收。已完成全部30日/723目标，经营时钟336.328278125秒，2880查询/96保存及原指定故障/冷开均由控制器执行，完整Engine等于独立故障参考。两个固定helper结果已发布；BeforeReopen与CompareEndurance整堆峰加尾部分别4309977/4310063B，均低于4456448B。业务image SHA f4c922bfc7a6a214e73d1c69c26386b3faa2465794dc9f77f99793a554246e02；末端完整字节相等后显式引用参考摘要。

已观察第0/300秒排程的两份快照，各29文件/3378784402B，完整记录恢复及压力文件回读成功，持久完成9.964206708/11.366972375秒。已运行满原600秒；尽管模型退出0，独立核验发现joint-endurance正常heap footer缺失，需同源码同负载完整复测，不能提前算整个联合通过。原始工件及失败分别保留，见失败说明及joint-01-audits/。

本轮真实接池轨迹是BeforeReopen和CompareEndurance；CompareSpeed复用实现，但本阶段尚未单独重跑完整四档速度，不将其称为速度门通过。BeforeReopen固定4MiB容量不是其原单进程峰值一定减少的承诺；它替换对应池页，不把池和同用途背板双算。Reference/FaultReference、普通Compare与Sim/Prepared/Store等仍EXTRA，normal64和共同8MiB未闭合。

正式分位、全四档、三场景各三轮正式稳态、真正28800秒和真实16GB仍未完成；机器32GiB M4不替代16GB，16GB not_run、M0 not_accepted、goal active。剩余模型工作按[三阶段收敛计划](next-model-workspace-plan.md)先做容量可满足性计算，再统一完整模型/事务与Store工作区，不继续用局部估算grant冒充闭合。

## 第二轮原负载终态

第二轮实际 session 1286 终态0，模型607.373279208秒（观测器608.274935542秒），业务331.196609167秒。实际入口为原full L1+L2的smoke600秒，未缩小冻结记录或数值步；仅时轴为短诊断，不是28800秒。运行ID仍为t13a65-joint-02，因中途清理文档先发布v65，本模型阶段版本为v66，两者不可混作发布来源。

[六项离线核验](joint-02-audits/exits.json)全部0，原始输出分别保存。720游戏小时、723目标、3013原票、2880查询和96保存完整；真实故障job39及提交后丢回复job75、冷重开均执行，完整Engine含metadata及规范字节等于独立故障参考；724逻辑窗口的输入／候选hash与既有冻结v48一致。六次broker会话包含四次Sim和两次固定helper，725次worker grant/Filled/实际OS wait/归还平衡。被真实杀死的进程缺正常footer不按零计，PID与持久instance配对。

两固定helper整堆峰加64KiB退出尾部仍为4309977／4310063B，低于4456448B。Owner实际背板4MiB，两份2MiB缓冲高水位和Filled均核验；父wait后退款，未将EOF视作死亡证据。第二轮最外层joint-endurance整堆峰加尾181005B，真实footer存在，第一轮缺测仍保留为未签收，不用第二轮改写第一轮。

600次L1扫描与第0／300秒排程两份29文件快照完整执行，每份3378784402B，最新全布局冷恢复通过；持久完成时长见[摘要](joint-02-summary.json)。L1仍是完整布局／容量压力替身，不能视为其全部城市功能已实现。

32GiB M4上进程组RSS采样峰3481731072B，取得的footprint峰3470103160B，另有2522次进程footprint缺测；短helper采样漏帧不按零。系统swap初始4964810752B、净增0，非normal最长观察区间3.182983541秒，不能称整机无swap。观测器热内存和部分组件额度检查不构成完整共同8MiB证明。

原始时序按producer／role／sealed分别统计；本次sealed Sim execution中有足量样本的Payroll、Region及Background观察p95/p99落入原20/40ms，Claims只有17样本，L0 P-E0/P-E1类别未包含在此full L2运行中，不能拼成正式十类总门。原业务短节奏与L1并发，测试／编译不并行；其他系统任务不由本实验保证独占。

## 本阶段回收检查

遵循[回收规则](../../validation-artifact-retention.md)。本轮未创建独立target；继续复用共享target。历史无依赖压力已在v65独立回收58.16GiB，本轮两次完整快照和最新v64基线／v50兼容依赖仍完整保留，不用hash替代恢复原文件。当前阶段证据与故障来源暂不退役；下一阶段成功建立新完整基线后复查替代关系。
