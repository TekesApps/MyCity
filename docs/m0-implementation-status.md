# M0 Rust模型：当前范围、运行方法与验收状态

记录起于v0.5.4，2026-10-19。用户授权从M0-T01按依赖写Rust，并明确本轮只做模型、不做UI。版本tag是仓库阶段发布，不是游戏0.1已经发布；所有阶段验证后提交、递增附注tag并推送。

当前发布v0.5.23增加[T12-A纯数值参考](m0-air-cpu-reference.md)，尚未写入城市世界；业务协调器为v0.5.21的[T05-B](m0-timeline-runtime.md)接入当前实验profile的真实来源、日历及持久游标；Region／Boot／Exchange／Payroll可联合，旧[T08经济](m0-economy-runtime.md)仍为独立profile，不能与区域重复计劳动。后文按阶段保留，旧not_run为当时状态；有限通过不能扩大为全部M0、真实公用网或16GB通过。

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
| T05 | T05-B有限profile联合已实测，v0.5.21 | 当前真实来源、P0—P5、日周月劳动／金额、钱物与游标同根恢复、有限续做票；全产品工资税表／假期／未来来源仍未实现 |
| T06 | 事务持久子阶段通过，v0.5.9 | 显式提交边界／单写锁／WAL／完整检查点与冷原票／进程恢复（后续v0.5.16—18）；当前profile时钟事实同根见v0.5.21；物理断电未测 |
| T07 | 空间／地图子阶段及不可变资产存档通过，v0.5.12 | 原生灰度、厘米几何／步行接缝；期初空间与模型同根恢复；真实搜索／车辆坡度／运行时空间修改／UI等未测 |
| T08 | 三项有限实验通过，v0.5.14 | P-E0／P-E1及完整P-BOOT有限输入实验；Boot与Region共钟见T09，完整行业经济联合／真实公用网／交通仍未验；当前profile执行器见T05-B |
| T09 | 实际履约、收件背压／追收／连续性有限子阶段通过，v0.5.15—18 | 三城共池、交付费用／损耗、持久收件／应急退出及30分钟连续实测；永久断链、破产、真实车辆／潮流水力、具名权益及完整P-CAP／P-XMAP仍未验 |
| T10 | T10-A有限CLI已实测；完整协议未完成 | 暂停／单步／四档实际等待、分页／热冷原票；当前profile接入T05-B step；目标对象revision／UI仍缺 |
| T11 | deferred_by_user_scope | 原生UI／绘图／窗口／输入，不算通过 |
| T12 | T12-A空气CPU有限参考已实测，v0.5.23 | 单通道32m EI、源／tile分页、任务检查点与新进程恢复、独立f64对照；未接世界／真实源投影／阈值，GPU与worker延期 |
| T13 | 有限模型30分钟连续性实测，完整联合未验 | v0.5.18取得32GiB宿主有限实验RSS与冷档案增长；真实16GB大城／显示联合／8h长跑仍未运行 |
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


## 8. T06：先验证事务持久提交与恢复

新增Rust `mycity-storage`，只依赖core／固定serde／SHA256／fs2锁，无UI。core重构为prepare→可信存储hook→CPU apply与journal发布；hook收到冻结原命令和固定结果，不重算业务，不成功则CPU不发布变化。core的model receipt继续标volatile_only；**Store返回的has_persistent_receipt才表示该原操作被可靠提交边界覆盖**，不把仅调用core当持久成功。

有限协议：新目录／单写LOCK→固定genesis（原有限来源及规则身份）→有长度／序号／前链／SHA256的逻辑WAL→同步WAL→写双槽提交根并同步→CURRENT绑定根hash／最新sequence／wal_end并同步目录→CPU apply→响应。逻辑WAL重放冻结命令，逐条校验原结果与幂等覆盖；CLI将实际代码内容hash作为规则身份，版本不匹配拒载，未实现自动迁移。创建新世界不能覆写既有目录。

恢复必须验证CURRENT、最新根、genesis及全部已提交WAL链与结果；任何提交区间内的截断／坏checksum／缺依赖／不兼容拒载，不能用旧根吞掉已确认操作。只有验证边界外的字节可截除，已知未引用原子临时文件可回收；任意用户文件不删。两个根保留完整前缀依赖，是有界重放恢复点，**尚未实现独立压缩数据页检查点／长时日志合并与任意历史加载**。当前frame≤32KiB、WAL≤64MiB、genesis≤512KiB、metadata≤4KiB、journal≤1024，在原运行／磁盘额度内准入，达到上界拒新，不能继续运行无限增表。该实验更紧的限制不修改全产品20GiB目标。

