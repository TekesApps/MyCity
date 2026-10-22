# 独立依赖审查

审查者：本任务 advisor `/root/boot_advisor`，2026-10-22。只读检查当前 crates、阶段 Python、PLAN/NEXT 和留存引用；未运行恢复或删除。

- smoke-L2 首代 pressure/reserve：暖场4.658秒超过3秒，测量尚未开始，非布局内容或恢复故障。后续正式控制器已实际运行，未见后续用例必须读取这两份原始载荷。允许精确退役；保留该轮业务、暖场时轴、pool、index、组件和失败日志，撤销完整原件保留承诺。
- 指定10份正常组件：旧formal-L2第7/8代由typed新7/8代接替；runtime-vm后续归因读取vmmap、身份、事件和地址报告；causal-clock和snapshot-backing故障在第二代，相关pending和分配/日志保留。未发现必须读取这10份native/meter原路径的独立诊断阻断。仅在与既有gzip完整解压逐字节相等、原index长度/SHA一致、单链接且无占用时退役。完整字节归档和路径映射保留，不能把还原组件描述为整份快照恢复。
- typed第7/8代、v50首代、report-lifetime首代及第二代pending完整原件继续保护。后者仍保留旧动态借还失败对照，不自动因typed通过解除。
- 所有其他pending、negative fixture、业务、日志、固定源/lock/exe保留。本意见不延伸到其他组件或构建缓存。

构建缓存另由主任务逐个核对依赖图：仅旧headless rlib/rmeta缺少当前强制sysctl/rustix，保留当前图及全部可执行文件；不将此项冒充advisor检查结论。
