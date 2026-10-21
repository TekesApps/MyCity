# 固定L1诊断恢复的分配前边界

阶段基线v0.5.55。原acceptance fixture和完整JPROD数量不变。仅Rust模型/CLI，固定L1原生记录与typed表的恢复：冻结本地类型/数量先计算容量再分配；实际capacity必须匹配，六个Vec及两个Arena按原数量一次预留，禁止serde Content/Value整表缓冲；N+1条在构造前拒绝。Native解码前预算整个目标graph及6MiB parser/IO/stack余量，不将原有常驻对象从本次贷款中扣两遍。原64MiB贷款和128MiB reserve不变，物理RSS/footprint另测。

写入codec不变。固定诊断Arena限定全live/空free，scope/generation/所有索引仍核验，不替换通用World/Arena读取。表按调用方冻结variant/count解码，records在tag之前也不缓冲；缺失/重复/错误tag、数量、超长token/总输入/尾数据均拒绝。读取器不回退无界通用解码。

定向验证：容量溢出及预算在记录解码前拒绝、额外记录不构造、短记录/EOF/token/字段顺序/重复字段、Arena scope/generation/free/live；真实完整原生存档文件读写与hash、旧已归档原始组件兼容。真实旧组件兼容与完整3.10GiB联合分开报告；没有重新准入并跑完整负载，不签L1联合/normal64/common8/steady/8h/16GB。

advisor已只读认可方向并指出实际重叠/字段顺序/token/引用/原字节兼容边界，见后续独立终审。实际测试期间冻结crates/fixtures，不并行运行其他模型负载。当前空闲磁盘约9.4GiB，下一原联合需重新校验多份快照和至少2GiB真实余量，原失败数据不可静默丢弃。
