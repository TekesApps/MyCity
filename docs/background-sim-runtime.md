# W3：独立 Sim、监督与恢复边界

发布v0.5.28，承接[冻结验收](background-compute-acceptance.md)、[纯worker](background-worker-runtime.md)和[城市接纳](background-store-runtime.md)。本阶段只实现Rust模型／CLI进程链；W4联合1800秒运行、全进程组资源及最终advisor联合审查仍须单独实测，16GB门仍未通过。

## 1. 运行角色与可重跑入口

`mycity-headless sim-host create|open WORLD`是独立Sim；`sim_client::Client`是宿主监督适配器。宿主通过管道提交有界命令及取得进度，Sim独占Store写锁，独立`compute-worker-bounded`只读取固定输入和产生候选。宿主不打开活动世界写入；实验探针只在Sim退出后重开Store核验持久状态。当前实际进程控制依赖Unix的ps／kill，macOS已测；不宣称Windows进程监督已实现。

`sim-probe NEW_DIR`是Rust宿主实验入口，创建八个有限实验世界、记录实际控制命令／事件JSONL和完整报告。它不发起后台自主日历推进。`sim-host-probe`和`compute-worker-bounded-probe`额外提供ACK屏障，仅用于确定故障注入位置；正常入口不等待这些ACK，普通Sim拒绝Probe命令。

控制帧至多64KiB（含换行），使用fill_buf／consume有界读取；每个Client或Worker读队列至多4帧，仅一个命令／任务在途。序列化结构／临时副本并非只占原始帧字节，完整进程组RSS留W4测量。宿主命令包括Status、Execute、RunRequired、Checkpoint、Stop。外部Execute不能使用内部D／E操作域，也不能直接执行后台生命周期控制，只有Enable可用。这个可信实验宿主不是Agent权限接口。

正常worker每次消息等待最多30秒、最多4个进度帧；尝试最多3次。观测超时本身不表示Sim已退出，宿主保留同一Child句柄检查；必要时显式停止／回收后恢复，不能因一次读超时并发重开第二个写入者。Probe ACK的人工等待不纳入正常超时承诺。

## 2. 原子租约与恰好一次接纳

`RebindLease(job)`在一次Engine／Store事务中增加incarnation、generation及attempt；允许接替此前仍标running的任务，保留固定输入／窗口／场版本。没有Restart已提交但Lease未提交的两事务恢复缝。incarnation是执行代际，并非OS PID；代际提交后、派发前再次崩溃也已消耗一次预算，重开不能清零。InvalidInput或三次耗尽后任务保留，P4继续阻塞，不增加第四次自动尝试。

每个job的最终接纳operation ID固定为`0xe000000000000000 | job`，job为非零低60位；接纳前限制范围并检查溢出。校验双向成立：保留ID只能接纳对应job，所有Accept也必须使用该ID。错误actor／动作／job、旧generation、缺候选及过期revision不会写失败回执占用最终ID。成功记录照常进入WAL／checkpoint／冷热回执索引。

宿主只需重发RunRequired(job)：先读原最终回执，存在则返回原JournalRecord，不用当前revision或generation重建Envelope。根提交成功而回复丢失时不再Lease／计算／转移钱物。内部生命周期使用D域和受检序号；Core限制该域只能由System提交单一后台生命周期动作，Host不把它开放给Execute。

## 3. 缓存寿命、持久身份与OS栅栏

不能凭旧Client内存中的PID清理重启后的缓存。每次尝试先停止并wait当前子worker；新Sim另从顶层`air-worker-owner.json`读取pid／原Sim／可执行路径／缓存路径，检查进程角色、父PID及路径后才停止旧worker，不能向猜测或已复用的不同进程发信号。

Sim随后取得顶层`air-worker.lock`的fs2独占锁。锁文件永不删除／替换，与可丢弃缓存分离；SIGKILL异步送达后仍须取得锁才能清理。Sim持锁完成reset、输入发布、spawn及owner登记（临时文件→fsync→rename→目录fsync），登记成功之前绝不向worker派发请求，之后释放锁再派发。bounded worker在每个请求计算前取得同一锁，覆盖输入检查、计算、候选发布直至Complete。原宿主未消费worker_started事件或已经退出，也不影响新Sim恢复该栅栏。

