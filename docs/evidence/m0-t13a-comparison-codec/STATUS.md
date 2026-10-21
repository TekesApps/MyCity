# 借用业务／状态比较编码（v0.5.75）

本阶段Rust模型及CLI实现已写入，冻结source `1b31214ad930d89b875f699b9b762a8da33c597b13a40f8db2f73ccf821336df`、169文件。完整同源回归、clippy、fmt、build及旧完整world实际兼容／比较验证均已终态通过；只签本有限实现阶段。正式性能、分级稳态、四档、实际8小时及真实16GB门均未新增通过声明。

## 改动与边界

- caller-owned Codec初始化4MiB背板，复用现有可信canonical遍历表；类型序列化后直接规范输出给writer或SHA，无完整Value树，不给每个Engine私藏一份codec。输入／规范输出均4MiB上限，错误返回不发布摘要，后续调用从零开始。
- business借用完整World，只将revision设0；state借用WorldView，账户按原ID顺序、库存owner转换为账户ID、真实revision、全部资源／租约／next_token／Timeline／Background保留。两种视图不可互换。
- Reference在目标轨迹和报告之间串行复用自己的codec。Sim业务摘要及Store联合状态摘要复用Store的codec，明确可失败借用；WAL独占路径沿用同一遍历表，无忙时新建替代槽。Store联合schema／spatial仍保留。
- ordinary Compare与Reference恢复尾部业务对照先写临时文件，关闭写端后用同一codec生成另一方、8KiB读回缓冲和4KiB栈片逐字节比较，检查短输入、差异及尾随字节；成功关闭后清理精确临时路径，错误保留文件供诊断。没有用仅hash代替完整比较。
- convenience Vec／state_hash接口仍可创建临时codec；ordinary Compare完整Engine及空气checkpoint Vec、legacy v1、真实池授权等仍未闭合，不把本阶段描述为整条路径已固定容量。

## 同源实测

| 实际证据 | 结果 |
|---|---|
| [源与固定exe](source.json) | source1b31214a…169文件，binary94de0e27…；源zip逐字节与当前源码相同，依赖锁未变 |
| [最终检查](checks-final.json)及原始workspace-release日志 | session27121终态0，554.620秒；383 harness pass合计，含nested、不当成383项独立业务验收；clippy/fmt/build均0 |
| 新增定向反例（已进入最终全仓） | full20,000claims／64payroll及真实pending Job，旧Value的business与state完整字节／长度／SHA相同；writer失败、输入超限后同槽复用；文件短输入／差异／尾随数据拒绝，Store/WAL共享槽保存重开及重试一致 |
| [旧完整world实际重开](compat-proof.json) | 原rules完整243文件输入，sequence3013，全部Engine2,507,418B与原CPU参考字节相同；原world及保留兼容副本前后SHA均未变 |
| [独立旧Value完整字节oracle](codec-proof.json) | business1,818,613B、state2,480,116B，均小于4MiB，完整长度／字节／SHA一致；Store schema/spatial联合SHA一致 |
| 终态 | 接续session29915的capacity／legacy／codec三个进程均退出0；未重启既有回归或把观察超时当终止 |

完整旧world的business SHA为`17b4cbd4271a18e8bd3ec0ddfe1b1bfe2b33081e3a3f87db79ccd7f87464f432`，state为`2c0464b46d2d83a48976f955a063e12f24ba547860b47f7e49bc45d29b9d48e8`。原完整Engine SHA仍为`f4c922bfc7a6a214e73d1c69c26386b3faa2465794dc9f77f99793a554246e02`。

[旧Value helper](codec-proof.rs)会分配独立oracle树，只作正确性证据，不作有界工作区或性能证明。10次复用hash核对共0.687877秒，含独立预期SHA重算；这是诊断观察，不能当冻结类别的p95/p99或证明快进吞吐。新有界编码减少DOM并不自动意味着更低延迟，后续应在完整任务计时中确认。

## 容量与advisor

当前Store比较4MiB与既有角色IO4MiB仍同时存在，Reference也有自己的codec。目标表分别列账，不将候选复用设想抵扣实际分配。另计保守Reference控制头和比较缓冲。实际[容量CLI](capacity-check.stdout)基础54,078,832B，加两最大Raw空气归一化暂态1,325,248B，共55,404,080B，低于原67,108,864B，余量11,704,784B；见[完整性及算术](integrity-capacity.json)。全部仅为目标算术，runtime_grant_accepted=false；最后必须完成实际全图broker准入与物理联合核算。

advisor只读审查发现Store抵扣RefCell头会漏计借用状态，已修成仅追加codec背板；Reference报告包含其非Engine头及背板。advisor确认具体修复消除该阻断，其余有限范围无新阻断；最终实测如上，静态审查本身不作为性能通过依据。

## 磁盘与下一步

共享target复用，无新独立缓存或压力快照。固定source.zip及后续验收exe、日志保留；当前完整业务、兼容副本、v70两代及v50旧格式完整布局仍保护。回归运行期间不清理；阶段终态后按验证产物回收规则重新检查占用和依赖，记录清理／保留原因。

[阶段磁盘记录](disk-stage-end.json)：work18,060,080KiB（约17.223GiB），共享target1,023,380KiB（约999.395MiB）。新增固定exe／helper及源码日志保留，无新压力副本；成功临时字节比较文件已由调用方关闭并精确回收，未积累新的完整world副本。阶段末无work文件占用，未发现其他确认可退役项，额外删除0B；缓存因当前后续构建及历史证明关联依赖保留，不把df变化归因于清理。v70／v50完整布局、业务原world及完整兼容副本继续保留。

下一步针对剩余的完整Engine/空气Vec、legacy和实际模型／worker／Reference／Sim全图准入与归还，确认原64MiB与common8MiB真实约束，再做正式分级、四速、三轮稳态及28,800秒8小时。原负载、CPU步数与门限不变；当前32GiB机器，16GB not_run，M0 not_accepted，目标继续。
