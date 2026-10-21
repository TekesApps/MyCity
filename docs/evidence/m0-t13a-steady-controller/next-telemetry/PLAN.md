# 下一来源最小观测补项（候选，未运行）

当前正式e49来源继续原样运行；此目录不进入其Rust源码或observer，不编译／执行候选，不改变正在测量的负载。

- status-allocator.patch：仅在已有Sim Status入口、报告分配前读取allocator snapshot和进程clock，绑定现有游戏目标与相位；不能用固定64MiB或生命周期peak代替内部used/free/current增长。后续源冻结后实测，当前formal内部泄漏门缺测不变。
- layout-allocator.patch：每次原有L1 scan后读同一全局Rust分配器计数，标snapshot_inflight；原scan、脏写、数量和快照频率均不变。System/native堆与角色arena明确区分。
- native_metrics.py／observe_native_candidate.py：候选使用本机公开SDK proc_listpids／proc_pidinfo获得进程树、host_statistics64获得系统内存，取消每250ms ps和每60s vm_stat短helper。不存在的footprint和RSS仍明确缺测，不以RSS替代footprint。原250ms、原pressure／swap／磁盘／热数据门保持不变；固定metadata另收费64KiB。
- 当前只完成源码和SDK声明对照、Python语法解析。未执行native API／未跑测量／未算性能通过。ABI要在当前测量真正终态后，用abi_check.c与本机公开SDK编译验证sizeof／关键offsetof／count，再实际验证自进程与模型子树、host计数和有界热数据，成功后才能接正式测量。该C文件仅作观测ABI检查，不是游戏实现或新业务unsafe例外。

当前模型／测量源不要修改。候选验证和必要定向回归后另行冻结来源，不跨来源拼成三轮通过；保留本轮真实容量、物理样本和业务证据。
