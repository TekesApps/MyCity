# W1：独立空气worker与连续窗口

发布v0.5.26。对应[冻结验收](background-compute-acceptance.md)的W1；W2—W4仍未通过。只做Rust模型与CLI，无UI／GPU／渲染／AgentKit。独立worker是真的子进程，但不是独立Sim、世界持久任务队列或全产品后台运行完成。

## 1. 已实现边界

`city-core::air::State::continue_from_boundary`只允许完整窗口结束后接下一个固定target，保留场、积分、绝对时间、步数和累计诊断。相同网格、初始定义与四边界值必须相容，新源不能开始于已提交时间之前。边界有效期可以延长；边界值改变需未来显式新策略，不能静默改当前数值路径。已有30日任务时间上限保留。

延续不是重做State.new；无源窗口仍继续旧场衰减。窗口边界属于显式切点，任意切窗不保证与未切窗逐位相等。测试在既有60秒锚点拆窗，逐位比较field／integral及累计源量；另验证网格、边界、历史源、未完成和非前进target拒绝且旧状态不变。

`mycity-headless compute-worker TASK_ROOT`使用stdin／stdout JSONL执行多个纯任务。任务目录只含`inputs/`和`candidates/`中的内容寻址空气检查点；请求不接受任意文件路径。worker不调用Store、不写世界、不付款；这是代码责任边界，尚无OS沙箱隔离承诺。stdout只发控制帧，stderr在父进程中丢弃，不形成无限管道。

## 2. 协议与上限

协议`mycity-air-worker-v1`，数值策略仍是`run36-air-f32-v1`。Ticket包括world／branch、job、incarnation、generation、attempt、base_revision、field_version、[start,end)、rules／source／input／完整起点检查点摘要。源码必须与worker编译身份一致；所有文件摘要必须是64位小写十六进制。输入检查点验证时间、target、输入身份、无半页及未完成。

当前worker能验证纯任务结构和输入身份，**不能独立证明世界版本或规则摘要属于当前城市**。权威ticket、不可重置的重试次数及版本接纳由后续W2／W3的Sim持久状态负责。W1探针的重试代际是实验驱动，不算持久重试通过。

| 项目 | 实现门 |
| --- | --- |
| 单worker在途 | 1，未完成不得再次submit |
| JSONL单帧 | 包含换行最多64KiB，fill_buf／consume分段读取，拒绝截断帧 |
| 父进程reader队列 | 4帧，不使用无限channel；最多约256KiB正文，属于已有1MiB控制预算 |
| 输入／候选文件 | 各最多8MiB，流读取先限长再解码 |
| inputs／candidates目录 | 各最多32文件、32MiB；发布前为临时名及目标名留槽 |
| 尝试字段 | 1—3；没有W2／W3持久责任时不能把它当全局重试封顶证明 |
| 数值工作页 | 每次256源或256格；不逐主体开进程 |
| 停止 | kill并wait回收子进程；丢弃receiver解开有界sender后join读线程 |

候选写临时文件、sync文件、hard-link到摘要名、不覆盖目标、删除临时名并sync目录。已有同摘要文件也重新核验和sync文件／目录；内容损坏直接拒绝，不覆盖“修好”。进程被杀留下的临时名纳入目录限额；W1不自动GC，W2必须按世界恢复引用管理资产。这些fsync和进程实验不是物理断电验证。

父进程收到候选只验证传输身份、长度和摘要；真正纯数值接纳用`verify_candidate`验证固定起点、完整性，并以同CPU路径完整重算、比较整个State（含场、积分、累计和工作缓冲位）。它有重复计算开销，不是加速证明；W2还必须先验证权威ticket／场版本，再将结果持久导入Sim资产。

## 3. 可重跑命令与实际故障

```sh
cargo test --workspace --release --locked --offline -- --nocapture --test-threads=1
cargo clippy --workspace --all-targets --release --locked --offline -- -D warnings
cargo run --release --locked --offline -p mycity-headless -- worker-probe /tmp/mycity-worker-new-directory
```

普通worker不停在诊断消息处。故障探针专用`compute-worker-probe-gated`在首次半源页、半tile页和候选发布后等待匹配ticket／stage的ACK；驱动在目标阶段不ACK而发送实际SIGKILL，并检查无正常退出码。非目标阶段才ACK继续。这避免把“收到进度以后杀”误报为“恰好杀在指定阶段”。普通路径不需ACK；探针握手不改变浮点运算。

探针先以同一PID执行[0,60)与[60,180)两任务；第二段不新增排放，仍保留第一段600000 microEI累计。另在源页、tile页、候选发布但Complete前实际杀进程，重新启动另一PID以代际2重算，逐位对照直接CPU。发布后重试要求相同内容摘要，不重复生成不同结果。源页和tile页用128×128、20000源、1日固定窗口，不冒充完整城市活动输入。

反例覆盖旧源码、未知schema／字段、错输入、错起点、非法路径摘要、超尝试值、超限／截断帧、目录超限、不完整候选、有限但被篡改的完整场、坏文件、文件超限及未读完回复时停止回收。现有空气f64参考仍是独立数值层证据；本阶段CPU重算用于一致性校验，不能被称为独立算法。

[阶段证据](evidence/background-worker-w1/gates.json)、[实际探针](evidence/background-worker-w1/probe.json)、[证据核验脚本](evidence/background-worker-w1/verify.py)绑定本版源码；任务文件压缩存放在同目录tasks.zip。压缩大小不是运行内存，短探针不是30分钟联合长跑。全进程同步采样、实际业务源、Sim崩溃、持久接纳和16GB设备均未验。

本版实际Release共201项测试通过，fmt及clippy（拒绝warnings）通过；源／tile杀进程均记录partial_cursor=256。冻结源码`f94333856ddab7a0265419adf259da198bbb216d98b3d09ebdbeaa6a45c22376`；探针墙钟1.293252秒，仅覆盖上述有限任务与进程恢复。command-rusage.txt是命令rusage，不是同时刻进程组物理占用。既有经济日实跑61.432940083秒，严格60秒门仍未满足。

## 4. 审查与下一依赖

advisor只读审查发现的两个阻断已修：阶段消息后的竞态改为探针ACK屏障；已有候选分支补持久同步。审查本身没有运行测试，实际结果以本版日志为准。

下一阶段W2把任务、场版本、必要P4屏障和独立预留并入Core／Store。大候选先由Sim导入不可变资产，再用小引用入WAL；恢复不能读取worker目录冒充Sim持久状态。全目标继续，不以本W1阶段发布宣称后台闭环或M0完成。
