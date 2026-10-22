# M0 Rust模型：当前范围、运行方法与验收状态

v0.5.125：[合并pool进程有限诊断](evidence/m0-t13a-combined-pool-125/STATUS.md)同最终exe的正常对照/两代完整快照/cold、单次half-wire和kill-layout/kill-root/cold-error实测完成，89项headless及严格检查通过。混合root不获诊断替换信用、raw照计；同完整三帧窗口减少1,327,440B但原normal仍超7,628,426B。正式joint-driver尚未接入该opt-in，原8h失败、三轮四速和真实16GB门不改变。初始错误归因及helper失败保留；旧8份纯合成文件已精确退役，最新完整恢复原件保留。建设候选最新126/129测试通过，材料sink仍缺，未签建设水电联调。

v0.5.124：[三档速度入口及观察器握手](evidence/m0-t13a-tier-speed-124/STATUS.md)完成各720小时/723目标/724窗口与全Engine参照、冷恢复一致，03超时失败留证。仅本来源unpaced功能验证；缺测265/153/99过程采样和L1缺席不签物理/正式速度门，三轮四速、原normal超限、8h失败及真实16GB not_run均不被覆盖。建设公用供给与合并进程的源码候选尚未实测。

v0.5.123：[full-R07 native归因](evidence/m0-t13a-full-r07-native-attribution/STATUS.md)已真实获得4角色VMmap、身份和完整映射时间窗；两代/冷开完成，辅助工具约136MiB另计，本轮不签性能。原完整帧超8431602B与8h/16GB未过结论不变。02已退役四份合成载荷，现不可直接恢复；03两代完整原件逐文件验证后保留。下一候选减少重复driver进程开销，尚未实现，不以页表/文件cache给免费额度。

v0.5.122：[0.1限定模型阶段验证](evidence/core-road-town-integration-122/STATUS.md)已完成真实CLI、28日经营压力矩阵、旧writer兼容和core/storage回归；headless诊断消费者的5项测试与全workspace构建/strict/fmt通过。formal01停机后业务WAL/检查点及布局57/58完整副本冷恢复通过，原件保留；此功能恢复不抵6h/8h。原32c完整帧normal物理超额8,431,602B仍未消除，分级速度和8h尚缺、实际16GB为not_run，full_M0=not_accepted。

v0.5.104：[R07独立背板/TTL/冷恢复](evidence/m0-t13a-r07-report-error/STATUS.md)最终1faa同源全workspace720.238秒及严格检查完成，原满级两代、实际补回、独立新PID零信用与旧格式冷开通过；修复metrics错误误报，实际CLI exit2无成功报告。完整帧物理仍超8,677,290B，不签normal/正式性能。精确8份旧payload退役11.827GiB、190原件全SHA保持；下一依赖实际VM归因，三轮/四速/稀有层/8h/真实16GB未完成。

v0.5.103：[集中回收](evidence/disk-reclamation-v103/STATUS.md)退役7份无依赖合成/重复文件，分配空间617,242,624B；190份恢复/故障原件全SHA不变，R07第一代未冷恢复继续完整保留，Rust WIP不动。仅磁盘治理，不签新模型/性能门。

v0.5.102：[原生整组观测与失主收尾](evidence/m0-t13a-native-observer-errno/STATUS.md)同冻结8f0c90ba来源全workspace/严格检查通过，实际errno、锁、owner及60s周期验证完成；原规模L1短测88帧85完整，原物理normal仍超9,758,898B，置换0，不签性能门。阶段精确回收约11.88GiB，旧完整原件保留；下一依赖为R07独立可回收背板。正式三轮/四速/8h/真实16GB未完成，完整M0未验。

v0.5.100：[原生单PID资源读取后端](evidence/m0-t13a-native-reader/STATUS.md)同冻结8b2c2594来源全仓及严格检查通过，独立SDK ABI/CPU raw校准、244请求及五类实际拒绝验证完成。未接进程树/压力/swap/时钟等正式观测能力，原normal失败不撤销，置换0，正式三轮/四速/8h/16GB仍未通过。28份无当前依赖的旧rlib/rmeta精确回收，不新建target或压力副本。

v0.5.99：[独立VM地址归因](evidence/m0-t13a-runtime-vm/STATUS.md)完成同冻结exe原满级L1一次15秒smoke、10份目标身份夹取/映射读取及完整冷恢复。arena3,261,424,096B地址全覆盖；原normal超限仍failed observed，可置换字节0，工具自身资源账缺项不签。只退役本轮两份合成载荷，原恢复基线完整保留；尚未完成正式三轮/四速/8h，真实16GB not_run，完整M0不签。

v0.5.98：[原L1正式单轮](evidence/m0-t13a-typed-workspace/FORMAL-STATUS.md)完成300+1800秒、2101扫描/8代恢复读回及最新7/8完整冷开；同源两类页分位满足观察门限，非完整业务吞吐。窗口7114完整/86 unknown帧，完整footprint仍超normal19,392,738B，failed observed；idle requested/arena/隔离delta0不签业务泄漏。旧基线4纯合成文件5.920GiB已退役，原始业务/故障/源/exe保留。先做独立VM组件归因，三轮/四速/8h未完成，真实16GB not_run，M0 not_accepted。

v0.5.97：[常驻解码工作区与原快照格式兼容](evidence/m0-t13a-typed-workspace/STATUS.md)完成同源全仓713.986秒、60个harness汇总/442条通过记录（含nested，不称唯一测试数），严格clippy/fmt/build及三份完整冷恢复。advisor认可冻结规范profile外部4MiB分账和有限功能发布；64MiB typed+64MiB并发COW仍在原128MiB内。下一步原参数多代formal；旧物理超界不撤销，三轮/四速/8h未完成，真实16GB not_run，不签完整M0。

v0.5.91：[固定L1区域与真实因果校准](evidence/m0-t13a-causal-clock/STATUS.md)完成同源全仓、原规模L1短测及新旧完整冷恢复；28帧同相位footprint超normal约18.57MiB，维持failed observed。下一步[原正式L1多轮快照](evidence/m0-t13a-causal-clock/NEXT.md)，不改负载／预算；三轮／四速／8h未完成，真实16GB not_run。

