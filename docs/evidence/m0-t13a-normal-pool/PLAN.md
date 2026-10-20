# T13A-3h：64MiB实体余额与真实worker借还

本方案先于新来源联合测量冻结。保留acceptance-v1的数量/计算粒度/失败保护；只做Rust模型与CLI。原full业务仍64工资、2万债、16周期义务、8实际交付、720小时、723目标、724窗口、3011原票；空气60秒与8192页不变。原L1为361272 native +1232426 stub、600完整扫描、0/300秒快照，manifest就绪120秒后启动原30日；最新快照新进程真实冷装。连续120秒normal才入场，非normal>60秒或swap净增>256MiB仍保护失败，不缩负载。

## 可证明的实现边界

独立normal-pool-host实际持有64份1MiB Vec，每份检查capacity并逐页填值/全量扫描；未借出余额每秒扫描。只允许一个4MiB worker grant。父进程私钥与worker私钥分开，源/boot/单调grantID/父实例/子PID绑定；私钥不写事件。父页释放确认在worker创建前；子进程完成两个真实2MiB槽的分配和触页后报告Filled。槽在整个worker进程生命周期保留，多job只重绑定身份/trace，不释放或再分配。父端持有Child并wait确认退出后才Reap；spawn失败未绑定子进程时Abort。重复确认不增加信用，旧代次/错误私钥不允许归还。转换时存在短暂不足压力，记录prepare至fill/reap时间，不宣称持续足额。

池进程在L1初始化前启动，业务和冷装期间持续存在；退出前必须64MiB实际余额、grants==returns==filled==724。池主线程固定8KiB控制输入、外部tag固定字段协议，监听父stdin EOF线程128KiB原生栈另列。父进程不接受旧目录/旧ready或错误新childPID。修改后的页/日志内部错误是Fatal，服务立即退出，不能转成普通拒绝继续授予额度。实际故障注入仅具名探针，正常Process启动清除该环境；ready保留注入模式。

Sim/helper普通Engine、serde及其它未转换分配**不释放对应填充，属于额外实际压力**。不能以历史heap峰相减填洞或声称常规3.10GiB已闭合。实体余额只解决空额度未触页/worker槽借还；准确全模型64MiB覆盖、Sim被杀后的旧grant接管仍未证。Prepare丢回复可fail-closed保留grant使整次实验失败，不能声称自动恢复或凭PID消失退款。

## 诊断收费与静态边界

仍共8MiB：Sim2.5、worker1、L1诊断0.75、observer2、supervisor0.25、driver1、helper0.25、pool0.25 MiB。Pool从原L1的1MiB中分0.25，不增加共同上限；旧L1实测719936B低于786432B，仍须新来源实际核验。池动态模型页最多64MiB，固定控制/事件/路径/静态元数据/解析临时和64KiB退出尾部保守收0.25MiB；整堆峰加尾部不超过64MiB+0.25MiB仅作总量附加检查，不用两个峰相减证明诊断。原生栈/foreign及RSS/footprint继续单列。

## 实测与独立核验

先同源全仓release/offline、fmt、all-target clippy及具名集成反例：真实64MiB、两job共用worker槽、spawn失败、Sources/Cells/Published三处kill后的父wait回收、旧活/死ready、旧boot、控制边界、父EOF、Attach后事件失败、第二补页失败。初始协议u128反序列化失败和clippy/编译失败原样保留，不混作最终来源。

正常入场后执行原联合；若失败，保存实际保护原因、所有已发生事件和缺测，不能用旧v45/46的full结果填新来源。成功时原完整Engine/CPU/目标/3011票、724工作槽与旧hash、L1两代旧输入/冷开、734正常Rust实例、池grant与真实worker/PID/实例逐一对应并独立核对。费用、物理采样与转换间隙分别报告。

这不是三轮稳态、四档或8小时；普通工作区额外压力可能使常规预算失败，即使业务正确也不签normal-owned。真实16GB无设备保持not_run，M0不宣称完成。advisor先审方案/源码阻断，测量后独立核验结果；阶段提交/tag递增并推送。

## 原联合失败后的独立功能分支（仍不替代联合）

原联合在81秒保护失败、未启动业务后，advisor认可用同一Rust来源独立执行原full30日+实体池，无L1。Python只负责保持池stdin、匹配新child PID/source、传MYCITY_NORMAL_POOL、等待业务再关闭池并wait，异常保留工件不重启。observer包住整个进程树，normal120和原pressure/swap保护不变。runner脚本及真实Rust binary分别hash绑定；observer的binary_sha256此分支是入口runner脚本hash，不能当Rust binary hash。Python runtime hot tracer限1MiB，imports/interpreter/native单列，不能用它签完整共同8MiB。原联合失败不会被该分支覆盖。

独立核验724次全链借还/worker实际槽/完整CPU与Engine/3011票；正常退出Rust实例应为731（原730+池）。若normal不恢复不启动该分支；若通过，只证明无L1的真实功能装配，压力、性能、正式稳态、8h和16GB仍未验。该方案先于分支执行冻结，源码保持dc7115不改。

## Transport repair and same-load functional rerun

The dc7115 functional run failed at grant125 (125 returned,124Filled,two generic request rejections); it is archived separately, never signed as complete. Before editing, unchanged binary prefix-only probe returned rejection at7.64ms before suffix; 2,000 fragmented fake-capability requests produced3 generic rejections. Accepted sockets now explicitly blocking with the original2s read/write timeout. Fixed error categories distinguish timeout, EOF, decode, bound, trailing, IO, source, boot; event logs contain category only, never request/capability text. Advisor requires delayed suffix, timeout, EOF recovery, source/boot process tests. New frozen candidate reruns the unchanged original full business without L1 under the same observer/readiness/protection. It cannot replace the archived joint pressure failure or sign performance/8h.
