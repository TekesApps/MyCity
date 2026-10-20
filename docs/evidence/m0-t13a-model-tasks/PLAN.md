# T13A-3e：把模型证明工作与驱动诊断分开

冻结原acceptance及full规模、720小时／723推进／724空气窗口／3011原票，8192页及60秒空气步长不变。本阶段仅Rust／CLI。目的不是更改业务粒度，而是去掉父驱动中的参考Engine、完整存档字节和双Engine比较，使随后诊断全堆收费可审计。

## 实现边界

三个helper：Reference执行原CPU参考／独立oracle／逐目标hash及完整期末checkpoint；BeforeReopen读取原CURRENT→root→index→image链；Compare在Sim已停后验证原保存image不变、原完整业务比较及完整空气checkpoint。helper是同一进程树中的实际参考／验证负载，不是免预算旁路；所有正常堆footer仍写共同MYCITY_HEAP_LOG，缺失不算零。

helper请求绑定source、tier、opening／policy／oracle hash及规范目录。新任务目录拒绝覆盖。每份工件先有界写.tmp、fsync，再以不覆盖的link发布；result最后发布。父驱动先确认正常退出，再核验请求、PID、精确工件清单、长度与hash，随后发布原顶层CPU路径。引用工件不是提交城市状态；中断后不能靠已有文件继续计算为成功。源码／fixture哈希和旧原票语义均保留。

父函数错误／超时由Job kill+wait；监督路径由父专有stdin写端保持连接，父异常死亡后EOF使纯计算helper退出。helper不再派生其他进程。退出确认与全局额度回收仍是不同责任；不因EOF就直接退64MiB额度。裸scale-model-task是诊断内部入口，未带parent-pipe不宣称父死亡回收。

## 费用与并发

| 阶段 | 同时存在的业务／证明工作 | 诊断归属 |
| --- | --- | --- |
| 参考 | 父驱动＋Reference（含原独立重建／oracle） | 父驱动全堆候选计费；helper的64KiB golden写缓冲、结果／请求／报告编码及watcher控制数据单列 |
| 业务 | 父驱动＋唯一Sim＋必要worker | 驱动全堆；Sim／worker原诊断费用；实际State／输入输出不自动纳入4MiB槽 |
| 冷开前 | 父驱动＋BeforeReopen，Sim已退出 | helper完整image验证为模型工作，root结果／交接为诊断 |
| 冷开验证 | 父驱动＋新Sim，无模型helper | 原冷开、原票重试与再次checkpoint |
| 期末 | 父驱动＋Compare，Sim已退出 | 双Engine与完整比较是证明工作；小结果、请求和报告为诊断 |

Reference报告改为借用字段流式输出，不构造整棵报告Value；固定19字段空气摘要保留旧f32→Value/f64表示，仍属诊断。旧run_tier_observed用独立Value出口，和流式出口共享运算，避免JSON往返造成f64尾数改变。整个进程终身heap峰继续记录，不能相减当作诊断用量，不能把不同时刻的峰说成同刻实测。helper栈／父监控线程栈及native仍在RSS／footprint外部观察范围，不包含在requested Rust heap中。

**本阶段不签8MiB诊断或64MiB真实所有者闭合。** 仍需给helper／驱动明确共同限额并实际验证，参考／比较工作区及未用余额需接全局grant。当前外部64MiB不能因为拆进程就视为覆盖新增阶段。

## 验证

1. 初版全仓Rust回归实际结束，修复版按受影响的旧参考API／流式helper进行目标回归、full CPU逐项对照及全target clippy。不同来源的结果分开登记，不拼为最终来源全仓通过。新增来源错误、部分输出／旧结果、坏工件／长度边界、杀helper及杀父进程反例。
2. 独立full CPU参考功能执行：同原20k债／64工资／16义务／8订单／720小时；723个目标business／air／windows和完整CPU Engine与v0.5.42归档逐项比较，仅来源和耗时元信息不同。该功能运行不冒充完整Sim、联合压力或速度通过。
3. 系统warning时不启动正式满压力／稳态／8h。保留原v43压力失败；只有连续120秒normal才可开启下一正式运行。未实际完成新来源的全Sim／冷开完整轨迹就保持未测。
4. advisor独立审查实现和实际证据后，按有限已验范围提交、递增annotated tag并推送；四档、三轮、8h、真实16GB和整个M0继续不通过。