v0.5.89：[实际边界计数与native观测](evidence/m0-t13a-native-boundaries/STATUS.md)完成同源全仓及原规模L1短测／完整冷恢复；观察到同相位物理驻留高于3.10GiB，未归因为System缓存。下一步实验L1固定区域复用，原负载／预算不变。正式三轮／四速／8h未完成，16GB为not_run。

v0.5.88：[统一稳态控制器及实际正式L2首轮](evidence/m0-t13a-steady-controller/STATUS.md)完成原300＋1800秒、720小时／723目标／724窗口及完整Engine独立对照，冻结全仓回归通过。稀有类别样本不足、物理／诊断预算与同边界泄漏仍未签；三轮／四速／8h未完成，真实16GB为not_run。下一源补实际allocator与native观测，不跨来源继承首轮。

v0.5.85：[整角色区域及原规模短联合](evidence/m0-t13a-role-arena/STATUS.md)通过冻结全仓、旧完整Store、近16MiB边界、实际池内full30日及606秒L1＋L2故障／保存／冷恢复。仅签该功能／短诊断范围，非正式内存／分位／稳态／四速／8h；真实16GB保持not_run。下一主线是[统一正式测量控制器](evidence/m0-t13a-role-arena/NEXT.md)。旧阶段文字保留历史范围，不覆盖本项更新。

记录起于v0.5.4，2026-10-19。用户授权从M0-T01按依赖写Rust，并明确本轮只做模型、不做UI。版本tag是仓库阶段发布，不是游戏0.1已经发布；所有阶段验证后提交、递增附注tag并推送。

v0.5.83的[固定词法解析](evidence/m0-t13a-fixed-lexical/STATUS.md)已接v1／genesis／v2正文，共用1MiB预建且计入owned的bit栈，保留旧有效范围和Ignore／Content差别。冻结源码全仓54组／415条通过记录、原full30日720小时／724worker、旧完整world及近16MiB合成CP实际新旧Store冷开通过；解析压力峰值单列，非业务规模或内存降幅。整Role控制暂态／broker及normal64/common8、原联合负载、分级／四速／稳态／8h仍未通过，真实16GB为not_run。下一步见[整Role准入与联合实测](evidence/m0-t13a-fixed-lexical/NEXT.md)。下列早期阶段段落保留其发布时范围。

v0.5.81的[Meta／legacy保留工作区](evidence/m0-t13a-metadata-workspace/STATUS.md)已消费实际预建文件、Engine字节及完整收据／控制表；保留原legacy有效大收据并移除Content树。冻结同源全仓54组／410条harness记录、旧完整存档、固定旧reader读取新v1输出及原full30日720小时／724真实worker与完整业务空气对照通过。Meta全部heap纳入保守表后旧拓扑显式不fit64MiB；实际角色账、逃逸scratch、broker/Filled、normal64/common8与正式分级／四档／稳态／8h继续实现，真实16GB保持not_run。下一步见[实际角色分配边界](evidence/m0-t13a-metadata-workspace/ROLE-ALLOCATION-NEXT.md)。

v0.5.79的[T13-A预建构造基础](evidence/m0-t13a-role-workspace/STATUS.md)已接实际Store／Sim／Reference：core双root与准备区、候选及codec，原257×257／16位源的固定解码、Large输出和几何背板；同源全仓404条harness记录、旧完整world逐字节与背板身份、原full30日720小时／724worker及独立完整业务空气对照通过。该轮无L1／normal pool，不是完整Role预算或性能门；Meta／legacy／控制暂存及整包授权继续实现，原分级／四速／稳态／8h和真实16GB仍未签收。

v0.5.23增加[T12-A纯数值参考](m0-air-cpu-reference.md)，尚未写入城市世界；业务协调器为v0.5.21的[T05-B](m0-timeline-runtime.md)接入当前实验profile的真实来源、日历及持久游标；Region／Boot／Exchange／Payroll可联合，旧[T08经济](m0-economy-runtime.md)仍为独立profile，不能与区域重复计劳动。后文按阶段保留，旧not_run为当时状态；有限通过不能扩大为全部M0、真实公用网或16GB通过。

当前后台计算连续目标的冻结标准见[W0—W4验收](background-compute-acceptance.md)（v0.5.25）；[W1独立纯worker](background-worker-runtime.md)见v0.5.26，[W2城市接纳](background-store-runtime.md)见v0.5.27；[W3独立Sim／恢复](background-sim-runtime.md)见v0.5.28；v0.5.29的[W4正式联合](background-joint-runtime.md)完成1800.435秒、实际故障、完整业务／空气对照和同刻全进程组观察，16GB及整个M0未通过。

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
| T12 | T12-A空气CPU有限参考已实测，v0.5.23 | 单通道32m EI、源／tile分页、任务检查点与新进程恢复、独立f64对照；未接世界／真实源投影／阈值，W1独立纯worker及W2有限场接纳已实测；独立Sim有限实现见W3；GPU仍未完成 |
| T13 | T13A-0已冻结；1a真实CPU、1b完整L1短诊断、2b存档下界与v0.5.36三档真实接入已测；正式门未完成 | [具名三档真实30日／完整oracle／冷开／Progress](evidence/m0-t13a-tiered/STATUS.md)与[完整L1布局短诊断](evidence/m0-t13a-layout/STATUS.md)分别留证；full输入与v0.5.35完整期末不变，单次推进227.99秒。集中到期性能、10类CPU计时、四档、三轮稳态、8h及真实16GB／图形联合仍未通过 |
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

## 27. v0.5.28：W3独立Sim、宿主监督和恢复

[运行契约](background-sim-runtime.md)接入Rust独立Sim／有界宿主IPC和纯worker。RebindLease原子换代、最终Accept固定ID及成功才占票；D域防抢占；6MiB缓存持久charge、每attempt清理与跨Sim OS锁／owner登记避免旧worker并发复用。实际源／Cells杀worker、直接从外部杀Sim后全新监督端重开、候选发布及回复丢失、取消与三次封顶均有执行证据。

[八分支CLI工件与核验](evidence/background-sim-w3/verify.py)比较完整空气State及规范化revision后的完整业务状态，原冷热票保持；全仓库门和工件复跑完成后才发布。本阶段没有1800秒联合、全进程组RSS或16GB设备证据；[W4具体计划](background-joint-experiment-plan.md)继续真实日／周工资、固定额营业税及合同债务／履约／空气联动。此前各版本not_run按历史保留，不将W3改称全部M0通过。


