> **2026-10-21 availability update:** Selected historical synthetic pressure payloads retired; listed snapshots are NOT directly full-restorable. Business/component evidence and original indexes remain. This supersedes older retention promises below. See [availability](t13a50-joint-01/artifact-availability-2026-10-21.json) and [cleanup record](../disk-reclamation-2026-10-21/STATUS.md).

# T13A-3k：Rust联合托管接入与原负载保护失败

v0.5.50只交付接入实现及部分联合失败证据，不宣称完整联合通过。只Rust模型/CLI，不做UI/GPU/AgentKit。

## 来源与实现

最终[来源](source.json)3931598f6b6b6d1b5fe6b297a0e18b53d9732a6eeaffd88b4cb3b144be50cd4a（144文件），binary29e86c69728fa5c2da6d37c4d37f401105880977cd074142d1a195b0878cf72d；原冻结输入和observer SHA均不变，[静态核验](static-check.json)可复跑。

joint-endurance由Rust监督实际pool/layout/business Child，替代Python业务启动器。完整L1 ready后启动原完整scale-endurance；业务完整退出后才允许向布局owner pipe发送S，布局至少600秒smoke/28800秒formal。EOF/非法命令/子进程失败和35000秒布局/36000秒监督上限均失败；固定publication end前的每个deadline排空、不重置时钟。扫描1秒/快照300秒、两代保留、128固定摘要未变，结束后冷读最新完整布局快照。新增首行identity在诊断计算前持久化，含PID/process instance/producer counter/source，与目录和正常报告精确配对；无footer仍缺失。最初额外identity文件会超原4096文件保护，已改写既有日志首行并计原额度，没有提高上限。

## 验证范围分源

最终来源[45定向检查](tests.json)（35 headless lib+10 compute_worker）、clippy/fmt/Release build均终态0。初版e5c831身份独立文件来源全仓Release317通过/0失败/0忽略/48sections，保留在[初版证据](initial-identity-file/tests.json)。不能把317当最终同源全仓通过。初版源码和日志独立保留，最终首行和manifest6MiB变更后的验证另列。

## 真实联合失败

25样本/120.276秒normal准入后，work/t13a50-joint-01（实际句柄77755）在316.368480秒因非正常pressure连续60.055959秒保护终止，终态1/子进程-15；swap净增0。[独立失败核验](failure-verification.json)通过，不是运行通过。期间没有本项目回归测试同行，没有减L1或实际业务负载。原因是实测保护条件触发，尚未证明由哪个后台进程或具体内存类别引起，不能归因后删掉失败。

451/723已完成目标业务和空气逐项等于原CPU参考；CPU期末参考和720+3目标文件与v48逐字节相同。L1完成316扫描，每次361272 native+1232426 typed stubs（替身不冒充产品算法）；1完整快照，第二份仅接纳，未完成最新冷恢复、600秒和完整30日。池最后454grant/453归还，还有4MiB grant，无正常退出/最终平衡证明。453正常heap记录、457初始化producer、453 sealed报告；5个producer无正常footer（包括保护终止时进程和原故障进程），身份首行可归属但不补内存或成功。所有已观测核心子进程随后ps不存在，只证明已退出，不证明业务成功收尾。

采样RSS峰3478306816B、已获得footprint峰3514340056B，1320次进程footprint缺测，分别报告而非绝对峰。observer热界704474B，名义外部诊断6MiB，合计6995930B未超名义8MiB；这不闭合pool独立费用或普通Engine/serde/helper模型分配，更不签精确常规3.10GiB。[归档清单](t13a50-joint-01/manifest.json)绑定非大文件；[大文件实际hash](t13a50-joint-01/large-files-retained.json)对应ignored work原文件，pending从未被声称完整恢复，未删除。实际测量保护和工件不能用旧成功smoke替代。

## 未签收

[advisor终审](advisor-final-review.md)未发现必须修的新代码阻断，但只同意上述限定范围。完整联合、共同8MiB、精确常规64MiB/3.10GiB、正式三场景各三轮、四档速度、实际8h仍未完成；32GB M4不是16GB设备，真实16GB not_run、M0 not_accepted。接入formal入口不算8小时已跑。继续真实预算/运行缺口；相同负载重测必须重新正常准入，保留本次失败，不新增故事或改小负载。