I/O返回错误可能已提交，Store进入RecoveryRequired，禁止继续读作当前完整状态或修改，必须重开验证并按原operation查询。当前写者生命周期结束显式best-effort解锁，记录创建PID，只由原进程解除自己的锁，Drop不补写／不发成功；锁FD不clone、不向子进程移交。活跃写者期间第二进程仍明确Locked，未给锁冲突添加自动重试。

实测与检查：

- [实际存储往返](evidence/m0-t06/storage.json)：保护／现金／容量共同提交两批，关闭／重新打开，业务状态、journal及旧ID重试一致，sequence仍2。
- [Release测试记录](evidence/m0-t06/test-transcript.txt)：52项通过（35 core单元＋6 CLI集成＋10存储集成＋1编译失败），fmt／clippy通过；存储测试含11个位置的独立子进程exit(97)，覆盖准备、WAL分段、同步、根、CURRENT、CPU应用和响应后；重开核对revision／现金保护／容量／journal，已提交重试不重复支付。
- 测试还覆盖第二进程锁、已提交损坏／缺依赖拒载、不兼容规则拒载、边界外残尾、注入ENOSPC后的隔离、超限命令在序列化前拒绝且WAL／状态不变、临时文件与用户文件区分。
- advisor定点检查要求补齐“序列化前动作门”，已加回归。重复测试发现过真实的立即重开Locked失败，记录保留于[修复前输出](evidence/m0-t06/lock-failure-before-fix.txt)；并发spawn继承描述符是合理解释而非已证实根因。增加显式生命周期解锁／PID约束及100次重开同时100次spawn测试后，[固定5次恢复套件复测](evidence/m0-t06/lock-retest.json)通过，不隐去失败。

可重跑（world目录必须尚不存在，父目录已存在）：

```sh
cargo run --release --locked --offline -p mycity-headless -- storage-probe fixtures/m0-e0-opening.json /tmp/mycity-m0-world
cargo test -p mycity-storage --test recovery --release --locked --offline
```

以上仅是本机进程退出／故障注入，不是断电／文件系统损坏后的数据保证；ENOSPC来自注入，并非灌满真实磁盘。调度bookmark与钱物同一提交、真实日周月应计与到期、1024之后的内部覆盖／归档、全场景输入、长期恢复增长、8h压力和16GB门仍未通过。M0-T14继续not_accepted，P-E0/P-E1完整经济执行仍not_run；不能为推进进度将未做项改为PASS。


## 9. v0.5.10：区域快照与交互账本子阶段

设计先按用户新要求改为[R54](regional-snapshots-and-interaction-ledger.md)，替代旧全城共同本地发展钟。单轮advisor指出事件到期顺序和履约磨损，已补设计；本轮按到期／到达边界截段，关闭城设备运行与磨损仍待实现，不能借审查签收。

Rust在现有World内增加固定区域表；所有区域动作走原Engine prepare／持久hook／apply与Store WAL，不复制另一套账户／库存或8个Engine。输入须显式带mycity-m0-region-v1，旧独立装配仍无区域；观察schema升级mycity-m0-state-v3。Store规则源码fingerprint变更拒绝旧规则保存；没有历史存档自动迁移。快照只存观测／版本与已消费水位，当前事实表随交互立即更新；切换更新投影水位，不再执行现金或人口delta。

固定上限8城／16条有向人口通道／16在途批／16义务，每命令最多16动作、每advance最多3600秒，沿到期和到达切有界子段。合同按ID稳定尝试支付，保护款不挪用；不足留原欠款，到期不续约；后续真实入资可显式RegionSettleDebt。输入中的固定秒周期不代表月薪，匿名同属性人群不代表具名家庭权益迁移。发现按最短米制距离排序，未实现完整偏好／市场撮合。通道整旅途保守预留，合成实验无运输费用或实际车队，不作真实交通／经济证明。

