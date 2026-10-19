# M0 Rust模型：当前范围、运行方法与验收状态

记录起于v0.5.4，2026-10-19。用户授权从M0-T01按依赖写Rust，并明确本轮只做模型、不做UI。版本tag是仓库阶段发布，不是游戏0.1已经发布；所有阶段验证后提交、递增附注tag并推送。

## 1. 本轮范围覆盖

保留[原M0计划](m0-implementation-plan.md)的模型／空间数据／工具命令／持久化任务；不做原生窗口、地图绘图、渲染／IME／AX／VoiceOver，不引入UI框架。CLI用于构造、执行和导出实验，不作为最终玩家操作界面。T11延期，GPU compute候选及wgpu版本冻结延期；这些不是通过或永久删掉总体目标。T12 CPU参考、T13模型与保存负载可继续实现，T13 GPU／显示联合和16GB门仍单列。

当前机器实际为Apple M4、32GiB统一内存、aarch64 macOS 26.6.2。它只提供该配置证据，不是16GB性能证据；无16GB设备不妨碍推进模型，但不得签收完整M0-T14。

## 2. 阶段登记

| 任务 | 当前状态 | 已验证范围／剩余门 |
| --- | --- | --- |
| T01 | headless子阶段通过，v0.5.4 | Release离线构建／测试／CLI实际运行、输入及规则／源码／锁文件hash、实际机器；wgpu随GPU阶段、macOS14兼容未实测 |
| T02 | 类型／固定表子阶段通过，v0.5.5 | checked单位／余数、typed scope及代际、记录／字节双限，实际探针布局；全域writer／真实领域记录随T03/T04验，整机长跑未测 |
| T03 | 有限实验装配通过，v0.5.6 | 4主体／600Q库存及成本的真实记录、来源唯一消费／引用校验／数量字节门；未推进经济 |
| T04 | 有限易失事务通过，v0.5.7 | 资金保护／钱货原子／带scope容量令牌／幂等与反例；保护款释放仅可信宿主；持久成功未实现 |
| T05 | 静态到期源／日历子阶段通过，v0.5.8 | 20000源分页、P0—P5、暂停／书签／随机；世界事实与游标持久原子提交、工资金额尚未集成 |
| T06 | planned | 持久成功／锁／WAL／根／恢复故障注入 |
| T07 | planned，模型数据部分 | 地形／几何／合法接缝；显示与拾取UI延期 |
| T08 | planned | P-E0/P-E1及完整启动实验 |
| T09 | planned | 同池／配额／退出责任 |
| T10 | planned，CLI／协议部分 | 查询／实验控制／来源；UI延期 |
| T11 | deferred_by_user_scope | 原生UI／绘图／窗口／输入，不算通过 |
| T12 | planned CPU，GPU延期 | CPU数值与恢复门；GPU未启用，无加速结论 |
| T13 | planned模型部分 | 32GiB命名配置可补充测量；16GB／显示联合／8h长跑均未运行 |
| T14 | not_accepted | 不把headless子阶段签收等同完整M0 |

## 3. T01：构建和证据基线

工具链固定Rust1.85.1、edition2021，提交Cargo.lock；直接依赖serde1.0.229、serde_json1.0.149、sha2 0.10.9，完整传递版本和许可见[依赖清单](../THIRD_PARTY.md)。无wgpu／网络／模型API／UI依赖，不运行付费模型调用。首次依赖获取需`cargo fetch --locked`；缓存到位后构建／测试及实验可离线运行。

```sh
cargo fmt --all -- --check
cargo test --workspace --release --locked --offline
cargo clippy --workspace --all-targets --release --locked --offline -- -D warnings
cargo run --release --locked --offline -p mycity-headless -- baseline fixtures/m0-bootstrap.json
```

macOS部署目标候选为14.0，当前只在26.6.2／aarch64本机编译执行，其他OS／CPU及14.0本身均未验；Rust版本不是“当前最新”的声明。`mycity-core`不依赖CLI／主机命令，`mycity-headless`负责读取有限输入和机器诊断。

baseline只校验一个有界实验世界配置，**不推进经济、不生产钱物、不写存档**。输入上限64KiB，拒未知／重复字段、未来schema、零world/branch、非实验世界。输出不同运行ID、稳定内容hash和明确`not_run`门；错误返回非零且不输出成功报告。

