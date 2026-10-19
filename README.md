# MyCity

仓库发布版本：`v0.5.18`。已开始Rust模型与headless CLI，当前不做UI。

**从[完整产品蓝图](docs/PRODUCT_BLUEPRINT.md)开始。** 总体覆盖市长／City Team、市民生活、企业经营和城市运行，实施分版本；原23类首期和90类目录不作为总体完成证明。

- [四视角覆盖与61条需求追踪](docs/product-coverage-and-requirements.md)：全部能力的完成条件／证据／主动边界。
- [玩家、空间、建设与治理](docs/product-governance-space-and-player.md)：精确选择、文字／语音、事项、协商补偿、议会司法与财政。
- [市民、企业与复合空间](docs/product-citizens-enterprises-and-spaces.md)：生活就业、住房共享、公司持股、复合建筑／商租摊位和访客经济。
- [全模式交通](docs/product-transport-and-logistics.md)：公交停车、电车／地铁／客货铁路、轮渡港口、三档机场与航空公司。
- [能源公用、环境与安全](docs/product-utilities-environment-and-safety.md)：燃气／供热、各种电源、供排水／固废、通信、天气绿化与公服事故。
- [设施／设备构造](docs/product-facility-and-equipment-profiles.md)：可计算的成本、工程、资源、维护、退出及自定义约束。
- [运行、16GB增量预算与版本路线](docs/product-runtime-budget-and-versions.md)：从M0到1.0的依赖和验收，全部机制同世界准入。
- [城市快照、全存档市场与交互增量](docs/regional-snapshots-and-interaction-ledger.md)：地方发展与区域履约分钟、近远迁移、物理网络约束；有界Rust实验不代表全部市场已实现。
- [T07空间、地图与wgpu缓冲布局](docs/m0-spatial-model-and-gpu-layout.md)：原生灰度导入、厘米几何、曲线／门／接缝反例，CPU缓冲打包；无UI／GPU后端通过结论。
- [空间资产与存档根](docs/m0-spatial-storage.md)：不可变地形／几何与资金／区域状态共同恢复，损坏依赖拒载；不等于运行时建设。
- [T08有限经济闭环](docs/m0-economy-runtime.md)：劳动、采购交付、消费、成本、工资税债及退出；P-E0／P-E1实跑，长期平衡、16GB仍未验。
- [T08启动互锁P-BOOT](docs/m0-boot-runtime.md)：真实有限电池／水箱、泵与机组、同池容量、故障残余物与持久恢复；有限实验不代表真实公用网或煤机生命周期通过。
- [T09区域实际履约](docs/m0-regional-fulfilment.md)：三城货物／电水、共池容量、实际费用与损耗、关图／断链／恢复；完整P-CAP及真实车辆／公用物理仍未验。
- [检查点与重试档案](docs/m0-checkpoint-and-retry.md)：完整运行状态、分段WAL、冷凭证与损坏拒载；保留既有责任，不靠清历史绕过容量。
- [M0本轮连续交付：验收方法与结果](docs/m0-continuity-acceptance.md)
- [M0：事实收件、有限背压、责任退出与查询](docs/m0-fact-inbox-and-emergency-exit.md)
- [M0具体实施计划](docs/m0-implementation-plan.md)：任务依赖、跨方向风险、经济原型与真实16GB基础验收，计划与实际状态分别登记。
- [M0实现状态与证据](docs/m0-implementation-status.md)：按当前模型范围记录每阶段实测、未实现／未测项及运行方法。
- [两种独立检查与修正](docs/product-design-final-checks.md)：漏整块功能与跨系统协同分别检查，原型验证另列。

总体方案D_full已交付；当前按M0实施模型。完整场景装配、真实Agent／语音、经济平衡、性能和可玩性仍需原型；16GB预算不是实测通过记录。

[原首期实施切片](docs/FIRST_RELEASE_BASELINE.md)／[历史资料](DOCUMENT_HISTORY.md)保留为来源，不覆盖新总体目标。仓库：[TekesApps/MyCity](https://github.com/TekesApps/MyCity)，公开、[MIT License](LICENSE)。每次文档提交递增附注tag并推送，见[协作规范](AGENTS.md)。
