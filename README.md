# MyCity

仓库发布版本：`v0.5.68`。已开始Rust模型与headless CLI，当前不做UI。

**软件结构先读[总体架构](docs/SOFTWARE_ARCHITECTURE.md)：当前代码、主体与事件、单writer、多进程目标、数据文件和预算。**

**从[完整产品蓝图](docs/PRODUCT_BLUEPRINT.md)开始。** 总体覆盖市长／City Team、市民生活、企业经营和城市运行，实施分版本；原23类首期和90类目录不作为总体完成证明。

- [四视角覆盖与61条需求追踪](docs/product-coverage-and-requirements.md)：全部能力的完成条件／证据／主动边界。
- [属性驱动的概率机制通则](docs/regional-probabilistic-decisions.md)：跨系统识别统计倾向，组合区域／经济／气候／居民／企业等相关属性，统一概率语义、抽样及恢复契约；仅设计，尚未实现／调参／性能验证。
- [玩家、空间、建设与治理](docs/product-governance-space-and-player.md)：精确选择、文字／语音、事项、协商补偿、议会司法与财政。
- [市民、企业与复合空间](docs/product-citizens-enterprises-and-spaces.md)：生活就业、住房共享、公司持股、复合建筑／商租摊位和访客经济。
- [全模式交通](docs/product-transport-and-logistics.md)：公交停车、电车／地铁／客货铁路、轮渡港口、三档机场与航空公司。
- [树种、地理适生与空气作用](docs/tree-species-climate-and-air.md)：经纬度／气候／场地筛选，树种差异与有界环境作用；设计已补，运行未接。
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
- [T05-B全源与持久日历](docs/m0-timeline-runtime.md)：已实现profile的真实到期、工资账期、P0—P5与有限续做票；逐页恢复实测，完整产品日历与16GB仍未验。
- [T13-A模型规模与性能](docs/m0-model-scale-performance.md)：已冻结L0真实包／L1完整布局压力／L2引擎上限、三轮稳态及实际8小时门；已完成CPU参考、完整L1布局短诊断及[三档真实Sim／worker／v2存档30日](docs/evidence/m0-t13a-tiered/STATUS.md)及[真实计算区间计时与完整诊断](docs/evidence/m0-t13a-cpu-regions/STATUS.md)及[稀有类别真实执行样本补齐](docs/evidence/m0-t13a-rare/STATUS.md)；另有[完整L2无等待30日速度实验](docs/evidence/m0-t13a-speeds/STATUS.md)与[实际session节奏修复](docs/evidence/m0-t13a-session-pacing/STATUS.md)，[P-E0实际session无等待30日](docs/evidence/m0-t13a-pe0-speeds/STATUS.md)已核验；[修复后P-E0实际60档](docs/evidence/m0-t13a-queued-pacing/STATUS.md)完整30日已核验通过（旧失败留证）；另有[固定L1恢复约束与旧组件兼容](docs/evidence/m0-t13a-bounded-restore/STATUS.md)及[Sim固定编码／候选owner与完整30日功能核验](docs/evidence/m0-t13a-sim-workspace/STATUS.md)（缺监督者heap记录、正式分位失败均明确保留）；[原完整L1＋L2短联合重测](docs/evidence/m0-t13a-joint-recheck/STATUS.md)已完成；[接纳流式摘要及同负载重测](docs/evidence/m0-t13a-stream-digest/STATUS.md)已有新证据；[worker固定空气模型背板](docs/evidence/m0-t13a-air-workspace/STATUS.md)将typed1MiB与IO4MiB合并为5MiB实际借还，性能分位失败与其余预算缺口仍保留；正式分位／full四档、完整联合预算、联合稳态及8小时门仍未通过。
- [可恢复后台计算验收](docs/background-compute-acceptance.md)：冻结W0—W4、独立worker／Sim唯一写入／联合故障实验与全进程测量；W1—W4有限闭环已实测；[1800秒联合运行与限制](docs/background-joint-runtime.md)及[逐项完成审计](docs/background-compute-completion-audit.md)有原始工件和最终独立结论，16GB及完整M0仍未通过。
- [W2任务与同根接纳](docs/background-store-runtime.md)：实际货运输入、P4屏障、Sim资产／小WAL、代际及独立预留；独立Sim见W3，有限长跑及进程组观察见W4。
- [W3独立Sim与恢复](docs/background-sim-runtime.md)：原子换代、固定接纳原票、持久worker身份及OS缓存栅栏、6MiB磁盘charge和真实进程故障；[W4联合运行](docs/background-joint-runtime.md)已完成1800.435秒及独立核验／审查，采样限制明确。
- [W1独立空气worker](docs/background-worker-runtime.md)：连续窗口、同进程复用、有界文件协议与阶段握手杀进程恢复；未接世界／Sim持久任务。
- [T12-A空气CPU参考](docs/m0-air-cpu-reference.md)：单通道EI、源／tile分页、数值位恢复及独立f64复算；此为数值基线，有限城市接纳见W2；GPU及worker池仍未实现。
- [进程、worker与GPU渲染架构](docs/process-worker-and-render-architecture.md)：App监督／Sim唯一writer、可重算任务与渲染部署、进程组16GB预算；worker纯计算见W1，独立Sim见W3，渲染尚未实现。
- [T10有限CLI会话](docs/m0-cli-session.md)：可信宿主身份、暂停／单步／实际倍率、冷回执与快照分页；T05-B分页用step，UI及16GB仍未验。
- [M0本轮连续交付：验收方法与结果](docs/m0-continuity-acceptance.md)
- [M0：事实收件、有限背压、责任退出与查询](docs/m0-fact-inbox-and-emergency-exit.md)
- [M0具体实施计划](docs/m0-implementation-plan.md)：任务依赖、跨方向风险、经济原型与真实16GB基础验收，计划与实际状态分别登记。
- [M0实现状态与证据](docs/m0-implementation-status.md)：按当前模型范围记录每阶段实测、未实现／未测项及运行方法。
- [两种独立检查与修正](docs/product-design-final-checks.md)：漏整块功能与跨系统协同分别检查，原型验证另列。

