# M0 Rust模型：当前范围、运行方法与验收状态

记录起于v0.5.4，2026-10-19。用户授权从M0-T01按依赖写Rust，并明确本轮只做模型、不做UI。版本tag是仓库阶段发布，不是游戏0.1已经发布；所有阶段验证后提交、递增附注tag并推送。

当前最新为v0.5.15：[T09实际区域履约](m0-regional-fulfilment.md)联合了Region、Boot、货物／电水及共享资源；[T08经济](m0-economy-runtime.md)仍是独立实验。后文按阶段保留，旧not_run为当时状态；有限通过不能扩大为全部M0、真实公用网或16GB通过。

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
| T06 | 事务持久子阶段通过，v0.5.9 | 显式提交边界／单写锁／逻辑WAL／恢复与进程退出；完整检查点压缩／全源时钟事实联合／断电未测 |
| T07 | 空间／地图子阶段及不可变资产存档通过，v0.5.12 | 原生灰度、厘米几何／步行接缝；期初空间与模型同根恢复；真实搜索／车辆坡度／运行时空间修改／UI等未测 |
| T08 | 三项有限实验通过，v0.5.14 | P-E0／P-E1及完整P-BOOT有限输入实验；Boot与Region共钟见T09，完整行业经济联合／真实公用网／交通及全源执行器仍未验 |
| T09 | 实际履约与共池子阶段通过，v0.5.15 | 三城类型化计量链／同池人货电水、Boot共钟、交付收费损耗／断链／共同恢复；真实车辆／潮流水力、具名权益、完整P-CAP／P-XMAP仍未验 |
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