## 28. v0.5.29：W4有限联合与后台计算目标收口

[W4运行](background-joint-runtime.md)冻结Joint期初／策略，Rust宿主监督独立Sim和一个纯空气worker。七日按真实1800.435秒分批推进，170点／173窗口的完整World、resources、leases、next_token、Timeline（仅归零revision）及完整空气与独立直接CPU一致。实际job2杀worker换代、job29根提交后丢回复杀Sim，全新客户端恢复原票；每日检查点及最终冷重开保留712原回执，不重复钱物／劳动／责任。

真实日／周工资336／528分，固定营业税实收1400000分；曾欠201612分，第4日实际借入850000分后清偿，借款仍为第10日到期负债，原合同1000分已付。两个货运实际交付，泵机启动、水电／废物库存、费用和容量释放均核实；收件费10分实际结清归档。新增Lend只把已有资金放款和Claims债权原子化，不扩通用银行或完整产业经济。

[完整核验](evidence/background-joint-w4/checks.json)含6419同刻采样，观察RSS和峰49823744B（47.515625MiB），最大采样间隔0.417930666秒，两短命worker漏采已披露；最后无Sim／worker／已知孤儿。最大必要任务往返1.169667583秒，实际队列峰值未插桩。实验只用M4／32GiB，不以这个有限小样本接受16GB、全城、8小时、经济均衡、真实污染或GPU。

当前完整232项Release离线测试、fmt／Clippy和文档检查通过；[最终advisor](evidence/background-joint-w4/advisor-final-review.md)只读核查原始工件、完整持久状态及测试记录，无阻断，未自己重跑实验。[逐项完成审计](background-compute-completion-audit.md)保留所有原目标与证据边界；后台计算有限闭环可以收口，整个M0仍未接受。总体架构已按实际独立Sim／worker更新，不再把它们写成尚无实现；通用池、UI、渲染、GPU和AgentKit未展开。


## 29. v0.5.30：T13A-0负载与验收冻结

用户授权连续推进模型规模、性能与8h验收，仍不做UI／GPU／AgentKit。[冻结契约](m0-model-scale-performance.md)与[参数](../fixtures/m0-t13a-acceptance.json)保留L1完整JPROD数量、L2当前共享世界真实上限，禁止复制小Engine代表大世界。三正式场景各三轮5min热身＋30min稳态，之后同一满级持久世界30日／8现实h，96保存／L1压力／真实worker及Sim故障／冷重开，不能缩负载或跑完空等。

计算p95／p99为20／40ms，P-E0实际经营30日≤1800秒，完整轨迹含持久化／IPC。计算类别／稀有样本、ready age起点、保存deadline到持久完成、诊断8MiB与世界20GiB／2GiB安全余量已冻结。advisor独立方案审查完成，不构成运行通过。32GiB下公开系统接口读取自身／子进程phys_footprint及CPU／I/O可行性已测，Mach tick经timebase校准；不是模型性能证据。

本次未改Rust模型，仅新增验收输入／文档和测量可行性脚本；W4原232测试及30分钟证据归v0.5.29冻结源码，不移作本版规模通过。[静态核验](evidence/m0-t13a-freeze/checks.json)通过。下一阶段先装配逐字段／宽度／索引和满级资金物资oracle，补长时有界观测，分级测出瓶颈再修复；稳态、8h及16GB均not_run，整个M0未接受。


## 30. v0.5.35：T13A-2c满级真实业务接入

[实际工件与边界](evidence/m0-t13a-full-sim/STATUS.md)：独立CPU与Sim／worker分别执行冻结输入；完整723目标／724空气窗口、64工资／20000债项／16义务／8单及完整期末／3011永久原票核实，真实新进程开城与最早请求重试无副作用。现有8资源池及v2配置显式绑定，IPC仍64KiB、预览仍有界。342.13秒推进及查询热点是实际诊断，未将其签作P-E0、计算分位、四档速度、L1联合、稳态或8h；完整M0仍未接受。


v0.5.36的T13A-2d[具名small／medium／full真实30日与Progress优化](evidence/m0-t13a-tiered/STATUS.md)已完成全部关键目标、独立oracle／完整期末及冷开原票对照；full期初和完整Engine与v0.5.35不变，实际单次推进由342.13到227.99秒。三档诊断不替代集中到期、10类计算计时、四档实际速度、联合预算、三轮稳态或8h。下一依赖为固定大小的瞬时批次埋点，区分quote／replay／实际提交及worker／接纳复算，再按原冻结门继续。


v0.5.37的T13A-2e[真实计算计时与完整诊断](evidence/m0-t13a-cpu-regions/STATUS.md)已完成：有界瞬时收集器区分报价／提交与空气尝试，worker完成前持久诊断；满级723目标／724窗口和P-E0／E1实际执行，完整Engine与v0.5.36一致。旧观测器超2MiB的真实失败保留，流式修复后同二进制重测完整30日。Claims20／源页4样本不足，后台接纳最大474.96ms长尾待定位；未签10类、四档、L1联合、三轮稳态、8小时或真实16GB。下一依赖为同负载稀有类别重复和联合验收，不通过减少业务／空气步数提速。


v0.5.38的T13A-2f[稀有类别同负载实际执行](evidence/m0-t13a-rare/STATUS.md)：完整v2锚点独立运行200 Claims／100 source分支，实际300个Sim／400个worker、完整归档与冷重开，分别补齐Claims／Sim源页／worker源页各200有效样本，原始分位通过本阶段20／40ms门。未减少原表／页／空气步数，所有独立世界原票归档保留；原路径及归档／扫描失败均留证。尚未签正式10类、联合预算、四档、三轮稳态或8小时；下一依赖仍为P-E1短case和其余明确分支／联合验收，真实16GB未测。

v0.5.39的T13A-2g[P-E1短case实际采样](evidence/m0-t13a-pe1-short/STATUS.md)：71独立Store、885成功事务，剔除16个AlreadyCanceled后四case活跃200/208/216/245，原票与raw区间独立核数，各达200样本和20/40ms门；完整World对照v37、71归档解压冷开通过。保留50失败Close及所有试验，不改小时步长或冻结输入。仅有限分支采样，四速度／L1联合／三轮稳态／8小时仍未测，16GB not_run，M0 not_accepted。下一步推进其余冻结性能分支与联合调度，不继续增加经济案例。

