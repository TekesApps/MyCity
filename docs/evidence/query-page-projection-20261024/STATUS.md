# #49 Store 分页投影限定修复

基线 v0.5.172 / 33f7ed988c9a0d1673a7cec48fc67e99837deb1c。共享 writer 为当前总协调；未新增聊天或开发线。

修复实际 Store::query：Arrears、Contracts、Fulfilment、Capacity、Blocked、Recovery 保持原行序和 cursor，仅对选中页构造 JSON。移除本查询的完整 WorldView 拷贝；source_status 未注册时也只读取原 scope。typed 表仍需扫描计数，state/snapshot hash 与必要状态读取仍有自身成本。

新增 getter 只借用不可变 Region/Exchange，不提供修改能力。原 Timeline 索引分页与 Receipts 原路径不改。未启用 Timeline 的非零 offset 旧空页语义明确保留；page limit 仍 1..64，无新快照/缓存/调度器。

原 advisor 首审发现 disabled Timeline offset 的兼容差异，已消除并加新旧 Page 对比；窄复核未见源码阻断。source02 测试编译出现游标闭包长期借用 Store 的 E0502，root 改为复制 generation，失败源码与日志完整保留。source03 运行 3 通过、1 因场景 full inbox 新增承诺被原门拒绝。原 WAL 明确为 InboxFull，完整失败世界保留；root 将配送与 Timeline 分成两个真实世界，不改准入。e0 无 Region 的 payroll 明确使用 home_city=0；source04 no-run 构建被 root 精确 SIGINT 取消以修该已知场景，取消不算通过。source05 release all-targets 检查、固定程序构建通过；6 项限定串行测试全通过：新 query 测试 4 项、既有 inbox 分页 1 项、既有 Timeline 实际支付和分页 1 项。448 文件固定源码 SHA-256 `9319b46363d3fdb8589a9aaf0b3752c588a4311bcbbd5490435a1a25d7be6925` 与固定 exe 运行前后相同。

测试分层：test-only 冻结 v172 全量投影参考（公共 Store 方法仍是当前版本，不是旧可执行文件）；实际 Store 在暂停运输/费用、检查点冷恢复、配送完成、Timeline 已登记 claim/payroll 等状态，对八类 query 按 1/2/3/64 页长逐页比较整个 Page。还覆盖终页/越界、未启用 Timeline offset、失效 cursor、不改 hash/sequence，以及 JSON 闭包只执行页内次数。损坏 outbox 证明公共 snapshot source 读取失败；分支内部页外错误次序来自源码审查，不冒充故障注入。

限定门不覆盖所有 arrears 状态或产品要求的全域 query/Goal/Monitor。单行可有嵌套对象，64 行不是编码字节上限；闭包计数不是 allocator/RSS/CPU O(page) 证明。没有原生内存、16GB、GPU 或 M0 冻结验收结论；不关闭完整 #49。真实教学/公服与 #54 Store 接线仍另行推进。

使用既有 HP P900 USB/APFS 外置 target/work、Rust 1.85.1 locked/offline release -j1，32GiB 设备只做正确性回归。所有原始失败/成功日志、固定源码/可执行文件与生成世界保留在 query-page-projection-20261024；128MiB 阶段证据预算，后继验证版本与恢复依赖复核后才讨论精确退役，不清理未知 WIP。

## 复验与证据

所有固定源码/程序/原始失败及成功日志、完整世界留在 `/Volumes/HP P900/dev/MyCity/work/query-page-projection-20261024/`。source01/02/03/04/05 ZIP 与 manifest 均保留，不能把 source03 的失败改写为通过；source04 取消没有运行结果。完整旧投影 oracle 是测试代码，正式 Store 路径不编入。

未来最小串行复验须明确外置证据根（目录先创建，世界以独占新目录保留）：

```sh
MYCITY_QUERY_TEST_WORK='/Volumes/HP P900/dev/MyCity/work/query-page-projection-20261024/recheck-worlds' CARGO_TARGET_DIR=/Users/frankqin/dev/MyCity/target cargo test --release --locked --offline -j 1 -p mycity-storage --lib query::tests:: -- --test-threads=1
```

不使用 RUST_MIN_STACK 绕过原默认栈。已有 Timeline 20,000 条原测试是正确性回归，不是新 M0 长压力或支持上限承诺。
