# Meta／legacy保留工作区接入（v0.5.81有限阶段）

权威源码已冻结为 `ece8cc51d70a0a3622f9260771084362389c53fc6cc70f18faf68148d559b5e5`，186文件；完整源码及Cargo.lock见metadata-source.zip／metadata-freeze.json。验收fixture、full opening、原257×257／16bit灰度源和依赖锁保持原SHA。当前所有结果仍按原负载；没有新建target或L1压力副本。

## 实现范围

`StoreWorkspace`拥有实际Engine／Spatial／Metadata backing，正常Sim创建及重开消费现有包。Metadata预建并初始化的heap为23,855,232 B：原16MiB legacy文件、原4MiB Engine、32KiB控制、各64KiB空间JSON／PNG／Opening，1024份完整JournalRecord、1024个operation和8个Resource；header另计。不是以长度上限冒充已分配背板。

原genesis、CURRENT/root和legacy字节数组使用固定writer／严格u8 seed；收据、索引重用固定表。v1写出借用journal而非整份clone，保留原字节格式、容量准入、原子发布、observer、fsync和poison顺序；Index编码仍发生在CP文件发布之后，失败可能留下孤儿文件，不能声称所有编码都在首写前完成。运行时root/CURRENT在WAL、checkpoint v1/v2、容量、来源注册、背景资产及worker缓存费用路径复用small backing。

Store实际owned计账补入两份保留准备工作区、Metadata所有heap及checkpoint引用字符串。历史3root／5generalAir保守目标表另显式计入新Metadata，因此超过原64MiB，不再沿用旧fit结论；这张表还含旧重叠，不是当前双root完整角色的授权账。没有提高64MiB或共享8MiB标准。

## 兼容性修正与开发证据

Advisor发现“每份legacy原收据文本≤32KiB”的做法会拒绝旧有效空白／可忽略字段，已移除该收缩：legacy继续受原16MiB整文件边界约束。新receipt decoder以借用字段、两遍标签和流式标量校验替代内部标签Content树。保留普通Record未知字段忽略、unit map忽略、struct严格字段、重复标签／已知键拒绝、标签末尾和序列形式。不是Value规范化，重复键不会被覆盖。

`development-checks.json`、`receipt-direct-v3-result.json`、`next-development-checks.json`、`cold-open-development-checks.json`保留实际开发结果。最初两个oracle运行因新测试的enum拼写错误而失败，原日志保留；修正为原snake_case后通过。首轮严格clippy发现13处多余借用，修正后exit0。storage整批tests实际exit0；新完整CP／index重签后大收据与位置式头冷开、全Engine和原收据／重试／背板对照亦实际通过。开发结果不代替最终同源全仓验证。

## 未验收边界

仍有Opening／控制解析和转义字符串scratch暂态，legacy在不同嵌套解析器上的深度上下文也不能从当前对照推导为全部非法输入接受／拒绝完全等价。大转义key或unit忽略字符串可能使多份serde scratch同时存活，**仍是EXTRA**。固定收据字段表不证明整个解析峰已固定。下一步须以固定词法扫描增量识别key、验证／丢弃允许忽略的长字符串，并区分旧Content与IgnoredAny的标量语义，保留原有效输入范围；不得恢复一刀切token上限来制造适配。

实际固定CLI输出actual-conservative-plan.json：保守旧拓扑77,934,872B > 原67,108,864B，target_slots_fit=false、runtime_grant_accepted=false；不是对实际双root拓扑已经最优化的结论。

目前broker仍是原IO4MiB与worker5MiB权限，模型／Meta不是完整grant。必须建立实际Sim／Reference／Compare同时存活清单、解决上述解析暂态，再分配前授权整个角色、Filled核对全部真实背板和真实Child::wait后返还。

同源全仓测试实际exit0／575.134秒，54组harness／410条通过记录（含嵌套子进程，不是410个独立业务样本）；严格clippy、fmt及build均exit0。固定新helper通过旧完整world normal prepared冷开：序列3013、完整Engine 2,507,418B与原独立reference逐字节一致，原件及兼容副本各243项SHA未变，所有预建core／spatial／Meta backing身份保留。实际原full30日Sim/worker已终态exit0，整段355.824秒，Sim含IPC及持久化297.379秒：720小时目标、723次drive、724真实worker、64工资／20,000债项／16周期义务／8实际交付，完整业务和空气与原独立oracle对齐。完整冷开0.170752秒、序列3011、原票重试无副作用；日志与完整新world保留。

固定v79旧reader及本版reader均实际读取新v1 writer原文件成功：序列2、完整Engine1882B、SHA一致；见new-writer.rs、metadata-old-reader-new-output.stdout、metadata-new-reader-new-output.stdout。没有以新reader自读替代旧reader运行。

730条成功Rust heap记录见heap-summary.json／原jsonl：model3个最大请求峰加64KiB尾预留16,502,739B，Sim2个58,120,089B，worker724个5,980,239B，driver614,546B。它们是各进程生命周期请求量上界观察，不是同刻物理总量，也不包含栈／native／mapped／foreign，不能相加签normal或16GB。源码及固定exe终态逐字节／SHA复查见source-terminal-integrity.json。

[Advisor独立最终审查](advisor-review.md)未发现阻止该有限阶段发布的证据阻断，未替主线程重跑负载、未自行重散列旧world；实际核查范围与限制分别保留。

正式L1+L2性能、normal64/common8、四档／分级稳态、真实28800秒8小时均未由本阶段通过；真实16GB保持not_run，完整M0未验收。

## 磁盘与恢复依赖

见stage-retention-intent.json：只复用共享target，新增实际业务world和小型v1兼容world各有用途和解除条件，不额外复制L1。现有三份完整布局、旧业务兼容原件、v79业务输入、无损组件证据及固定源码／lock／exe继续保留。终态全部退出且work／target无占用，见retention-result.json：本阶段可确认新增删除0B，无独立target／无新L1副本。work15,744,376KiB／target1,301,208KiB；两个新输入目录分别91,752KiB和44KiB，固定exe目录21,316KiB，按预先声明依赖完整保留。检查窗口df观察减少122,880B，不归于清理或APFS独占回收。没有退役当前恢复输入。
