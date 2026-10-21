# 接续：v0.5.69固定核心表与准备租约

> 当前恢复可用性更新（v0.5.70）：旧v66 joint-02两份layout压力已退役，对应布局快照不能直接完整恢复；business world及完整兼容副本保留。最新完整布局基线已换为t13a70-joint-01两代，v50旧格式依赖仍完整。详见[本阶段报告](../m0-t13a-background-resident/STATUS.md)；下文旧保留承诺仅代表当时状态。

当前来源cc1ca0fb3e32ff336b20367fe43870677f32ab8bb3d60b8266a532bceb629893（160文件）已同源全仓Release／clippy／fmt终态0，实际46538；完整旧world兼容32333终态0，3013票、2,507,418字节与独立fault-reference完全一致。源码与固定binary复核一致。无新源码变更无需重跑这些检查。

候选原按v68准备，v68用于独立清理发布，模型阶段v69。原始证据不改写。当前仅Rust模型CLI，normal64/common8／正式性能／四档／3x3稳态／8h未完成，真实16GB not_run；goal active，M0不签。

下一实际依赖：将Background的field/pending及准备clone改为常驻固定存储，复用原State数值算子；capture/frozen/CPU reference/Accept/staged validation的重叠槽真实列账，不能把候选1MiB空气槽或v67单Prepared拓扑当作当前已闭合。随后完整World/Arena/Opening seed、Store WAL/receipt/canonical DOM流式化、进程池授权与wait全图闭合，才进行原完整联合、正式分级、四档及真正8h。保持全部冻结记录数、数值步、原规范字节和门限，不缩负载。

磁盘按disk-stage-end.json与回收规则：复用共享target，不新建每轮缓存；成功joint-02两代/v50旧格式和完整业务兼容副本仍是恢复依赖。v64和失败joint-01压力已在v68退役，不能直接完整恢复，历史日志与manifest保留。勿重复计清理成果或批删其他工作。