v0.5.40的T13A-3a[并发接入与恢复修复](evidence/m0-t13a-joint/STATUS.md)：完整记录L1扫描600次／两份快照与实际full30日同行，业务正确性通过，但原整组因外部预留恢复规则不一致失败。修复创建／恢复上限并前置拒绝反例后，最终来源对同一原3.38GB快照实测冷开7.34秒。原失败及三份源码分开保留，不拼成最终来源600秒通过。64MiB未用预留未补压力，观测还漏计L1 Metrics及Python临时量，诊断／满级联合预算均未签；不得用于正式三轮或8h。下一步补额度借还与共同事件／完整观测账，再推进正式负载；16GB not_run，M0 not_accepted。


v0.5.41的T13A-3b[诊断费用与进程堆证据](evidence/m0-t13a-budgets/STATUS.md)：同来源第一次205秒因系统pressure保护失败，原样保留；第二次612秒完成L1全量600扫描／两快照、full30日723目标／724worker／3011原票及9.41秒冷装，730正常堆记录完整。直方图及定宽摘要降低诊断费用，Rust监督与共同事件时钟接入；完整workspace回归、advisor独立核验通过。已量组件小计4,812,526B，但业务驱动诊断份额及64MiB真实占位借还未闭合，不能签8MiB或正式满级。quote后台采集／接纳p95约20.93ms热点保留，footprint仍有缺测。下一步先修PID目录复用和费用／压力，四速度／正式三轮／8h未测；真实16GB not_run，M0 not_accepted。


v0.5.42的T13A-3c[实例身份与驱动复制收敛](evidence/m0-t13a-process-instances/STATUS.md)：原子创建日志实例目录，保留原预算及部分初始化目录；显式实例配对取代仅PID关联，固定日志／参考行容量并减少响应副本。原full30日单独实测完成，723目标／724worker／3011票及完整Engine含metadata与CPU一致，726报告与727退出记录独立核验通过；34项定向／共享调用回归及advisor终审通过。当前没有L1同行，不作为联合压力或旧版提速对比；诊断总账、64MiB占位、四速度、正式三轮、8h仍未通过，真实16GB not_run，M0 not_accepted。下一依赖是受控owner的额度、真实容量和已触页余额，而不是再次复核本阶段。


v0.5.43的T13A-3d[worker固定槽及失败实测](evidence/m0-t13a-worker-slots/STATUS.md)：真实输入／候选接两个已触页2MiB槽，借用／归还不重新分配；metadata另计，core借用codec保持字节兼容并移除整份State克隆，既有候选流式校验。全仓含借用compile_fail及clippy通过；原full在216.89秒因系统非正常pressure持续60.197秒保护终止，382个正常worker局部余额与旧hash核验通过但**完整30日／724窗口not_completed，本次实测failed**。383报告中1缺footer保持缺测，5分钟观察未恢复normal，未重试／减负载／停其他应用。advisor准许按实现与回归完成、实测未完成提交；64MiB总池、诊断总账、四档、正式稳态、8h及16GB仍未通过，M0 not_accepted。继续其他代码依赖，条件满足后补完整测量。

v0.5.44的T13A-3e[独立模型证明helper](evidence/m0-t13a-model-tasks/STATUS.md)：参考／image验证／完整期末比较移出驱动，成功退出及文件hash交接、父管道断开回收接入。最终6项目标回归／全target clippy及full CPU的723目标、完整Engine和参考报告对旧版核验通过，初次f32报告兼容失败保留。初版全仓与最终目标回归分来源，不拼最终全仓通过；构造image链不是实际Store冷开。最终来源完整Sim、驱动／helper诊断总账、64MiB grant、正式性能／8h／16GB仍未通过，M0 not_accepted。

v0.5.45的T13A-3f[同源真实full与费用依据](evidence/m0-t13a-runtime-ledger/STATUS.md)：在120秒normal采样后，以v44原Rust／二进制跑完原full，730正常实例／724槽及旧input-candidate hash、3011完整原票链／真实Store冷开和整个Engine含metadata核验通过；316.365秒全程／279.960秒业务／0.08952秒冷ready。驱动整堆峰加尾部600,825B低于候选1MiB，仍不是共同8MiB已闭合；helper动态JSON和64MiB真实grant待实现。采样RSS峰76,546,048B、已得footprint峰63,685,976B及1334次缺测分开，不外推绝对峰／16GB。L1联合、四档、正式三轮、8h与真实16GB仍未通过，v43原失败保留，M0 not_accepted。


v0.5.46的T13A-3g[有界诊断与原联合失败](evidence/m0-t13a-bounded-diagnostics/STATUS.md)：helper v2固定控制槽/字段/三artifact及root资产、共享8MiB候选份额和固定4KiB空气报告hash接入；最终来源全仓293项与clippy/fmt通过，full CPU723目标/完整Engine/参考报告对v42不变。原L1联合在61.051秒因warning持续60.229秒按保护失败，尚未启动业务，245RSS帧/部分工件归档；没有用旧full补本次。共同8MiB实测、64MiB真实grant与触页余额、四档、正式三轮、8h与真实16GB均未签收。先继续常规工作区owner实现，正常准入后补原联合；M0 not_accepted。


v0.5.47的T13A-3h：[实体余额与协议修复](evidence/m0-t13a-normal-pool/STATUS.md)。全仓303项、clippy/fmt及实际分片/超时/EOF/身份回归通过；原full无L1分支365.873秒完成，724实际借还/Filled/父wait、3011原票、完整Engine及旧hash独立核验通过，731正常退出记录。原L1联合在81.061秒保护失败、首个功能分支grant125协议失败均保留，未拼成联合通过。普通Engine分配仍为额外压力，64MiB精确常规工作区/共同8MiB、正式稳态、四档、实际8h均未签，真实16GB not_run，M0 not_accepted。下一依赖是Sim死亡时broker持有Child的代次撤销回收及8h安全日志，再进行原长时运行。