实际[区域报告](evidence/m0-regional/region.json)：三城总4400人，旧城development_elapsed=3153600000秒，新城从0仅前进30秒，区域交互30秒。2000意向、1998实际、2缺额，经3000m多跳／20秒在途后目的城2098人，源城2002，中间城300；未借快照再次补人。有限义务原公司600000分支付耗尽后留600000分欠款，世界现金仍3000000分；换城和存储重开／幂等重试不重复支付／迁居。缺额不是已知住户、钱或货的舍入。

布局实测Region内联4640B、World owned capacity20336B、Engine含1024预留回执共2078144B，分别在World128KiB／Engine4MiB原门内；临时解析／视图／allocator metadata不含其中。完整产品域内增量划分见R54，不能从这个小表推出完整16GB通过。

[Release测试](evidence/m0-regional/tests.txt)共59项通过（35 core单元＋5区域集成＋6 CLI集成＋12存储集成＋1编译失败）。新增覆盖多跳／瓶颈、缺额守恒、休眠欠款、换城水位、分步与整步应付结果、权限／溢出／无效来源、持久hook失败和整批回滚；另在11个真实子进程exit(97)边界核对区域钟／人口／现金／欠款／幂等回执全有或全无。fmt与[clippy](evidence/m0-regional/clippy.txt)通过。进程退出不等于物理断电证明。

重跑（最后的存档目录必须不存在）：

```sh
cargo test --workspace --release --locked --offline
cargo run --release --locked --offline -p mycity-headless -- regional-probe fixtures/m0-region-opening.json /tmp/mycity-region-fresh-world
```

[短进程观测](evidence/m0-regional/process.txt)在本机M4／32GiB取得，仅是此合成CLI命令及宿主探测／保存恢复的时间与资源观测，不是城市吞吐、长跑或16GB设备证据。P-E0/P-E1、P-XMAP真实全场景、完整P0—P5世界游标检查点、经济平衡、闭城生产／磨损、电水流、真实货运、具名迁居、16GB联合负载仍未通过。下一步回到T07空间／T08经济装配依赖，不继续堆区域审查故事。


## 10. v0.5.11：T07空间与地图子阶段

当前实测及边界统一见[T07说明](m0-spatial-model-and-gpu-layout.md)。新增独立city-map不可变空间模型，不更改经济或区域账。已实际导入灰度8／16位、三档目标及4097²源，检查缺测／水域／固定三角表面／完整平台、矩形孔／1cm侵入、曲线保守胶囊／步行门接路、两图真实接缝及净宽接近区。曲线／门／平台支持子集有界，复杂输入返回Unsupported／NeedsDetailedValidation，不借层标签豁免碰撞。

按用户补充固定wgpu交换契约：64B头、SoA f32地形／u32标记、16B局部顶点、内容版本／job generation及CPU最终精判。76项Release测试和fmt／clippy通过，包含CPU打包／误差／陈旧标识，**无shader编译／Metal／DX12后端运行**。空间场景Terrain594968B＋Scene17617B；布局门和导入预检来自原几何／运行域，完整16GB联合预算仍未验证。

[T07实际报告](evidence/m0-t07/spatial.json)、[测试](evidence/m0-t07/tests.txt)、[clippy](evidence/m0-t07/clippy.txt)、[短进程观测](evidence/m0-t07/process.txt)。报告含实际源码／锁／输入／规则hash及M4／32GiB机器；未实测不算通过。空间还未接Store提交根／实际建设权限与经济消费；P-IMPORT搜索／投影等、P-XMAP完整路线及T07全验收保持未通过。原有存储测试不升级为空间恢复通过。


## 11. v0.5.12：期初空间与模型共同恢复

当前边界、文件格式、故障结果与完整证据见[空间存档说明](m0-spatial-storage.md)。Genesis v2将固定空间输入／PNG的长度及hash、派生CPU地形／几何摘要绑定原模型opening与同一提交根；恢复使用同一份已校验缓冲，空间错配拒开，发布结果未知必须重开而不再初始化。v1非空间存档仍严格按原语义支持，不能把v2缺空间降为v1。

[本轮实跑](evidence/m0-spatial-store/store.json)完整Store owned2691560B，含Engine2078144B与空间资产612663B；排除allocator／临时恢复工作集，不能当整个应用或16GB验证。86项Release测试、fmt／clippy通过；新增13个创建与11个带空间交易的真实子进程退出位置、I/O注入、空间损坏／丢失／身份／派生规则反例。空间、资金及区域状态同根重开，重复回执不重复支付／迁人或注资。