未登记的子进程收不到任务，stdin EOF会退出；已经派发的工作有持久身份。身份记录丢失而旧writer仍持锁时安全阻塞，不能清缓存强行继续。必要的OS信号／进程状态未知或锁超时同样保留任务；它们不是接纳成功。

每次重试（包括同一job换PID）都在栅栏内清理旧输入、候选和.pending临时文件，不只换job时清理。清理先有界预检全部文件名／类型，未知文件或symlink拒绝，不删除用户文件。权威Sim资产仍是Store顶层air-result文件，与IPC缓存分开；不能为清缓存删除已提交恢复依赖。

## 4. 磁盘配额与内存边界

W2每个必要窗口预付10MiB资产／缓存加16个45120B控制页。W3首次使用worker时，同一Root提交把其中6MiB转入全局`worker_cache_bytes`，相应background预留扣6MiB并消耗1个元数据控制页；world revision和业务sequence不变。以后任务继承已付6MiB，额外只预留4MiB加控制页，不能每窗口重复领缓存。

capacity.used_bytes现在是顶层实际文件字节加持久缓存charge；同时单列worker_cache_charged_bytes和worker_cache_actual_bytes，后者仅作有界诊断，不重复加进used。缓存删空／缺失后仍扣6MiB，权威模型可重开，缓存从固定输入重建；不能靠删除缓存逃避配额。

单输入／候选至多2MiB，整个air-worker目录（含临时文件）最多6MiB。新发布按最坏的临时名和最终名两份字节预检：一个2MiB输入加2MiB候选及2MiB发布临时名恰为6MiB；硬链接同inode仍按两份保守计峰值。最多两个已知子目录，每目录至多8个文件，超限不继续写。owner、锁及元数据临时文件在世界顶层实际计字节，由有限控制页的余量承担，不能藏在6MiB之外免费增长。

这仍是有限逻辑配额，不是OS fallocate或物理断电保证；ENOSPC／写入结果未知须重开判定。已有预留正常空间耗尽测试增加首次缓存转账，验证已有义务可以继续；新义务仍须完整正常准入。Rust额外帧／reader／CPU复核副本和OS文件缓存没有被这6MiB磁盘额度证明，原16GB门未通过。

## 5. 实测范围与未完成项

独立进程测试覆盖唯一写锁、真实货运输入、原子换代；源阶段两个实际货运源分成1条一页，杀在cursor=1后，Cells杀在cursor>0后，替换worker并返回相同完整场。W1另有更大源集合中途被杀证据，不能把本阶段两条源说成大负载。

Sim测试从外部直接SIGKILL，保留旧Client但不调用其worker清理辅助，再用全新Client恢复；注入租约提交后未派发、worker网格页、Sim候选已持久发布但根未提交、根提交后未回复。接纳后checkpoint再取同一原票。取消一次仍保持P4及任务，可手动重试；连续三次取消后不增加第四次。取消注入目前通过显式实验ACK控制，不宣称产品级异步取消接口已完成。

`sim-probe`八个分支比较完整空气State检查点摘要和规范化revision后的完整业务WorldView摘要；失败代际／回执历史允许不同，不能把它们归零伪称完整Engine一致。其数字对照仍是相同CPU算法及Core全量复算，不是独立算法或物理标定。探针只到30游戏秒，两条货运仍在途；工资税债、完整交付及长时间运行是W4的实际工作，不能用本阶段通过替代。

advisor在新增实现中指出并修复了“旧worker复用缓存竞态”和“普通Execute提前占用D域”两项阻断，随后定向只读确认修复成立；故障与测试证据由本地实际运行给出，不能把只读意见当作其独立跑测。当前[阶段证据](evidence/background-sim-w3/gates.json)已通过，工件与实际复跑由[核验脚本](evidence/background-sim-w3/verify.py)检查；W4完成前整个连续目标保持进行中。

存档与owner记录本阶段按原绝对世界路径恢复；路径不匹配时安全暂停，未提供移动存档时的活worker接管协议。历史阶段的证据与规则仍绑定对应源码／tag，不将旧v0.5.27普通Accept回执静默迁移为新固定ID。

发布核验：227项Release测试、fmt及clippy通过；冻结源码`b31269a45648b77f51c8d5e9102900b1137921dbdcf645525e243974371dbeb6`，八分支探针实际5.003868秒，121文件归档207823B，核验器已独立复跑。该短时探针不是1800秒耐久或性能门；阶段结果见[checks.json](evidence/background-sim-w3/checks.json)。
