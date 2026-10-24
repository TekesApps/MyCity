# v0.5.184：固定连续供给与原账联合生产

基线 v0.5.183 / `8295ad192b33ec56dc0001a24fdd4ce52f9f4898`。
原 #85/#86 实施方和 #57 原实施方负责专属模块，root 单写共享核心、串行集成验证；
原 advisor 审查与失败后的窄复核无剩余阻断。**完整 #85/#86/#57/#120 继续 open，无 PR。**

## 实际接线

`ContinuousGensetTwinPumpV1` 显式新有限 profile，复用原7个 Stock、原钱包、3个
Resource/Lease 和原 Timeline/P5、Store 原票。机组每60s实际消耗 Fuel1Q＋冷却水60L，
产生电400Wh、本征热600Wh、废水60L。每台实际运行泵消耗6kW及原水10L/s，产生
过程水10L/s及泵热6kW；未接收电转明确热接收库存，总热60kW。没有新电库存或第二账。
热和废水在有限接收库存持有，**没有据此宣称已排放、已处理或取得卫生许可**。

共同可执行前缀受原料、热/废水及各出水口容量约束；原 Timeline 在首个硬边界切段，
保留已经运行的前缀并明确停机。真实 Stop/Fault 返还完整未转化预取料及原成本，
分数燃料 WIP、成本余数和原身份不重置。启动费为机组300分＋每实际泵200分，
同一 Prepared 一次扣至原供应商；重试不重收、零工时停机列费用、真正新 run 再收费。
固定 mask 的原 lot 累计成本按实际过程水分配；改 mask 必须实际停/启，不重新分摊旧水。

V3 在该接线中核对实际同窗 uniform 物理前缀，原 owner 持久精确终身表数及余数。
V3 的 ceil 计量不产生预付电库存或下一窗能源。只支持本次有界零损耗路径，
不是开放网络、完整爬坡/备用/电池 SOC、跨资源任意联合投入或自动卫生认证。
当前入口是原 Timeline/Advance；#101 的 RunDue 新供给 lane 尚未登记，旧 NoWork
只代表它已覆盖的 lane，没有将活跃供给或所有必要业务宣称为空闲。
当前 profile 排斥旧生产领域绑定；补货、混料和分类输出出口待独立接线，通用 MoveStock
不能绕过受管 lot。最多16个 run 后背压新启动，原停机与恢复责任不删除。

#57 组件修复了实际返程后历史前缀、截止前未消费历史，以及 WorkComplete 零工时覆盖。
22项组件测试通过；**真实工资、派遣资格/活动账接线、现场卫生水、实际到返履约仍未完成**。
自然 need 的现有 `delivery_ref` 和已发生事实没有因组件测试而被改成“已交付”。

## 实际验证

[结果](results.json)、[汇总与独立恢复PID](summary.json)、[实际 Rust 容量](layout.json)、
[固定构建输入](source-manifest.json)。全部串行，无默认栈或 M0 负载/标准放宽。

| 组 | 不同 release 父测试 |
|---|---:|
| 原单位、lot曲线和校验后的 Carry 恢复 | 6 |
| 联合候选预算边界 | 1 |
| 原 Construction/Boot、原 lot 兼容控制 | 2 |
| #86 连续机组组件 | 24 |
| #85 V3 有理计量组件 | 20 |
| #57 派遣状态组件 | 22 |
| 原账联合供给、回滚、硬边界及未知原票 | 9 |
| 原供给 Store WAL/CP/重试/非零时点重启 | 1 |
| 原 OpeningVenue、原 OpeningService 兼容 | 2 |
| 合计 | **87** |

同一新供给 Store 父用例另以默认 debug 执行1次通过，不算新增不同测试。
8个 release、2个 debug 独立恢复子进程全 exit0，无 timeout/wait error；
辅助子进程不重复计入父测试。新供给父用例真实读取 WAL、生成并读取检查点，
核对原启动票重放不增加 sequence，再执行 Stop3→CP→Start新run于3→Advance4→Stop4→CP。

## 失败及修复原件

- source03 workspace check 发现 Carry 缺 Deserialize。共享修复经既有 restore 门验证
  分母/余数，原 Serialize 及累计算法不变；原 advisor 窄核后 source04 构建通过。
- source04 的83项前置测试通过，新 Store 父用例在最终显式 Stop 后冷开失败：
  控制票缺原时钟锚，恢复拒绝认证。失败世界、原 WAL、两代 CP/根、源码和可执行文件保留。
- source05 静态审查指出时钟锚赋值位置错误，**未编译/执行**。source06 在原 Receipt
  最终生成处加入 ContinuousSupply，并要求控制命令已有原 Timeline；恢复要求真实
  Some(at)，不将缺失时点补成0，不放宽认证。source06 重编和上表实际验证全部通过。
- source04 是未发布实验，不声称新策略能直接打开其缺锚坏检查点；保留全套原件和
  原程序，而不是只留 hash 或抹去失败。

## 资源与保留

实机 Mac16,1 /32GiB、Rust1.85.1，外置 HP P900 USB/APFS；target/work 仍唯一原链接。
实际 Rust owned：Engine3,603,736B（初始化、启动、推进1秒相同），新 Runtime1,856B，
机组候选952B、7个 stock 读副本672B，样本 CP 编码5,884B。
新 optional World v14 缺省省略；原128KiB基础 World、4MiB Engine、8MiB角色及64MiB
normal 总门不变。16KiB owned cap／96KiB联合准备预留经原共享候选门核验；
**这些是逻辑容量/预留，非 native 峰值、全进程预算、真实16GB或可持续速度验收。**

429份构建输入、固定程序及24份无关 WIP 执行前后哈希一致。构建源码归档约1.8MB，
只含实际构建输入；不像上一阶段归档9717份历史记录。初始产物预算768MiB，未事后扩额。
完整原件、固定程序和所有失败位于 `/Volumes/HP P900/dev/MyCity/work/continuous-service-supply-20261025/`；
保留至替代完整恢复基线验证且无依赖后逐文件退役，当前不删恢复数据/target或其他项目内容。
真实16GB、native/进程联合峰值、GPU、快进承载、长压测及原M0验收均未在本阶段执行。

两棵已停写专属树 #85/v147、#86/v158 已按[精确退役记录](tree-retirement.json)清理。
全部14份 untracked 原件已逐文件完整复制到上述持久 work 的 `retired-tree-originals/`，
复核 Git 身份、无 tracked/ignored 新改动、lsof 无占用后才仅移除这两棵标准树；
分支、common repo、main WIP、target和全部恢复证据保留。记录的卷空闲变化不是隔离的
APFS物理回收测量，不能按目录逻辑大小宣称释放。未新增 worktree/定时任务。
