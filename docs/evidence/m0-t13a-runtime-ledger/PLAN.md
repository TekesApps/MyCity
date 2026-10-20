# T13A-3f：最终来源full轨迹与诊断费用依据

先在连续120秒normal之后，对已发布v0.5.44来源执行原full standalone：64工资／2万债／16义务／8单、720小时、723目标、724空气窗口、3011原票；保留8192页及60秒空气步长。仅Rust模型／CLI，无UI／GPU／AgentKit。observer与原v43同字节，继续执行原pressure／swap／日志／磁盘保护，不缩负载。

本轮实际full恢复前后应有730个正常进程实例footer：驱动1、Sim2、worker724、模型证明helper3。新增helper不能从共同进程树／堆记录删去；短命进程footprint缺测继续单列。独立核验原始角色数据、全部724份输入／候选与v42 hash、完整CPU字节／实际Engine、全部3011原票链和真实Store冷开。成功也仅为单次full诊断，不是L1联合、正式三轮或8h。

## 共同8MiB候选契约（不是本轮已实现上限）

Sim 2.5MiB＋worker 1MiB＋L1 1MiB＋observer 2MiB＋Rust监督0.25MiB＋业务驱动1MiB＋当期唯一helper诊断0.25MiB＝8MiB。advisor认为可作待验证契约：Sim既有费用约2.447MB，2.5MiB留量有限；CpuLog当前实际仍放行3MiB，报告树和编码临时量也必须计入。驱动应保守收整个进程requested Rust heap lifetime peak加64KiB尾部，不能只算日志buffer。

v44 helper仍有CURRENT／root／manifest／before.result等动态Value重叠，输入64KiB不是Value堆上界，故本轮无论结果如何都不签helper256KiB或共同8MiB。下面是下一个源码阶段的具体实现条件：

- CURRENT／root／manifest具名元数据解析，有界控制槽与固定hash，未知字段跳过，禁止用通用Value保留树。
- root原文若需在最终报告保留，发布校验资产并传递小凭证；父驱动可在自己的整堆额度内读取，helper不复制多份root树。
- request/result解析需限制字段长度／artifact数量；控制文件容量门只约束控制协议，原full业务记录数／日历／空气步数不变。
- 每目标行、固定19字段空气摘要和计数结构使用借用／具名Serialize，同时保留v44已验证的旧f32→Value/f64数值语义；旧Value API保持直接构造，不能增加JSON往返。
- 明列最大同时存活容量：64KiB golden与64KiB校验读取的阶段互斥性、控制槽／解析scratch、路径／元数据及64KiB退出尾部。所有分配前拒绝超额；未知native／栈仍需RSS／footprint观察，不把requested heap当物理内存。
- 三种helper的正常、边界、坏输入／发布失败和父死亡路径实测；然后按原full实测共同费用和真实所有者。不能用模型helper总峰减去Engine峰来证明诊断额度。

模型工作区仍属于外部64MiB真实grant目标，包括参考／比较、Sim／worker State／编码／IPC及未借出触页余额；本次无L1布局压力，没有全局grant闭合证据。正式三场景各三轮与实际8h仍按原标准执行；真实16GB无设备则not_run，不签完整M0。
