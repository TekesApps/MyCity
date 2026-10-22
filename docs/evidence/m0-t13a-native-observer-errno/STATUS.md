# 原生整组观测器接入与errno修复（v0.5.102）

同冻结来源完整回归和功能实验已完成，advisor认可本有限阶段发布。仅完成观测/异常收尾接入，不签性能或整个M0。当前normal依然超限9,758,898B，置换信用0；正式稳态三轮、四速度、8h和真实16GB均未完成。

## 来源及验证

最终来源`8f0c90ba81d0fe56ce8eadc25f18caacae127ce596a86eab790ee0ce4860f64f`，215份Rust/fixtures/配置/lock完整归档，固定exe SHA `b9b327e8a50ea7c55ece1479d6ca6039d882c369da42516a2178adccab30c203`。见[source](source.json)、[freeze](freeze.json)、[源码包](frozen-source.zip)。11个新增registry包均校验lock checksum，固定原始crate档保留在work/fixed-dependencies/native-observer-errno，所有既有依赖checksum未变，见[依赖记录](dependency-audit.json)。

- workspace release/locked/offline实测726.639秒exit0；严格全workspace/all-target clippy8.891秒、fmt0.488秒、build0.127秒均exit0。60份成功harness摘要共451条passed含nested，**不是唯一测试数量**，0failed。见[终态](regression-terminal.json)、[汇总](regression-summary.json)及完整stdout/stderr。源包与固定exe前后不变。
- [生命周期功能实验](preflight-result.json)：真实正常/失败/时钟拒绝、errno空/有子进程、短命/孤儿、owner初始化前后丢失、独占锁继续采样。短命子进程本次被实际采到，原脚本“必须漏采”的错误断言被修正，复用已结束的原报告，没有重跑已完成样本；[更正](preflight-harness-correction.json)保留。
- [发布锁反例](publisher-lock-result.json)：外部持锁1.405秒，root发布在锁释放前超时并退出2，5帧busy仍采样且不完整，cleanup无错误。它是功能实验，不作物理性能比较。
- [周期实测](periodic-result.json)：实际61.898秒248帧，第0/240帧带CPU/I/O与命名VM计数，真实越过60秒。raw计数保留，不冒称跨主机纳秒转换或8小时。
- [失主故障](owner-fault-results.json)：实际L1大初始化、L1 partial pressure写入、L2 Reference helper已输出65,528B targets且CPU计数继续增长时杀observer；记录出生身份全部退出、无关组存活、无正常footer。**不等于空气worker计算阶段或崩溃恢复验证**。另保留仅注册后即中止的L2样本及最初误选无计算L1的[受控中止更正](worker-trigger-correction.json)，不将它们当活跃计算证明。

机制见[原生观测器说明](../../m0-native-observer.md)。后代不得逃离root专属进程组；内核枚举8192只是返回后准入界，外层2MiB arena才是硬分配边界。safe清入口errno仅消除旧值污染，库内两次调用的歧义仍按Unknown。独立[advisor终态意见](advisor-final.md)和条件审查保留，未把主线程运行冒充advisor实测。

## 原规模L1短测结果

[结果](smoke-result.json)与[逐帧审计](raw-sample-audit.json)：实际21.837秒（wrapper自身21.807秒），88帧85完整/3未知，2帧登记busy，2项瞬时身份问题不抹除；SDK外部诊断共87次对照、6个独立PID/birth匹配。外部Python oracle未计入native组，实验被诊断扰动，不能签正式物理门。

peak RSS 3,347,136,512B在第7帧、footprint 3,338,358,552B在第14帧，均为5成员完整帧；footprint比原normal3,328,599,654B高9,758,898B（约9.31MiB），继续失败观察。最大采样间隔0.257秒、采集窗口0.011秒；pressure保持normal、swap净增0只属于本次短诊断。

2MiB observer arena、请求峰550,753B和combined 5,505,024B是分配/外部诊断投影，不是全组物理或完整L2共同8MiB门通过。L1是布局扫描，真实业务吞吐不由本短测证明。原三十日业务与压力替身证据继续分别报告。

两次L1短测均实际完整冷开后继续扫描，原report留在各自work目录；首次[旧来源](../m0-t13a-native-observer/STATUS.md)的87帧均不完整、227枚举错误及超限原样保留，不被新结果覆盖。

## 阶段回收及恢复可用性

所有测试/探针终态后，进程和lsof复查无活体MyCity/Cargo/rustc或work/target占用，按[精确清单](retirement/deleted.json)退役12文件：9份本轮合成pressure/reserve、3份缺当前强制errno的过期静态库。旧158份恢复/故障原件清理前后全SHA核验，其他128,391份work/target元数据和源码/WIP字节未变，无独立target。

|指标|删除前|删除后|
|---|---:|---:|
|work|35,970,712KiB|23,538,232KiB（22.448GiB）|
|共享target|2,094,100KiB|2,069,296KiB（1.973GiB）|
|文件系统可用空间|87,927,296,000B|100,685,635,584B|

删除inode分配空间 **12,756,258,816B（11.880GiB）**，其中payload12,730,859,520B、静态缓存25,399,296B，无新增压力归档。df增加12,758,339,584B只作独立观察；差额2,080,768B不计成果，APFS共享块/快照及其他任务会改变实际可用空间。历史v101释放量不重复计入本次。

[availability](retirement/availability.json)覆盖本轮原路径保留承诺：这些新短测及owner-loss目录**不能直接完整恢复**，pending不是有效恢复点。原index/typed组件、业务、partial CPU targets、原始采样/故障日志、固定源/lock/exe、依赖档完整保留；仍需恢复的typed正式7/8、v50和report-lifetime首代/pending2原件不动。回收许可及原因见[advisor](advisor-review-pending-regression.md)，实际结果见[result](retirement/result.json)，不以hash代替受保护原件。

## 下一依赖

实现[R07独立可回收背板及同用途实际组件替换](r07-next-contract.md)：只拆纯padding、保持原数量/域预算及128MiB工作能力/64MiB池，先验证全额装配→实际替换/回收→失主/独立冷开补回。当前尚未实现，不先授权layout/pool未归属残差。原normal失败保留；没有完整门通过前不启动或宣称正式8h通过，真实16GB保持not_run，full_M0 not_accepted。
