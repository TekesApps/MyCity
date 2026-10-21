# v0.5.67 完整模型工作区容量依据

本阶段增加 `mycity-headless model-capacity-plan`，按实际Rust类型、schema最大记录数和同时存活拓扑形成可执行的容量依据。[方案和槽集合](PLAN.md)、[实际输出](plan.json)、[来源及重算](verification.json)可复核。

| 项目 | 字节 |
|---|---:|
| 每份完整Engine（含World、Arena、journal、Timeline、Background） | 3,926,544 |
| 事务Prepared（含时间线／背景副本及完整dirty表） | 1,274,616 |
| 单份通用空气State的类型化内容最低需求 | 662,624 |
| 实际目标空气槽 | 真实AirWorkspace的1MiB背板含padding，加结构本体；见plan.json |
| Store等类型化元数据 | 145,352 |
| 最重目标拓扑总需求 | 39,415,656，约37.59MiB |
| 原常规预算 | 67,108,864，64MiB |
| 代数余量 | 27,693,208，约26.41MiB |

最重拓扑保守计入3份Engine、1份Prepared、5份额外空气槽、安装后的空间payload、共享codec及并行worker5MiB，没有先假定genesis已经释放。各场景独立求和取最大；不能把互斥场景全部累加，也不能让两个活模型共用一个槽。Opening已列类型化头、最大记录行数和固定字符串，编码输入复用codec；不拿文件长度代替解码对象上界。

`target_slots_fit=true` 只表示**目标方案**在代数上可装入原预算。`runtime_grant_accepted=false`、`core.runtime_ownership_proven=false` 始终保留；余量不是当前代码额外分配许可。现有Clone／Vec扩容／DOM／普通serde／WAL及receipt编码仍需统一迁移和实际授权，不以本报告签收normal64或共同8MiB。

## 实测验证

最终来源5877f0c624fd993ecbd3afd3b4c469e51175e6dac7d314f5f412f46c4c82aa0f，158文件；固定验收fixture hash仍876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508。固定候选binary SHA7453fe14af0f45f1a2286aa2aca3967baec71128d2e0e6e0a4b6998be77078b6，保留于work/candidate-binaries/model-capacity/mycity-headless，完整来源见[source.json](source.json)和source.zip。

[最终检查](checks.json)全部实际退出0：core/storage容量定向测试共2项（其余测试被filter，**不是全仓测试通过**）、Release CLI构建、workspace all-targets clippy、fmt。实际固定候选CLI成功0／多余参数拒绝2，见[CLI回执](cli-checks.json)；逐字段乘法、分项及各场景总和由独立Python重算。原预算恰好满足及少1字节不满足的边界测试通过。没有运行城市业务或性能负载，也不将此诊断算作分级性能验收。

[advisor独立结论](advisor-review.md)确认可作为完整工作区实施依据，无剩余实质容量缺项；实际运行固定候选输出与归档逐字相同，并独立重算上述需求／余量。其结论不是现有动态实现的容量担保。

## 修正与历史

- advisor指出初稿以64KiB编码Opening替代解码容量，已改为类型化记录；pre-opening-fix保留原来源及日志。
- clippy先后报告常量断言、手算位宽，均实测修复；pre-opening-fix、pre-bitwidth-fix分源保留失败，不将旧通过拼接为最终来源。
- 初次capture脚本误将CLI非法参数的预期退出码写为1；程序既有main约定为2。已按既有契约修正脚本，未改程序来满足错误断言。capture-before-exit-fix.py保留原脚本；最终实际回执为2。此前pre-header-dedup-plan.json、tests.txt、build.txt是早期草稿检查，不属于最终来源验收。

## 预算与回收边界

按[回收规则](../../validation-artifact-retention.md)复用现有target；未创建独立缓存或新压力快照，仅保留本阶段必要源码／失败证据和固定可执行文件。共享target仍供当前依赖使用，未认定为废弃；本阶段无新增可删除的独立target。此前完整恢复基线和当前快照继续保留。后续阶段成功替代后再按依赖退役，测量期间不清理。

下一步完整固定ModelWorkspace、Prepared和Store所有入口接入，保留规范字节、旧存档、持久后发布、失败回滚，实测分配前授权／填充／实际wait回收，再验normal64/common8。正式十类性能／全四档／三场景各三轮稳态／真正28800秒仍未完成；真实16GB not_run，M0 not_accepted，goal active。
