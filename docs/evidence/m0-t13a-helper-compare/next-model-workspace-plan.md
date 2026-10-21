# 剩余normal64的收敛路径（advisor只读建议，尚未实现或验收）

不能把当前Engine4MiB、Timeline2MiB和JSON4MiB检查当成分配前界；Engine已包含Timeline，不能重复相加，也不能从整堆生命周期峰相减。helper固定比较完成不等于其余EXTRA已消失。

先做容量可满足性测算：覆盖World/Arena及索引、Timeline、journal、Background/Air、Prepared dirty集合、Store元数据/空间/源收件箱、IO/解析和同时存活模型槽。按真实Rust类型/最大记录数/固定容量算全部背板，加当前worker5MiB及SimCodec4MiB，必须在原64MiB内；不能先发估算大grant再用未触页空洞补账。若超限，释放genesis、流式WAL或复用不重叠槽，不改原负载与上限。

1. 一次覆盖Engine+Prepared类型图的固定ModelWorkspace。数组容量固定、全部初始化，活动长度独立；Prepared复制活动记录到专属槽，禁止普通深Clone；成功交换/复制活动区，失败清空但保留背板，持久化先于发布。Air输入/候选/验证参考按真实重叠关系设槽。
2. Store读取/写入/校验经过同一工作区。restore_checkpoint普通from_slice改直接seed；open保留genesis验证但提前释放或复用恢复槽；checkpoint_v2流式写及受控第二模型槽保留原深校验；WAL Value/from_value(record.clone)/hash拼接换具名记录和固定缓冲；state_hash/business_comparison的DOM用保持原canonical结果的借用适配器替代。仅有文件长度上限不算解析内存受控。
3. 各角色复用同一模型实现并接池。Reference/FaultReference使用同一Engine/Prepared背板；普通Compare保留允许metadata差异的语义，用两个明确槽或等价流式规范比较，不能直接套严格字节相等。Sim常驻模型/短期事务/恢复槽具名。broker分配前授权，空闲真实填充，实际Child wait后归还；诊断归8MiB、模型归normal64，不重复收费。

最后统一验收：分配前超限/事务失败回滚/进程故障/旧存档及canonical字节；原full30日所有目标/原票/业务/空气；原L1+L2及指定故障，每角色grant/实际背板/填充/重叠槽/wait实测；审计全部普通路径无未登记分配，再分别审normal64、共同8MiB和RSS/footprint。allocator元数据、栈与原生库仍单列。正式四档/稳态/真正8h/16GB门不因此自动通过。

该建议来自/root/boot_advisor本轮对当前源码的只读定位，没有构建或测量；下一实现必须先补真实容量计算，不能将本文件当成实现证据。
