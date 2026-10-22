# 2 MiB 候选联合诊断 01：控制器失败，恢复验证完成

候选冻结源 `ade347ccd333e9f4cdbda7f08066f720bbd7649a6d58762987445750a0db4e05`、固定 CLI 及原输入见 [source.json](source.json)。这是隔离候选，未替换 main 的 1 MiB 实现。原 normal 阈值、负载和共同预算不变；运行于 HP P900 外置 USB/APFS SSD，主机 32 GiB。

实际模型与 observer 退出 0，但控制器仍以 1 MiB 格式上限验证合法的 2 MiB extent，在 initial-layout-observation 断言失败，四次预定 VM 捕获均未完成。原始异常和终态保存在 [terminal.json](terminal.json)，不能把 wrapper 成功等同于本轮验收成功。原始 footprint 峰值 3,337,113,128 B 仅属不完整诊断，不能当完整四捕获对比或正式性能通过。

失败后使用同一固定 CLI，分别新进程完整冷读两代快照，实际 hash/load/scan 均退出 0，见 [cold-result.json](cold-result.json)。未重启旧 PID；保留失败脚本、原始模型日志、索引、源码/依赖和验收程序。16 GB 与真实 8 小时均为 not_run，完整 M0 未通过。

独立释放审查后，终态、句柄、原 index SHA/长度、既有 gzip 解压字节及迁移后 190 份保护原件全部核验，再精确删除本轮 4 个压力/预留文件和 50 个重复中间组件，共 54 个文件，分配空间 6,748,422,144 B；不是按目录名删除。见 [回收结果](retirement/result.json)和同目录清单。文件系统可用空间观察增加 6,748,430,336 B，单列，不视作 APFS 独占物理回收。其余 129,582 个文件身份/metadata 未变。两代原快照现已不能直接完整恢复；索引与 hash 不替代删除的载荷。仍需恢复的 190 份原件完整保留。

下一轮 [joint-02](../m0-t13a-extents-2m-joint-02/README.md)将验证器上限绑定到已认证冻结 Rust 源，并增加边界测试。它是独立新轮次；本轮失败不覆盖、不改写。更大 extent 不自动证明内存收益，完整 payload 已用尽 normal 的预算归因须另行保留。
