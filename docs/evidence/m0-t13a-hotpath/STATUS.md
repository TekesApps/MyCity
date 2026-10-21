# 规范整数编码与ARM SHA运行时分派

> 当前可用性更新（v0.5.68）：v64 joint-01及v65 joint-01的四份压力载荷已退役，对应快照不能直接完整恢复；原始证据保留。v66 joint-02两代及v50旧格式依赖仍完整。详见[后续清理报告](../disk-reclamation-2026-10-21-followup/STATUS.md)。旧文中的完整保留承诺仅代表当时状态。

v0.5.64阶段发布，仍只做Rust模型/CLI。[计划](PLAN.md)保留原冻结负载/SHA、全部数值步、独立CPU参考和20/40ms门；未修改UI/GPU/AgentKit。[advisor方案及代码检查](advisor-plan-and-code.md)未发现编码/错误传播/深校验阻断。[独立最终审查](advisor-final-review.md)实际重跑四项核验，同意有限阶段发布，完整M0验收仍未签。

## 定位与实现

[只读诊断](diagnostic-comparison.json)使用原full source/claims实际锚点（64工资/20000债项/128²场），先验证root/index/image SHA后复制只读image，不打开或重写旧Store。固定warmup5/样本40，原始每次纳秒和来源分别保存；输入、检查点摘要及报价结果一致。这不是正式分位或吞吐门，包含抢占及返回值释放，不能外推严格因果加速比。

原checkpoint摘要约3.6ms，clone约0.004ms；只优化整数编码后约3.06ms，说明仍有其他成本。再对现成字节单独SHA，未加速约1.2–1.3ms；锁定sha2 0.10.9注册源码显示aarch64默认软件后端，asm feature才启用运行时SHA2检查，本机FEAT_SHA256=1。最终小样本摘要约1.06–1.08ms、报价约2.29–2.35ms，来源/阶段不合并样本。

`air/wire.rs`仅把field/scratch/integral的原u32/u64位整数用安全十进制pair digits＋固定8KiB chunk成批写入。Input/Progress/Totals等仍serde，字段/外层顺序及字符串转义不变，State原derive Serialize保持独立参考。全部输出沿原BoundedEncoding及write_all；没有缓存、减少深校验/独立CPU比对、改浮点kernel或减少扫描。两份HashWriter8KiB＋数组chunk8KiB为24KiB显式缓冲重叠，另有20byte digits/serde/编译器副本/原生栈，不当完整栈峰值或已闭合内存门。

仅aarch64目标为原sha2=0.10.9启用asm，依赖通过cpufeatures运行时选择硬件/soft fallback；同目标feature合并可能影响core/storage/headless，非“只改core”。[特征图](features-aarch64.txt)与[x86图](features-x86_64.txt)为实际Cargo输出，后者不代表跨平台编译通过。本仓unsafe_code=forbid保持；依赖内有unsafe/汇编，不声称全树无unsafe。其他平台构建、无SHA2设备和本机forced fallback执行均not_run。

新锁定sha2-asm0.6.4/cc1.9.0/find-msvc-tools0.1.15/shlex2.0.1；[包块/校验和证明](dependency-proof.json)确认原包版本/checksum/依赖块保持（只给sha2增加asm边），四个原始.crate/许可证完整归档于new-dependency-sources.zip，作为固定依赖证据保留。

## 最终来源验证

[最终来源](source.json)895abc10f694812e9055774535bb0dcec48670715a2df074a3c14209ac3c3ba2／155文件，binary fa9f4490f7eb5326903788c063e7ac6ebe8bd8f398cb0d592b52b87c982db7f0，保存的独立candidate参与测量。最终全仓Release19211终态0，harness343含nested非去重独立数；Clippy82445/fmt/全目标Release构建均0。[测试记录](tests.json)与各早期profile源码单列，不拿早期小样本替代最终全仓。

