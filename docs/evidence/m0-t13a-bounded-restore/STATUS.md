# 固定L1恢复分配前约束与旧组件兼容

v0.5.56交付固定诊断读取路径的预算约束及旧组件实测；不签完整城市内存或完整L1联合。原acceptance fixture、原JPROD数量与写入codec不变，仅Rust模型/CLI。

## 实现

[测前方案](PLAN.md)与[原输入冻结](freeze.json)区分固定记录恢复和完整压力负载。Native读取前先算完整目标graph，Vec与诊断Arena按冻结数量一次预分配，核对实际capacity；保留scope/generation/所有账户库存索引与owner引用、全live/空free。专用Arena入口不替换通用World/Arena读取。表按本地variant/count解码，records在tag前也不走Content整表缓冲，缺失/重复/错误tag、字段/数量和尾部数据拒绝。第N+1条在构造前拒绝。

JsonReader在serde前限制标识符/数字token、128MiB总输入与128层嵌套。实际反例证实忽略未知字段时serde_json可越过默认递归限制增长scratch，故不能只限制记录数。工资周期Cycle改用固定字段读取，写入字节不变，消除其内部tag的Content缓冲。Records<T>只对本阶段具名固定大小类型证明，不作为任意Copy+Deserialize的通用分配保证。

原native恢复先倍增解码再重新打包，保守临时上界56362928B；现在固定目标29625968B+6MiB parser/IO/stack=35917424B，无整缓冲替换。该预留是分配论证，未等同于RSS实测；旧表/模型常驻与本次贷款分开计。这里的64MiB是L1的128MiB reserve中恢复贷款，不是已闭合的常规工作区64MiB。

## 来源与测试

最终[来源](source.json)fd551d1f79697d727185f19230d7dc4ebd973a05d0e9a2cec2150553366dce17，共148文件；binaryef228c12042b3ec4a1062e57cf4602d7d607451e5da73fec0d016e3ad5c2ee2a。[最终87项](tests-final.json)为46 corelib+2完整native文件+39 headlesslib，failed/ignored均0；全仓all-targets clippy、fmt和Release build均实际退出0。

[前一来源全仓Release测试](workspace-tests.json)实际93594终态0，15166…来源的日志有330个harness报告通过计数（含嵌套进程harness，未当独立测试数量）。之后[逐文件差异](workspace-to-final.json)/[补丁](workspace-to-final.patch)只有诊断读取器深度保护及反例，最终相关目标重测；不能称最终来源已再跑全仓。所有源码快照与日志分源保留。

[失败记录](failed-attempts.json)：candidate clippy51123终态101为测试模块位置；另一个source309725…深度反例81837终态101，256层未知字段被实际接受。修复后同反例在最终相关测试通过。原失败/日志未覆盖，未把前一源码的通过拼成最终通过。

## 真实旧组件实验

原始work/t13a50-joint-01完整snapshot-000001的[index](original-index.json)与v50归档逐字节绑定。work/t13a56-records-admission实际30103 fresh120normal终态0；随后单独运行work/t13a56-records-01，实际50177终态0，来源全程冻结，无本项目测试/build/其他benchmark同行。

`layout-record-restore`读取全部27份原始记录组件共200526364B，native361272、typed stub1232426（不是额外实际市民/业务），全部原文件hash及重编码字节数/hash相同；native保留完整索引/scan，表逐条与原冻结pattern比较。组件实际3.069205541秒，观察器4.31542525秒，无新速度门。[独立核验](verification-records.json)另流式重算所有旧文件hash，不只信报告通过字段。压力/reserve映像没有加载，这不是缩减负载后签完整L1，而是明确独立的组件兼容实测。

本次RSS采样峰119357440B，已取得footprint峰110641872B，18次进程footprint缺测保留；最大采样间隔0.273秒/采集窗口0.062秒。采样normal、swap净增0，但初始swap9147187200B，不称无swap或物理绝对峰。whole Rust heap页脚是整个组件进程，不能将其减成native贷款用量，也不能从此签常规64MiB/共同8MiB。

[原始工件](t13a56-records-01/manifest.json)完整归档；[静态核验](static-verification-records.json)绑定当前文件清单/字节、binary、冻结输入/observer及全部归档。[advisor独立终审](advisor-final-review.md)实际重跑两个核验，同意上述有限阶段发布。

## 剩余门

完整L1联合及物理峰值、常规64MiB、共同8MiB、full L2四档、正式三场景各三轮、真正8h仍未签；真实机器32GiB M4，真实16GB not_run，完整M0 not_accepted。下一依赖是完整联合的生命周期归属、普通工作区和实际阶段观测，重新准入原足额负载。磁盘准入必须重新计算多份快照及2GiB余量；不删除唯一失败证据，不沿用旧可用空间。正式稳态接入约束仍为准备材料，没有实现或新通过。

原命令日志按字节保留；[格式检查](diff-check.json)只有5份raw测试日志的EOF空行例外，排除raw txt后源码和手写文档检查退出0，未修改原证据消除空行。