本轮只保存不可变期初空间，不代表产权／工程施工、运行时地形修改或完整交通路径；T07仍有搜索／投影／车辆等剩余门。T08 P-E0／P-E1、完整检查点压缩、物理断电／真实满盘、16GB联合负载仍未通过。后端兼容交给wgpu，本轮没有另做后端适配；下一主线进入T08最小钱物活动，不继续扩本专题复核。

## 12. v0.5.13：劳动、生产、消费、工资税债

[T08运行契约](m0-economy-runtime.md)和[输入](../fixtures/m0-economy-opening.json)已落Rust。采购在途→到货→8小时劳动→受保护工资及工资税→实物转换→整单接纳／拒绝→产成品在途→实际消费与收款／成本→累计利润税应计→day30缴纳，均经原Engine和Store事务；没有按天直接赋终值。正式市场／UI／AgentKit未加进来。

P-E0实际推进30日，G/F/C/S现金8080/560/8160/13200，总30000；S余300Q、消费F180/G120Q、企业与在途余货0、税清零。P-E1独立变体实跑低现金拒单及带成本余货、20Q耗尽无新工薪、旧债部分清偿后登记移交退出、预付只退未履约部分和退款现金不足；恢复／重试不复制权益。另实测停工保证工资费用化、累计税下调及月末缴税现金不足保留税债。财政与家庭在P-E0持续吃本金，明确不是长期均衡。

全仓库Release离线测试102项通过（含1个预期编译失败测试），fmt与clippy零警告通过；[原始输出](evidence/m0-t08/tests.txt)、[CLI实际报告](evidence/m0-t08/economy.json)、[clippy](evidence/m0-t08/clippy.txt)。采购、交付、退款、day30税四类事务各11个WAL／根／响应退出点，共44个真实子进程退出；恢复验证旧／新完整状态及重复操作只入账一次。另注入CURRENT已改名后的IO不确定，原writer隐藏状态，重开和重试保留唯一实际采购。不是物理断电或真实满盘保证。

advisor完成一轮只读机制与代码核查；直接劳动资本化、旧批FIFO、累计净利润税以及业务拒绝继续／IO失败暂停已对齐。其建议的在途、部分退款、税前税后恢复已补实测，无新增阻断后收口，不反复审查旧专题。

T08只签收本轮P-E0／P-E1有限子阶段：完整P-BOOT泵／机组互锁、公用网／真实运输共池、T05全源执行器与Region双钟统一、日周公历月工资完整P-TIME、真实16GB联合压力／长跑仍未验。当前拒绝经济与Region或generic Resource同时启用以避免假联合；这道拒绝只是安全边界，不是整合已完成。后续先补T08剩余完整P-BOOT，再接T09真实交互，不能跳过依赖签收M0。

## 13. v0.5.14：P-BOOT启动输入、同池容量和故障恢复

[P-BOOT契约](m0-boot-runtime.md)采用显式pboot-cold-micro-v1独立实验，而非改写LC38煤机／泵生命周期。真实电池／水箱引导，有限燃料／源水，60秒未到期不产出；作业与generic Resource同池，按稳定键认领两个租约。Start同时为正常和最坏故障预留接收空间；资金、WIP、费用、输出接收、时钟及lease都走原事务／WAL。

实跑无启动存量的两设施都拒绝、有限电池先泵后机组、真实清水箱的替代引导；逆序争同两池无半份锁等待，旧失败ID不因释放而变成功，新ID才能再试。实际投入后Fault保留服务费及损失，转真实有界废水／热接收；ClearFailure只释放工作租约，接收物／损失／累计保留；新作业还需新投入。正常两轮完成后燃料3Q、电池700Wh、清水1080L、源水4800L、废水120L、热1400Wh、实付10币，与初始5Q／100Wh／6000L守恒。

真实进程退出覆盖两设施Start、Complete、Fault、ClearFailure各11个WAL／根／响应点，共88次；每点恢复完整旧／新根，再以同operation重试验证只入账一次。另测CURRENT已改名的IO不确定时隐藏writer，重开恢复与不可变空间共同根，半程WIP／租约继续且费用不重付。advisor完成一次方案澄清及一轮最终只读代码核查，无新增阻断；其没有执行测试，实测以本仓证据为准。

