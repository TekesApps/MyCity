# 后续Rust联合托管接入点（待实现）

不扩大有限案例，不修改冻结业务或L1计数。独立advisor确认共同8MiB可先闭合，精确64MiB仍需真实所有权改造，不能只加计数器或从总峰相减。

1. 从joint_driver现有真实Child托管演进：normal_pool::Process持池，Rust supervisor持layout和scale-endurance两个Child；各子进程实际退出后取回报告，故障退出明确失败。移除正式路径中的Python业务runner。observer仍按原2MiB单列，supervisor原0.25MiB不扩大。
2. 布局完成初始化后须持续持有全部native/stub与填充，1s扫描/300s快照保持原截止时间。当前固定600s及layout最长28800s不能直接包住“布局先启动+CPU参考+业务28800s+收尾”。新增联合会话需要在至少28800s且业务完成后正常收尾，允许记录额外启动/收尾时段的成本，不把这些时段从业务或峰值中减掉。全部发布扫描必须完成；保持两代快照，原恢复读回门不变。
3. layout ready及business初始化后的clock_started都由同一个supervisor单调时钟记录，证明28800s业务窗完全落在实际L1持有与扫描期间。query/save使用控制器自己的原绝对期限，不随外层握手重置。保存96次及最终检查点分别审计。
4. 联合结束后真实冷读layout最新完整快照，逐项验证记录/压力读回；检查725实际借还、四Sim会话以及三故障进程缺尾部。诊断和heap以PID+instance绑定，不能靠PID单键归并。
5. 先同源短联合验证调度/预算/保护逻辑，再原三轮稳态/四档与真实8h。短联合不替代正式时间窗或吞吐；所有保护失败保留。

精确常规64MiB另一条真实依赖：Sim常驻Engine/索引固定容量；恢复/事务候选/编码缓冲按最大同时存活副本预留并复用；helper参考/双Engine比较阶段与Sim互斥持额度。每个所有者证明实际持有和已触页余量，才允许broker释放填充。普通serde派生会分配，需有界visitor/seed或受控中间容器；未完成部分继续视为额外压力。safe Rust可做，不能用“soft limit还有空间”冒充normal3.10GiB通过。
