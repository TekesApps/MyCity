# 中间失败与边界

- check-initial.txt：引入显式Work参数后，Frame的capture调用漏传；编译失败，随后补齐。不是运行通过证据。
- clippy-first.txt：容量清单中的Lease名称被本模块泛型Lease遮蔽；改为super::Lease。clippy-second.txt：Resident存在标志布尔式触发nonminimal_bool；等价简化。最终以冻结来源的终态全仓检查为准。
- lifecycle.txt：第一次新增测试把field/scratch各自地址当作不变，原数值核会交换两张等容量固定数组，导致有序地址断言失败。数值算子未修改；改为完整地址集合一致，同时继续比较checkpoint完整原始字节。lifecycle-fixed.txt定向退出0；不将之前失败改写为成功。
- core-first.txt是早期主路径改造的全core结果，尚未包含最终容量与租约重入断言，不能拼接为最终全仓来源。

本阶段仍有小额identity字符串、兼容拥有式入口和一般Raw解析分配。固定主准备背板不等于全图零分配、实际池授权或normal64通过；性能以真实运行分开报告。

- 新静态归档核验初次错误假定snapshot目录每份辅助文件都列在正式index中，遇到native-restore-bound.json时StopIteration。该文件是额外恢复校验证据；修正为保留并实际核验其完整hash，其余文件继续与原index逐项核对，没有忽略缺失条目。模型实测和原五审计不受影响。
