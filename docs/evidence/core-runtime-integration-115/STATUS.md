# v0.5.115：主线有限运行时集成与2MiB诊断

建设分支2724347/f966acf/531192d/fe85588与主线交通resolver、企业退出planner集成。root修正建设example在Close后仍打印旧Operational现金/库存的问题，改读最终已提交World；按Rust1.85.1格式化新增/集成文件。首次fmt失败和原分支各轮编译/栈/操作ID失败全部保留。

## 实际主线验证

最终冻结源 `d19e4a8800f9b41f7c3ad249c943d8259794d5debe1d2994f332c491b0b71ef1`，246文件，包括旧格式测试依赖的固定文档fixture；[冻结](freeze.json)、[源码归档](source.zip)、[逐命令结果](results.json)及固定CLI程序/SHA同目录保存。验证后每个源文件再次逐SHA核对不变。

Rust/Cargo1.85.1、release、locked/offline，显式复用外置共享target，RUST_MIN_STACK未设置：core纯建设7、Engine建设9、交通resolver16、退出planner13，共45项通过；Store建设WAL/恢复4项通过，总49。core/storage全targets strict Clippy与workspace fmt检查通过。这是列明harness的有限集成，不是全workspace测试，更不等于交通经营已经真实联调。

实际release主线程[建设CLI](construction-cli.log)退出0，最终 `Closed, owner_cash=1142150, stock_qty=599000, site_occupied=0, waste_occupied=2`；审批→建造→验收投运容量→消费者占用阻止拆除→释放消费者→撤回供给→拆除工时/成本→废物收费接收占容→关闭释放场地均沿既有Engine事务。禁止复制运营供给、未投运不能提供正常能力、恢复重试不重复收费/施工，相关反例及Store冷恢复/丢回复原票重放实际通过。旧分支打印1171200为旧运营采样，不作为最终现金。

[原B验证](../core-demolition-runtime/attempt-05/RESULTS.md)的默认debug测试线程栈溢出保留；同harness16MiB debug通过。本次release默认测试条件和真实CLI另行通过，不能把两种编译/栈条件合并。Project<=既有2KiB、Engine<=既有4MiB为有限拥有容量断言，不是OS栈/RSS或16GB物理预算通过；当前只绑定单Project的固定Resource贡献，非全城电网、补偿司法或任意工程工具全部完成。

## M0与后续

[M0固定2MiB候选02](../m0-t13a-extents-2m-joint-02/STATUS.md)实测4捕获和2代完整冷恢复通过，但complete peak仍超原normal8,415,170B；内存门失败。固定ade来源与本阶段d19主线源严格分开，2MiB未设为main默认。真实16GB/8h not_run，完整M0不通过。

该轮54份无恢复依赖载荷/重复组件精确退役，分配空间6,748,553,216B，df观察增加6,748,536,832B，不混作APFS独占回收；190完整恢复/故障原件保留。root不另建target，验收exe、源/锁、原日志和必要恢复原件保留；共享缓存供排队经济场景继续复用，本阶段不盲删在用缓存。

下一共享writer按[可信交通接入契约](../../core-town-transport-integration.md)替换TownSettle静态报价，特别阻止跨日OD重用；经济七类28日CLI正在独立验证，尚不纳入本版本通过。首期总体目标、真实8h、16GB及三线限定真实联合继续保留，不以本阶段49测试替代。
