# 下一阶段实际角色分配边界（待实现／未授权）

此表来自当前冻结源码，不是normal grant收据；旧model_capacity保守表含3root／5generalAir与额外codec重叠，已显式加入Metadata并标为不fit，保留作历史比较，不能直接据此放大授权或修改64MiB。

| 所有者／入口 | 必须计入的同时存活对象 | 当前边界／下一动作 |
|---|---|---|
| Sim::run_inner → Codec::new | worker_slots::Owner实际4MiB、Codec头、epoch、单独BackgroundStateSlot冻结输入 | 原broker只确认IO Owner；冻结输入实际typed backing未随此授权。与Store同时存活，不能当同一candidate槽抵扣 |
| StoreWorkspace → EngineWorkspace | initial与restore双root、每root保留prepare、Store air candidate、Store canonical Codec+traversal | 包真实初始化并消费；不再计第三个genesis Engine。legacy恢复先导入spare，再覆盖历史缓存，全部校验成功后交换 |
| StoreWorkspace → SpatialWorkspace | Large厘米地形网格、保留PNG decoder及场景13背板 | 实际prepared/live容量而非仅header。空间输入typed解析与已存在Scene可短时重叠，单列其临时容量 |
| StoreWorkspace → MetadataWorkspace | 16MiB原文件、4MiB Engine字节、32KiBsmall、各64KiB JSON/PNG/Opening、1024JournalRecord／u64、8Resource | 已分配heap23,855,232B，header另计。即使用v2，也不能把未用legacy槽记为零或隐藏作slack |
| Store／Persistence | 根及引用字符串、目录PathBuf、lock/wal控制头、live background哈希、WAL缓冲、Root/CURRENT临时header与clone字符串 | owned已补双prepare及checkpoint引用heap；完整候选root与I/O/序列化控制暂态仍须逐字段核对 |
| Sim IPC／诊断 | 8192B输入BufReader、32KiB协议frame、Request/Envelope/actions、status/JSON响应及diagnostic实际缓冲、进程控制 | 小额字段上限不自动代表已预分配。必须分别归入normal或共享diagnostic，不得双记或悬空 |
| Sim required → Worker | 实际child与通道、ticket、进度、候选发布；独立worker5MiB | 只可在完整Sim角色账后并列计算；旧child已真实wait后才可返还，candidate grant不是整个worker |
| Reference → Reference::from_workspace | EngineWorkspace双root双prepare、candidate、comparison codec；opening临时、oracle Value、记录输出控制 | 无Store空间／Meta义务，不应强塞通用Store包；oracle/报告也需收费。无worker权限 |
| Compare／fault reference | 实际消费的比较包、两份Engine／受控codec、输入/输出及Restore暂态 | 以调用图计量，不因名字相同继承Sim预算；同一背板归一个所有者 |

## 转义与旧输入范围

Advisor建议固定缓冲JSON词法扫描：逐字符增量识别key，已知字符串写有语义长度上限的固定槽，允许忽略的长字符串验证后丢弃。有限深度栈/位置保留在真实固定背板；不能让serde在visit_str前先生成长转义String。保留数字原token、重复键顺序、转义后同名识别和字段任意顺序，不能用Value“最后键覆盖”。

普通Record未知值的IgnoredAny，与内部标签unit忽略内容的Content标量校验须分开。后者仍须拒绝1e400或非法surrogate；前者维持旧解析实际接受范围，不能全局套新验证。还须对照CP上下文中的深度边界，不能因借用RawValue后重建Deserializer而误宣称全体非法输入拒绝行为完全一致。

最小反例：接近16MiB且仍在原整档上限内的长转义key、unit忽略字符串、普通未知值；缺失/null、转义重复键、kind末尾、struct/unit的map/seq、边界深度及非法标量。对照固定旧reader接受性和值，新增词法临时容量必须不随字符串长度增长。

## 全角色准入顺序

先完成每角色同时存活清单并证明所有容量 → 分配前申请准确页集合 → 构造/初始化全部真实背板 → Filled绑定profile版本、来源、PID、实例与地址/容量/页身份 → 实际入口只消费包 → 子进程真实wait后归还。构造失败、丢回复、撤销、迟到与死亡均要实际测试。不得先把grant增到64MiB，再把未控制对象叫作预留余量。

本阶段被冻结后的验证只证明功能及兼容接入，不证明此表已经实现；formal L1+L2、分级／四档／稳态／8h仍按原冻结标准执行，真实16GB保持not_run。