构建源码hash覆盖按相对路径排序的`.cargo/config.toml`、Cargo.toml／lock／toolchain、crates与fixtures全部文件；每项依次散列路径长度u64小端、路径UTF-8、内容长度u64小端和内容。文档／证据文件不参加，避免报告包含自己的hash。规则hash是CLI输出rules对象的serde_json紧凑编码SHA256，不包含机器或墙钟；原始输入字节单独散列。代码hash用来对应提交中实际源码，不拿构建前基线commit冒充最终提交；可在tag中按同算法复核。

本阶段Release测试5项通过：已知SHA256向量、流长度限制／溢出、非法配置拒绝、两次独立CLI运行hash稳定且不误报经济／16GB、非法命令／缺输入失败；fmt／clippy通过。实跑报告见[T01 baseline](evidence/m0-t01/baseline.json)。这些是构建／实验入口的本机证据，不是城市模型或16GB通过；没有伪造后续原型的PASS。


## 4. T02：精确单位、代际句柄与有界固定表

Rust金额／物量／能量／水量／时间分别为独立新类型，负值解码和算术溢出拒绝；release也开启整数溢出检查。`Carry`按不可变率保留来源余数，分步与整段可组合，恢复需验证分母／余数，失败不改变旧余数。厘米坐标允许有符号世界位置。编译失败测试确认不能把游戏时间加到金额。

固定表同时检查记录数和实际Vec capacity字节，计入slot、header与free索引；预分配后不因插入增长。raw引用逐一检查world／branch／domain／index／generation，槽复用使旧引用失效，generation耗尽永久退休该槽。该表只接受Copy固定记录，不允许隐藏堆所有权；输入／正文／其他索引另计，泛型表不是全世界所有内存的测量。

Release共12项测试（含1个compile-fail）通过，fmt／clippy通过。`layout`实际填写并读回32768个u64探针：record8B、slot24B、header80B、free索引32768项，owned capacity917584B，checksum536854528。见[布局证据](evidence/m0-t02/layout.json)和[本次进程资源观测](evidence/m0-t02/process-resource.txt)。后者是整个短CLI进程的time观测，包含启动／主机探测，不能归因于单张表，更不是16GB或长时泄漏证明。allocator metadata不在917584B内，报告明确标记；真实财务／库存记录及其索引后续各自测量。

可重跑：`cargo run --release --locked --offline -p mycity-headless -- layout fixtures/m0-bootstrap.json`。对应v0.5.5代码hash按第3节算法复核。尚无异步求解／存档恢复运行实例，未宣称旧异步结果或完整writer门已通过；它们在T04/T06/T12接入。


## 5. T03：有限实验世界装配

[期初输入](../fixtures/m0-e0-opening.json)以分／毫Q登记4个真实账户合计3000000分（30000币）、600000毫Q原料及240000分取得成本。5条明确期初来源分别给现金与非现金权益，来源类型／金额必须匹配且恰好用一次；股票／现金或库存来源不可复制。不允许无所有者库存、重复ID／来源、空库存有成本、超schema／数量／金额范围或未消费来源。只返回完整的新实验世界，不提供对运行世界重复注资API；恢复不能重跑初始化。

本轮暂将实验规模限制为64账户／128库存／192来源、输入64KiB、常驻固定表及索引128KiB。它是可测的小原型边界，不是改小完整产品容量；未来扩大必须重新验证真实字节和索引。输入JSON在64KiB总字节门内解析、再验数组数／关系、再分配领域表；短期解析／观察投影页与allocator metadata未归入表字节，不能拿表字节代表整进程。

当前实际账户record24B／slot32B，库存record56B／slot64B；预留固定表、索引、来源容量和剩余世界头合计11600B。完整实跑来源和布局见[装配报告](evidence/m0-t03/assembly.json)。按稳定ID装配与来源排序，相同输入内容换序仍为同状态hash；world／branch继续参与引用，不把输入顺序当永久身份。

Release共17项测试通过（13单元＋3 CLI集成＋1编译失败），fmt／clippy通过；覆盖输入乱序、重复／错成本／缺主体、额度和未消费资金源，以及CLI装配真实记录。`cargo run --release --locked --offline -p mycity-headless -- assemble fixtures/m0-e0-opening.json`可重跑。输出revision0、经济not_run；没有交易、劳动、在途、生产或存档通过结论。


## 6. T04：有界易失事务、权限和共享容量

单CPU writer先在有界dirty集计算全部账户／库存／容量候选，校验版本／权限／余额／保护／配额后一起替换。每批最多16动作、32触及账户／32库存、8资源／64租约；没有持部分资源锁等待。现金保护不新发钱，保护转实付只扣一次。当前尚无完整来源化保护条款，**ReleaseProtection／PayProtected只接受可信System实验内部调用**，普通账户不可自行解除或改付他人；宿主Caller不从工具JSON反序列化。后续业务层须绑定真实保护来源和受款方，不能把这个内部原语直接暴露给Agent。

