# 统一稳态控制器与实际正式L2首轮（v0.5.88）

Rust模型／CLI阶段，无UI／GPU／AgentKit。新增单Instant控制器统一暖场、固定窗口与排空，L0完整经济／启动／交易案例，L1原规模压力，L2原完整30日轨迹。原始批次offset、PID／实例与跨进程因果clock关联；独立筛选保留不确定、失败、重试、quote、不同case／空气规格，核对raw全集。经济探针在每个独立案例报告完成后释放其Store，修复实际59MiB角色区内多案例同时驻留失败；全部原分支、票据、断言保留。

## 实测

冻结来源e49b29d55ca76a562d1e497701333393746d3f9031651b814c65590fdf5db0b5，198files；固定exe与锁身份见[source.json](source.json)，原负载／门不变。全仓release locked offline实际777.109s退出0，共60成功harness记录、55非空、427通过条目（含nested，非427唯一测试）；strict clippy／fmt／build退出0，见[回归摘要](regression-summary.json)。5个离线筛选反例通过。原失败及修复后日志全部保留。

L0／L1仅短诊断，不能签正式稳态；L0已接纳经济案例完整排空，跨短窗口分别记账，不中断或缩小案例。L1原完整布局及冷恢复通过。L2短暖场4.658s超固定3s，正确拒绝开始测量，故障原件保留，没有改3s或重锚来制造通过。

正式L2第一轮原300s暖场＋1800s固定测量实际结束，observer2143.738s，真实120s准入先完成。原完整720小时／723目标／724窗口、3011原票、完整Engine逐字节对独立CPU一致；最后业务1797.968760334s、必要Checkpoint1798.107336125s，180查询／6保存。冷恢复及64MiB池最终完全归还通过。布局保持原数量／容量／1s扫描／脏写／300s快照，两代轮换，最新原件完整冷恢复。

逐轴证据：[业务](formal-l2-01-business-audit.stdout)、[raw窗口](formal-l2-01-window-audit.stdout)、[资源窗口](formal-l2-01-resource-window-audit.stdout)、[独立结果复核](advisor-result-review.md)。raw全集103,029,138B闭合，保守窗口内样本充分的执行层满足原20／40ms观察门；Claims20及两种源页各4不足200，保持未通过采样门，不与其他层拼样本。

## 未签收

首轮不等于三场景各三轮。窗内7200帧RSS观察峰3,514,400,768B，取得footprint峰3,423,277,736B，7346个进程样本缺footprint；缺测不仅有短ps helper。没有用缺测填0、RSS代替footprint或称全部预算通过。normal压力、swap净增0只在本轮成立，宿主本有约3.13GiB swap。

同边界allocator占用／历史增长归属缺测，normal3.10GiB／共同8MiB未闭合，内部泄漏未评估；固定64MiB只是backing。三轮、四速、实际28800s／8h尚未完成，真实16GB为not_run，完整M0不签收。

## 留存与后续

[集中回收v87](../disk-reclamation-stage-followup/STATUS.md)已退役旧v85两代及短测L1压力／预留原件，不能直接完整恢复；当前formal第7／8代、v50旧格式、smoke-L2失败原件完整保留。真实业务、raw延时、日志、票据、固定源码／依赖和exe继续保留。唯一target/release继续复用；阶段不新增独立target。

下一来源实际验证native provider及SDK ABI，补现有Status入口与L1 scan后的真实allocator计数，随后另行冻源做同源各轮、稀有补样、四速与8h，不跨来源拼成通过。本次仅发布上述已测控制器范围；next-telemetry候选不作为已实现资源门。