本轮补齐计划表P-BOOT有限原型，T08的P-E0／P-E1／P-BOOT均已执行。完整真实设施生命周期、连续电水网、T05全源执行器与Region统一、P-TIME全日历业务和P-MEM／P-LOAD仍是独立未验门，不签收全部M0。后续依赖推进T09真实区域交互与容量来源整合，避免继续堆本专题复核。

本轮[Release原始输出](evidence/m0-t08-boot/tests.txt)118项、[fmt](evidence/m0-t08-boot/fmt.txt)、[clippy](evidence/m0-t08-boot/clippy.txt)通过；[实际探针](evidence/m0-t08-boot/boot.json)与[进程观测](evidence/m0-t08-boot/process.txt)有源码／输入／锁文件／规则fingerprint。Boot448B内联一次，World26608B／Engine2084448B／带空间Store2697862B；本机M4／32GiB短测RSS8798208B。它们按模拟／运行时／几何原域计，不重复加父子owned，也不据此签收16GB。源码指纹及独立算术核对见[证据核对](evidence/m0-t08-boot/provenance-checks.txt)。

## 14. v0.5.15：区域实际履约与同池资源

[T09联合契约](m0-regional-fulfilment.md)把三城／Boot、货物及电水交付、人口通道、已有分期义务与共享资源接到同一交互推进器。明确账户／保管位置、三城厘米坐标及类型链，单一有限地形模板绑定同一Store根；关闭城只有既有作业继续，不自主新开生产。发运真实出库到唯一在途，预付运费、保护货价与目的空间，交付才确认货款／购入成本／源成本及损耗。原批次凭证只读保留，不复制库存；型别资源绑定及生成lot命名空间有反例门。

实跑正常三种多跳交付与切图、1998人／货物双向共池争用、Boot与电交付双向争用、电／水各自断链／跨截止后走余程、满4在途槽拒第5、旧合同过期后保债及真实部分偿付。正常G1/C2/C3/S4现金10000／2006.46／5967.94／12025.60，合计30000；1Q货、98.01Wh电、98.01L水到达，两个1.99损耗分别有接收终点，源／目标／在途／WIP全量与成本对齐。1204秒交互后只有活动城2地方年龄增加1204；旧城不追赶，不重复加delta或注资。

[134项Release测试](evidence/m0-t09-fulfilment/tests.txt)、[fmt](evidence/m0-t09-fulfilment/fmt.txt)、[clippy](evidence/m0-t09-fulfilment/clippy.txt)过；新增121个真实进程退出点验证货／电／水发运和结算、关城作业完成／故障、断链／修复及切图同根恢复、同ID只生效一次。到达IO不确定亦隐藏writer并由重开确认；不是断电保证。[实际探针](evidence/m0-t09-fulfilment/exchange.json)与[独立指纹／算术](evidence/m0-t09-fulfilment/provenance-checks.txt)对应本源码。

Exchange内联3808B／Boot472B／World33320B／Engine2091224B／空间Store2704637B按原域归属，不重复加父子容量。M4／32GiB七分支短测RSS20316160B；16GB联合峰值及真实吞吐仍未测。P-CAP本轮只签自愿满槽背压及既有责任／收容保留，非自愿收件箱／满箱停钟／预留退出仍未实现；P-XMAP仍缺车辆、真实潮流水力、具名金融权益与全行业／环境／日历联合，不能将本轮标成完整T09或M0通过。下一主线补P-CAP及持久覆盖增长。

## 15. v0.5.16：完整检查点与持久回执轮转

[检查点契约](m0-checkpoint-and-retry.md)接续T09持久增长缺口：完整World／allocator／runtime／容量状态专用恢复，512条热窗口轮转，磁盘哈希链保留成功和失败原回执，独立发布generation避免空检查点覆盖根。advisor的一轮只读核查发现轮转阈值处热重试可能被当作新操作，已修为先识别原热记录再轮转并增加边界测试。探针操作ID改从持久sequence派生，不能依赖会归零的热日志长度。

本阶段测试及边界见[原始输出](evidence/m0-t09-checkpoint/tests.txt)；不据此签收完整P-CAP、30—60分钟组合持续运行或16GB。下一阶段继续事实收件、应急容量与退出、分页查询。

## 16. v0.5.17：事实收件、应急背压、独立追收与分页

