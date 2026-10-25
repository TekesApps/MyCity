# MyCity 当前实际执行与验收关闭

2026-10-25，当前已发布基线v0.5.189；v0.5.190有限功能证据已通过，正在发布/回填。只调整执行与验收追踪，不重做全产品规划。root会话 `01a166fc-e444-7602-ad22-3a8551dda487` 是共享核心唯一writer、集成/验收/发布负责人；旧主会话停止，不联系。

## 实际在制

共享核心上限1项、真正独立准备上限2项；本次实际 **1+0**。共享工作是#132设施操作/工资接线与回归；#148是同一工作内单独记录的预算缺陷，不计第二条共享开发线。可见subagent均已结束。领域父任务的“持续推进”、排队任务、冻结原交付和24项无关observer/M0 WIP不计活跃开发。改变上限须说明原因；当前没有新增会话/派工/target/定时任务。

当前root独占#132/#148共享修复；原#55模块owner会话 `01a16e84-6dc2-7e23-9ad9-c3ef3b8c7683` 的facility_work.rs冻结source04，root不改。全部具体文件白名单和近期合同在[机器台账](inputs/issue-ledger/execution-flow.json)。root不处理无关WIP。

## 下一轮验收顺序与责任

|顺序/任务|具体负责方与执行状态|能力/接线位置及验收入口|当前差项|
|---|---|---|---|
|#148 → #132|当前root，唯一共享在制|原Engine/Prepared/P5、成员账、原资金/工资窗口、JournalRecord/Store；core facility_operation、Store housing_move_store_runtime/ facility_operation_store及最新共享/默认栈回归|source22实测旧18单元Enable回到131072B门内、flat wire/null/copy、原96B Wait拒绝/原票冷恢复、设施Store5+3cold均通过；最新同源18组共享、资金保护补验、10组受影响及6组原默认栈均通过；正在准备v190发布/最终Issue回填，未提前关闭|
|#133|root接线责任；排队未派工。原#92 owner `01a16ffe-dbe1-7a62-b610-c5e72d490d7e` 组件冻结|复用原Electric/资金/票132、源144/v185、净化组件v186/187与费用曲线v188；同源Runtime/V2→原Stock/P5/Water工作/Store|真实gross/outflow/net、联合12/24kW、原质量/药剂/残余/Water人员成本及原票恢复尚未接线；仅架构修订3已定。计划root adapter/tests路径见机器表，不授予并行写权|
|#134|原#57 `01a16a4c-ba5a-7dd1-8c04-51461a76b405` 交付冻结；root负责接线，当前阻塞未启动|原service_delivery_runtime、成员/Resource/Lease/need/资金；实际服务Store入口待固定profile后实现|真实人员、交通到达返程、用水及有限接收/残责尚缺；73/74计划/状态不能当履约。缺具体来源时先转可执行积压，不等完整交通父领域关闭|
|#135|原#101 `01a1682f-f3ce-7f61-833c-a31d55a43b34` 源冻结，root后续接线；未激活|原task_registry冻结版本+130/v185真实due消费者，接原Timeline队列/writer；专属registry/Store入口|通用source/due/dirty、版本/age/覆盖、恢复唤醒到原调度尚缺；不另造执行器|
|#136|原#118 `01a1682f-f0de-7241-b895-0bbfa1386c5b` 源冻结，root后续接线；未激活|v149固定manifest的physical_capacity模块/测试/fixture；复用原Resource/Lease及125场所能力|通用canonical别名/模式/时窗/实际占用与安全退出恢复桥尚缺；激活前确认原文件清单/API和真实输入，不以source候选为容量池|
|#137|原#120当前聊天worker交付冻结，root后续接线；未激活|原opening_loader/显式文件provider，131同槽Store有限能力；typed provider装配入口|通用factory/schema/单位clock/引用provider连接及原同槽失败恢复尚缺；文件hash不是权利认证，启动前固定专属adapter路径|

上表“负责人”区分后续集成责任与正在持有写权的实施方。只有#132/#148占用当前共享写权；其余是有来源的排队合同。各拟新增测试/adapter注明planned，尚未创建/运行不算交付。未来独立准备优先原实施方补独立oracle/反例/输入/恢复证据，不为填满两个名额派新源码模块。

## 当前证据与停止关闭门

source18共享回归13组终态：前12组通过，迁居组3通过/1失败；余5组未运行。没有把早期source16/17专项当作共享通过。原18-unit合法前置越界反例source19：120584+8456+2048=131088，超原131072门16B。source22物理可选carrier实测省16B，World74224B/align8；原96B Wait原断言和冷恢复也通过。原advisor解除公开DerefMut封装阻断；它没有执行测试。

#148/#132同游戏字节的18/10/6组共享/受影响/默认栈、资金保护与独立冷恢复及完整证据核对已通过；提交/tag/PR最终回填后才关闭。角色8MiB、normal64MiB、Engine4MiB/World128KiB、原M0负载/标准保持。Mac32GB功能结果不等于真实16GB/native/快进/长压通过。完整失败源码、固定程序、原票/WAL/checkpoint和恢复原件仍在HP P900 work，不作测量期回收。

## 交付速度基线

基线观察时点：2026-10-25 02:39 UTC。按GitHub实际状态回读，业务交付积压为21项待集成、1项待验证（132）；父任务、UI延期与缺陷不混入此数。列表见机器表及work/execution-flow-20261025/baseline-live-readback.json。

- 基线以前：历史业务补登记关闭11项；此前原F/E有限业务验收2项；此前独立栈缺陷关闭3项。它们分别列账，不相加作为本轮新交付速度。
- 本统计窗口：新验收业务0、历史补关0、缺陷关闭0。预算单测或专项过门不是一个新验收业务。
- 可实施→验收、源码交付→验收：历史可靠首时点不足的记unknown，当前不填0或凭Issue创建/mtime/tag推算。首次确认就绪和真正验收分别留事件；旧相关组件交付receipt只算其等待年龄，不冒称当前完整合同首交。
- #134相关旧组件有2026-10-23 20:19 UTC交付receipt，#135/#136为18:23 UTC，#137为18:53 UTC；与完整新合同版本的关系单独标明。完整验收尚未发生，尚无完成周期样本。

按阶段更新现有文档/JSON和Issue即可；不新增调度系统、后台统计器或定时任务，不报告总Issue关闭比例或tag数量作为产品完成度。

最新执行更新：source23同游戏字节18组主回归全部覆盖通过；原运行器缺环境错误与证据预算中断分别保留。资金保护/权限/实际4c/余数/失败原票和Stop恢复补验通过。额外10组受影响回归正在串行，随后6组默认栈；实际关闭仍为0。

终态更新：extra23-combined10组、默认栈6组和validate-acceptance23一次性完整有限证据核对均通过。当前仅待v190原子发布与Issue最终回填；尚未关闭，不把通过测试先记为业务交付。
