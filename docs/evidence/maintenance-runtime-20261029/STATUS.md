# #159 第一门：真实来源准备与维护工时种类

v0.5.227是进行中的接线检查点，不是维护验收通过版本，159/36/117保持开放。root唯一共享writer，两项专属准备已结束冻结，活动准备0；无新tree/target/计时器。消费原36冻结交付，不新造模型。

## 实际改动

- 接入原asset_maintenance.rs及专属literal候选测试，源码原SHA47174f…09ab与交付manifest一致；只加近期authority所需只读标识/时间/profile/代次入口。尚未在lib注册/接入World维护状态，不消费原库存、不改变condition，candidate不会提交事实。
- 原facility_work增加明确ElectricalMaintenance v2 role及私有ObservedWorkKind来源：只能接受维护order proof，生产contract与维护proof双向拒绝。沿原工资/余数/欠薪/付款内核，未增加工资账或持久字段；旧连续供给provider明确拒绝新role，不能把保养伪装成生产work_covered。
- 新有限maintenance fixture保留原10人、钱物/住房/Stock来源，显式为[0,4)增加Electric且保留Driver/Police，期限覆盖投运61200+30DAY；新资源81初始0/base0，投运才新增2。不改原base2或M0。无多余issuer或Payroll承诺，不拿它们当实际维护来源。
- 新fixture当前不登记维护现场，不复用旧owner1/operator3、0..7200的生产offer。现场、实际工单、材料和原票属于后续同一共享接线；[完整有限合同](../../asset-maintenance-runtime.md)。原advisor[只读一次窄核](advisor-narrow.md)仅提供架构建议，未运行验证。

## 实际串行结果

|门|固定结果及边界|
|---|---|
|原廉价兼容|sources01旧18/128KiB通过：World74224、120568+8456+2048=131072；原Journal3488≤4096。未改字段/原负载/Wait|
|初版来源红例|sources01读取资格前没执行Enable，实际InvalidOperation/exit101；失败原程序和源码ZIP完整保留，不是生产bug修复或通过|
|最终来源门|sources02执行真实Timeline Enable后，新4人Electric及旧2人Driver/Police由原writer实际读取Eligible；原人口/现金/Stock/住房全等，总现金3100000|
|工资来源与原回归|原facility_work内核19通过，包含新生产/维护来源双向拒绝、4人3600秒=14400人秒/8000分、切段一致、Stop保留8000应付、付3000仍欠5000及DTO恢复。它不是实际维护场地/Store接线|
|原维护候选|原literal10通过，精度/工料/到期与拒绝是候选算术，不当Store或业务验收；原桥/Interaction等测试不表示本轮扩展这些运行范围|
|原实际设备劳动|facility_operation13通过，仍只覆盖原连续供应业务，不将其直接转换为维护工时|

sources01→02仅专属来源测试补Enable，生产/旧输入与候选字节未变，已过廉价门和后续固定程序可按原manifest复用；没有重复全量测试。最终manifest与当前source逐文件一致。默认栈、未设RUST_MIN_STACK，原HP P900唯一target/work串行。

## 唯一当前接线点与未验范围

下一继续同一159：project7维护现场合法offer与实际Enter→临时停机/原资源互斥→实际材料/成员/Funds/P5→原资产费用/Town→部分退出/复位及原票冷恢复。不能用施工秒、纯付款或候选游标替代。完整共享、Store/中断、maintenance normal角色/owned与恢复workspace均not_run；原normal额度和M0不放宽。真实16GB/native总峰/人口速度/长期/GPU/UI仍未测。

Mac16,1/32GiB/macOS26.6.2/Rust1.85.1，HP P900 USB/APFS。全部新原件/源ZIP/固定程序/失败结果留`work/maintenance-runtime-20261029`，事前2GiB留证预算；逻辑文件字节见summary，不当APFS回收量。24无关WIP逐SHA不变，无源/恢复原件删除。

本阶段新验收业务0、历史补关0、缺陷关闭0，18交付包不核销。统计起止见[summary](summary.json)，完整合同可实施/源码交付至验收的可靠起点尚不全，不以本验证窗当开发周期。#159保持开发中，不能靠tag或这些专项关闭。
