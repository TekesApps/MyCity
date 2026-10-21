> **2026-10-21 availability update:** Selected historical synthetic pressure payloads retired; listed snapshots are NOT directly full-restorable. Business/component evidence and original indexes remain. This supersedes older retention promises below. See [availability](t13a63-joint-01/artifact-availability-2026-10-21.json) and [cleanup record](../disk-reclamation-2026-10-21/STATUS.md).

# Worker固定空气模型背板及实际借还

v0.5.63阶段发布，仅Rust模型/CLI。原[计划](PLAN.md)不改变冻结负载、CPU参考、规范字节或保护门；[边界](DESIGN.md)与[advisor代码审查](advisor-code-review.md)明确限制。没有新增UI/GPU/AgentKit或概率策略。

## 实现与来源

bounded worker将20000 Source、128²格field/scratch/integral、持久schema/摘要字符串及padding放入一个真实1MiB固定背板。复用原State数值内核；Rows只暴露活动切片，不暴露Vec扩容/替换。AirWorkspace不暴露State/Input且不能clone；seed直接恢复，不先造owned State复制。失败reset填充，规范编码只含活动切片，同进程跨job复用。输入身份流式hash保持原SHA。

worker启动前原64MiB池实际释放5MiB（IO4＋typed1），分别创建/扫描后Filled携同一实际child的IOidentity和typedinstance/cap/scan；换typed身份的重复Filled拒绝。worker页59..64、首个Sim编码页55..59，正常双owner池55MiB。实际Child wait后补齐5页才发布信用；部分初始化/补页失败保持关闭。成功在Published门后扫描loaded/reset再Complete，故障缺最后报告不推为reset成功。

1MiB只是固定heap背板capacity：typed662287B＋padding386289B，原生metadata/栈、解析/控制临时及Engine/Prepared/Store/helper仍EXTRA。active合法性/固定填充扫描不是独立数值oracle；后者由逐页对照和下列原全业务摘要证明。禁止把本次改动写成全worker无动态分配或normal64/common8闭合。

最终[来源](source.json) c23947ba5450b674ed391e83e9595e1ad8fcd24552adbb0bc55a1aeea4e1da4b／152文件；binary a91c5580f32266e8cbd5c2ffaab872a0ee3b6cea90b0d2169bb3b9076fb4bcef，实际测量使用保存的candidate executable。此前ee8af3来源全仓Release44351终态0、harness340（含nested非去重），之后Clippy因两处needless_lifetimes退出101，原日志/来源保留。最终仅省略HashSeed/Schema的两处显式生命周期，[精确差异](lifetime-only.diff)可从两份zip核对；最终来源针对性回归83177终态0、131个harness项，Clippy/fmt/Release build55299终态0。**未宣称最终来源重新跑了全仓**。完整[测试记录](tests.json)、[失败与早期来源](attempts.json)分列。

核心覆盖32/64/128边长×0/1/20000源，每个Sources/Cells页逐字节对照owned State并在同一背板恢复续算；超量在下一value解码前拒绝、非法token/depth、输出失败/回填及实际worker/Sim退出、丢回复、重复信用与部分补页失败均有实测。

## 原完整L1＋L2短联合

连续120秒normal准入后，实际84790终态0，wrapper 610.372237708秒，经营时钟 417.637036542秒。原600次L1全扫描、两代各29文件/3378784402B快照、最新冷装与续扫，L2全30日/723目标/724逻辑窗口/2880查询/96保存/实际Cells与提交丢回复故障/冷开全部保留。没有本项目并行build/test/benchmark；准入前未检测到其他编译/模型进程，不宣称整个机器绝对隔离。

[独立业务核验](verification.json)确认3013原票（含Fail/Rebind两笔）、完整Engine含metadata等于独立faultreference、全部724输入/候选hash仍等于原v48参考。[工作区核验](workspace-verification.json)确认4代Sim/725个实际worker分别借还，正常双owner池55MiB、最终64MiB；724份实际loaded/reset报告同一个kernel身份/稳定背板/真实capacity/完成恢复/空tail及填充通过。Cells故障无法生成末尾报告；提交丢回复故障发生于已完成计算后。缺正常heap/footer继续标缺测，不以此推零。

L1 native361272/stub1232426原全量，布局替身不冒充完整城市功能。两代持久保存/完整回读22.471245208、23.241005333秒，最新冷装监督时轴7.926647875秒。世界逻辑峰 6897257578B；[3940项工件](t13a63-joint-01/manifest.json)归档，唯一大压力文件全量hash后保留ignored work，不当缓存删除。

## 失败分位与未签收

[原始分层](latency.json)sealed execution capture/admit p95/p99=22.185695/46.696616ms、1334样本；sealed quote=38.140652/82.436250ms、2001样本，均未达原20/40ms门。partial故障分层另列，稀有/L0缺样保留。原v62经营356.202284167秒，本次417.637036542秒；本次没有证明提速，观察值较慢。不同故障job/样本与运行环境不构成严格受控因果对比，不挑最好值拼正式门，也不以固定工作区功能通过覆盖性能失败。下一性能工作需同负载定位，不能减少空气步/业务或放宽阈值。

RSS采样峰 3466674176B，取得的footprint峰 3486913192B、2552次缺测；normal持续，swap初始 5407047680B、观察净增 61341696B，低于原256MiB保护界，不说无swap。[覆盖](coverage.json)与[诊断收费](diagnostic-verification.json)保持原角色上限，但共同8MiB与normal3.10GiB仍未分配前闭合。脚本终态0证明其限定核验，不代表正式性能门通过。

正式十类分位/full四档、三场景各三轮2100秒稳态、真正28800秒8h及真实16GB均未签；32GiB M4不替代16GB，16GB not_run，M0 not_accepted、目标active。下一依赖先定位本次时长/分位并闭合剩余Sim/helper分配，再进入正式联合协议；不新增同类故事。

[advisor最终独立核验](advisor-final-review.md)实际重跑verify/workspace/static均0，认可仅固定背板/借还与联合功能阶段发布，明确上述性能失败及未签门。

## 构建缓存

全阶段复用原target，没有新的独立target。[清理记录](cache-check.json)：阶段验证结束后，两次lsof及进程检查确认旧debug/.fingerprint、build、deps无占用且后续全部Release不依赖它们；删除705个可再生缓存文件，独立文件allocated60801024B（约58.0MiB），可用空间实测净增54751232B（约52.2MiB，独立记录APFS/其他活动差异）。源码/WIP、Cargo.lock及固定源码/依赖快照、所有日志/原票/工件与验收可执行文件保留；共享Release供后续验证复用。最终source.zip复用同一冻结zip的硬链接；验收binary独立保存并供发布别名硬链接，不依赖可覆盖target。
