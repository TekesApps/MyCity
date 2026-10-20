# T13A-3j：耐久控制器功能与默认栈恢复修复

本阶段只验收控制器功能和恢复修复，不签正式性能。最终源码2b378e37683f390f5be24459b933c24b118e8d6071ca5ab116c1f3e8bb9ff885（144文件），binary 1fdd71dcc4c82a21df86f356c9a0a4ef613d9ec21a76f775e7acdd5f38d555cf；[来源](source.json)、[静态核验](static-check.json)可复跑。原冻结负载/门限未改，未实现UI/GPU/AgentKit。

## 实现与实测

scale-endurance用单调绝对时轴安排720小时及3个额外目标、2880查询和96定期保存；延迟请求逐个补齐，不重置时钟或吞掉请求。formal为40秒/小时，末业务边界须≥28800秒；smoke只压缩墙钟为0.1秒/小时，实际业务/目标不减。实际worker Cells故障、Sim提交后丢回复、冷打开及最终重开均由原票和独立CPU全Engine参考核验。故障参考只增加一次Fail/Rebind，普通参考不变。

最终work/t13a49-smoke02（真实执行句柄59179，终态0）在120秒normal准入后运行，没有本项目回归测试同行。包装432.112231秒、控制器业务358.180895秒；原720小时/723目标、2880查询、96保存、3013原票、725借还及4会话完整。[核验](verification.json)证明724逻辑输入/候选hash与v48逐项相同，最终Engine含metadata与独立故障参考逐字节相同；故障相对正常参考只增加revision 2及incarnation 1。此压缩时轴会积压，不是72秒速度门通过。原[工件](t13a49-smoke02/manifest.json)逐文件hash、ZIP保留。

[诊断](diagnostic-audit.json)732正常heap记录；3个刻意故障进程缺正常footer，按缺测保留，其中1 worker已有封存CPU报告。727正常报告、2部分目录，不把部分数据混进封存样本。驱动整Rust堆峰加尾部839797B；Sim组件2446940B、worker组件848052B均在各自额度内，但普通Engine/serde仍为额外压力。pool整堆界合格不证明其诊断份额独立闭合，不能减历史峰补账。

采样RSS峰170852352B、已获得footprint峰146966496B，1811次进程footprint缺测；4个helper均有RSS帧，但采样不证明绝对生命周期峰。压力始终normal，swap净增0。[覆盖](coverage.json)保留实例与缺测，不能推成16GB通过。[原始延迟分析](latency-audit.json)区分producer/quote/execution/replay和部分记录；例如封存Sim CaptureAdmit execution n1318、p95 13.519ms/p99 22.921ms，quote p95 22.893ms超20ms。缺L0/L1及稀有样本，不能签十类正式门或宣称相对旧并行回归提速。

## 实际失败与修复

旧ce9efd源码full debug句柄63357终态101，在scale_rare原锚点恢复发生栈溢出；[旧来源与失败证据](attempt1-source/stack-failure.json)、旧smoke01全部独立保留。最终Engine.world改Box，正确将World计入heap，不增加RUST_MIN_STACK。原失败锚点fixture逐字节保留旧线格式。

最终默认栈debug旧锚点测试0.39秒、原scale_rare 153.54秒均终态0；全仓release [316通过/0失败/0忽略](tests-verification.json)，clippy/fmt/build通过。不能将原full debug失败改写为full debug通过；新全仓证据是release，debug为两项实际故障路径。见[栈修复说明](STACK_FIX.md)。

## 尚未签收与下一依赖

Rust联合监督替换Python功能启动器、完整L1覆盖业务时轴、故障实例初始化身份、共同8MiB和精确常规64MiB尚需完成；普通分配不得在pool触页之外隐匿。正式三场景各三轮、四档速度、实际8小时仍not_run；当前32GB M4不能替代真实16GB，16GB门not_run、M0 not_accepted。控制器存在formal入口不等于实际长跑完成。

下一实现按[联合计划](JOINT_PLAN.md)和[速度计划](SPEED_PLAN.md)推进，不继续堆有限故事；[快进粒度约束](../../m0-model-scale-performance.md#10-快进与计算粒度精度和调用次数分开)区分精确账与稳定区间批处理，未来优化先证明关键边界一致，不能替换当前冻结负载。
