# T13-A：固定词法容量与完整业务兼容

v0.5.83修复legacy解析转义key／长错型字符串的隐性动态scratch，并让v1、genesis和v2收据正文共用同一固定扫描器。范围仍是Rust模型和CLI。没有新增L1压力副本、独立target或缩小原full输入。

源码 **25326e509d9a64b818ab2b9b220d8fc21235cc327e4d03d50bf9513fbc484ba7／188文件**；[冻结](lexical-freeze.json)、[完整源码](lexical-source.zip)、[终态逐字节一致性](source-terminal-integrity.json)与[固定CLI](lexical-source.json)对应。Cargo.lock、原负载／阈值、full opening及257×257／16位PNG原件SHA不变。当前设备32GiB；真实16GB保持not_run。

## 实现边界

- Metadata增加真实预建、初始化并计入owned的**1MiB bit栈**；16MiB合法JSON最深空数组每层至少两字节，1bit／层足够。压入前检查，不递归或扩容；畸形超栈输入拒绝后可复用。
- 保留借用原始span和祖先深度；已知标识符上限64解码字节、封闭schema的已知值字符串128字节。溢出的未知key使用独立状态，仍验证后续字符，不能截断后别名为已知字段。
- Ignore和Content语义分离：普通未知字段按旧IgnoredAny验证和丢弃；Content维持原Unicode、数字范围及上下文递归限制。数字转换仍交给锁定serde_json处理原token。没有新增32KiB raw receipt限制，也没有全局缩小原有效档范围。
- 收据／Action／六种Command、v1及genesis固定字段表解析；v2正文校验完整长度／hash后，逐条在固定Meta文件背板读取，无大型Content树。v2回调只填临时查询／分页结果，整段语义成功才对外发布。
- Meta heap为**24,903,808B**，含header owned为24,904,104B。有限String、v2小索引Vec、根控制和其他暂态仍须整Role核算；固定词法器不等于整个冷打开零分配。

## 实际验证，分三类报告

**真实业务与兼容**：[精确源码验证器](lexical_validate.py)执行全仓release离线测试、clippy/fmt/build、旧完整world正常配置冷开、原full30日真实Sim／worker，以及既存完整v1的新旧reader对照，全部终态0。

- [全仓](metadata-workspace-final.json)575.739秒，54组／415条通过harness记录；包含嵌套子进程，不冒充415个独立业务样本。[clippy](metadata-clippy-final.json)、[fmt](metadata-fmt-final.json)、[build](metadata-build-final.json)通过。开发期原clippy失败日志保留，修复后冻结才作本轮验收。
- [旧完整world](metadata-compat-proof.json)序列3013、2,507,418B完整Engine与独立原参考逐字节一致；原件及完整副本各243文件SHA不变，实际core／spatial／Meta背板按预建身份移动或保持。
- [原full30日](metadata-full-business-report.json)整体334.640秒，Sim业务含IPC／持久化295.674秒；720小时目标、723驱动、724实际worker，无失败尝试，工资／债项／周期义务及八单实际交付与完整独立业务／空气结果一致。冷打开0.162744秒，完整映像不变，序列3011，原票重试无副作用。未装配L1及normal pool，因此不是正式性能或原联合负载通过。
- 复用原v81完整小型v1输入，新旧reader均得到1882B相同Engine，未再复制一个常规v1世界。

**独立解析压力替身**：[生成／执行脚本](parser_heap_probe.py)与[22份进程结果汇总](parser-heap-results.json)，11组old/fixed接受性一致。包含64KiB与近16MiB转义key、错型bool、Content字符串、深Ignore、16MiB畸形超栈和晚出现的非法surrogate。固定背板地址／容量稳定；原CP上下文128层附近、重复转义key和完整16MiB嵌套另有同源单元对照。

| 近16MiB输入 | 旧解析requested-heap lifetime peak | 固定解析requested-heap lifetime peak |
|---|---:|---:|
| 长转义key | 23,068,938B | 17,826,060B |
| 错型bool字符串 | 68,157,739B | 17,826,114B |
| Content字符串语法 | 33,554,703B | 17,826,066B |
| 合法深Ignore | 29,360,397B | 17,826,063B |
| 畸形超栈 | 41,943,317B | 17,826,133B |

输入在独立生成进程构造；测量子进程包含fs::read输入、固定栈及启动分配，无peak reset。Content比较使用锁定serde_json::Value的标量／语法oracle，不冒充完整私有Content布局。这些数字是**解析压力进程**，不是业务内存改善比例、同步物理内存或normal grant证据。

**实际Store合成边界**：[构造器](store_boundary_probe.py)从原完整v1原件生成16,777,215B CP，在第一条收据加入可忽略的长转义key，重算CP→index→root→CURRENT身份，移除备用root防止错误时WAL fallback制造假通过。[新旧实际reader](store-boundary-proof.json)均证明序列2及完整1882B Engine相等。新helper至构造／Store::open返回的requested-heap生命周期峰值50,893,320B；不含后续export／比较／输出，不称整个进程最终峰值。Meta背板及1MiB栈身份保持。合成存档是边界诊断，不计真实业务规模。

[730条真实业务Rust heap](metadata-full-rust-heaps.jsonl)及[汇总](verification-summary.json)：Sim最大59,168,565B、worker5,980,225B、model helper16,502,731B、driver614,532B，均为各进程生命周期峰值加64KiB记录尾预留。不能相加冒充同刻物理总量；不含栈、分配器／native／mapped／foreign，不是精确退出峰值。

## 独立审查与剩余验收

[advisor最终复核](advisor-final.md)检查归档、原stdout、源码调用边界及合成原文件，签收**本解析有限阶段**，未发现发布阻断。[静态审查](advisor-static.md)另存。

[历史保守容量表](model-capacity-plan.json)加入实际新栈及header后总78,983,480B >64MiB，target_slots_fit与runtime_grant_accepted均false。这是旧3root／额外air与codec重叠的保守拓扑，不是实际双root Role最小需求；该表保留的“解析暂态未验收”说明也不能替代本轮具体词法器证据。下一步按真实同时存活关系收费并消除重复槽，不能放大冻结授权或用单进程峰值相减填预算。

**尚未通过**：完整Sim／Reference／Compare Role的所有控制暂态及分配前broker／Filled／wait授权、normal64／共同8MiB、当前源码原L1+L2联合、分级／四档／三轮稳态、真实28,800秒8h、同步全进程物理预算；真实16GB为not_run，完整M0不验收。后续见[NEXT](NEXT.md)。

## 终态回收

[预先用途](stage-retention-intent.json)与[精确删除清单](retention-deletion-manifest.json)：在验证及advisor终态、work／target无占用、原完整布局逐文件核验后，删除本阶段11份生成JSON及合成world内9份文件，共20文件，inode去重分配空间**101,556,224B（96.852MiB）**。原生成器、清单、stdout、故障结果、源码／依赖和固定exe保留；临时输入路径已退役，不能直接恢复，原v81完整v1原件仍在，不用hash替代原件。

[前后测量](retention-result.json)：work 15,953,892→15,854,716KiB，共享target 1,296,244KiB不变；df可用空间观察增加98,988,032B，独立报告，不当作APFS独占回收量。新增完整30日world为下一Role恢复／兼容输入，固定exe四文件作为验收证据保留；无新增target或L1复制。下阶段创建目录前重查替代关系，阶段终态回收可再生临时数据，不默认永久堆积。
