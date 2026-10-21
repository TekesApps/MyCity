# 历史验证产物集中回收（2026-10-22）

暂停开发与验证后检查进程、cwd、打开文件、硬链接及后续恢复依赖；未发现活动MyCity或work／target占用。精确计划经advisor审查，补充删除后fsync快照父目录。没有停止其他任务或运行构建。

|口径|本次实测|
|---|---:|
|初始work（归档前）|15,853,172 KiB，15.119 GiB|
|删除前work（含新增归档）|15,882,116 KiB|
|删除后work|14,171,100 KiB，13.515 GiB|
|删除文件／独立inode|529／529|
|删除文件分配空间|1,752,080,384 B|
|新增完整无损归档|29,638,656 B|
|扣除归档后净分配减少|**1,722,441,728 B，1.604 GiB**|
|唯一共享target前后|1,350,904 KiB，未删除|
|删除窗口df可用空间|121,106,780,160 → 122,858,520,576 B|
|df观察增量|1,751,740,416 B，独立记录|

设备／inode去重的st_blocks×512是文件分配空间，不等于APFS独占物理块。df受共享块、快照及其他任务影响，不能将其增量全部计为本次成果；此前约74GB及历史回收量不重复计算。原始指标见prepare-metrics.json和[result.json](result.json)。

## 删除范围及恢复可用性

只删除[14轮简表](removed-rounds.md)中23份旧正常布局的529份重复合成stub，涵盖家庭、住宅、OD、旅客等23种组件。原pressure此前已退役。逐文件确认原index声明layout_not_business、长度／SHA一致、单链接且非pending，并与v70原件逐字节比较。详细路径、轮次、理由、原index和归档映射见[plan.json](plan.json)与[deleted.json](deleted.json)。没有按t13前缀批删。

完整原字节保存在work/retained-component-evidence/duplicate-stubs-v1的23份gzip；实际解压核验长度及SHA，归档和目录持久化后才删除，canonical原件另保留。**所列组件原路径已删除，旧布局不能直接完整恢复。**归档只支持提取对应组件，不重建pressure。原index／历史测量不修改，本availability覆盖其旧留存承诺。

## 保留原因与核验

- v70 joint-01两代及v50 joint-01旧格式：三份完整布局各29项原文件清理前后全SHA核验。三份pressure共9,132,122,112 B、预留页402,653,184 B仍为完整恢复依赖，原样保留。后继完整基线实测通过并解除依赖后才可退役。
- v65 joint-02业务world和v68兼容world各243文件按原固定manifest核验；其余业务world、WAL、收据、财务权益、fault-reference及当前阶段输入保留。历史脚本实际业务依赖未连带删除。
- 原始延时样本约1.26GiB、必要日志、故障pending、negative及共享inode反例保留；固定源码／依赖快照、验收exe和既有无损归档保留。用途不明确的其他内容保留，待逐项确认无恢复／验收依赖。
- 103,183个其他既有work文件元数据未变，详见work-before.json.gz及result.json。源码、Cargo.lock及未提交改动字节前后相同，见source-wip-before.json和source-wip-after.json。
- 唯一共享target供当前角色区域阶段复用，无独立target或有量incremental缓存；未删除立即需要重建的依赖，也未清理其他项目或全局缓存。

## 后续阶段回收

执行[统一回收规则](../../validation-artifact-retention.md)：阶段终态→归档必要证据→明确精确下游输入→复查进程／占用／硬链接→精确清单回收→核验保护文件→分别报告分配空间、du、df。新目录创建前写用途和解除条件；复用既有输入及唯一target，复制须说明理由，测量期间不清理，每阶段必须报告回收或保留原因。

本次只完成清理及留存验证。未完成Rust工作独立保留；性能、正式8小时和真实16GB验收状态不变。
