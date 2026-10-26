# #136 canonical 时窗容量：完整有限合同验收

实现源码固定在 `1ab0578fe26253368bfae0fa308b0672f8c1b8f5` / `v0.5.196`。该 tag 发布时尚待验收；随后运行的当前共享、默认栈及真实角色门已结束通过。v0.5.197仅发布这些终态、完整合同决定和关闭台账，不新增游戏代码。原审计入口保持其冻结时点，不倒写历史。

## 合同与实际证据

|原子任务合同|实际接线与验证|
|---|---|
|别名同池、时窗冲突、硬峰不平均、无第二池|原Resource/Lease唯一量/actor/token权威；time_capacity仅版本化登记/窗口元数据。Count、ItemsPerSecond、Kilowatts原整数1:1；相交峰拒绝，相邻窗可接。core5、Store场景及满64槽反例通过|
|候选不冒充实际；取消/过期/关闭不抹义务|两个不同账户consumer共享同canonical。A实际占用越窗后B必须等待；原A退出才准B进入。B越计划终点25秒仍占用，关闭/减容保存责任。Reserved可取消，Actual只能原退出；错actor/缺钟/过窗/旧policy均拒绝且业务不变|
|原writer原子提交、失效/重试/保存恢复|原Envelope/Prepared/Store及原receipt解码接线。Prepared、WalPayloadHalf、Applied中断/丢回报保留完整正常参考；原票重试后state hash、sequence及完整Engine字节一致。checkpoint/独立进程冷读通过；普通磁盘预算满时只允许真正移除原Lease及window的Cancel/Exit必要进展|
|多consumer真实冲突与联合布局预算|原V2、原Venue与独立timed资源701共存。旧规则只读数量视图先过完整身份门，保留原64 Lease索引和真实分配；满表旧Venue拒新，取消后可接，无过滤视图假槽。真实normal角色运行同场景、冷重启及完整直接参考一致，旧生产/劳动实际推进25秒|

本有限内核使用原Scope/Timeline已完成秒钟；不是通用跨clock API。实际Enter/Exit来自原game-host游戏操作，时点保留原typed Action/receipt；窗口只是计划，不推造到达/教学出勤/服务履约。交通、床位、机组等具体consumer仍归各领域任务，父#118保持开放。IntervalLitres仍明确unsupported，不能将区间总量平均成硬峰。

## 固定源码及运行结果

- source12核心5项通过；source13 Store9项通过（含一个冷读helper，另8个父业务测试）；旧venue纯10、旧venue Store10、facility_operation13通过。
- source13与source14便利清单409项完全相同；source12与14仅Store冷读helper别名选择不同，生产实现/core测试未变。460项完整审计清单补充root tests/fixtures，逐项核当前源码一致；这些fixture与v195未变。直接门按明确影响复用，无无据重跑。
- 当前source14受影响6组全部exit0：continuous_supply9、facility_operation_store5（另1 ignored冷helper由父场景实际调用）、checkpoint6、task_registry_directory9、continuous_supply_due4、water_treatment_due2。checkpoint预期unwind诊断不是失败；完整发布边界子进程日志保留。
- 当前source15默认debug/原栈8道门全部exit0，无RUST_MIN_STACK覆盖：原股权、跨城供给、课程资格、facility_operation、两个facility Store门，以及新core/Store门。与release有重叠，不累加成不同功能数。
- 当前source16真实shipping normal Sim角色两次59MiB授额/归还；保存重启原票、越窗等待、原退出，最终与完整直接参考一致。原V2实际原水消耗500000ml/出水各250000ml，cursor25秒，两名原员工各25秒，未停旧业务来降低验证负载。

固定headless SHA256 `e60f2b22eb57c72d84c02679d7085e11656ae43438b3d3415d6f5b1070f55493`；实际app source `264f520be05c6becfcc60f718c0f46f162fa9c14783ef813535266f29a57e3c2`。完整程序45份（含历史失败程序）已按原字节或完整ZIP成员再次核SHA，见[程序身份](verified-programs.json)。ZIP解压至原记录路径并chmod755恢复；hash不替代原件。

[机器验收](acceptance.json)、[复用边界](source14-reuse-plan.json)、[当前受影响运行](run14-results.json)、[默认栈](source15-default-results.json)、[真实角色终态](source16-role-result.json)及同目录原stdout保留。角色完整调用、输入与断言见timed-mixed-role16.rs；它是测试工具，不是第二运行时调度器。

## 资源、保留及未测范围

实际Mac16,1 / 32GiB、Rust1.85.1、HP P900 USB/APFS，复用唯一target/work串行。原World128KiB、Engine4MiB、normal64MiB/共同诊断8MiB及M0冻结门不变。

原角色两次观测allocation_failures=0，arena used最高56697584B <= usable61865544B，requested历史峰最高56774642B；包含完整Store实际owned最高52238087B，不只报Engine。最终Engine3702320B；角色完整Store和恢复/准备重叠另计。两个旧模式只读quantity视图及ID暂存按8KiB在原4MiB工作预算内收费，不扩原上限。整个64MiB pool最终完整归还，workspace grants/returns均2。独立完整参考Store属于测试工具开销，未藏在shipping角色账内；Rust arena观测不等于native/物理进程峰。

全部阶段原件跨canonical-capacity和r39–r44共442240008B，低于536870912B证据盘预算；[完整原件索引](retained-originals.json)列每份字节与路径，不能用索引代原件。保留源码、固定程序、失败世界、WAL/checkpoint和中断参考，直到后续完整基线明确解除恢复依赖；本次无world/源码/WIP回收，无新增tree/target/timer。24份旧WIP逐字节未变。

失败沿原样保留：source02测试误用已拒opID；source03 tagged Action直接u128数字解码失败（修为完整十进制字符串）；source11别名重名、source11b合法混合数量冲突分列；source12生产操作通过而cold helper用错旧alias（只修helper）。source07/08/09旧共享/角色/default结果只作历史，不替代当前门。

未证：真实16GB、全游戏/native峰值、可持续倍率、长期、完整M0/GPU/UI及所有consumer集成。缺陷与硬件任务维持原归属；没有把求解/槽/工作区额度耗尽判作真实物理短缺。无PR，root直接集成发布。该有限完整合同可关闭#136，父#118不随之关闭。
