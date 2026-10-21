> **2026-10-21 availability update:** Selected historical synthetic pressure payloads retired; listed snapshots are NOT directly full-restorable. Business/component evidence and original indexes remain. This supersedes older retention promises below. See [availability](t13a3g-joint-01/artifact-availability-2026-10-21.json) and [cleanup record](../disk-reclamation-2026-10-21/STATUS.md).

# T13A-3g：有界诊断实现通过，原联合实测保护失败

本阶段只交付有界控制数据、共同候选份额实现及其功能证据；**同来源完整联合运行未完成，共同8MiB尚未签收**。仍只做Rust模型和CLI。

## 已实测

- [最终来源](source.json)的全仓 `cargo test --release --locked --offline --workspace`：293项通过、0失败、0忽略；[完整日志](tests-workspace.txt)。`fmt`与workspace/all-target严格clippy通过；初次clippy失败及修复后日志分别保留。
- [真实功能探针](functional.py)完成三种helper及四个拒绝反例。full CPU的723目标和完整Engine字节等于v42，CPU报告除source/wall外所有字段相同。三种helper成功、坏image/参考资产/旧result/错误oracle均被拒绝；杀进程、父管道断开及资产完整性另有全仓测试。构造image链不是真实Store冷开或完整Sim证据。
- v2控制协议采用固定Text、三个artifact槽、8KiB输入上限与分配前owner检查；root原文单独发布并传小凭证。helper静态容量256KiB中40KiB解析/错误为保守预留，不是全allocator硬池；主线程之外仅父死亡监听，未来并行解析需全进程额度。128KiB监听栈等native不属于这个诊断额度。
- 共同候选份额Sim2.5/worker1/L1诊断1/observer2/supervisor0.25/driver1/helper0.25 MiB，共8MiB。驱动/监督终身requested Rust heap加64KiB尾部检查在无heap日志时也执行；并非事前拦截每次malloc，更非全部物理进程限额。空气报告以固定4KiB scratch产生相同完整little-endian hash；没有减少数值步或业务输入。

## 原联合尝试：failed

连续120.213秒normal、最大采样间隔5.017秒后，执行原600扫描/0与300秒快照/就绪120秒后full30日的联合命令。约3.1GiB完整L1负载加载后系统pressure变为warning，持续60.229秒，观察器在总61.051秒按原保护停止。swap净增0不能豁免pressure门。

[失败核验](failed-verification.json)及[原始归档](t13a3g-joint-01/manifest.json)保留245帧RSS、事件和部分布局工件；所有帧RSS加和已核验。共同采样RSS峰3,368,796,160B，取得的footprint峰3,378,105,656B，245次process-footprint缺测保留。尚未到120秒业务启动，**本次没有30日Sim、724窗口、3011原票链、两代完整布局及最终冷装结果**。保护终止没有正常heap footer，不能计为0，也不能拿旧v45结果填本次。

本次完整L1已按原规格加载，未缩小；30项大布局文件共3,378,784,572B保留在ignored `work/t13a3g-joint-01`，Git归档其index/hash与元数据。没有停止其它应用，失败后pressure仍warning，不立即重试。[静态核验](static-check.json)核验137源码文件、55项归档、二进制/观察器与入场样本。`verify_full.py`、`verify_joint.py`、`coverage.py`是未来成功同规格运行的核验器，本次未执行成功，不能作为通过证据。

## 后续与未签收

继续常规64MiB真实所有者、grant与未借出触页余额实现；候选诊断协议在正常准入满足后补原联合实测。冻结集中/分散到期、其它分支、四档paced、三场景各三轮35分钟以及实际8小时/96保存/三类故障仍须完成。经济平衡与完整产品吞吐不能由有限L2或布局替身证明。当前机器32GiB，真实16GB not_run；M0 not_accepted，目标继续有效。
