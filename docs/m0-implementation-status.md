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
| T03 | planned | 实际领域输入装配／有限初始来源 |
| T04 | planned | 易失事务／权限／幂等／原子预留 |
| T05 | planned | 共同钟／阶段／稳定随机／到期分页 |
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