独立State Serialize→原始payload SHA→Legacy外层逐字节比较扩展至所有Sources/Cells页及complete；128²×0/1/20000源，配合32/64/128背板逐页恢复测试。整数0/u32max/u64max/十进制边界/固定16384随机整数、空数组/chunk尾、短写/底层错误/原限额，及Python固定SHA期望和1/7/63/64/8191增量块均实测通过。原故障、存档、工资税债、区域等全仓回归保留。

## 原完整L1＋L2联合

连续120秒normal后实际28854终态0，wrapper607.920064834秒，经营304.118845秒，仍是短联合smoke，不是8h。源码/fixtures冻结，无本项目并行test/build/benchmark；不声称整机绝对隔离。

[独立核验](verification.json)确认723目标/724逻辑窗口、3013原票含Fail/Rebind两笔、完整Engine含metadata等于独立故障参考、全部724输入/候选hash等于旧v48。2880查询/96保存及实际Cells/提交丢回复故障/冷开不变。[工作区](workspace-verification.json)仍725 worker/4 Sim、724份loaded/reset，正常双owner55MiB/最终64MiB，不以缺故障末尾报告推定reset。故意退出的Sim1/worker2缺正常heap继续列缺测。

L1完整600扫描、native361272/stub1232426；两代各29文件/3378784402B，持久写入与完整回读7.065230917/10.772401917秒，最新冷装监督时轴5.336675416秒。[3946工件归档](t13a64-joint-01/manifest.json)绑定源码/binary；唯一大压力文件完整hash后保留work。布局替身不冒充全部城市功能。

## 分位观察和未签收

[原始分层](latency.json)sealed execution capture/admit p95/p99=6.875247/10.039755ms、1278样本；sealed quote=13.602415/18.677290ms、1917样本，均在原20/40ms观察门内；covered sealed execution没有超门项。partial故障分层另列，L0/稀有缺样和正式协议仍未完成，不能签正式十类总门。

原v63执行22.185695/46.696616、报价38.140652/82.436250ms均失败，本次观察改善；经营417.637036542→304.118845秒。故障job原32/57、本次45/85，sealed样本原1334/2001，本次1278/1917；运行环境也不同，不能合并最好值或宣称严格受控整组加速比。

RSS采样峰3480223744B，取得的footprint峰3463533224B、2523次缺测；normal持续、swap初始5283840000B且净增0，不说无swap。[覆盖](coverage.json)/[诊断收费](diagnostic-verification.json)分别报告。世界逻辑峰6898536779B；普通Engine/Prepared/Store/解析/helper及栈仍EXTRA，不按硬件SHA自动闭合normal64/3.10GiB或共同8MiB。

正式四档、三场景各三轮2100秒稳态、真正28800秒8h和真实16GB仍未完成；32GiB M4不替代16GB，16GB not_run、M0 not_accepted、goal active。下一依赖转回Sim/helper分配前预算和实际调度耗时，再进入正式协议，不继续追加已收口专题故事。

## 缓存清理及更正

全阶段共用原target，无独立target。初次依据Cargo文件名清理时，Cargo以无hash别名列出当前库，误删四份可再生依赖库；已真实重建并验证硬件诊断binary SHA不变，随后全目标构建全部fresh。源码、未提交内容和全部验收可执行文件/原证据始终保留。

[原始记录](cache-cleanup-initial.json)保留15项删除及占用，不把它们全部算成无用缓存。[更正](cache-correction.json)分清4份重建当前库与11份真正过期库，后者allocated62570496B（约59.7MiB）。删除前两次lsof及进程检查无占用，但初次后续依赖识别有错，不隐去。保护规则已补入.rmeta对应.rlib；证据脚本为一次性、已执行后拒绝再删，未来需重新审计当前图。

重建后观察可用空间较初始增975888384B，期间有其他回收/APFS活动，不能全归因于上述11份缓存。当前共享Release/新依赖包快照、所有源码/失败成功日志/票据/验收exe继续保留，具体最终占用另记录。
