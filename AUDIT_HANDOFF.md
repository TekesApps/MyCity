# MyCity 当前审计入口：v0.5.202

#149已实际验收关闭，见[回执](docs/evidence/timeline-target-emergency-20261026/closure.json)；v202仅同步回执与台账，游戏源码与已验证v201相同。新审计可固定v201（源码与实际验证）或v202（另含实际Issue关闭），请记录完整commit。既有v200审计tag保持不变。

当前main的#149已完成[有限合同验证](docs/evidence/timeline-target-emergency-20261026/STATUS.md)：生产字节与v200相同，专属测试修正held预留设置；原共享／默认栈／恢复及有限normal角色门通过。Issue实际关闭另看后续回执，不以tag代验收。新审计可固定v0.5.201；已经固定v200的审计继续有效，旧tag不移动。

## v0.5.200 原审计时点与说明（历史保留）

审计请固定附注tag `v0.5.200` 并记录完整commit；main后续可能继续推进，历史tag不移动。当前root是共享核心唯一writer。其他Agent先只读审计，按文件、可复现的正常游戏场景、影响与具体建议回报；构建／测试由root在原外置SSD唯一target/work串行排程。

## 最新内容与验证边界

- main包含截至v199的既有集成与验收文档，以及**#149当前修复候选和测试源码**。当前候选未编译、未运行、未验收，Issue开放；[实际失败与差项](docs/evidence/audit-main-20261026/STATUS.md)。不要把本tag当稳定性承诺。
- #149实际原版本失败已保留。后续还须候选专属／影响回归、默认栈、保存恢复和必要资源检查；本轮只是审计固定，不重复启动验证。
- #136与#137、#150已在各自固定版本完成有限验收，见[136证据](docs/evidence/canonical-capacity-20261026/STATUS.md)及[137/150证据](docs/evidence/typed-opening-provider-20261026/STATUS.md)。这些结果不自动覆盖v200。
- 24份无关M0／observer WIP以[完整原件ZIP](docs/evidence/audit-main-20261026/unintegrated-wip.zip)、[清单](docs/evidence/audit-main-20261026/unintegrated-wip-manifest.json)及[tracked差异](docs/evidence/audit-main-20261026/unintegrated-wip.patch)提供。ZIP包含未跟踪原件，patch只覆盖tracked变化；原路径保持WIP，不按已集成功能处理。
- 既有18项源码交付积压仍待集成，原交付与私有树不等于main已有能力；具体归属见[Issue台账](docs/issue-execution-ledger.md)和[机器台账](docs/inputs/issue-ledger/registry.json)。仅本机外置SSD保存的交付包／运行原件，异机拿不到时须标未独立核对，不假定repo包含它们。

## 审计顺序

1. AGENTS.md、[当前执行](docs/execution-current.md)、[产品蓝图](docs/PRODUCT_BLUEPRINT.md)和对应Issue完整有限合同。沿原目标，不重复全产品规划。
2. #149 transaction.rs的动作分类／Prepared前后状态、原timeline_driver的完成边界、原Store应急准入与continuation，再读新增timeline_cancel_emergency测试预期。重点是取消未执行目标后实际退出，不能取消已有义务、款项或占用。
3. 相邻业务共存、原票重复、暂停继续、保存加载、写入中断和丢回复；不要扩为无关安全认证或要求全产品先通过。
4. 区分实际实现、候选、已有设计和未测。M0冻结标准、真实16GB／native／长期速度、UI／GPU各保留原未完成边界。

可执行 `python3 docs/evidence/audit-main-20261026/verify_snapshot.py` 只读核对源码与WIP包身份；它不是编译或游戏测试。旧[AUDIT v196快照](docs/evidence/audit-baseline-20261026/snapshot.json)保留历史，不覆盖本入口。

完整源／失败程序／World／WAL／checkpoint与恢复证据保留在本机HP P900 `work/`，本轮不删除或覆写。异机审计可复核仓库内源码与概要；无法访问的本地原件必须明确未核。无PR、无新增Issue关闭，最新统计和差项以本次STATUS与Issue实际状态为准。
