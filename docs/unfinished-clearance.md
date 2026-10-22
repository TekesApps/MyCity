# 未投运半成品的清场闭环

`city-core/examples/unfinished_clearance.rs` 描述一个最小 Engine 场景：住宅工程原计划 28,800 工时，先完成 14,400 工时并消耗 500 毫单位材料；业主暂停后申请清场，原有业主同意与许可两道 gate 分别由各自代表重新批准。清场开始后保留原场地 token，不发布运营容量；按 600 秒基础工时加实际已消耗材料对应的 1,800 秒，完成 2,400 清场工时。清场废物按 500 毫单位/容量单位向上取整为 1 单位，接收方收取合同约定的完整固定费用 250。

示例断言材料不退款、不重复扣料、施工支出新增清场工资、接收方收到清场工资与处理费、废物占用接收容量、原场地只在 Close 时释放，且基础运营容量仍为 2、从未发布过新增运营容量。它只是建设工作流、两项 gate 和费用/容量事务的模型样例，不代表完整司法、行政或现实许可流程。

新生成的Project使用`recoverable_v2`执行策略；新请求ID与内部effect序列分离，旧请求ID仍保留原成功或失败。缺省旧项目为`legacy_v1`，不自动获得本轮退出行为，旧WAL需要按原规则回放。详见[请求重试与兼容边界](construction-retry-policy.md)。

预期入口（使用包含未投运项目清场 Engine bridge 的整合源码）：

```sh
cargo run --offline --locked --release -p mycity-core --example unfinished_clearance
```

实际证据：21号Store清场测试5/5通过；64号`unfinished_clearance` CLI exit 0，报告built_work=14400/28800、clearance_work=2400、waste_units=1、disposal_fee=250，释放site且`operating_capacity_published=false`。其输出明确标记`model_only_not_full_legal_process=true`。59号道路清场Store CLI另验证两日销量各40、现金守恒及冷恢复。以上只覆盖这些Engine/Store fixture，不代表完整司法、行政或现实许可流程；失败及未测门继续保留。

清场规格与历史都是项目内的固定长度结构，最多沿用8个材料槽，不为每份废物创建常驻主体，也不逐设备模拟。原Project的2,048字节状态上限和公共事务工作区上限不扩大；72号最终core/storage all-target release测试exit 0，71号road-epoch测试3/3通过。该两日fixture和回归不等于64MiB全尺度预算或16GB性能通过。正式8小时M0因压力守卫提前结束。实际状态入口为[core-road-town-integration-122](evidence/core-road-town-integration-122/STATUS.md)。
