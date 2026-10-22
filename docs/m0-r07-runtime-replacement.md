# R07独立背板与运行时置换

本项只针对M0压力替身的R07同用途padding，城市业务模型／真实资源／权益不随之削减。正式负载、域配额、原normal、128MiB工作能力、64MiB池及共同8MiB诊断门均保持。Metal/GPU、UI和AgentKit不在范围内。

## 背板及预算

原R07纯padding按64KiB独立匿名映射，余数仍在原arena；独立映射全额容量从arena授权等额扣除，没有第二份全额背板。固定generator为`r07-immutable-splitmix64-v1`，每段按逻辑index完全写入；至少每4KiB触达，非仅虚拟地址预留。

4236个满段共277,610,496B，尾段52,432B；原R07逻辑277,662,928B不变。控制容量（主Vec、Arc/Mmap头、最大snapshot引用表）203,328B在既有4MiB metadata中承担，不是额外授权；allocator/内核元数据、native及stack仍须整组物理采样。不能把payload扣额解释为同时免掉控制头。

## 动态输入与实际回执

仅当前来源、nonce、PID+birth绑定的observer／纯supervisor／steady-driver独立进程可作输入；layout、pool等已收费模型背板进程不凭footprint残差取得信用。observer整进程footprint已含自己的2MiB arena，不能再加一次。

每份Reading包含单调时间区间、sequence、固定三个角色槽；总量使用发布footprint与新捕获footprint较小值，按总和向下取整到64KiB。TTL为1秒（从采集开始算），同PID重复、错误source/nonce、倒退sequence、身份变化、缺失／过期均撤销相应额度。它是受控同源进程协议，不是对恶意同用户写文件的权限隔离。

只有最后唯一Arc实际drop后才确认少映射；snapshot pin存在时不能继续减少映射、不能领取未发生的替换。撤销信用须实际重新映射并写满suffix，再发布新的epoch及mapped/replaced量；旧reading highwater保留。回执与观测帧按owner／sequence／epoch关联，保留转换区间，不声称原子同时采样或连续零间隔兑现。真实整组footprint直接测量，不能用旧峰值减信用当新结果。

## 快照和恢复

新布局manifest-v2／snapshot-v3明确版本化：仅R07独立且不可变；非R07保持原数量扫描和每次精确1MiB确定性脏写。与旧格式不同，不声称旧pressure hash／dirty轨迹不变。

保存时pin同一份映射prefix和State，并冻结generator、逻辑长度、尾段、epoch、文件长度/hash；补回suffix不改旧prefix。仍写读所有原native/stub/pressure/work文件，原index不改写。冷进程验证全部历史文件和prefix内容，重新建立全额R07，历史替换量和运行时reading信用不继承。首次新scan只能依据本进程的新输入重新决定。

旧snapshot-v1/v2仍走原单arena及原generator恢复分支，没有强加R07新schema。已删载荷不得依靠hash继续称直接可恢复。

## 功能诊断与错误退出

`r07-layout-driver NEW_DIR`装配原满级L1、真实64MiB池及两个实际快照。专用session最多120秒，第一代durable且已有实际置换后才开始第二代，期间每秒继续原数量扫描与1MiB脏写。这个事件驱动周期仅为机制实验；正式300秒周期和其他原门不变。

只在该诊断root支持`r07-provider-pause`，它暂停发布，不暂停观测进程、pressure/swap检查和owner安全。第二代pin期间令Reading过期，验证补回全额后历史prefix仍可冷开；独立新PID无provider恢复全额、零历史信用。

`layout-r07-session NEW_DIR --report-fault`是显式诊断反例：在大模型分配前注入未知metrics class，走与正常结束共用的报告发布器。先完成`metrics.report()?`，再写成功JSON；错误必须CLI非零退出且无成功report。正常compute结构为直接对象，不能把Result的Err序列化成成功报告。

## 当前证据边界

首个8秒重复保存诊断实际`layout_snapshot_busy`，原失败证据保留；后续两代事件驱动实验不把它改写成通过。旧来源7fd全仓回归及功能数据、修复报告错误后的新来源独立保存，不混称同源。

有限功能通过不代表normal或整个M0通过。观测已显示整组物理仍超原门，下一依赖是同birth/epoch/映射量下实际地址与VM用途归因。footprint减arena／mapping／pool授权只是总量核对，不能直接授权未归属残差。真实业务与L1 layout替身仍分别报告；正式三轮、四速度、稀有层、8小时与真实16GB各门按原证据单独验收。
