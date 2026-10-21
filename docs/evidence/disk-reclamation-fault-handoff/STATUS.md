# v0.5.94：旧故障合成载荷交接与回收

先用固定82 reader实际完整冷恢复最新首代，约3.650秒exit0，并继续全量扫描；[报告](cold-report.json)保留。advisor[条件式依赖交接](advisor-review.md)随后以逐字节全文件/前缀等价、保护原件核验兑现，删除精确9文件。没有按目录名删除，没有用hash替代仍需恢复的原件。

|指标|删除前|删除后|
|---|---:|---:|
|work du|39,971,992 KiB|25,606,584 KiB|
|共享target du|1,400,732 KiB|1,400,732 KiB|
|文件系统可用空间|88,162,131,968 B|102,872,944,640 B|

删除9个单链接inode，文件分配空间14,710,177,792B（约13.70GiB）；work剩约24.42GiB。df观测增加14,710,812,672B，比删除分配块多634,880B，不将差额或df全量归因本次回收。st_blocks不是APFS独占物理块，快照/共享块/其他任务可能影响df；不重复计算历史删除。无新压力归档、无target删除。

## 精确范围与新可用性

[逐文件清单](deleted.json)保留轮次、原路径、长度/mtime/inode/分配块/链接数、SHA、逐字节等价目标和理由；[availability](availability.json)覆盖历史PLAN/STATUS的原路径保留承诺。

- layout-region-l1-smoke-01（a702）pending：仅partial pressure.bin，实际与最新首代确定性输入的同长度前缀逐字节一致。不是完整恢复点；原stderr、源/exe、阶段与文件长度证据保留，当前不再保留完整旧合成故障载荷。
- causal-clock-formal-l1-01（v91）：首代及第二代pending的pressure.bin/work-reserve.bin，共4文件，与最新82对应代完整文件逐字节一致。旧首代现在不能直接完整恢复；旧pending不再保留完整载荷。旧v91仍仅证明当时SIGABRT，不能用新phase倒推旧轮的精确分配点。
- snapshot-backing-formal-l1-01（c0）：同上4文件，逐字节等价后退役。旧首代不能直接完整恢复；原成功冷恢复和第63索引分配失败轨迹保留。

没有修改原index/manifest/日志。旧源、依赖、exe、typed组件、故障及原始性能样本、真实world/WAL/收据保留；被删压力不能凭hash重开。

## 完整原件、WIP及后续

最新82首代完整恢复输入与整个第二代pending完整保留；formal第7/8代、v50旧格式及smoke-L2故障输入完整保留，均在删除前后逐文件SHA/原索引核验。其余119,103份work文件的身份/长度/mtime/链接数不变，本轮无分配块差异；全部源码/WIP前后hash相同。前后占用为空，无模型进程；唯一target继续复用，未清理其他任务或全局缓存。

下一步常驻typed工作区仍是未提交WIP；原128MiB必须严格分账为64MiB解码+64MiB并发COW，不能为解决碎片减少负载或并发容量。新压力目录创建前明确用途/预计新增/依赖/解除条件；阶段终态必须先完成回收决策再开新轮。当前保护原件只在真实替代恢复与独立交接之后退役。

本次签回收及原件留存，不签性能/正式窗口/8h/16GB。报告延后82原参数实验仍实际失败，见[实验状态](../m0-t13a-report-lifetime/STATUS.md)。

原始reclaim.stdout与focused-unit.stdout末尾空行保留，git diff --check各报一项EOF空行；结构化源码/文档检查通过，不修改原始输出字节。
