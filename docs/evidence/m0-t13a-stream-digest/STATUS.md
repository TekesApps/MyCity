> **2026-10-21 availability update:** Selected historical synthetic pressure payloads retired; listed snapshots are NOT directly full-restorable. Business/component evidence and original indexes remain. This supersedes older retention promises below. See [availability](t13a62-joint-01/artifact-availability-2026-10-21.json) and [cleanup record](../disk-reclamation-2026-10-21/STATUS.md).

# 接纳校验的固定缓冲流式摘要

v0.5.62候选。仅Rust模型与CLI，改动前[计划](PLAN.md)保持原full负载/CPU参考/保护门；没有引入UI、GPU、AgentKit或概率。确定性校验不需要随机策略。[advisor代码审查](advisor-code-review.md)未发现生产逻辑/限额/分块哈希阻断。

## 实现及验证来源

air::State新增checkpoint_digest，沿原checkpoint_to验证和编码一次生成规范字节计数及SHA-256，不生成整份编码Vec。payload哈希也使用固定8192B缓冲批量计算；空输入、大块直通、补满、flush及最后尾部均保持原顺序。Background捕获/冻结输入/Accept与storage恢复的staged长度校验使用新API，原schema、ASSET_LIMIT、身份/输入/数值CPU reference保持。

摘要逻辑缓冲嵌套两份8KiB，另有SHA状态/长度与调用帧；属于原生栈，不能说零成本。State/reference/Prepared/Store/helper/worker数组仍EXTRA，不将摘要缓冲改动扩写成完整64MiB控制。

最终[来源](source.json)5347b7f0fc22d5cfd6c9badb230b7c2337f181e44a3842e52137fe8cb7978c35／149文件，binary66f437af86bf79b768c5b387aeb0b203a1e0ee835e7c98af9121837512f242ae，实际运行保存的candidate executable。同源[全仓Release回归](tests.json)实际81774终态0，harness汇总337含nested而非去重独立测试数；Clippy全workspace/all-targets、fmt、Release build均0。测试覆盖legacy规范字节、原SHA/长度、Source/Cells中间态、完整/续算、原后台/存储/实际故障恢复。

两次失败及[来源快照](failed-attempts.json)保留：c75a…新测试误用私有hash导致编译失败；7b57…新测试cells=8192越原1024上限被拒。最终使用独立SHA-256参考并仅修fixture至原1024，不提高生产上限、不为测试开放私有接口。不能借中间来源通过结果替代最终全仓。

## 原完整L1＋L2实际联合

连续120秒normal准入后实际43204终态0，原600次L1全扫描/两代快照/最新冷装及L2全30日、723目标、724逻辑窗口、2880query/96save、真实Cells和提交后丢回复故障及冷开均保留。源码/fixtures冻结，无本项目test/build/其他benchmark同行；他项目构建曾在准入前观察到并保留，不停止他项目，外部环境不宣称绝对隔离。

[独立业务核验](verification.json)确认3013原票含故障Fail/Rebind两笔，完整Engine含metadata等于独立faultreference，全部724输入/候选hash仍等于原v48参考；[4代Sim/725worker](workspace-verification.json)分别借还、双owner池56MiB、最终64MiB。故障缺最后槽trace或heap/footer继续缺测，不推为零或完整。

经营356.202284167秒，wrapper610.629363208秒；smoke发布区间72秒并非实际72秒或8小时。两代L1各29文件/3378784402B，native361272/stub1232426记录原样全量；保存/恢复校验16.951104208／22.231023958秒。最新冷装监督时轴8.226595334秒，续扫及两恢复点核验一致。L1为布局压力替身，不冒充完整城市功能。

## 分位观察及剩余失败

[本次原始分层](latency.json)和[前后对照](comparison.json)使用同一冻结原负载及observer，来源独立，不合并样本：sealed execution capture/admit p95=12.009171／p99=13.642998ms，原v61为16.682350／45.111834ms，本次该分层观察值达门；sealed quote p95=23.801760／p99=25.323174ms，原29.304506／60.467261ms，**本次quote p95仍超20ms**。partial故障层另列，稀有和L0缺样仍保留，正式十类门不签。

故障按墙钟选择job，本次38/72、原32/65，sealed execution样本1304/原1318，quote1956/原1977；背景状态也不同。因此只能报告观察改善，不能宣称严格受控加速比或把两次最好值拼成正式通过。完整数字及限制可由compare.py重算。

RSS采样峰3489775616B、已取得footprint峰3440120512B，2501次进程footprint缺测；normal/swap净增0，但初始swap6530465792B，不说无swap。[覆盖](coverage.json)与[诊断收费](diagnostic-verification.json)分别报告。故意kill的Sim1/worker2没有正常heap，其中worker之一有sealed report但仍缺heap；不混同footer。物理峰差异不能全部归因于本次编码改动，不按历史峰相减填预算。

世界逻辑文件峰6888485396B，[3950工件归档](t13a62-joint-01/manifest.json)及[静态完整性](static-verification.json)绑定真实终态/源码/binary/冻结输入。大压力文件完整hash后保留work而不塞入zip，不删除唯一验收快照当缓存。verify/workspace/coverage/diagnostic/latency/collect/static/compare均实际执行，不以脚本0替代上述失败门。

## 未签收与下一依赖

normal64/3.10GiB与共同8MiB未分配前闭合；正式四档、三场景各三轮2100秒及真正8h未完成；32GiB M4不替代真实16GB，16GB not_run、M0 not_accepted。原v50失败、v59缺监督heap和v61联合结果独立保留。

下一实际方向见[类型化固定工作区](next-workspace-route.md)：先覆盖worker完整空气任务而非只有字节槽，避免违反unsafe_code=forbid的透明allocator改造；后续控制Sim接纳/常驻及helper，并处理quote仍超门，不扩预算/缩负载。概率通则的泛化工具及显式领域接入另推进，不改本次历史oracle。

全阶段复用原target，约607436KiB；无新独立target/incremental。源码/WIP/固定依赖和源码快照、失败/成功原日志及验收可执行文件保留；阶段清理检查见cache-check.json，没有可确认无用的新缓存，实际释放0。