[本阶段契约](m0-fact-inbox-and-emergency-exit.md)实现合同内预留Delivery、真实到达一次登记处理费和Incident责任；新增显式注册来源的持久pending→World→确认协议。来源事实满箱不确认／不写死失败，重开需来源frontier对齐才继续区域推进；已注册文件缺失／损坏／跨scope拒载。普通4门限、工作12、独立追收16、来源pending16的边界分别有限，外部接纳不能占掉已有运输Incident预留。

已收到未付清的责任可原样移交独立追收池；取消／修复／交付／清理／真实部分支付／释放不清债或生成资源。普通磁盘配额与8MiB应急预留持久化；应急资格由授权成功且实际推进已有责任的Prepared决定，拒绝动作标签、失败、空操作及重复开关旁路。普通检查点不能花应急区。CLI分页合同、履约、欠费、资源租约、阻塞及恢复状态、热／冷原凭证，写入／来源变化后旧游标拒绝。

[Release证据](evidence/m0-t09-inbox/tests.txt)与[clippy](evidence/m0-t09-inbox/clippy.txt)分别登记；真实30—60分钟组合运行另行交付，不将短测或模拟时长算通过。来源真实性、永久断链实物处置、破产减记及完整企业损益是明确后续边界；本轮不签全部T09、M0、UI或16GB。

本阶段[155项Release测试](evidence/m0-t09-inbox/tests.txt)、[fmt](evidence/m0-t09-inbox/fmt.txt)、[Clippy](evidence/m0-t09-inbox/clippy.txt)及文档检查通过。另以同一冻结Release库执行[独立Rust探针](evidence/m0-t09-inbox/retained-capacity-probe.rs)，[实测填满16条追收记录](evidence/m0-t09-inbox/retained-capacity.txt)：第17次移交原子拒绝，实际支付1分并归档后腾位，新请求移交成功，旧失败ID跨检查点／重开仍拒绝；未偿16分＋已付1分＝应计17分，现金3000000分不变。新回执按动作保存创建事实ID及实际支付额，归档后可直接连接来源和各次结算。

回执新增两组固定数组使1,024条热窗口增加512KiB；收件／追收数组和Root字段另增若干KiB，当前含空间Store探针owned约3.09MiB。所有城市共享这一World／Region和热窗口，不能按每个地图重复计量；磁盘冷段及暂存另计。它不是全城规模或16GB峰值通过，长跑RSS／日志增长待后续独立证据。

## 17. v0.5.18：本轮六包实际验收

[六包验收](m0-continuity-acceptance.md)完成：冻结v0.5.17源码后，实际工作循环1800.456601秒，6091循环／48895提交，121次真实子进程退出与367次控制器重开／恢复；不是模拟游戏时长或短测拼接。只读分页扫描全部回执，35条事实／支付／移交轨迹可直接追溯，独立复算现金3000000分、未偿3305000分、实物去向、资源释放及各城年龄增量。查询不改变状态hash或sequence。

当前档案总43809176B，热回执／活动WAL结束时均已轮转，历史凭证没有清空。循环owned3234083B，恢复阶段固定多32B；命令报告峰值RSS20234240B，采样父子合计峰值31834112B。最慢组合循环3.53秒包含同步落盘、进程恢复、查询和额外校验，不当作GUI帧耗时或真实全城性能。原始报告、过程／内存采样、独立检查代码、来源及原始失败记录见验收页。

本轮六包完成，不等于完整T09或M0完成。永久断链实物处置、破产／减记、完整企业工资税日历联合、全规模车辆／公用物理以及T13真实16GB联合负载仍未验；UI和AgentKit没有展开。

## 18. v0.5.19：T10-A有限profile CLI会话

[CLI契约与操作方法](m0-cli-session.md)接入已有区域／独立经济／独立启动推进器，提供可信宿主Caller、状态／区域快照age、暂停／恢复／单步、四档真实等待、显式事务、热冷原票查询及原有七类分页。重开默认暂停，不按离线墙钟追赶；run一次只执行一个明确Envelope，不自行发业务或拼接尚未联合的profile。同步pause在当前等待／提交后确认，未实现抢占式输入或自动运行至目标日期。

输入限制32KiB，JSON不能声明System身份或任意set_property；写会话必须以当前源码指纹打开匹配规则的世界，拒绝新代码写旧指纹世界。暂停仍可处理非时钟责任；时钟不能混进submit旁路控制。重试查原票后由Store核对actor／payload，不能因重开暂停重复收费；来源未对齐、磁盘背压继续拒绝，落盘不确定／Store污染锁为unknown并要求重开。