v0.5.48的T13A-3i：[broker实际Child与有限故障恢复](evidence/m0-t13a-broker/STATUS.md)。Client持Sim Child/独立监督cap，Sim持租约，broker实际wait后归还4MiB；撤销回执确认前禁新会话，晚到旧ACK无新事件，Start丢回复须撤销重开、broker死亡失败关闭。最终同源312回归及12池有限故障工件核验通过；原full无L1运行408.605秒，724借还/Filled、731正常heap、3011原票、完整Engine及旧hash独立核验通过。旧首轮7启动就绪超时原因未证明，旧overbound测试BrokenPipe失败及晚ACK错误各留原来源；不把重跑覆盖为从未失败。扫描仍1s，周期日志30s累计、行1536B/总8MiB有界；实际8h尚未运行。普通Engine额外压力、共同8MiB、精确常规工作区、L1联合、正式稳态及四档均未签，真实16GB not_run，M0 not_accepted。下一依赖是原28800秒/96保存/指定故障时点控制器及实跑，不再扩本阶段有限案例。


v0.5.49的T13A-3j：[耐久控制器功能与默认栈修复](evidence/m0-t13a-endurance-controller/STATUS.md)。单调绝对时轴、2880查询/96保存、实际Cells故障/提交丢回复/冷重开及独立故障参考接入；最终同源full短时实测432.112秒，723目标/725借还/3013原票/完整Engine及724旧hash核验通过。旧全仓debug发生真实栈溢出，Box<World>修复后原默认栈故障路径及旧线格式精确回归通过，全仓release316通过；旧失败独立保留，不声称全仓debug通过。无L1，控制器formal入口尚未实际8h；共同8MiB、精确常规64MiB、正式三轮/四档/实际8h仍未签，真实16GB not_run、M0 not_accepted。下一依赖是Rust联合监督与完整L1时轴/实例预算，不再增加有限故事。


v0.5.50的T13A-3k：[Rust联合托管及原负载保护失败](evidence/m0-t13a-joint-endurance/STATUS.md)。Rust监督实际pool/layout/business Child，布局owner S/EOF区分、至少覆盖完整业务时轴、固定截止排空及最新冷恢复路径接入；诊断首行持久身份避免新增数百文件。最终来源45定向/clippy/fmt/build通过，初版全仓317分源保留。原完整联合在316.368秒因非正常pressure60.056秒保护失败，451目标业务/空气对原CPU相同，316扫描/1完整快照，但600秒/30日/最新冷恢复及池最终平衡均未完成。所有大文件hash并保留，缺footer不算零；advisor仅认可接入实现及失败留证。完整联合、共同8MiB、精确常规工作区、正式三轮/四档/实际8h仍未签，真实16GB not_run，M0 not_accepted。


v0.5.51：[完整L2无等待30日及速度控制器](evidence/m0-t13a-speeds/STATUS.md)。最终来源41定向/clippy/fmt/build通过；真实实验总373.132秒、经营337.905秒，723目标/3011原票/完整Engine含metadata及冷开/724借还独立核验通过，原负载不变。full无等待局部60秒/日目标满足，不替代P-E0或旧session限速验收。Claims仅20样本，L1缺席；共同8MiB、精确常规工作区、正式十类/四档/三轮稳态/实际8h仍未签，真实16GB not_run、M0 not_accepted。下一依赖是实际session发布节奏修复及真正P-E0完整30日测量。


v0.5.52：[实际session节奏修复](evidence/m0-t13a-session-pacing/STATUS.md)。计算/IPC占用时间间隔，首次立即、迟到不积累追赶，幂等/冲突及失败不重置节拍，原权限上限保留。最终16定向及clippy/fmt/build通过，原实际session一日24持久推进58.216秒、终值对直接Store相同；这是同行回归，未含父进程Store预创建，不能签正式30日。初版sleep断言失败及来源分开保留。P-E0完整30日、full四档、联合预算/稳态/实际8h仍未签，真实16GB not_run，M0 not_accepted。


v0.5.53：[实际P-E0完整30日无等待](evidence/m0-t13a-pe0-speeds/STATUS.md)。54定向/clippy/fmt/build通过，实际Store创建前计时至末Checkpoint/日志持久完成18.514秒；722目标/722原票/原现金物料税务终值/同切分完整Engine含metadata与冷开核验通过。722为新整点加在途切分，非旧半点命令；首轮Core/Store联合hash比较失败保留并实测修复。只签无等待P-E0，真实60档、full四档/L1/共同预算/正式稳态/实际8h仍未签，真实16GB not_run、M0 not_accepted。


v0.5.54：[真实P-E0 60秒/日完整30日](evidence/m0-t13a-pe0-speeds/STATUS.md)已实际完成，722目标/原票/原终值/完整Engine及冷开正确，但经营1800.240858542秒、末日含Checkpoint60.240858542秒，严格日/总段门失败。721次后续请求均提前等待，实际release锚点累计迟到2558500583ns，独立整数核验定位，不全部归因OS sleep。旧源/阈值/输入未改、不加容差，advisor实际重核认可失败留证。下一修复scheduled-release锚定并完整重测；full四档/L1/共同预算/正式稳态/实际8h未签，真实16GB not_run、M0 not_accepted。


v0.5.55：[P-E0计划释放节拍及完整30日重测](evidence/m0-t13a-queued-pacing/STATUS.md)。新来源55定向/clippy/fmt/build通过，完整step经营18.510858625秒，真实60档经营1797.659120708秒、最大日预算57.642627666/末日含CP57.659120708，原严格门通过。722目标/原票/原终值/完整Engine含metadata及冷开核验一致；721后续正等待、scheduled相位漂移0，实际迟到保留且不扣耗时。原v54失败不改写。仅签此P-E0范围，full四档/L1/共同8MiB/精确常规64MiB/正式三轮/真正8h未签，真实16GB not_run、M0 not_accepted。下一依赖回到完整联合内存分配前预算及原负载重新准入。


v0.5.56：[固定L1诊断恢复与旧组件兼容](evidence/m0-t13a-bounded-restore/STATUS.md)。固定记录先算完整graph/容量、一次预分配及token/嵌套上限，表任意字段顺序无整表Content；Arena引用语义保留，写入codec不变。最终fd551…来源87相关/clippy/fmt/build通过；前一15166…来源全仓Release0单列，原clippy/深度反例失败分源保留。实际读旧v50全部27组件200526364B、native361272/stub1232426并重编码hash一致，native保守界35917424B；未加载压力/reserve，不能签完整L1或常规64MiB/共同8MiB。32GiB采样normal/swap净0但基线已有9147187200B，不能称无swap。full四档/正式稳态/真正8h/真实16GB仍未签，M0 not_accepted。


