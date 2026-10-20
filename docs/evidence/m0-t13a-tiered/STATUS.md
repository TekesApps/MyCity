# T13A-2d：具名三档真实业务与Progress查询优化

v0.5.36阶段诊断；不是T13A-2或正式性能门全部完成。仅Rust模型与CLI。full指已实现Engine的冻结上限，不是57000居民的完整产品业务。三档均无L1同行，设备M4／32GiB，真实16GB仍not_run。

## 冻结输入与实际结果

新增具名small／medium opening、policy和独立oracle，由Tier静态嵌入；运行时不截断full数据。生成器默认只核对，初次`--write`在实测前完成；[只读冻结核对](freeze-check.json)及[manifest](../../../fixtures/m0-t13a-tier-manifest.json)绑定三个档位的全部输入hash。full原有opening、policy、oracle逐字节不变。small／medium只在各自合法Household范围内重映射收款人，保留20个债项到期桶、工资费率／周期、8城、16周期义务、8单货运／4在途峰值、两启动设施、8池和相同720小时目标。空气页与60秒数值步长未改。

| 具名档 | 账户／库存 | 工资／债项 | 实际工资／债项实付分 | Sim经营秒 | 永久原票 |
|---|---|---|---|---:|---:|
| small | 16／32 | 8／2000 | 15080／2000 | 218.569667875 | 2955 |
| medium | 32／64 | 32／10000 | 59624／10000 | 216.616185709 | 2979 |
| full | 64／128 | 64／20000 | 118784／20000 | 227.992519875 | 3011 |

每档实际执行完整30日、723目标、724空气窗口，第7日关城／第8日恢复不补劳动，第15日第二批货运及月底终止均保留。独立算术按29工作日、30次周期义务、8单交易和启动／收件费计算，三档现金总额均3000000分；没有读取运行输出产生预期值。[生成核对](generate_fixtures.py)也重算full并要求与旧oracle完全相等。

Sim单写者、实际worker及v2 Store沿用原路径；三档rules身份分开。每档冷开、最早原票查询／重试、再次保存后完整Engine字节保持。23段原票分别连续覆盖2955／2979／3011条，没有删除唯一财务原票。最终61个世界文件各自通过大小与hash审计，总字节约6.16／7.64／9.48MB；这些是实际世界非稀疏文件，压缩zip只用于归档。

## 查询变化及独立等价性

Progress只读返回sequence、revision、完整Timeline游标和必要job／generation／incarnation，不生成全部业务／空气摘要或大表预览。`raw_at`复用同一次只读进度，避免执行前重复全量查询。当前串行单连接，没有另一条业务写入插入其间；模型仍检查expected_revision。读取revision仍构造WorldView，不称零分配接口。

每个原有目标结束仍调用完整Status，比对完整业务及空气摘要、时间、版本和无pending。每档保留726次完整Status：723个目标，加保存／冷开／重试末端3次。没有拿preview前8条作全量。终局停止Sim后读CURRENT→Root→manifest→Engine，冷开后再读完整图像；[独立验证](verify.py)比较Engine所有字段，仅在CPU／Sim比较时归零revision、后台incarnation。full的CPU及实际Engine还分别与v0.5.35对应Engine**所有字段原样相等**，这项版本对照未排除字段。

原full单次诊断342.132092291秒，本次227.992519875秒；完整Status请求4461降到726，其往返累计约125.665降到20.613秒，新增3010次Progress累计约0.091秒。必要worker仍724次，原业务／原票及完整目标未减少。两版各只有一次短诊断，不是受控三轮基准，不能直接签收四档或推断真实城市快进能力。源文件／二进制指纹、原始逐目标记录及每档完整世界均已归档。transcript只存响应hash／长度，逐目标一致依赖已冻结程序当场断言，未归档全部Status载荷供离线逐点重算；终局完整状态、原票链和算术则可独立复核。

[IPC诊断](ipc-summary.json)的计时从submit返回后开始，到收到结果为止，排除请求编码／提交本身；包含其中的持久化、worker等待和执行，不是纯CPU。表中的完整Sim经营壁钟从ready开始、包含实际编码／IPC／经营持久化，不扣这些开销；不含创建、CPU参考、终局保存及冷开。v0.5.35的IPC摘要使用同一计时起点。不能用IPC分位替代20／40ms计算批次门。

## 资源、验证与剩余门

原始observer观察到的RSS峰small 64274432B、medium 80183296B、full 110133248B；footprint分别缺测1043／1058／1126个进程样本，不能称严格峰值。每档CPU参考先运行并释放，未与Sim同时驻留；无L1。观察范围为observer及模型子树，外层只负责依次启动的Python launcher未计入。other_diagnostic_owned_bytes仍0，新宿主诊断缓冲未完整计费，8MiB联合诊断门未通过；不把这些短诊断数值接受为全组或16GB预算。各档观测swap净增均0。

定向测试覆盖三档配置、原票、错rules拒开、进度只读和小于2KiB输出。初次定向测试误沿用full的sequence88断言，在small实际32时失败；仅测试改为具名工资数＋24项启动输入，原失败日志保留，没有减业务或放宽模型。全量253项Release测试、fmt／Clippy均已实际结束通过，见verification.json及原始日志；[advisor审查](advisor-review.md)仅为阶段审查，不替代最终验收。

下一依赖是实际计算埋点：prepare/apply、编码、write/fsync、IPC、worker等待、完整边界分别报告；Claims／Payroll等按同一8192步页合并，不以逐笔微操作充样本；空扫描成本可见但不填活跃分位；quote／replay与实际提交、worker与接纳CPU复算分开；瞬时收集器不进入Engine／Receipt／检查点。仍须完成集中到期／跨页及64租约峰分支的性能证据、10类至少200有效样本、四档实际速度及P-E0完整30日≤1800秒、L0／L1／L2各三轮35分钟、联合owned／RSS／footprint／磁盘、同满级8h／96次保存及2h／4h／6h故障恢复，最后独立advisor验收。真实16GB not_run，完整M0 not_accepted。