[最终Release测试](evidence/m0-t10-session/tests.txt)167项全部通过（含12项真实CLI测试），fmt／Clippy通过；经济一日以24个持久小时事务实际运行61.373119625秒，完整业务state hash与无等待对照相同（0e0277ef5f873108e1e55d5b3522203344810b63018bc7efba229fee75223b02），实际1407.782游戏秒／现实秒。严格60秒一日候选门**未达到**；四档短测只通过节奏与业务结果不变，不能说全城四档吞吐达标。最终验证汇总见[独立校验](evidence/m0-t10-session/verified-summary.json)，源码／宿主见[来源](evidence/m0-t10-session/provenance.json)。

独立经济Store owned2623391B只是有限、无空间包实验的Rust容量，不能与完整区域空间Store混比或当RSS。单会话从现有工具域规划划8MiB临时解码／序列化页，不扩3.10GiB全产品预算、不按城市复制，原Store／检查点原域计量；此规划不是allocator硬门或16GB整机通过。本轮没有UI、AgentKit或新GPU实现。

T10-A有限功能交付；完整T10／P-CMD、目标对象revision、T05全源／阶段游标与完整工资日历联合、T12 CPU参考、T13真实16GB联合仍未验。下一主线为T05-B真实业务源与游标／到期事实共同持久提交，使用本会话观察和控制，不继续重复复核T09。

## 19. v0.5.20：架构文档先行，T05-B未发布

用户在T05-B实施中提出独立计算worker及渲染进程问题，当前先交[运行架构A55](process-worker-and-render-architecture.md)，区分App监督、Sim唯一写入、纯计算池及GPU所有权，列明协议、崩溃／旧候选恢复、整个进程组预算与独立实测门。文档可验证静态链接／算术，不是worker、图形或16GB通过。

T05-B初稿Rust改动保留在本地未提交；新调度／日历／持久游标联合尚未验收。advisor只读发现处理中满收件可能死锁、P4误含税款副作用、必要阶段页缺应急资格三项阻断，尚未修复／实测。本次发布仅含文档；最近已发布模型仍为v0.5.19的T10-A，167项测试及61.37秒经济日的证据只属于该冻结版本，不能转用于工作区初稿。后续先按本架构修复T05-B并验证，不因有架构图宣称多进程已实现。

## 20. v0.5.21：T05-B真实到期与持久日历联合

[T05-B契约](m0-timeline-runtime.md)把当前实验profile的全部实际到期源接入P0—P5。实际Claim、劳动应计／账期、区域义务、泵机作业、报价失效、货物和人口到达与旧经济阶段直接发现，不使用预制统计计数冒充业务。进度／钱物／有理余数同一根持久提交，处理中查询有标记，开启Timeline后旧独立Advance拒绝。新汇总劳动支持日／周／公历月结、月末锚、周期更改保留旧到期、停工／终止及不足现金留债；它目前没有独立个税表／周末制度，不据此签收全产品P-TIME。

三阻断已修复：满收件允许合法Handoff／已清事实Archive而不放开任意付款；税款拆成P0应计／P2实付／P4观察；单一最多一小时边界先以正常容量购买有限续做票，纯阶段／零付款页也可续到完整边界，远期目标下段不得透支。Advisor单轮新增指出失败Advance挤占回执槽和后页dirty账户过多的风险，均已修复并加入洪泛与64账户实测；advisor只读检查不替代测试。

[实际CLI结果](evidence/m0-t05b/timeline.json)包含2万笔真实1分钱债权全部实付，79页，密集Engine owned3645280B；一个经济日；以及两日区域＋启动＋货运＋迁移＋独立家庭劳动联合。联合实际现金为G1000000、公司2为200980、公司3为596688、S1202220、家庭5为112分，总3000000；旧区域义务3000、泵费200、货物及运费2020、工资112分别真实转账。电池100Wh转热100Wh、源水600L转清水600L，原料5Q分成源端4／到达1Q；人口3990／110／300守恒，只有活动城增加两日本地年龄。送达事实另欠5分处理费，未悄悄清除。独立复算见[verify.py](evidence/m0-t05b/verify.py)。

