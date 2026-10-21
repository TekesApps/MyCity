# 原完整L1＋L2短联合重新实测

v0.5.61候选。仅Rust模型与CLI，本阶段只增加真实联合证据，无代码改动，不重跑旧定向测试。[来源](source.json)为已发布v0.5.59的e6dd2bad02f0816f78ebc5d2d0794baac07e0f9dd8c12bbf16282965cd01acab／149文件；启动保存的binary f26244705a1fc53cf874eaca24884b11dfdb96eae8939629191a4c9ec48bc7c0。继承[原方案](PLAN.md)及[advisor启动审查](advisor-plan-review.md)，原acceptance SHA/数量/保护门不变。

实验期间v0.5.60/5fd8957仅新增概率通则文档和开发规范，不改crates/fixtures；本次来源与oracle保持冻结。work/t13a60-joint-01的60是实验目录名称，不代表发布tag。复用现有target且没有构建、测试或其他自有benchmark同行，静态advisor阅读与轻量日志状态查看另列。

## 实际联合结果

新的连续120秒normal准入后实际session80279终态0，wrapper613.3965335秒。完整L1在业务spawn之前就绪，持续到业务完整退出后才接受stop，并遵守600秒最短段；不是各自跑过一段后拼成联合。

[独立核验](verification.json)确认L2完整720小时/723目标、2880query/96save、724逻辑空气窗口及全部输入/候选hash等于原v48 CPU参考，原3011票加故障Fail/Rebind两笔为3013票，完整Engine含metadata等于独立故障reference；相对正常参考只允许world.revision+2/background.incarnation+1。经营385.62097275秒；smoke的72秒最小发布区间不是实达72秒，也不算正式四档或8小时。

真实Cells故障job32在18.316084417秒、提交后丢回复job65在36.866840042秒，正常冷开在54.309838709秒发起/54.852056917秒完成，均为smoke计划，不能移称2/4/6小时故障。丢回复恢复有原operation proof而不重复副作用。

[独立owner核验](workspace-verification.json)确认4代Sim全部Prepared/Filled/实际wait后Returned，725次worker借还包含故障额外尝试，不误称725个逻辑窗口。每次worker Filled都有真实SimCodec，池56MiB+Sim4+worker4；最后64MiB全部归还。Killed进程缺最后槽trace/heap时继续标缺测，不推断为完整或零内存。

L1原361272 native/1232426 typed stub每秒600次完整扫描，native28200页/stub100800页；仍是布局压力替身，不是住房/交通等完整业务。两代快照各29文件/3378784402B，实际持久完成25.956753792／23.705139833秒，完整记录恢复及压力文件读回通过。最后新进程冷装第二代、扫描续做及两恢复点保留均核对。监督时轴冷装9.247892333秒，[本次时序明细](timing-summary.json)单列，不用smoke代替formal时序门。

## 资源与仍失败的门

[采样覆盖](coverage.json)与[诊断核验](diagnostic-verification.json)分开记录：本次顶层MYCITY_HEAP_LOG已配置，监督者正常heap记录齐全，不借前一缺测结果补本次。故意kill的1个Sim及2个worker没有正常heap footer，其中一个worker已有sealed report仍没有heap，不能混同两种footer。727 sealed生产者/2 partial生产者各自保留，短命/重用PID缺测不当0。

RSS采样峰3576922112B，已获得footprint峰3538866544B，2560次进程footprint缺测保留，不能当绝对峰。全采样normal/swap净增0，但基线6706692096B已在swap，不能称无swap。世界逻辑文件峰6894941067B，实际原始大文件作为验收证据保留，不是可删缓存；全部未缩负载，本机32GiB不能代表真实16GB。

[分层CPU诊断](latency.json)保留50935473B原计时；sealed execution capture/admit p99=45111834ns超40ms，quote p95=29304506/p99=60467261ns超门；稀有及L0缺项仍不足。不能因业务和恢复正确就签十类性能。两个L1页类局部样本量/分位可见原报告，但不冒充正式2100秒三轮。

[归档](t13a60-joint-01/manifest.json)及[静态核验](static-verification.json)绑定实际终态/源码/binary/observer/全部业务与元数据。大压力文件完整hash后仍留work，不能仅靠index自述hash；旧v50压力失败原样保留。本次成功与旧失败的环境背景不同，不声称已单独证明某改动造成全部改善。

## 未签收与下一依赖

本次只签完整原负载的短联合功能与实际承压证据，Engine/State/事务/serde/helper以及worker计算数组仍EXTRA，正常64MiB/3.10GiB和共同8MiB没有分配前闭合；[下一实际分配路径](next-allocation-paths.md)已有advisor具体定位。先消除隐藏完整编码并建立真实有界接纳/常驻分配，再重测原负载；不得改预算或oracle制造通过。

正式四档、三场景各三轮稳态、真正8小时仍未完成；真实16GB not_run，M0 not_accepted。概率通则的代码对齐用独立版本及有界机制推进，不静默改变已冻结历史profile。
