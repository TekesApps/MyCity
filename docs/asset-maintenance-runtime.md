# 单资产维护的实际履约接线

范围：#36 的业务子任务 [#159](https://github.com/TekesApps/MyCity/issues/159)。复用[P53设施规范](product-facility-and-equipment-profiles.md)、原维护候选、原建设资本与[实际劳动](facility-operation-runtime.md)。本页是有限接线合同，不是完整维护、现实工程认证或已验收声明。

## 当前实际状态与权威边界

v0.5.226基线有原#36隔离候选，但没有真实维护工单、材料消费、停机和原票接线。root已核对原patch SHA与完整原件：Snapshot/advance/deliver/due_page可消费；candidate不能证明钱物或工时已经发生。现有facility_work的实际工时来自连续供应设备work_covered，不能直接用于停机保养。原advisor只读窄核建议薄维护authority与原劳动内核；不是执行验收。

当前实现第一步仅分离维护与生产工时来源、接入候选及核验新开局资格/钱物来源；完整维护authority、停机/材料/费用/Store和normal门仍待实际完成。源码、测试、当前结论见[执行清单](execution-current.md)。不得据单元或来源门关闭159或36。

## 一个明确的最低可信策略

1. 单个construction实际投运资产、单被动电气支撑部件，普通30DAY服务周期从真实commissioned_at起算。只支持新开局明确注册，不给旧资产重建年龄或猜历史。原capital basis/残值/投运/折旧曲线保持；物理设备仍可在折旧完成后工作。
2. 本地冻结维护profile只适用于该电气部件，明确要求原Electric基础资格及实际在场，不让所有维修默认适用Electric。新有限fixture可显式注册合法的初始资格和有效期，但不改旧档，不以非零source ID替代实际writer读取。保留旧Driver/Police资格以验证业务共存。
3. 本首门被动部件只按日历磨损，操作磨损率明确0，无事故损伤输入，不冒认Boot批次为另一资产的运行暴露。磨损/改善参数由新开发者profile冻结并单独标游戏参数，不能引用测试数当实测物理规律。替换、桥、Interaction、自动事故及多档另属原父域。
4. P53至少一净班组小时必须4名实际相容人员工作3600秒=14400人秒。原工资算法按每成员小时价与真实区间累计，余数、原保护资金和PaidWageWindow来源保留；不复制工资/资金账。maintenance JobKind与生产RUN来源类型明确区分，旧连续供应provider不能接收维护工单为产出。
5. 新工单的明确配方需有相容的有限实物Stock及原lot成本；参考0.001K和参考单价只决定合法配方，不把现金变材料，也不改实际basis。候选只有在root Prepared实际消费人秒/材料后才能推进；部分工料的实际改善沿原min(work fraction, material fraction)及累计差额，不能凭付款恢复condition。

## 真正的停机、现场与退出

- 临时停机不调用Construction Withdraw，不触发永久停折旧/拆除。首个明确新fixture的资产operating_base_capacity=0、Resource初始0，投运真实发布2单位；原base2 fixture和M0保留。这样停机不撤回归属不明的旧共同容量。
- 停机Begin必须核现有及未来已接受占用/预留，先完成安全停止；从原canonical资源暂时扣除本资产实际贡献，新的使用入口不能获得被停贡献。拒新不取消已接受义务；资源使用与维护互斥，复位只恢复原贡献一次，不建立另一容量池。
- 维护现场必须有自身明确的owner/operator、用途、成员范围/完整窗口及空间容量来源，并沿原Venue Accept/实际Enter记录。不能延长旧owner1/operator3的0..7200生产场地offer冒充project7在30DAY后的准入。显式可信host Enter与自动交通到达要区分：即使采用原显式host活动版本，也不得把路径计划/预计到达当Enter，不能声称员工通勤已经自动模拟。
- Timeline按真实完成边界分段：日历due、资格变化、工单开始/停止/完成、人员退出、日结均须切段。无产物的维护时段独立于生产覆盖，但仍由原成员活动账排他；不开第二时钟/worker/调度器。
- Stop只释放未来人员活动，不归还已耗材料/抹除工资和余数/重置进度。必要due、已改善condition、未付工资保留。部分完成保持停机，可冷恢复继续；实际配方齐备且安全复位才解除停机。工单的结束状态不能取消法律或财务责任。

### 停工再续仍是同一个劳动合同

原运行工资记录要求work_seconds=settled-started、Stop终态。维护不能照搬此连续假定：维护专属Resume保留首次begin/started、累计应得/已付/余数及工作量，另留真实空档秒数和最近续办来源（新成员活动id、当前资格读取、上次停止票）。只有明确ElectricalMaintenance v2可续，原生产合同仍终态。总历时=实际工作秒+空档秒；空档不计工或工资。每次续办仍由原成员/资格/在场/Funds核验后准入，不自动复用旧活动或重新计一份工资。

新增摘要字段在旧记录为0/None并省略序列化，旧格式缺省读回；仍需原Store回执核对，DTO结构验证不是历史真实性证据。已丢的逐段细节不能由历时平均补造。按需payload/encoded最大包络及恢复副本一起计费，不能加大8KiB或省略必要字段换通过。

## 钱物、费用与保存恢复

材工仅按实际发生费用化一次，进入原资产实际费用与Town税消费者；不是Construction.spent变动、不是自动资本化。支付只清原债，不能重复费用化。参考年度维护费、独立报价及实际工资/材料的承担方须明确，同一来源不得双收。资金保护沿原Funds，原工资应付和已付分别保留，恢复不能重抽或重计。

可选按需维护backing绑定原project/owner/投运票/部件代次/profile、完整candidate snapshot、最早due、工单状态/区间与材料余数、原劳动/费用/复位回执。全套同prepare/validate/commit和Store root发布；无配置不分配此payload。owned包括heap、索引/恢复工作区与候选副本，Store包含Engine不能重复计费。旧明确版本保持旧语义，缺来源旧候选不猜迁移。

## 文件所有权与执行门

root共享唯一writer：maintenance_authority、assembly按需backing/owned、transaction/原票/资源、timeline_driver、facility_work来源适配、旧facility_authority生产拒绝、member_activity活动kind、asset_cost_authority真实维护费用；restore/decoder额外文件先按实际接线核定。独立准备仅原asset_maintenance模块/专属测试与maintenance_opening/独立oracle两项，交付后冻结，由root接收。

验证依次：原18/128KiB、Journal4096、无配置/同槽缺省及默认栈；新来源装配和独立劳动/候选算术；实际due→停机→部分材工→复位与旧业务共存；成员资格/在场/容量/材料拒绝、欠薪后付/暂停恢复；原票重试、WAL/检查点冷恢复及写入中断；受影响共享和原normal角色/恢复workspace门。0项选择不算通过。完整合同通过才回填关闭159；完整36/117保留剩余范围。

仍用原HP P900唯一target/work串行，不改M0负载/Wait/预算。24无关WIP及所有失败源/程序和必要完整恢复原件保留。真实16GB/native总峰值、产品持续速度/人口、长期、GPU和UI不由本门签收。