v0.5.59：[Sim具名固定编码／候选owner](evidence/m0-t13a-sim-workspace/STATUS.md)。4MiB实际双槽先释放再Filled、实际Child wait归还、部分补页失败不发信用，原64MiB池及诊断上限未扩大。最终e6dd…来源56定向/clippy/fmt/build0；原full L2实际30日经营343.253854834秒，723目标/3011票/完整Engine及冷恢复核验一致，2代Sim和724worker分别平衡，双owner池56MiB。漏顶层heap env导致缺1份监督者记录，原核验失败保留，完整遥测门不签；quote p95=24.811810ms失败/稀有样本不足，不签正式分位。Engine/serde/helper仍EXTRA，L1/共同预算/四档稳态/真正8h/真实16GB未签，M0 not_accepted。


v0.5.61：[原完整L1＋L2联合smoke](evidence/m0-t13a-joint-recheck/STATUS.md)本源重新实测完成，无代码改动。e6dd…/f262…来源实际613.3965335秒，完整723目标/3013票/724逻辑窗口及故障reference完整Engine一致，725worker/4Sim分别归还；L1全600scan、两代快照及最新新进程冷装9.247892333秒。监督heap本轮齐，实际kill缺footer及2560footprint缺测分别保留；RSS3576922112B/已得footprint3538866544B，normal/swap净0但初始已有6706692096B。execution接纳p99=45.111834ms、quote分位超门，正式十类仍失败；Engine/serde/helper/worker计算数组EXTRA，normal64/common8未闭合。正式四档/三轮稳态/真正8h未完成，真实16GB not_run、M0 not_accepted。advisor实际独立重核有限发布，旧v50失败不改写。


v0.5.62：[接纳校验流式摘要](evidence/m0-t13a-stream-digest/STATUS.md)。固定8KiB批量摘要复用原规范checkpoint，捕获/冻结/Accept/staged长度不造整份编码Vec；原schema/hash/CPU比对保持。最终5347…/66f4…来源全仓Release81774终态0，harness337含nested，clippy/fmt/build0；两次测试失败分源保留。原full联合43204实际610.629363208秒、经营356.202284167秒，全部业务/恢复/725worker4Sim/L1全600scan两快照及最新冷装8.226595334秒核验一致。sealed execution p99观察13.642998ms，quote p95仍23.801760ms超20；样本/faultjob/背景差异明确，不拼正式门。normal64/common8/State等EXTRA未闭合，正式四档/三轮稳态/真正8h/真实16GB未完成，M0 not_accepted。advisor实际独立重核有限发布。


v0.5.63：[worker固定空气模型背板及实际借还](evidence/m0-t13a-air-workspace/STATUS.md)。20000源/128²三数组与持久字符串/padding使用1MiB固定背板，seed直接恢复，共用原State算子；无fixed State/Input/Vec克隆逃逸，同进程跨job复用。IO4＋typed1实际预留/Filled/wait共5MiB，Sim4MiB独立，双owner池55MiB、最终64MiB；原上限不扩。ee8af3全仓Release44351终态0/harness340含nested；最终c23947仅省略两处显式生命周期，精确差异留证，最终131定向/clippy/fmt/build0，不借作最终全仓。原full联合84790终态0/610.372237708秒，经营417.637036542秒，3013原票/完整Engine含metadata/724旧hash/725worker4Sim/724 loaded-reset/600L1两快照及最新冷装7.926647875秒独立核验。sealed执行p95/p99=22.185695/46.696616ms、报价38.140652/82.436250ms均失败，观察较v62慢；不宣称性能通过/提速。其余模型/解析/栈/helper仍EXTRA，normal64/common8、正式四档/稳态/真正8h/真实16GB未签，M0 not_accepted。已无占用清理705个旧debug缓存文件，allocated约58.0MiB/可用实增约52.2MiB；共享Release及所有验收证据/源码/WIP保留。

v0.5.64：[规范整数编码与ARM SHA分派](evidence/m0-t13a-hotpath/STATUS.md)。固定8KiB批量整数输出保持原serde规范字节，原State序列化独立作参考；aarch64条件启用锁定sha2的运行时硬件检测，其他平台/强制软件fallback未实测。最终895abc…源码全仓Release终态0，343 harness含nested，clippy/fmt/build通过。原完整联合负载28854终态0，实际607.920064834秒、经营304.118845秒；完整Engine/3013原票/724旧输入候选hash/725worker4Sim/600扫描两快照与冷恢复一致。sealed执行p95/p99=6.875247/10.039755ms，报价13.602415/18.677290ms，为原门内观察，不签正式十类或严格受控提速。advisor独立重跑四项核验同意有限发布。normal64/common8、正式四档/稳态/真正8h/真实16GB仍未签，M0 not_accepted。缓存初次误删4份当前可再生库已重建且全目标fresh，错误留证；仅11份确认过期库allocated62570496B计有效清理，不把全局可用空间变化归因于此。

v0.5.66：[固定期末helper工作区及原联合短诊断](evidence/m0-t13a-helper-compare/STATUS.md)。BeforeReopen及严格规范Compare共用4MiB真实owner，两块2MiB读取背板、分配前授权／Filled／实际wait后归还已实测；business摘要明确引用已验证独立参考，普通Compare语义不改。最终同源全仓350条harness通过记录含nested、clippy/fmt/build通过；原完整L1+L2第二轮模型607.373秒／经营331.197秒，723目标、3013原票、725次worker借还、724旧候选hash、完整Engine与独立fault-reference字节一致、600扫描／两快照及冷恢复通过。第一轮缺顶层heap不签收，第二轮日志齐全，两helper整堆加尾4309977／4310063B符合局部门。短联合／局部额度不是normal64或共同8MiB通过；普通模型／Prepared／Store／参考仍EXTRA。正式十类分位／四档／三轮稳态／真正8h仍未签，真实16GB not_run、M0 not_accepted。下一步先核算完整模型及重叠槽容量，再统一固定模型／事务／Store工作区。

