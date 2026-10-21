# 完整模型工作区：容量可满足性

本阶段是完整 ModelWorkspace 的实施前提，不是 normal64 或 T13 总门签收。原 fixture、记录数、数值步、预算及性能标准保持不变；只增加 Rust/CLI 容量诊断，不做 UI/GPU/AgentKit。

## 可重复的计算

`mycity-headless model-capacity-plan` 输出真实目标架构的 sizeof、每行记录数和字节数。Engine 包括 Box<World>、Arena 槽和完整 free backing、索引、来源、资源、租约、1024 journal、20000 Claims／64 Payroll 以及 Background 的 field/pending input。Timeline 在 Engine 内只算一次，Prepared 的副本另算；Region／Economy／Boot／Exchange／Inbox 都已在结构本体中，不能再另算深堆表。Prepared dirty stocks 按全部128条计，而不是初始 Vec 的32条预留。

额外空气槽覆盖20000源、128²两组u32及一组u64、字符串和结构本体；操作场景按真实 AirWorkspace 的1MiB已填充背板（包含padding）加其结构本体预留，不能只数有效载荷。并行worker的5MiB另计，包括其自己的空气与IO，不能与Sim临时状态混用。

Store元数据列真实根／索引／信封／收据类型、源队列和操作索引。Opening以结构头（已包含各领域Opening头）、账户／库存／来源／城市／路线／义务的最大行数及字符串逐项计算；64KiB文件长度不当作解码容量，编码输入复用codec。路径8槽×4096、token4096、64个64字节schema/hash槽及64KiB parser scratch是**目标限制**，当前尚未强制。未来实现必须在分配前拒绝超限，不能先解析再发现超过上限。receipt body须流式逐记录核验，不能在这些元数据行外悄悄构造1024个JournalRecord的Vec或DOM。

## 同时存活与复用

| 场景 | 常驻完整模型槽 | 事务槽 | 额外空气槽 | 外层对象 |
|---|---:|---:|---:|---|
| Reference/FaultReference | 1 | 1 | 5 | 4MiB codec及完整元数据 |
| Sim open/genesis/当前／校验保守重叠 | 3 | 1 | 5 | 空间常驻、codec、元数据、并行worker5MiB |
| Sim prepare/capture/accept/checkpoint | 2 | 1 | 5 | 同上 |
| 普通Compare | 2 | 1 | 5 | codec及完整元数据；两模型不能共槽 |

以上按保守超集预留，不假定词法作用域结束前已释放genesis。五个空气槽覆盖外部候选、staged重载、独立CPU reference、新result副本和嵌套frozen验证／输入构造；字段已在Engine／Prepared的另计，不提前抵扣被替换的旧field。

quote、execute、replay和checkpoint通常串行；只能在所有入口显式借用同一Workspace、禁止重入并完成归还后复用Prepared／CPU scratch／codec。checkpoint验证模型与事务槽不能凭“通常串行”就当已实现复用。当前普通serde、Vec容量扩张、Value、深Clone及WAL长度前缀副本仍在，需要后续完整迁移；容量报告不测量或证明这些路径的上界。

空间资产只计已安装Store的常驻payload，结构本体在Store头中计，避免重复相加。最大地形导入有128MiB单独预算，不能塞进本64MiB目标；此计划不签收任意地形导入的64MiB常规预算。原T13固定空间资产不变。

## 验证与完成条件

类型核算、恰好预算／少一字节边界、完整dirty表，以及实际CLI与非法参数实测。报告中 `runtime_grant_accepted` 必须保持false，即使 `target_slots_fit` 为true。记录来源及可执行文件，advisor单轮检查遗漏。

下一阶段必须交付完整固定模型／事务／Store类型图，所有序列和字符串seed直写有界背板、无隐藏深副本、失败重置保留背板、持久后发布、规范字节与旧存档不变、所有角色分配前授权和真实触页／wait归还，才有资格实际核验normal64。之后仍需共同8MiB、正式性能类别／四档／三轮稳态／真正8h；真实16GB继续not_run。
