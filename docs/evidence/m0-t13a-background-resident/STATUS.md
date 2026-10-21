# v0.5.70 Background常驻背板与显式计算工作区

仍只做Rust模型／CLI。按依赖实际替换后台模型主准备路径的大State复制，原冻结fixture、全部记录数、数值算法／步和门限不改。本阶段不是完整normal64／共同8MiB或M0签收。

## 实现与容量

field及pending用常驻背板加存在标志；field按128²三数组和原SOURCE_LIMIT预留，pending独立保留原源数及输入身份。原null／字段顺序／活动记录线格式不改。BackgroundLease从Engine工作区取得准备背板，复制活动内容，失败／持久hook错误／展开归还；成功持久后只交换权威和Prepared背板，旧权威复位归还。

另一个显式WorkLease负责capture、pending摘要验证及Accept独立CPU reference，单次操作串行复用，不递归借锁、不返回拥有式临时State。候选只读比较后复制到Prepared既有场；Store staged验证及Engine恢复也复用该计算槽。权威上限320KiB、Engine4MiB和总64MiB均不扩。

[同源容量CLI](capacity.stdout)另计每Engine常驻准备表1,250,688B、后台计算payload262,304B、七槽header和保守Arc预留3,248B，共1,516,240B；多Engine不假定共享。保守保留额外活动Prepared与五个通用Air槽，最大目标需求43,964,888B（原67,108,864B预算）。控制预留不是allocator／RSS实测，runtime_grant_accepted仍false，不能沿用旧单Prepared公式蒙算现有用量。

## 同源验证

冻结source926a61a2df328e5c4767f82de045659fa416f91858e62a6c5292ee24c171939f，161文件；固定binary4d0188460082ca51e74b73b02f2db3cebbf82310d9f74fa03c6213c49df635af，保留work/candidate-binaries/background-resident/mycity-headless；[来源](source.json)和source.zip绑定原fixture及未改观测器。

全workspace Release实际95173终态0／543.611秒，359条harness通过记录包含nested invocation，不称去重测试数；clippy all-targets、fmt、build均同源退出0，见[终态检查](checks-final.json)／[测试记录](tests-final.json)。包括背景双窗口canonical完整字节与原路径一致、全部backing地址集合稳定、错误候选／重入／hook失败／展开和成功交换。初版编译与断言失败如实留存于[中间失败](failure-notes.md)，不拼作最终来源。

[旧完整world兼容](compat-proof.json)实际链接当前确切库，保留旧rules身份，调用Store::open后导出完整Engine；243文件原world及已有完整副本前后完整hash一致。sequence3013、2,507,418字节与旧fault-reference逐字节相同，SHA f4c922bfc7a6a214e73d1c69c26386b3faa2465794dc9f77f99793a554246e02；不是只比预存hash，不改变旧存档身份。

## 原完整L1＋L2联合短诊断

[测前方案](PLAN.md)沿用原观测器、正常压力120秒准入与完整smoke600秒。最终实际27370终态0；model605.689100084秒，observer606.373663750秒，业务316.484145958秒。测量期间本任务无编译、测试或清理并行；不保证整机其他应用独占。

[独立业务核验](verify.stdout)确认720游戏小时、723目标、3013原票、2880查询、96保存，故障Cells job42、提交后丢回复job79及冷重开均实际执行；完整Engine含metadata等于独立fault-reference，724逻辑输入／候选hash与原v48全部一致。725次worker授权／Filled／实际OS wait／归还、四Sim及两helper会话平衡，见[工作区核验](workspace_audit.stdout)。实际杀死进程缺正常footer不按零计；保留故障partial producer。

L1为完整布局／容量压力替身，不能冒充其全部城市功能已实现。600扫描、361272 native／1232426 stub每扫描完整；第0／300秒两份各29文件／3,378,784,402B快照，持久完成7.271150000／11.041187333秒，完整记录及压力回读通过；最新完整布局新进程冷恢复监督时轴4.591280959秒。新两代全部原文件保留，不用hash替代恢复。

[诊断核验](diagnostic_audit.stdout)／[覆盖](coverage.stdout)／[时序](latency_audit.stdout)均退出0，[静态归档核验](static-verification.json)绑定源码／binary／fixture／未改脚本／3948归档工件，原生大文件及压力仍完整在work。sealed Sim execution背景p95／p99观察7.518／15.992ms，quote14.715／25.300ms，属于该短运行的观察；Claims17样本且L0缺席，不能拼成十类正式门，更不能称严格受控提速。

RSS采样峰3,482,304,512B，取得的footprint峰3,474,182,848B，另2513次缺测；swap基线4,713,152,512B、净增0，最长非normal观察区间1.965141375秒。不能写全程normal或整机无swap。五项独立离线audit均0；[advisor最终审查](advisor-final-review.md)实际重跑两项并认可本有限发布。

## 仍需完成

主路径大背板复用不是零分配：input.identity仍有短字符串，公共frozen_input／validate拥有式兼容入口仍可创建对象，一般Raw解析仍同时持有旧解码对象和新背板。部分外部Reference／FaultReference／Sim输入仍调用旧入口，未宣称全角色固定恢复。

继续完成World/Arena/Opening直接seed、Store WAL／receipt／canonical DOM流式迁移，以及全部模型角色的真实池预授权／填充／实际wait与完整物理账。当前每Engine工作区已列入目标容量，但仍不是真实整图grant证明。normal64／共同8MiB、正式十类分位、全四档、三场景三轮稳态与真正28800秒尚未签；真实16GB not_run，M0 not_accepted，goal active。

## 阶段回收

终态并归档新完整基线后，advisor确认替代关系，主线程再次检查进程与打开文件，只退役旧v66 joint-02两份pressure.bin。实际inode去重分配空间6,088,081,408B（5.67GiB），work24,717,136→18,771,744KiB；df观察多增2,326,528B不归因本次成果。85,129个未删除work文件元数据不变，共享target755,872KiB保留、未新建target。详见[精确实际清单](retirement/deletion-result.json)。

**旧v66 joint-02两代layout快照现不能直接完整恢复。** 原manifest／hash／历史实测证据保留，不当作原文件；重建后需校验完整原index才能恢复可用性声明。旧v64及v65首轮此前已退役，不重新称可恢复。新t13a70-joint-01两代、v50旧格式snapshot1、全部业务world／WAL、旧规则完整兼容副本、源码／固定依赖／验收exe及故障日志继续保留。[回收规则](../../validation-artifact-retention.md)要求每阶段复查依赖，测量中不清理。

最终阶段盘点见[disk-stage-end.json](disk-stage-end.json)，与精确删除窗口分开记账；后续target／df变化不归因于压力文件回收。本阶段没有显式删除构建缓存，也未创建独立target。
