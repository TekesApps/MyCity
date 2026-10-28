# v0.5.212：原角色与 Air owner 的资源来源（#102 有限接线）

root唯一共享writer，专属测试原worker准备后冻结，由root核对与串行执行。本增量完成并冻结，当前0共享+0准备，无PR；完整102/103/104保持开放。不新增业务验收或关闭，不改原M0冻结门。

## 实际实现

SimHost沿原Authorize→activate→ack→ready保存实际角色准入：`BrokerAuthorizeReceipt`含原kind、pool boot、generation、capacity；standalone只表示本地arena激活。原`activate_role(kind)->epoch`保留薄封装，其他角色不新增权限。角色报告有独立schema、进程实例、epoch和采样起点；全角色Rust arena当前及生命周期峰值不冒充任务scratch、CPU配额或native峰值，不能与原角色容量相加重复计费。

Air当前owner和历史execution分别报告。仅原同源码固定normal-pool-v1 Prepare/Started成功owner所对应的固定输入/候选槽+typed Air工作区合同容量可知；这不是RPC回了bytes、使用量或全子进程峰值。pending未确认不补造容量；停止已发但未确认保留retirement_unconfirmed。确认退休后current owner为空，旧CPU和原合同来源仍可保留，不能计为当前活授额。下一attempt使用原新身份。现有12KiB共同报告界限、64KiB元数据规划收费、59MiB Sim角色与64MiB normal池不变；没有新池、授额器、返额路径或wire控制字段。

## 实际验证

2026-10-28 04:35:15–04:51:26 UTC为本阶段窗口，不是完整Issue开发周期。全部同一source01、debug/default stack、原SSD唯一target/work串行；48项父测试通过，无本轮失败。2个ignored入口由其父测试显式隔离调用，不计父测试通过数。

|检查|父测试|证据范围|
|---|---:|---|
|真实pooled Sim/broker Air、standalone、direct Air|3|原授权与池账、精确分页暂停/取消、历史与当前owner、新attempt身份、真实Prepare/Attach/wait及整池归还；共同报告有界且无key|
|原broker组件与启动owner|4+3|原注册/角色权限与重复处理；真实owned child在原wait/返还之后丢回复仍未确认，原票重试后确认，返额仅一次；组件fixture不称真实Sim/Air业务|
|原共同执行摘要/精确页/共同协议|1+3+9|原绑定、实际候选、公服/供水共存、拒绝/重复/过期、取消保存冷恢复、未确认发送/丢终态|
|原Air控制/恢复|13|暂停/Continue、自动控制、原broker和实际退休、取消与冷恢复|
|原丢Started/Stopped回复|1+1|真实进程/原租约等待重试、原票恢复，不重复授额/返额|
|Sim实际写盘/观察故障与Air投影|5+5|半WAL、提交后丢回复、不可用时钟、记录拒绝/重放与原保存恢复；候选和未知计量不冒充完成|

全部460个递归构建输入（加AGENTS原件）逐字节对照完整source01 zip与当前源码；实际ready SourceSHA为`f73d690584adc0aa284ed77b795ffa693c44885af680cdfe4e458a21f9c84cb3`，与原build.rs算法重算一致。原pool ready boot与角色回执boot一致，原epoch等于实际generation。没有用旧程序结果替新报告/启动路径回归。未受影响core/Store业务算法的既有验收仍对应其原固定版本，不称本轮全套重跑。

实际pooled Sim原授权61865984B，Air固定合同5242880B，原总池67108864B。真实两次Air授额/归还与一次Sim角色授额/归还后整池可用；direct两次原grant/return平衡且未虚构Sim授额。已观察角色Rust请求峰56805038B、分配失败0。这不是物理内存或代表城市容量/速度证明。停止丢回复组件中原池已返还一次但客户端仍unconfirmed，原重试确认后returns仍1；局部报告不是返额许可证。

## 未完成与保留

完整102仍缺真实模型细化/额外资源授权接口、每任务scratch/常驻/转换成本来源及更广真实模型覆盖；103兼容闭包、104全局竞争预算/公平/最低可信背压各保留完整范围。GPU、真实16GB、native/进程峰值、长期漂移/代表城市推进速度、完整M0与UI均未运行或未验收。advisor仅针对来源与语义的架构疑点提供只读建议，不替代实际证据或全实现审计。

完整初始/最终源码、固定程序、角色/pool事件、World/WAL/checkpoint及故障恢复原件位于HP P900 `work/task-resource-facts-20261028`，短pool根`work/rf102`；约336MB逻辑字节，事前1GiB预算内。保留至明确后继验证与恢复依赖释放，hash不替原件。无新worktree/target/timer，现用target与原件继续保留。24份无关WIP逐文件字节一致。

硬件Mac16,1/32GiB，macOS26.6.2；HP P900 USB/APFS原target/work。资源数字为合同/有限Rust观测，不作性能基准。新业务/历史补关/缺陷关闭增量均0，源码包待集成积压18、待验证0保持；完整102不据此关闭。
