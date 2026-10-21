# 独立代码审查

/root/boot_advisor，冻结来源ee8af3e54b963022777f34e8194c8626fd8bfed4eafc722bd617d95cb69f6032，只读代码、未跑测试/编译/负载。未发现新增守恒、生命周期或兼容阻断，可以继续真实回归；未把尚未终态的测试算通过。

- bounded TaskState::Fixed直接恢复AirWorkspace，不能调用返回Vec的legacy_checkpoint；无State/Input/Rows隐藏clone。
- I/O4MiB及typed1MiB均创建检查后才Filled；更换typedinstance重复Filled拒绝。
- worker预留/归还统一5MiB，Sim使用WORKER_START_PAGE以下页面，正常双owner55MiB；无旧4页回收或重叠。
- 部分补页不发布额度，沿实际Child wait回收。成功在Published门后reset记录loaded/reset再Complete；错误退出沿实际wait回收，不伪造reset日志。
- kernel/slots跨请求复用，legacy owned仍保留。

1MiB背板不覆盖typed metadata、控制/解析临时分配和原生栈及其他模型。事件与报告增加后须实测原日志/诊断上限；故障缺末尾报告只能依据wait回收，不推断reset。源码审查不替代实测。
