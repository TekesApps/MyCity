# 独立功能及下一依赖审查（未作最终发布确认）

advisor `/root/boot_advisor` 实际读取r07.rs、r07_runtime、构造/扫描/快照/恢复、observer及三份真实实验结果：功能结果支持实际置换/TTL撤销/跨进程恢复，原预算与完整原件仍保留。按sequence关联不是原子同时快照证明；metadata实测符合4MiB，不是全部异常路径静态证明。

完整原始物理峰3,337,293,352B保留。有信用完整帧仍超4,122,538B，TTL补回后完整帧峰超8,660,930B，不能签normal/正式性能。下一步VM归因须同birth/epoch/映射量夹取idle与inflight，记录工具扰动；实际地址区分arena/独立映射/pool，具名剩余区域逐项确认生命周期，无法归属保持unknown。footprint-minus-grant只作总量核对，不授权layout/pool残差信用。

后续发现有限诊断helper把`metrics.report()`的Result序列化到JSON，错误可能exit0。advisor认定有限功能发布阻断，要求传播错误、无成功报告及CLI非零退出反例、同步正常JSON核验器，并重新冻结同源验证。当前actual Ok结果不受影响；旧来源回归自然结束留证，不将其当修复后的回归。

日志算术更正：4085条×9B=36,765B，19扫描平均1935B/次。当前L1 host+compute的8h投影不足以证明超256MiB，也不能证明完整L2/8h日志满足上限；不得把未测投影当已发生的超限。
