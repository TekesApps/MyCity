# #149 完成边界取消目标与原实际退出：有限合同验证

生产修复已在v0.5.200审计候选入库；该版本当时未编译运行，历史声明不倒写。当前source02保留相同transaction.rs生产字节，只修专属反例的配额设置；v0.5.201发布当前测试与实际验证。Issue实际关闭在后续回执记录，不以tag代验收。

## 原问题与最小修复

原v198/v199实际baseline01：完成3600秒、phase=None、未来target=7200，普通额度已不足，应急空间仍可用；退出、取消目标、继续分别Capacity拒绝，状态不变。预期合法取消成功的断言exit101，完整失败源／程序／world保留在work/timeline-target-emergency-20261026，概要见[原实际失败](../audit-main-20261026/baseline-observations.json)。

transaction.rs只把经过原prepare/caller/CAS的单独CancelAdvance纳入责任控制，并核旧target>last_complete、已完成边界、新target清空且时间／serial/lane/cursor不变。claims/payroll/来源、资金库存、Background、原Resource/Lease及各已接业务必须保持；仅允许原driver重置页报告，不将其视为已付款消失。实际占用仍须原Exit。原Busy门、driver、Store、continuation、持久字段和预算未改。

原advisor窄复核当前生产差异无新增阻断：合法phase=None本来不持有continuation，既有Some票必须phase=Some；原WAL/admit路径继续生效，未提前退额。advisor不运行测试，实际证据由root串行取得。

## 当前实际结果

|门|实际范围与终态|
|---|---|
|专属release|4父测试通过；热态与先恢复后取消、已付17与未来137债务、原占用保留再真实退出、错误caller/CAS/scope／无目标／进行中拒绝、与实际设施劳动／供给／venue共存|
|真实中断与原票|Prepared／WalPayloadHalf／Applied三种中断或丢回复，与完整未中断参考的hash／sequence／完整Engine字节一致；重放不二次取消或释放|
|独立冷读|专属每次完整运行均实际启动7个冷child并exit0；冷helper不计新业务，debug与release重叠不累加功能数|
|当前受影响共享|core Timeline／time_capacity／旧venue，Store Timeline／time_capacity／旧venue／checkpoint／facility_operation，加真实headless task_registry_directory，共9组全部exit0|
|当前默认栈|原股权、跨城供给、资格、设施工时、两道设施Store、typed core和Store共8道原门，加本次取消路径1道，共9道debug全部exit0；无RUST_MIN_STACK覆盖|
|原shipping normal角色|在普通额度不足的真实Store中取消目标、停进程、冷重启、实际退出，与完整直接参考一致；原取消和退出票重放不改hash/sequence；已付与未来债务保留|

固定源码468项见source02-manifest.json；共享、默认、角色和专属均对应当前字节，发布前逐项再次核对。正式终态见[机器决定](acceptance.json)、[共享9组](shared-gates.json)、[默认9门](default07-results.json)、[角色调用](role06-execution.json)。本检查不扩为全产品前置验收或无关安全认证。

## 资源与证据保留

Mac16,1/32GiB、Rust1.85.1、HP P900 USB/APFS；使用原唯一target/work串行。原World128KiB、Engine4MiB、normal64MiB、共同诊断8MiB和M0标准不变。没有新增持久模型布局或第二池。

角色四个观测点allocation_failures=0；arena used最高56602928B <= usable61865520B，已观测lifetime requested peak最高56682720B。角色完整Store owned最大52143358B，是角色内子集，不额外相加；参考／creator Store属工具开销，最终unpooled Store owned42892536B不当作角色峰值。Engine owned3607584B，模型双槽／候选／codec工作区按原角色实际口径另列。normal池两次59MiB授额全部归还，最终64MiB全回收。详见[分列资源](resource08-summary.json)。这不是native物理峰／整机／16GB／可持续速度或长期验收。

最终完整原件索引覆盖work/timeline-target-emergency-20261026及短socket目录work/r46，快照584711538B低于事前调整后768MiB证据盘上限。23份完整固定程序按实际字节核SHA保留，不以hash替代源／可执行／失败和正常World/WAL/checkpoint。无打开句柄、所有已知构建／运行均终态；[完整保留索引](retained-originals08.json)排除可变coordinator，另保冻结副本，后续发布／Issue回执单独新增。保留至后续完整基线明确解除恢复依赖；本阶段无源码／world／worktree回收，无新tree/target/timer。

失败不改写：direct02前三父通过，第四在配额设置时遗漏原3.835MB续页预留，尚未进入取消；仅修测试，保留held且normal limit仍used/2，direct03全组通过。affected04首次遗漏旧测试显式留存目录环境，在游戏操作前退出；驱动补齐后复用同固定程序通过，其余已过组未重跑。原件均保留。24份无关WIP逐字节未变。

本有限缺陷合同已满足，发布核对／Issue回填关闭另记。#136原结果、父#118、完整交通／公服／M0／真实16GB／native／长期／GPU/UI各沿原归属，不由本项代关。无PR，root直接集成。
