# 运行时VM地址归因诊断

v0.5.99候选；仅Rust模型与CLI，同一冻结exe/source `757907…d588`，没有构建或修改游戏代码。原L1正式轮normal物理门失败保持有效，本轮不是性能验收。

## 实测

实际代码的steady smoke为3秒热身+12秒窗口，不是交接摘要曾写的600秒。只运行一次既有模式，原361,272 native及1,232,426 stub全量不改。observer终态0，实际21.596秒；supervisor20.169秒，16个已发布扫描全完成，1份29文件/3,378,784,402B快照完成实际读回；随后新进程完整冷恢复3.788秒，pool平衡退出。未取得末尾快照VM组，不能报告三组完整。

[terminal](terminal.json)、[地址审计](address-audit.json)、`inflight/idle-trigger.json`与十组逐工具JSON/raw stdout/stderr保留实际证据。五角色在首次snapshot_inflight与首次idle各取一次，10次vmmap成功，目标PID/Mach出生身份/parent前后不变，各组调用区间有交集。每份只以调用前后已发布事件标相位；端点同phase不等于瞬时或连续相位证明，更不能将不同进程最大值相加。vmmap扰动调度和压缩，不能修正或替代原正式样本。

arena实际范围 `[24729616384,27991040480)`，长度3,261,424,096B，全部地址交集落在MALLOC_LARGE；grant仍3,261,490,790B，bootstrap66,534B、adapter160B。地址交集证明映射归属，不证明每一物理字节归属。已收费Native/stub/IO/metadata/decode/COW仍由同一arena覆盖，不能再扣一次。

VM还显示malloc metadata、主/辅助线程栈、内核页表、可写dylib页和共享映射。页表原文明确写charged to process physical footprint；页数字段可能取整。Memory Tag 22用途未确认。observer Python VM、root/driver原生运行时、pool外部运行时需要各自单列；pool真实64MiB能力已经收费，不能二次抵扣。栈的虚拟保留量不能冒充物理驻留量。

十份工具的目标身份有夹取；vmmap辅助进程由本协调器启动，在observer模型树之外，原命令/时间/raw输出留存，**本轮未记录每个vmmap自身PID/出生身份/资源用量**。因此不签完整工具资源账，更不把工具成本算成模型常驻成本；后续需要工具资源量时补直接Popen身份夹取。本轮仅签目标映射可读和arena地址覆盖。

## 下一实现边界

当前批准置换字节为0。下一阶段依据R03 runtime域设计运行时组件账：唯一owner/用途、同用途来源、已收费排除、实际生命周期和缩小时占位回补。不能按本次/上次差额猜扣19MB，不能减少记录、频率、64MiB pool或128MiB工作能力；先实现可核验的运行时成本替代，再冻结并重跑原formal。共同8MiB诊断、三轮/四速/8h、真实16GB及完整M0均未通过。

## 留存与回收

本轮唯一完整快照只用于本轮实际读回/冷恢复，没有新增后续恢复依赖；原typed正式current7/8、v50、故障原件均不动。终态依赖审查后，仅精确退役本轮`pressure.bin`和`work-reserve.bin`，组件、原index、原始观测/VM/业务日志、固定源/exe仍保留；退役后该诊断布局不能直接完整恢复。分配空间及独立df变化见retirement结果，不重复计入上一轮回收。

本轮删除2文件，分配空间2.961159GiB；df独立增加2.953541GiB，不全归因。work剩24.992790GiB，target1.338104GiB未改；两代全部64原文件元数据/SHA复核。
