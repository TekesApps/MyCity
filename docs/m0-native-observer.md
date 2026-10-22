# M0原生CLI观测器

范围是T13-A测量与异常收尾，非城市调度器、业务writer、UI或AgentKit。原冻结负载、数值规则、normal/soft/hard门及真实16GB门不变。最终冻结来源与当前证据见[接入记录](evidence/m0-t13a-native-observer-errno/STATUS.md)；首次errno污染及超限[原始结果](evidence/m0-t13a-native-observer/STATUS.md)保留。

## 调用与所有权

macOS下运行同一固定exe：

```text
mycity-headless native-observe NEW_DIR -- SAME_EXE steady-run NEW_DIR/model l1 smoke
```

只接允许清单中的同exe根命令，最多32个参数、每个4096字节；不执行任意外部命令。不自动重试或拼接窗口。城市原票、Store唯一writer与既有业务恢复协议不改。

observer创建独立进程组及root stdin存活管道，仅直接root获得`native-observed-root` argv标记。root开始业务前核对实际parent及`getpgrp()==getpid()`，先安装owner线程再登记。后代继承环境但无root argv，不会误抢stdin。observer在root初始化前丢失时，root重新托管不视为普通后代，启动失败。

owner管道EOF触发root专属组SIGKILL，包含root自身；不等待已被杀root自行升级信号。受监督后代不得创建新进程组逃离，当前检索仅wrapper设置进程组；这不是对任意外部程序的通用容器隔离。失主不算正常归还，不生成成功footer；后续恢复仍按原持久协议，不能从强杀成功推出崩溃恢复成功。

正常关闭保留既有drain/退出路径。root实际退出后继续采样三帧，仍有已知活体或未知后代则失败并清理。异常清理只处理直接Child或已证明的出生身份，重新夹取当前birth才发信号；不会对上次采样的任意PID发信号。TERM/KILL分级有界等待；无法确认退出保持cleanup error。observer自身被杀时无最终报告，原始日志/触发证据标明失败。

## 身份、日志和采样

- 直接Child在注册前先以实际出生身份纳管。枚举按parent身份前后夹取，已见孤儿由birth继续跟踪，PID复用不继承成员资格。
- headless生命周期记录携带nonce、source、role和实际`proc_pid_rusage`出生身份。Started增加成员；Finished不是OS退出证明。短命子进程实际被采到或明确标为unpolled，不能从末帧为空推断从未出现。
- nonce不是对恶意同用户程序的安全边界；限定本实验固定exe及受控文件环境。外部未登记的短命helper不保证捕获，不能因此签完整资源门。
- observer读取日志使用非阻塞共享锁，锁忙继续采样pressure/swap并标帧不完整；发布者独占锁等待100ms后失败，不无限阻塞业务。日志总16MiB、单记录1024B、读缓冲64KiB，半条记录不消费。
- pending frontier在本帧成员/资源采集后读取，busy或pending非零即不完整；这不是对整个时间轴连续完整性的证明。身份未知有原始原因，不补成零。
- `libproc0.14.11`的空列表错误判断依赖errno。调用入口通过固定`errno`依赖的安全API清旧值，不在返回后吞掉真实错误；库内两次系统调用及中间分配仍可能产生歧义，出现时继续Unknown。

目标250ms采集整组RSS/footprint及pressure/swap，记录begin/end窗口和最大间隔；不相加不同时间的单进程峰值。每60s另记CPU/I/O raw、命名VM sysctl及磁盘/可用空间。CPU保留原计数，本机既有校准不能冒充跨主机单位约定；正式CPU秒转换须绑定校准。命名VM计数不叫`host_statistics64`，不可取得的可选字段为null。

`host-samples.jsonl`保留PID/parent/Mach birth、成员RSS/footprint、Unknown、登记忙/pending及系统读数。`observer-clock-map.json`使用nonce因果往返区间，不假定跨进程Instant同原点。`observer-report.json`给出原始峰值帧索引、未知数量、退出结果、arena及采样间隔。`passed_measurement_wrapper`仅表示该流程正常终态，不表示完整帧覆盖或任何性能门通过。

## 有界资源及验收边界

|项目|约束|
|---|---|
|observer分配|固定2MiB角色arena，含bootstrap及64KiB尾保留；没有System fallback。所有角色共同8MiB账独立验，arena不等于物理进程上限。|
|身份|512个已知成员/帧成员/Issue的固定capacity；超量报错，历史统计不无限保留。|
|枚举|8192是libproc返回后的准入检查，**不是调用前分配界**；外层固定arena耗尽使本轮失败。满buffer可能截断仍报Unknown。|
|输出|单帧固定256KiB，写缓冲64KiB；host日志+compute durations≤256MiB，world逻辑总量≤20GiB，可用空间≥2GiB。|
|运行保护|非normal持续超过60s或swap相对初始净增超过256MiB失败；不杀外部应用来制造通过。最长36000s防无界运行，不代表8h已测。|
|同源|固定源zip/lock/exe，前后hash核对；冻结后不改Rust/fixtures，完整回归和功能实验逐项记录，源变更须另立来源。|

实际2MiB授权、请求峰或5.50MiB诊断投影都不能替代整组物理实测。当前other来自supervisor manifest的external_owned_charge_bytes，combined仅是此投影加observer；它不另展开已在normal pool内的Sim/helper诊断分项，不能将它作为完整L2共同8MiB账。完整共同门仍需逐角色原账独立核验。本阶段物理峰仍超原normal，置换额度0。未来R07同用途替身替换须有独立可物理回收背板、明确owner/出生/排除项、采样关联序号与实际剩余触页；不能在全预触arena内drop Vec冒称释放，也不能用“旧峰值减信用”签通过。原离散采样合同不增加连续零间隙要求。

已测/未测以证据入口为准：L1短测没有真实业务吞吐；活跃Reference helper故障不等于空气worker计算故障。正式L0/L1/L2三轮、四速度、稀有层样本、实际8h和真实16GB分别验收，不能互相代替。当前M4 32GiB不证明16GB设备通过。

阶段终态后执行[产物回收规则](validation-artifact-retention.md)，保留固定源/依赖/exe、raw、收据及仍需恢复/诊断的完整原件；无依赖的本轮合成压力按精确清单退役，更新不可直接恢复声明。复用唯一target，不在正式测量期间构建或清理。


R07独立可回收背板与同用途运行时置换见[实现契约](m0-r07-runtime-replacement.md)。回执与raw按owner/sequence/epoch关联，不是原子同时帧；物理峰保持直接整组实测，layout/pool残差未取得置换授权。