总体方案D_full已交付；当前按M0实施模型。完整场景装配、真实Agent／语音、经济平衡、性能和可玩性仍需原型；16GB预算不是实测通过记录。

[原首期实施切片](docs/FIRST_RELEASE_BASELINE.md)／[历史资料](DOCUMENT_HISTORY.md)保留为来源，不覆盖新总体目标。仓库：[TekesApps/MyCity](https://github.com/TekesApps/MyCity)，公开、[MIT License](LICENSE)。每次文档提交递增附注tag并推送，见[协作规范](AGENTS.md)。

最新有限阶段：[规范整数编码、ARM SHA分派与原负载联合实测](docs/evidence/m0-t13a-hotpath/STATUS.md)。最终同源全仓Release测试通过；原30日联合负载的完整业务、旧hash与恢复核验一致，执行和报价观察分位进入20/40ms门。正式性能协议、完整内存预算、8h和真实16GB仍未通过；不宣称M0完成。

最新实测：[730实例的真实full30日与驱动费用](docs/evidence/m0-t13a-runtime-ledger/STATUS.md)。724个worker、3011原票和真实Store冷开完整对照通过；业务279.96秒，驱动整堆峰加尾部600,825B。共同诊断／64MiB／L1联合／四档／稳态／8h及真实16GB仍未通过。

本阶段v0.5.47：[64MiB实体余额与724次真实worker借还](docs/evidence/m0-t13a-normal-pool/STATUS.md)。同源303项回归通过，修复accepted socket分片误拒；无L1的原full30日、完整CPU／原票／候选hash和731正常实例核验通过。原联合压力失败及首个功能失败保留；共同内存预算、四档、正式稳态、8h及真实16GB仍未签收。

本阶段v0.5.48：[CPU broker实际持有worker及有限恢复](docs/evidence/m0-t13a-broker/STATUS.md)。312项同源回归、原full30日／724次真实借还／完整Engine核验通过；旧启动超时与测试失败保留。实际8h及联合性能仍未签，下一步实现原长时控制器并实跑。

[Validation artifact retention](docs/validation-artifact-retention.md) / [2026-10-21 cleanup](docs/evidence/disk-reclamation-2026-10-21/STATUS.md)

v0.5.66: [Fixed end-of-run helper workspace and original full-load joint diagnostic](docs/evidence/m0-t13a-helper-compare/STATUS.md). Full model ownership, formal performance, 8h and real16GB gates remain open.

v0.5.67: [Typed full-model capacity plan](docs/evidence/m0-t13a-model-capacity/STATUS.md). The proposed backing fits64MiB; current runtime ownership and performance gates remain unaccepted.

[v0.5.68历史验证产物后续回收](docs/evidence/disk-reclamation-2026-10-21-followup/STATUS.md)：释放11.34GiB分配空间，保留完整恢复依赖、真实业务与当前模型WIP；不代表新模型／性能通过。
