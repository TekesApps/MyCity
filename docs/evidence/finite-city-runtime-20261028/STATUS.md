# #21 本城共同输入：首段共存与恢复

v0.5.215有限增量，**完整#21仍开放，本阶段新验收业务0／历史补关0／缺陷关闭0，18完整源码包积压不机械减少**。这不是可持续经济、全产品、原M0或真实16GB验收。

## 已实际验证

新增显式 `pboot-local-development-v1`，无Region/Exchange/Economy，可含6..MAX_STOCKS总库存，六个Boot绑定保留同一商品/owner/容量/来源/配方与物量成本守恒。原micro/scale接受形状不变。仅增加标签及装配/恢复分支，无新增常驻字段。显式原JSON可用；typed Opening provider未新增支持。

同一seed107/city1开局，家庭7/账户2入住、原公服Police任务实际逐边到返/工资、企业7的原Timeline实际计时薪酬与两个Town日结，原Boot先完成泵再完成机组，未使用未来产出启动。每日现金总量3100000分；原料每日减少100；400000mWh已完成发电量每日实际消耗100000mWh；Boot.time与已完成Timeline一致。每日销售1000分−材料100−公用费用100−工资80=720，原50%日税360分，留存360分。代码独立常数断言，不以运行输出生成预期。

restore09默认栈3场景通过，包括原两profile继续拒绝额外库存本地形状、新本地profile拒绝Region。restore10同Store正常场景通过，**两次实际独立冷进程**分别从day1 WAL和day2 checkpoint恢复、验证service原来源、原票重试不增加序号/改变hash，继续完成第二日。完整存档及子进程原日志保留。

受影响的旧Boot、Town用电、Timeline、Store Boot和Town用电恢复沿restore06/07串行通过。之后生产唯一差异是FrozenLabel明确新增此标签，旧分支和类型未变；当前checkpoint冷恢复实际覆盖此差异，无依据不重复所有旧回归。逐程序SHA、源码对照与命令见[summary](summary.json)和[source manifest](source-manifest.json)。未运行其他共享全量检查，不把这段说成全库通过。

## 剩余业务差项

- **下一实际接线：Boot输出到公服消费。**当前公服991/992仍是独立有限期初库存；不能称为本次Boot产出。直接把stock引用指向Boot输出会漏记原exports/basis，原守恒门拒绝。需沿同一Prepared、原Stock和明确service分类计量，不造第二池，不让Town/工程代记；水仅原有限Police非饮用用途，不伪造卫生资格。
- Town Payroll是既有账户级合同计时工资，与公服逐成员活动账的唯一成员/时窗联合约束尚未证明；不能用这份工资场景宣称完整劳动配置已接通。Town货物流通仍固定quote，不代表内生全城交通。
- 整体供需/融资持续性、真实资源门和更长产品支持速度未验收。只针对实际阻断补齐，不升级为全交通/全资产/公用网络前置验收。

## 失败、原件与预算

01/02因原Boot profile要求Exchange与至少14库存而拒绝。03共同输入缺税权所需股权开局，04在泵完成前启动机组被拒绝；各自独立源码ZIP/manifest、固定程序/log保留。08 WAL冷恢复通过，但checkpoint因FrozenLabel未登记新标签失败；原失败存档、程序/源码、stderr保留，09/10补明确标签后通过。所有失败不重标成功。

完整证据 `/Volumes/HP P900/dev/MyCity/work/finite-city-runtime-20261028`；唯一target/work沿既有外置HP P900 USB/APFS；Mac16,1/32GiB/macOS26.6.2。24份无关WIP逐文件SHA保持。原512MiB证据磁盘估算被多个固定debug程序超过（观察726348KiB），已记录并修订2GiB证据额度；不是游戏内存/M0预算调整。保留当前与失败程序、完整源码及恢复原件；未退役恢复载荷，不清其他项目。无新worktree或定时任务，无PR。
