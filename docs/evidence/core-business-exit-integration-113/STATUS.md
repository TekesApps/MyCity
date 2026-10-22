# v0.5.113：有限企业退出计划器与 M0 失败归因

集成经济分支 `c432906f53a35beb2afda9f60ea328bda55c8422`。实际验证使用 Rust/Cargo 1.85.1、locked/offline、现有外置共享 target：13 项企业退出测试和 mycity-core 全 targets strict Clippy 通过，见 [原始日志及来源](../town-business-exit-validation/attempt01/validation-manifest.json)。主线相对该分支基线 v0.5.111 的 Rust/依赖差异仅为该分支的四个 Rust/fixture 文件；root 对冻结归档中的六个源码/fixture/依赖文件逐字节核对，并重新核对归档与验收程序 SHA。这是来源一致性核对，不冒充新增全仓测试。

计划器保持 Engine 为唯一 writer，不另建时钟、账本或持久化；区分公司与个人资金、保护现金、已获法律优先级、未决/未来债务、库存待售与实际收入、工资义务及剩余股东分配。缺失完整债务/税费/法律/雇佣条件时不凭空清偿或分红。64 条债项、64 工资、128 库存、64 股东是明确有限接口，超过即拒绝，不表示全规模公司清算已经完成。64 KiB 只为 planner 工作/输出估算，JSON schema String 仍先分配再校验，公开 JSON 入口须复用有限原始读取；解析峰值未测。

尚未接入 Engine 原子退出执行，现有 28 日持久化经济场景仍单列；没有把这 13 项测试称为新的 28 日闭环或长期经营平衡。交通 resolver 正在独立验证；建设拆除的 Ready/Paused 可达性问题由 advisor 找到、writer 修复中，均未在本版本宣称联合通过。

M0 本阶段保留 [2 MiB 首轮控制器失败及两代实际冷恢复](../m0-t13a-extents-2m-joint-01/STATUS.md)，精确退役该轮 54 个无后续依赖的压力/重复组件，分配空间 6,748,422,144 B；当前原快照不能直接恢复，190 个必须保留的恢复/故障原件完整保留。另保存 [2 MiB 第二轮准备及 22 项辅助测试](../m0-t13a-extents-2m-joint-02/README.md)，运行尚未开始，不能算性能结果。2 MiB Rust 候选未替换 main 的 1 MiB 实现。

[32c 原冻结负载预算归因](../m0-t13a-budget-attribution/README.md)已从原 raw frames 实际重算：原 normal 3,328,599,654 B 已由三个固定额度恰好占满；full complete peak 3,337,031,256 B，超 8,431,602 B。没有把 VM 地址覆盖、permitted helper 或 map 数量当成物理节省，尚未定位可安全削减的单一分配。后续先处理可证实的 native/进程开销，保留原输入与阈值。

工作与共享 target 均在外置 HP P900 USB/APFS SSD，源码留内置盘；功能检查不等于正式性能。没有另建 target，保留测试 exe/固定依赖/源码/原日志；共享缓存供随后交通与建设验证复用，不删除。真实 16 GB、真实 8 小时均 not_run，完整 M0 和三线真实联合尚未完成。
