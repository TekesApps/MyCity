# 原满级R07的native归因诊断

使用原冻结source `32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2`、固定CLI `59aa365ed46b5d373218fcbcf1d34f930d674d15607ed510190361891439b4e0`，没有编译、改fixture、减载或改阈值。原source ZIP与身份文件仍在[原冻结目录](../m0-t13a-extents-joint-retry-02/STATUS.md)。场景是原361272 native records、1232426 layout stubs、64MiB实际正常池、同一277610496B满R07映射及两代保存/冷开。stubs和压力数据不是实际城市业务；不能以此替代30日业务、分级速度或8小时。

Apple M4/32GiB，HP P900外置USB/APFS SSD。**本轮为外部工具有意干扰的归因实验，不是M0性能通过。**原normal完整帧超额8431602B仍是原run的结论，本轮没有给native或VM分类任何物理减项，`memory_credit=0`，`memory_16gb_gate=not_run`、`full_M0=not_accepted`。

## 三轮实际结果

| 轮次 | 实际结果 | 当前恢复状态 |
|---|---|---|
| 01 | controller漏发既有`vm-start-first/second`标记；模型按原120s截止退出2，488帧、零VM capture | 没有snapshot获准，不是恢复基线；失败脚本/日志完整保留 |
| 02 | 补齐原诊断协议，两代保存与live-provider冷开exit0；捕获谓词误要求epoch≥3，漏掉实际epoch2完整归还窗口 | 此后已精确退役两代的4份合成压力/工作预留文件，**现已不能直接完整恢复**；历史当时通过记录、其余组件与日志仍保留 |
| 03 | 改用实际映射/归还/身份条件，4份vmmap均exit0；两代及live-provider冷开完成，observer exit0，28.320s观察时长 | 两代完整文件均保留；退役02之前，03每个列出的组件都重新核对完整SHA/长度 |

两次controller错误由根代理修复，未改模型原120秒截止。controller不自动重启；每轮为独立新目录。协议及捕获谓词的纯测试保存于protocol-tests-02/03，不算模型或性能验收。

03触发条件为：完整组/覆盖帧、source匹配、同一layout PID/birth、R07实际mapped=277610496、replaced=0、permitted=0、provider paused，且此前见过实际替换并已执行第二代准入。epoch是版本号，不固定为阶段3。触发t=14.275826792s；各vmmap于控制器约14.565–15.921s执行，pause于15.944s解除。所有target身份未变，捕获前后均为epoch2、完整映射。[原捕获收据与输出](attempt03/captures/layout-r07-session.json)、[实际协议](attempt03/protocol.json)和[index](index.json)为权威证据，不按进程名猜身份。

## 定位到的类别与限制

vmmap页单位为16384B。layout的named role arena落在MALLOC_LARGE，R07落在VM_ALLOCATE；pool的4096 resident页对应64MiB正常池。layout kernel page table约112.16页（约1.75MiB），另有malloc metadata、stack、库的可写/COW页；pool/driver/observer也有各自内核页表及运行时开销。完整原始摘要分别见[layout](attempt03/captures/layout-r07-session.stdout)、[pool](attempt03/captures/normal-pool-host.stdout)、[driver](attempt03/captures/r07-layout-driver.stdout)、[observer](attempt03/captures/observer.stdout)。summary是有舍入的分类观察，不是逐字节可安全删除清单。

同一trigger原始帧group footprint=3337047616B，四成员footprint之和严格等于该值，超原normal=8447962B。[同帧对账](trigger-accounting.json)不冒充原run frame104或本轮无干扰峰值。01同帧也不存在先前口头分析所称的约1MB差异；该解释已撤回。不同时间窗的数值不混合。

页表/metadata/stack已包含在各进程footprint中，不再加一次；resident和swapped不能按两份物量重复计，共享库映射也不能简单跨PID相加。辅助controller与4个vmmap的独立观测峰142608760B（约136.00MiB），显式缺测另记；没有把它们当免费，也未从原运行组里扣除。其原始时间序列见attempt03/auxiliary-samples.jsonl。这个干扰量本身就禁止将本轮签为正常物理、速度或耐久通过。

## 下一项可审查候选

[advisor聚焦意见](advisor-direction.md)建议先减少真实重复开销：尝试把本次独立driver的控制逻辑并入既有pool-host，保留observer和layout独立，原64MiB backing、授予、扫描、快照与失败回收合同不变。必须实测是否减少group footprint，并验证服务不被控制逻辑阻塞、角色拥有量/common上限不扩大、单一失败责任和子进程回收。当前driver约1.31MB footprint，即使全部消除也不足抵消8.45MB超额；不预告能过门。该候选尚未实现/构建/运行。

不能因为此诊断grants=0就长期借走业务容量：既有SimCodec等并发生命周期必须仍获得原要求。把native记到池里需要真实backing/容量替换、身份/生命周期和峰值授予证明；内核页表不能伪造为用户arena。也不先把压力页改成干净文件映射来取得较低footprint；相同字节与扫描不证明同等压力，文件cache/缺页/重读IO必须完整计费。两者目前都不能签成本门修复。

## 保留与回收

原失败formal01、原190份保护输入、源码/依赖、固定exe、业务存档与既有恢复原件都未删。03两个完整snapshot是本诊断后续恢复基线。02仅四份确认过时且可重建的`pressure.bin`/`work-reserve.bin`按精确清单退役，不按目录批量删除；检查无进程/打开文件、保护身份不重叠、03完整文件SHA后才执行。清单、理由、所属轮次和保留证据位置见[执行收据](retirement/receipt.json)。02“已不能直接恢复”已同步写入其外置RETENTION.md和本index，hash不代替恢复原件。

删除文件分配统计6359040000B（约5.922GiB）；卷可用空间观察增加6359023616B，单独记录，不宣称APFS独占释放。恢复/失败证据仍在外置work；公开归档只含小型日志、报告、控制脚本和清单，不复制GB payload。原M0负载、三档/四速、稀有层、真实8小时及实际16GB未由本轮获得通过；分级速度入口仍是另工作树未测源码。
