# v0.5.114：已验证交通市场 resolver，真实结算仍待接入

集成独立分支 `65b1b1c9152892c64368430899803c005b64ce62`（基线 v0.5.112）。本次 Rust 改动仅 `market_access.rs` 及其 integration tests，现有 Engine/Store/Timeline 均不改变；先前退出 planner 保留。独立功能通过不等于两者真实联合通过。

最终源码的实际 release 验证见 [attempt-03](../core-market-access-resolver-validation/attempt-03/README.md)：Rust/Cargo 1.85.1、locked/offline、共享外置 target，16 项目标测试及 core all-target strict Clippy 通过。root 对最终 resolver、tests、原 example、锁文件、工具链及源码归档逐 SHA 复核；固定 CLI exe SHA 也与收据一致。attempt-01 的 15/16 授权撤销分类失败保留；修复校验顺序后 attempt-02/03 均 16/16。未在 main 另跑整个 workspace，因此不称全仓回归通过。

六个实际 CLI 场景见 [原输出](../core-market-access-resolver-validation/attempt-02/raw/cli-scenarios.jsonl)：normal、cross-city、unrelated-cross-city、capacity-expanded 可达消费量 200；road-closed 为 0；bottleneck 为 40、未满足 160。全部明确为 pureplan，未落账，不表示实现真实运输企业价格或全城经济平衡。授权出入口、两向 OD、共享已完成行程配额、道路关闭和扩容的结果可核对，不把道路剩余容量重复发给不同商品。

resolver 不再额外执行市场 planner；返回动态 DeliveryQuote 和逐项审计，由现有调用决定一次规划。snapshot generation/profile 和完整 Opening/Assignment/input 进入 digest，generation 只作可信调用方的对象选择器，不能独立证明 world、branch、日期或授权。后续唯一 Engine writer 必须从完整权威快照校验这些域并将报价输入既有 RecipeExecution/plan_day/reconcile 路径，不引入第二账本或把提案直接当收据。当前未实现该 Engine/Store 连接。

本阶段还补充 [原冻结分配容量归因](../m0-t13a-budget-attribution/allocation-capacity.md)：原 heap/trace 实际复算，arena 峰请求距容量只约 1.18 MB，两代解码低谷 free 约 1.35/1.33 MB；没有证据支持删掉 metadata4MiB 或128MiB工作区。normal 超额仍保留，不变更原负载/阈值。2MiB02压力诊断仍未运行。

建设线默认2MiB测试栈失败、16MiB复跑通过的中间结果尚在隔离分支处理，本阶段不纳入通过范围。真实8小时与16GB门仍 not_run，M0和三线联合未完成。没有另建 target；共享缓存留给紧接的建设验证复用，不删除在用产物。固定 exe、源/依赖、各轮原日志及必要恢复原件完整保留。性能使用外置HP P900 USB/APFS SSD；本次功能检查不是正式性能。
