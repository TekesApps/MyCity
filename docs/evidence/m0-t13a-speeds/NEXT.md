# 当前下一依赖

canonical /Users/frankqin/dev/MyCity。v0.5.51待advisor终审及提交推送。81528实际终态0，verify/latency/collect/static均0，完整结果见STATUS.md。没有活跃基准或本项目测试。源码65d5e659、binarycffdef留证，不能将下一源码运行拼入本阶段。

下一步真实session pacing修复：当前每Run先睡完整游戏时长换算再计算，导致一天超过60秒。采用volatile next_release，当前实际release=max(now,saved_release)，成功新Run保存release+本次interval；首次立即，晚到不补跑。保持原interval<=60秒权限上限；精确幂等重试先验证同请求/调用者再无等待返回，不改变release。普通失败保留release，明确暂停/速度切换可开新时段，未知提交先查票。重开后不是持久墙钟限速。相关源码session.rs、tests/session.rs、economy_probe.rs、fixture m0-economy-opening.json；P-E0原30日G/F/C/S期末cents=808000/56000/816000/1320000，必须真实Store/session IPC运行并独立核验，不用full L2替代。

完成后再推进full四档真实600/60/10/1、常规内存与L1联合、正式三轮及真正8小时；原冻结负载不改，准入重新120秒normal，保护失败留证。32GiB机器不算16GB通过，目标active/M0 not_accepted。
