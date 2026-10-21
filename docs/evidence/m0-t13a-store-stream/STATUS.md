# Store流式检查点与回执阶段（v0.5.73）

只做Rust模型／CLI。原acceptance SHA `876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508`及原所有负载、CPU数值步、时间档和性能门限不变。本阶段消除v2 world／receipt完整编码Vec及receipt body反序列化Vec，不是整图ModelWorkspace或normal64签收。

## 实现与边界

- Engine新增reader恢复，4MiB实际输入限制及384B词法／128层保护位于serde读取路径；严格end后复用原全部模型不变量检查。原slice入口及固定L1类型／布局不变。
- Store固定8KiB分块读写。计量遍提供准确长度／SHA供容量准入，写入遍重新计量并对照；同一不可变Engine／借用journal来源。tmp fsync、实际完整恢复验证后才rename和目录同步。CURRENT仍是唯一提交根，原WorldSynced等observer仍对应命名文件持久事实。
- reader在返回临时模型前校验完整EOF、预读缓冲消费、实际长度和完整SHA，合法前缀、截短、坏尾部、读取错误、文件增长均不能通过。历史world仍验证原字节且不自动回滚。
- receipt借用journal序列输出，原字段顺序及线格式保持；读取以seed逐条核对index和全部字段，额外记录在物化前拒绝。index可信上限1024且不信size_hint；分页以固定64条暂存，整body校验后才发布倒序页。
- checkpoint顶层对v1／v2持续poison，覆盖CURRENT发布、内存root／journal更新、GC和Retired observer。unwind后必须重开；仅已确认首写前的Capacity拒绝解除，不按错误枚举盲目恢复可用。

WAL仍有Value与编码拷贝，legacy v1 checkpoint及business comparison仍有整段编码／DOM；查询Value、部分字符串和元数据也未全部池化。没有将本轮I/O缓冲称作整图池已授权。

## 同源实测

| 证据 | 结果 |
|---|---|
| [固定源与exe](source.json) | source326a47fa83ee9b717025edce88cf817c8da47e67f49c0cbd1a9f5baa44d46edc，165文件；binary d7935994fd7378a13c4714a9dee40c5d39581f9e57f838accaa0b2a5b0853a37 |
| [最终检查](checks-final.json) | workspace-release终态0，593.984450083秒；372 harness pass sum含nested，不是去重测试数；clippy／fmt／build均0 |
| [完整旧存档恢复](compat-proof.json) | 当前实际rlibs／不改旧rules；原world及完整兼容副本各243文件前后hash一致；3013seq、2,507,418B完整Engine逐字节一致 |
| [新流式实际往返](roundtrip-proof.json) | 旧完整world的独立临时副本连续checkpoint两次并实际重开；全部Engine字节和3013条原始receipt页序列／内容指纹相符 |
| [源／容量核对](integrity-capacity.json) | zip逐源相符、固定binary及v70／v71既有exe SHA未变；runtime_grant_accepted=false |

最终回归含两遍不一致、验证失败不发布及tmp回收、未消费尾部／文件增长、损坏但自洽hash的receipt字段／数量／尾部拒绝、索引超额前拒绝；Engine reader碎片／长token／超4MiB／读错误拒绝。v1／v2新增CURRENT持久后panic恢复，原实际进程退出、跨热窗口成功与失败回执、跨归档分页以及损坏拒绝全部保留并通过。[开发期失败](failure-notes.md)单列，不冒充最终同源结果。

完整Engine SHA仍为 `f4c922bfc7a6a214e73d1c69c26386b3faa2465794dc9f77f99793a554246e02`；receipt倒序页指纹 `626a5c302f164806040a615e2fab8f0470bb262f6402bec07cd05b19d12e1b56`。旧world两次保存不代表重写所有历史receipt body，新body实际写入由跨热窗口回归覆盖。往返日志中的0.943s保存／0.428s重开只是本次正确性诊断，非完整联合负载或冷缓存正式性能通过。

## 预算及未签收门

[当前CLI容量输出](capacity-check.stdout)：基础目标44,147,720B，含固定receipt页165,888B及输入／输出控制与缓冲16,872B。Raw Background串行归一化仍补两个最大typed State共1,325,248B：合计45,472,968B＜原67,108,864B，余21,635,896B；没有扣除原五个Air槽，没有把串行暂态错误乘上三个Engine。此为目标算术，实际授权、整图所有权和物理内存仍待闭合。

当前主机Mac16,1／32GiB／macOS26.6.2／Rust1.85.1。没有真实16GB设备，16GB门not_run。未新增正式分级、四档、三场景三轮稳态、完整联合或实际28800秒8h；不能用旧源v70的600秒诊断替代新源性能，M0 not_accepted，goal active。[advisor](advisor-review.md)只签本有限实现／恢复阶段。

## 阶段回收

复用共享target，无新增独立target或压力副本。临时roundtrip fork成功终态、无占用、归档后已逐文件退役；[精确清单](roundtrip-retirement-plan.json)及[结果](roundtrip-retirement.json)记录243文件、10,199,040B（9.727MiB）分配空间，85,128个非选中work文件元数据未变。**该临时fork现在不可直接重开**，proof／hash不是存档；完整原world和兼容副本仍保留。

删除窗口work 18,028,588→18,018,628KiB；target 780,764KiB未变。df可用空间反而减少306,876,416B，与其他任务／APFS等共同变化，不能把它算成本轮清理效果或将此前其他任务释放的空间算入本阶段。首次清理因检测到其他项目Cargo而保守停止，未删除；核实是其他项目后改为检查本项目实际占用，不终止其他任务。

完整v70两代布局、v50旧格式快照、v66业务world、t13a68兼容副本、源码／固定依赖／exe／故障／收据证据均保留。旧v64／失败v65／v66 layout早已退役，仍不可完整恢复。当前target与库供后续构建／验证复用，没有已确认无用的独立缓存，待下一阶段结束复查。执行[统一回收规则](../../validation-artifact-retention.md)。

下一依赖见[NEXT](NEXT.md)：WAL及canonical、legacy／comparison编码，再接实际整图64MiB／common8MiB授权，整图闭合后进入原正式负载与8h。
