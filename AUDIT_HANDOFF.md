# MyCity 审计入口：v0.5.196

本tag固定当前非UI源码审计检查点。main同步到该提交；审计请记录tag对应完整commit，后续main移动不改变该基线。当前root仍是共享核心唯一writer，其他Agent先只读审计，把问题定位到文件/场景和影响；编译、测试、集成修复由root统一串行排程，避免同时占用原target/work。

## 本次main包含什么

- #136已接入的显式时窗模式：一个原Resource/Lease池、canonical/别名、半开窗口峰值预留、真实进入/退出、原票及原Store保存恢复；旧模式独立兼容。
- 与旧venue共存的root只读适配：完整身份门后按模式校验数量，真实分配始终用完整原64 Lease槽，不把过滤视图当新池。
- 对应设计、执行台账、实际验证结果和未验证边界。
- 24份旧M0/observer WIP的[完整审计补充包](docs/evidence/audit-baseline-20261026/unintegrated-wip.zip)、[原字节清单](docs/evidence/audit-baseline-20261026/unintegrated-wip-manifest.json)及[差异](docs/evidence/audit-baseline-20261026/unintegrated-wip.patch)。这些原件在本地原路径保持未提交；补充包只供审阅，没有作为已验证实现合入其原路径。

## 当前验证层级

#136保持Open/待验证，本检查点不增加业务或缺陷关闭数。

|证据|范围与结果|
|---|---|
|source12核心5项|当前生产实现；Count/Rate/KW原整数单位、完整u128标识、旧资源及V2角色排他，通过|
|source13 Store9项|当前生产实现；旧场所与新时窗共存、原满槽、等待退出、关闭/减容、完整正常参考与写中断/丢回报重试、保存/新进程恢复，通过；包含一个cold helper，不能按9项都算独立业务|
|source13旧规则/Store/设施|旧venue纯10、旧venue Store10、facility_operation13通过，原冻结断言未改|
|source14编译|当前11组共享/专项的固定源码构建exit0；source13与14的409项便利清单完全一致；运行尚未开始|
|source07/08/09|修复前11组、normal角色、原6+新2默认栈实际通过，保留为历史，不能替代本次共存修复后的相应门|

完整源460文件的审计身份清单还包括原便利ZIP未列出的root tests/fixtures；这些fixture与v195一致，执行时的固定程序/实际开局原件另行保留。用 `python3 docs/evidence/audit-baseline-20261026/verify_snapshot.py` 可只读核源码与补充包身份；这不是游戏测试。

尚待：共存修复后的受影响共享回归、默认栈和必要normal角色联合资源，以及最终完整合同回填/关闭。真实16GB、全游戏/native/持续快进/长期/GPU/UI门各保留原状态，不能由源码、编译、advisor或tag推导通过。原M0负载和标准、World128KiB、Engine4MiB、normal64MiB及共同8MiB均未改变。

## 推荐审计范围

先读AGENTS.md、[当前执行](docs/execution-current.md)、[#136设计](docs/physical-economy-runtime-contract.md)、[冻结快照](docs/evidence/audit-baseline-20261026/snapshot.json)及Issue #136当前合同，再按实际调用链核原事务/提交/恢复、模式共存和责任保留。采用正常游戏操作、暂停继续、保存加载、中断写入和原操作重试复现；发现疑点不扩为无关安全认证或全产品前置验收。

完整运行原件在本机HP P900的`work/canonical-capacity-20261026`及`work/r39/r40/r41`，包括失败世界、WAL/checkpoint、固定源、程序及部分完整可执行ZIP。恢复ZIP须按记录解压并恢复执行权限；hash不代替原件。仓库上传概要/源码/日志，不把数百MiB本地运行数据混入Git；异机审计若无这些原件，应标记运行证据未独立复核。

## 项目当前状态与统计

- 共享在制：#136，root唯一writer/验证，代码在本检查点冻结；独立准备0项。
- #137只读准备已交付，见[具体API与最少接线准备](docs/evidence/audit-baseline-20261026/next137-preparation.md)，尚未授予实施写权；之后优先消化。
- #134仍须核实际到达/返程/履约来源的有限缺口，不用#73/#74计划代替，不要求整个交通领域完成。
- #49/#50剩余合同未派工，已校正stage:todo；领域父#16持续推进不计活跃开发。
- 累计统计窗口、可靠截止时间见snapshot.json：新验收业务3（132/133/135）、历史补关0、缺陷1（148）；积压从20待集成/0待验证重分类为19/1，总积压未减少。本审计检查点关闭0项。
- 起点不可靠的交付周期保留unknown；统计窗口不当开发耗时，tag数量不代表产品完成度。

UI延期、完整产品及其它领域范围继续沿原蓝图和已有Issue，不在本次审计重新规划或批量派工。无PR，当前按AGENTS直接提交和附注tag发布。
