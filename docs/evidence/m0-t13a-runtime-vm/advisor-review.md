# advisor独立审查

独立只读检查terminal、地址审计、逐角色phase区间及实际layout-restored/report：本轮可收口为VM地址归因诊断，不能批准置换字节。唯一快照实际完整冷恢复3,378,784,402B。本轮arena准确为3,261,424,096B，前轮activation差50B不能沿用。

下一实现应为逐组件置换账，先只接能证明所有权的组件：用途、runtime份额、PID/出生身份、分配/映射身份、有效生命周期、arena/metadata/诊断账已收费排除。arena、64MiB pool、128MiB工作区不能再次抵扣；新增可控制的独立运行时背板按真实容量替代同用途stand-in，剩余额度继续触页，退出/释放前完成余额交接。

malloc metadata、栈、页表分别建立归属和计量方法，虚拟长度不兑换物理额度；共享/压缩/MemoryTag22继续unknown。异步有扰动的十份vmmap不能证明整个生命周期可稳定置换多少字节。replacement_bytes_authorized=0准确，不按19MB差额写常数，不能把footprint−arena包装成组件。下一原完整测量仍可能失败。

同意退役本轮snapshot-000001的pressure.bin和work-reserve.bin：实际冷恢复通过，VM后续依赖地址/raw/阶段记录，不依赖压力文件。精确占用/inode/保护项核验后删除，保留组件/index/全部VM原文和报告并标不可直接完整恢复。原current7/8、v50与既定故障原件不动。advisor未执行删除或负载。

方案审查另要求：phase来自实际事件，跨phase标mixed；每个工具夹取目标身份，异步观察不累加最大值；工具成本单列，映射归属与物理收费分开。本轮工具自身PID/资源缺项已在STATUS明确，不签完整工具资源账。