v0.5.67：[完整模型工作区容量依据](evidence/m0-t13a-model-capacity/STATUS.md)。新增实际Rust类型／最大记录数／同时存活拓扑CLI，Opening解码图、Engine／Prepared、五个含padding空气槽、Store及空间／worker全部列账。目标需求39,415,656B可装入原64MiB；advisor实际运行及独立重算认可实施依据。定向2测试／CLI／clippy／fmt通过，未运行业务负载，runtime_grant_accepted始终false。不是现有深克隆／DOM／普通解析的容量证明；下一步统一完整模型／事务／Store工作区，不再以候选表代替接入。normal64/common8／正式性能／四档／稳态／8h仍未签，真实16GB not_run，M0 not_accepted。


v0.5.69：[固定核心表与事务准备租约](evidence/m0-t13a-model-workspace/STATUS.md)。Timeline、资源／租约／journal使用固定有效背板；五类准备lease失败／展开归还，发布交换背板，Store保持poison至apply与Applied结束。cc1ca0…同源全仓Release46538退出0，358 harness记录含nested，clippy/fmt0；独立CLI实际Store::open旧rules完整world副本243文件，3013序列、2,507,418字节与旧fault-reference完全相等，原world未改。advisor独立审查有限发布无阻断。每Engine独立scratch，v67目标公式不可直接作为现有内存证明；Background深clone、一般恢复／WAL／DOM和实际池图仍未闭合。未新增全负载／正式性能／四档／8h，真实16GB not_run，M0 not_accepted。v68独立回收11.34GiB，原测试run id不改写。


v0.5.70：[Background常驻背板与显式计算槽](evidence/m0-t13a-background-resident/STATUS.md)。field/pending固定、prepare lease复制活动数据、持久后交换，capture/validate/Accept及Store staged/恢复显式借单Work，保持原数值及规范字节。926a61…同源全仓Release95173终态0／359 harness含nested，clippy/fmt/build0；旧规则world完整重开／3013seq／2,507,418字节相等。原full联合27370实际605.689秒、723目标／3013原票／724旧hash／725借还／600扫描两代及最新完整cold通过，五audit0及advisor独立认可有限发布。每Engine工作区重计后目标43,964,888B仍fit原64MiB，runtimeGrantfalse；普通恢复/拥有式入口及池图未闭合，normal64/common8／正式性能／四档／稳态／8h仍未签，16GB not_run，M0 not_accepted。末尾退役旧v66两压力5.67GiB，新完整基线及业务兼容依赖保留，复用共享target。


v0.5.71：[一般模型有界解码](evidence/m0-t13a-general-decode/STATUS.md)。普通Arena可信Domain64/128 visitor直写、不信文件max/size_hint，restore_capacity纯验证，保留稀疏free顺序与MAX最后代次；World/Opening/region及通用空气数组固定Rows，标签64B和lexical384B/depth128先于serde scratch，L1结构及负载不变。87796b…同源全仓92234终态0，362 harness含nested，clippy/fmt/build0；当前库旧规则完整world实际重开／3013seq／2507418字节全等，advisor独立认可。Raw串行归一化补两最大State后目标45,290,256B<原64MiB但runtimeGrantfalse。未重复新联合或正式性能；Store流式及整图池继续，normal64/common8/四档/稳态/8h未签，16GB not_run、M0 not_accepted。共享target复用、无新压力副本，完整当前／旧格式／业务恢复依赖保留。

v0.5.73：[Store流式检查点与回执](evidence/m0-t13a-store-stream/STATUS.md)。v2 world与receipt两遍输出，实际tmp完整恢复后发布；8KiB有界I/O、逐条receipt seed、可信1024索引和固定64条页暂存，v1/v2 checkpoint展开后持续poison至退役完成。source326a47fa同源全仓83997终态0，593.984秒，372 harness pass含nested，clippy/fmt/build0。当前库旧rules完整243文件world实际重开，3013序列和2507418字节全等；临时副本保存两次并重开，全部3013回执页相同，advisor独立认可。基础容量44147720B加Raw Air暂态1325248B等于45472968B，小于64MiB但仍仅目标算术，runtimeGrantfalse。WAL、legacy、comparison及整图Pool继续；正式分级、四档、稳态、联合与8h未签，16GB not_run，M0 not_accepted。复用target，临时往返副本逐文件退役9.727MiB，完整业务及兼容恢复依赖保留。


v0.5.74：[WAL有界规范编码](evidence/m0-t13a-wal-canonical/STATUS.md)。原record Value／Envelope clone及整段checksum复制改为保留的可信遍历表、借用Envelope与双32KiB缓冲；维持旧Value规范字节和全部子树有效性、未知字段拒绝、半帧及提交边界。source63c1642e同源Release30935终态0，595.302秒，379 harness pass含nested，clippy/fmt/build0。旧rules完整243文件world重开仍3013序列／2507418字节相等；在临时完整fork追加一成功一失败，两份实际WAL均与旧编码器逐字节相等，实际重开重试及两次checkpoint后3015回执一致，原3013条不变。advisor有限发布无新阻断；零额交易仅验证持久化，不代替经济或性能。基础44939296B加Raw暂态1325248B共46264544B，小于64MiB但仍仅目标算术，runtimeGrantfalse；comparison/state_hash DOM、legacy及实际整图池继续。正式分级／四档／稳态／8h未签，16GB not_run，M0 not_accepted。[集中回收复查](evidence/disk-reclamation-current-audit/STATUS.md)核实work17.204GiB，完整恢复输入保留，本阶段临时副本逐文件退役9.734MiB；复查不追加删除，历史回收不重复计入，复用共享target。


v0.5.75：[借用业务／状态比较](evidence/m0-t13a-comparison-codec/STATUS.md)。完整World revision0与借用WorldView分别保留，固定caller Codec无DOM；Reference目标轨迹、Sim摘要、Store含spatial联合hash复用显式槽，普通业务比较改临时文件完整逐字节而非双Vec或仅hash。source1b31214a…169文件，全仓27121终态0／554.620秒／383 harness含nested，clippy/fmt/build0；满20000claims/64工资及pendingJob旧Value完整字节、失败复用与短输入反例通过。当前库实际重开完整oldworld仍3013seq／2507418B，原243文件及兼容副本前后不变；业务1818613B、状态2480116B完整字节及Store联合hash一致。advisor发现Store头漏计已修、再次确认有限阶段无新阻断。基础54078832B＋Raw暂态1325248B＝55404080B<64MiB，明确另计Store／Reference／既有IO槽，仍仅算术、runtimeGrantfalse；ordinary Compare完整Engine/空气Vec、legacy及全图准入继续。十次hash约0.688秒仅诊断，不签正式分位或快进。正式分级／四速／稳态／8h未签，16GB not_run，M0 not_accepted。共享target，无新压力副本，成功临时比较文件精确即时退役；阶段末追加删除0B，恢复原文件和必要证据保留。