30日新执行器与旧经济最终WorldView一致；密集支付及劳动半页可实际检查点／重开并精确重试，纯阶段、零现金债权和P5完结在正常磁盘区耗尽时各11个真实进程退出点共33次恢复，无重复履约。有限票可完成已接纳边界，下一段无正常额度即拒绝；不宣称真实硬盘满或物理断电仍能运行。本版旧经济paced run一日实际61.807127292秒、状态与无等待对照相同，严格60秒门仍未达到；新Timeline只验step分页，不转用旧倍率证据。最终全仓库185项Release离线测试、fmt、clippy均通过，冻结源码记录见[验证汇总](evidence/m0-t05b/verified-summary.json)。

本轮M4／32GiB实测三Store短探针9.78秒墙钟、峰值RSS65683456B（全仓库回归同时运行）；与owned分别记录，不能当城市吞吐或16GB通过。核心检查点4MiB、外层16MiB上限及Timeline2MiB份额按[预算增量](product-runtime-budget-and-versions.md#t05-b动态来源份额v0521)登记；3.10GiB全产品进程组没有扩容。worker池／监督IPC、渲染、UI、AgentKit未实现，本轮仅Rust模型和CLI。完整经济＋区域产业装配、工资税表／节假日／全产品来源、T12 CPU参考、T13真实16GB联合及长期经济平衡仍未验，M0不签收全部完成。


## 21. v0.5.22：软件总体架构独立汇总

[总体架构S56](SOFTWARE_ARCHITECTURE.md)按当前源码职责、主体状态／待办／历史、排程、提交恢复、进程／数据文件及资源预算整理，区分现有单进程CLI与目标多进程部署。共享事件只一份权威；共享文件不授权多个writer。没有修改Rust、依赖或fixture，没有新增运行能力；v0.5.21的185项测试和有限证据归原冻结源码，本次只做文档检查。后续优先T12 CPU参考及独立worker／监督门，UI／AgentKit／渲染仍不展开。


## 22. v0.5.23：T12-A有限空气CPU参考

[T12-A契约](m0-air-cpu-reference.md)实现RUN36单通道EI公式，32m环境格对应三档32²／64²／128²，固定分钟锚点并在真实源启停／固定目标截断；已投影源按稳定ID、整数区间积分后f32累加。先完成全部源页，再用同一旧场计算全部tile；候选未完整不交换场或累计窗口诊断。不是实际污染浓度／健康标准，也未接Engine／Store的城市指标。

core.air用f32／f64位整数保存完整旧场、候选源累加／tile、积分、进度及诊断；原型文件绑定当前源码与输入，源中段／tile中段落盘后由新的CLI进程继续，结果与直接计算逐位一致。直接反序列化同样经过状态验证，数组解码即限制行数，拒绝缺边界、非有限数、坏游标／计数、错输入／旧源码／损坏和覆盖已有输出。不可变任务文件不替代Store权威提交，不代表计算worker池已实现。

实跑配置日、64²／128²日、固定30日和两万源五任务；独立Python f64从压缩的完整任务工件复算最终场及梯形区间积分／时间平均值，检查预先定义的有限工程误差门。全部证据与精确实测数值见[验证汇总](evidence/m0-air-cpu/verified-summary.json)，[计算器](evidence/m0-air-cpu/reference.py)与[完整性校验](evidence/m0-air-cpu/verify.py)可复跑；不将f64误差当现实准确度或GPU资格。

单任务owned2MiB门从A55既有32MiB任务页内划出，源／场／积分与恢复临时页归属见契约及运行预算；命令rusage短测与进程组联合峰值未测分别报告，16GB整机不签收。最终195项Release离线测试、fmt／clippy与独立数值／工件检查均通过，包含已实现业务profile回归，结果归本版冻结源码。仍缺真实活动空间投影／跨城边界、完整NI／家庭暴露、S11同世界提交／恢复、通用纯任务worker／监督、GPU与16GB联合；完整T12／M0不算通过。


## 23. v0.5.24：树种与经纬度约束设计补充

[树种契约](tree-species-climate-and-air.md)补地理适生、树种差异与有界空气作用。当前T12-A无树木项、T07无真实配准，均未改为实现通过；文档从M-env规划48KiB而不假定旧状态免费容纳。Rust／fixture／依赖没有变化，195项测试仍归v0.5.23冻结证据，本次只验文档、预算及来源。单worker隔离仍是下一模型依赖。