容量令牌携world／branch／单调token，跨世界即便数字相同也拒绝，释放后不复用。请求两资源但其中一个不足时整批不占有；已提交的他人租约不可被调度器强行释放。股票／票数／司法不在本阶段，不以System实验调用冒称有真实法定流程。

库存按固定取得批次Q0／C0和原批次offset结转累计差额：`floor(C0*(offset+dQ)/Q0)−floor(C0*offset/Q0)`。内部调货保留原批次和位置，拆3个1与一次3成本一致。每槽目前只支持一个连续原批区间；不同批次或非连续区间合并显式拒绝，需要未来独立批次行，不能重新平均洗成本。新增批次字段使state投影版本改为mycity-m0-state-v2；没有既有运行存档被热迁移，v0.5.6布局证据仍按原tag解释。

1024条固定journal保留成功／经接纳的失败结果及原动作，预留空间满后拒绝新操作，旧回执仍可读。同ID同载荷／同调用身份返回原结果，同ID改载荷拒绝；world／branch隔离，旧revision／错引用／越权拒绝。业务state hash不含拒绝回执，故拒绝可增加journal但不能改变钱货／占用／revision。当前journal只在内存，所有成功明确为volatile_only，不能宣称崩溃后不重付。

实际CLI实验检查保护阻止挪用、支付转换、共享10容量先占8后另一方申请8失败、整批释放及重复操作。见[T04实跑](evidence/m0-t04/transactions.json)；`cargo run --release --locked --offline -p mycity-headless -- transaction-probe fixtures/m0-e0-opening.json`可重跑。Release共29项测试（24单元＋4 CLI＋1编译失败）、fmt／clippy通过，包含advisor指出的保护自解、跨world同号token、拆单成本三项回归，以及满journal、后动作失败不半扣钱货。

该实验引擎owned capacity1934272B，含真实世界表／索引与固定journal／容量状态；整体硬准入4MiB。世界表按模拟域计，journal／诊断及16KiB以内dirty工作页按运行0.45GiB域计，不新增预算、不双计世界表；该数字不含allocator metadata／JSON投影及临时解析，亦未测footprint长时峰值。未来T06持久化／到期事实不能靠永远增长这个journal，应有可恢复覆盖／归档和背压；尚未实现的不算通过。


## 7. T05：同刻阶段与静态到期源

以world／branch绑定静态到期来源，按游戏秒／P0—P5／稳定source排序，每页最多8192项；每个已接受source只处理一次。最多65536条来源，源本体计M-events／既有模拟事件额度，不另开无限未来队列；当前还不支持业务动态增生同刻事件，完整业务调度须后续集成。区间最多1h并在更早事件截断，P0旧区间封闭标记只一次，P5完成前last_complete不前进。暂停不推进游标；页回执含本进程实例与序列，内存重建后旧票不可确认。

书签携来源hash／scope／当前处理时间／最后完整时间／phase／cursor／序列／封闭标记，恢复逐一校验，错world／branch或来源、非法游标／时间拒绝；旧页号不能直接改新实例。书签当前仅验证serde内存往返，**没有落盘或与世界钱物事实原子结合**，不能宣称P-TIME的完整持久门已通过；跨进程异步请求代际仍待宿主／T06集成。

日期采用实验epoch2001-01-01到9999年的确定公历，不读取本机时区或节假日。固定7日／30日与公历月独立，月锚31在短月夹到月末，下一月回原锚；闰年含2100非闰／2400闰。当前验证的是日期原语，尚未运行工资金额／改约／退出／租金应计业务。来源随机按seed／domain／source／window的SHA256派生，不按显示帧／分页／线程抽新随机。

[实跑报告](evidence/m0-t05/clock.json)处理20000个同刻P2源，单页8192，第一批后恢复书签继续，无漏重，最终完整边界3601秒；来源容量480000B，位于M-events额度内，临时排序／hash／诊断页计运行域，未测allocator或长时峰值。`cargo run --release --locked --offline -p mycity-headless -- clock-probe fixtures/m0-bootstrap.json`可重跑。

Release共40项测试（34单元＋5 CLI＋1编译失败）、fmt／clippy通过。完整P-TIME还缺T06的磁盘／故障恢复和与实际业务的共同提交，P-E0/P-E1经济仍not_run；本阶段不把排序计数器冒充实际支付或城市运行吞吐。