v0.5.76：[角色恢复槽及实际调用接入](evidence/m0-t13a-role-slots/STATUS.md)。EngineSlot直接seed/copy/reset/swap保留48主要背板，Store v1/v2恢复与tmp验证、Reference终局和ordinary Compare双槽接入；失败不发布、完整EOF/字节/旧Value语义保留。920a7574…172文件，同源全仓388 harness含nested／563.767秒退出0，clippy/fmt/build0；旧完整world仍3013seq／2507418B，原243文件与兼容副本未变。原full实际业务327.077秒，720h／723目标／724真实worker、64工资／20000债全付／8交付、完整business+air、冷开及原票重试通过；本轮无联合故障，seq3011，不冒称旧3013联合。730条Rust heap退出记录见专题，仅生命周期请求峰值加尾，不是同步物理或实际授权；无L1不签联合性能。容量基础54078840+Raw1325248=55404088B<64MiB但runtimeGrantfalse，候选空气/全role准入继续；正式分级／四速／三轮稳态／28,800秒8h未签，16GB not_run、M0 not_accepted。终态回收检查无占用，追加删除0B，当前完整world作为后续恢复输入保留、共享target复用。

v0.5.77：[固定空气候选槽](evidence/m0-t13a-air-candidate-slots/STATUS.md)。内部4源/128²候选direct seed、两层hash/EOF失败失效；Store导入/staged/附件/WAL、Reference与Sim预建槽，独立CPU重算和唯一writer不变。受控page代替可替换根的可变State引用，未使用EngineSlot::engine_mut同类接口也移除。通用20k源不缩；checkpoint hash改流式digest，Reference容量场景补2root。68d0247…173文件，同源392 harness含nested594.769秒0、clippy/fmt/build0；旧完整world仍3013seq/2507418B，243文件与兼容副本不变、旧Value完整字节相等。原full实际339.326秒，720h/723目标/724worker、64工资/20k债全付/8交付、完整business+air/冷开/原票通过，本轮无联合故障seq3011。730条退出heap仅Rust请求生命周期峰加尾；基础54079296+Raw1325248=55404544B<原64MiB仍runtimeGrantfalse。实际角色包、normal64/common8、正式分级/四档/稳态/28800秒8h未签，16GB not_run、M0 not_accepted。advisor独立结果核对认可有限发布；终态回收新增0B，完整当前world/恢复证据保留，共享target复用。


v0.5.113：[企业退出计划器及原预算归因](evidence/core-business-exit-integration-113/STATUS.md)。13项退出测试及core strict通过；有限只读planner已集成，不等于Engine实际清算。2MiB01控制器格式上限失败、2代完整冷恢复及精确54文件回收留证，2MiB02辅助22项通过但尚未运行。32c原normal满额、full complete peak超8,431,602B已按raw重算，native成本不能靠地址映射抵扣；原负载阈值不变。16GB/8h not_run，M0 not_accepted。


v0.5.114：[交通市场resolver](evidence/core-market-access-integration-114/STATUS.md)最终源码16项release测试及strict通过，六CLI仅pureplan，尚未Engine/Store联合落账。原allocation trace复算未发现可安全回收容量，2MiB02未运行。建设默认测试栈失败待处理，真实16GB/8h not_run，M0不通过。


v0.5.115：[主线有限集成及2MiB诊断](evidence/core-runtime-integration-115/STATUS.md)。d19来源49 release测试、strict/fmt、真实建设关闭CLI通过；默认debug栈失败与release通过分开。ade固定候选原负载4捕获/2冷恢复通过，但normal超8,415,170B；该轮快照54文件精确退役，保留190恢复/故障原件。2MiB未替换main默认。真实交通经营联合未完成，16GB/8h not_run，M0 not_accepted。


v0.5.116：[28日经营场景主线集成](evidence/core-market-scenarios-integration-116/STATUS.md)在2c18来源七场景+参数覆盖及example strict/fmt通过；当前Engine4100728B仅拥有容量，非M0物理证据。替身/无税/基础家庭现金耗损/退出未落账保留，M0原标准、normal失败及16GB/8h not_run不变。

v0.5.117：[原负载完整联合smoke](evidence/m0-t13a-joint-smoke-control-02/STATUS.md)固定32c/59aa来源实测611.85秒，720h业务/723目标/2880查询/96保存、725池借还及最新布局第2代冷恢复完成。44帧不完整及115帧外部编译保留，日志筛查拒绝formal准入；normal/common仍not_accepted，四速/正式稳态/8h/16GB未通过。新增两代完整恢复输入与raw仍有依赖，本轮删除0B。

v0.5.118：[03观测器离线修复](evidence/m0-t13a-joint-smoke-control-03/STATUS.md)在保留缺测和原标准下拆分功能启动资格与物理验收；16纯测试及2441原始帧无损往返通过，按固定业务／布局扫描分开预测日志161392895B，写盘前复核原256MiB上限。新smoke/formal未运行，不替代normal/common失败或剩余原规模门；16GB/8h not_run，M0不通过。本轮删除0B。

v0.5.119：[交通履约／营业额税主线集成](evidence/core-transport-tax-integration-119/STATUS.md)717148来源主线110 core lib+4建设恢复测试、4真实交通CLI、11经济配置与strict/fmt通过；候选349完整core/storage测试另列，93源码／Rust测试文件相等已核对。proof跨日／恢复／容量反例真实完成，成本压力欠税工资债务均保留，首轮fixture失败保留。新增运行heap未测，动态工程道路效果与真实退出仍在隔离线；M0原32c冻结来源/负载/标准不变，8h/16GB not_run，不签完整M0。本轮删除0B。
