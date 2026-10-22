# 2 MiB 候选联合诊断 02：功能与冷恢复通过，normal 仍失败

固定候选源 `ade347ccd333e9f4cdbda7f08066f720bbd7649a6d58762987445750a0db4e05`、231文件，CLI SHA `93623d74dc58e13a7c98f516a49512795fe40c99d21104d0d8363dd9546856ea`；原负载/输入/normal阈值不变。它不是本阶段主线集成 d19e4a88 来源，2MiB候选没有替换 main 的1MiB默认。

先实际重跑22项辅助测试，通过；原01错误的验证器上限已绑定冻结Rust MAX_EXTENT_QUANTA。新进程 observer/model 退出0，controller error null，4个layout/pool的idle/inflight VM捕获身份及阶段/epoch/list端点一致；还贷后的拓扑也实际验证。见 [终态](terminal.json)和 [地址审计](address-audit.json)。跨 snapshot admission 的固定pin同一性不作声明（原固定事件没有该身份）；地址覆盖不当物理credit。

[原始采样重算](measurement-summary.json)：113帧、81完整帧，complete peak为3,337,014,824B，原normal3,328,599,654B，超8,415,170B。没有非normal压力采样、swap净增长0，但这只是28.099秒短诊断，不是正式分级/速度/稳态/8小时验收。与旧1MiB短诊断complete peak相差-16,432B；不是同机受控重复统计，也不证明2MiB有足以解决超额的收益。

[两代独立冷恢复](cold-result.json)分别在新进程完整hash/load/scan退出0，R07历史credit不继承。16GB/真实8h均not_run，完整M0不通过。旧01错误、旧1MiB超额和本轮原始采样均保留，不覆盖失败。

## 回收与保留

独立advisor逐项审查恢复依赖后，针对新02建立一次性回收计划。首轮prepare在一个进程恰好退出时遇到ps返回1，安全停止且未删文件；错误日志保留，修复为明确验证“进程不存在”，并将lsof递归检查改为解析后的外置work/target实际路径。所有运行/冷恢复终态、实际占用、190原件全SHA和其它文件身份守卫通过后才执行。

[实际回收结果](retirement/result.json)：精确54文件，包括两代4个pressure/reserve及50个与原index SHA/长度、既有gzip完整解压字节都相同的组件，分配空间6,748,553,216B。卷可用空间观察增加6,748,536,832B，另列且不声称APFS独占物理回收。回收时work剩26,699,140KiB（约25.46GiB）、target3,716,080KiB；这是该时点，后续构建会继续复用target。其余134,064文件身份metadata及源码WIP不变，190保护原件再次全SHA通过。

两代新02快照已退役，不能直接完整恢复。保留原index、原始采样/VM/heap/拓扑、运行/冷恢复/故障收据、既有gzip、源/锁/固定exe以及不匹配组件；190份仍需恢复的原件完整保留，不以hash替代。未新建target或新增归档载荷。外置HP P900 USB/APFS SSD用于work/target，旧内置盘IO结果不复用。
