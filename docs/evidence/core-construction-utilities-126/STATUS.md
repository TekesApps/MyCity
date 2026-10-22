# v0.5.126：施工真实水电与永久材料核算

仅Rust模型与CLI。最终build-input source为`4d44b663c997d2cc6c8136bc9ecd17c3bcc009023cc3de15a9df9830060c9a09`，262个输入；[文件清单](final-source-manifest.json)、[可执行文件与完整源码归档绑定](final-binaries.json)、[各轮收据](receipt.json)保留。完整源码ZIP、固定可执行文件、Store/WAL/checkpoint与失败原件在外置`work/construction-utilities-current-01`，不以本页hash代替恢复文件。

## 有限实现

施工通过不可变合同使用Boot真实电、水库存及运营账户，按累计有效工时差计量、采购和付款，与材料、资金、工时及进度在同一候选事务提交。Timeline完整边界与Boot时间须一致。永久材料行记录实际源lot的quantity/basis；材料已由业主持有，消费basis不会再产生同额现金付款。暂停、清场和拆除不抹除历史；旧None不静默升级。[机制与限制](../../construction-boot-material-accounting.md)说明公式、权限、参与方与容量边界。

实际完整生命周期需要Boot/Exchange六项容量、场地、运营和废物资源共九项，原八项上限装配失败的证据保留。固定目录改为16，17条及重复/未知/超额反例拒绝；4MiB Engine、64MiB model、8MiB common与文件解析字节预算不扩大。M0原布局仍使用原八条资源。

## 实测与失败原件

- 14：2个资源边界测试、8个Store测试通过；半建清场与投运后拆除包含实际许可、容量撤回、废物/费用、丢回复及冷恢复。共享raw7的材料sink与私有stock15的完整生命周期分别覆盖，不称raw7完整生命周期已测。
- 15：86个测试结果摘要，645项通过、一项失败，完整workspace命令946.53秒后**exit101**。唯一失败是metadata测试仍预期八条资源。原日志完整保留；不将后续定向修正改写成该命令通过。
- 16：修正测试预期为16后，storage library 21/21通过。生产metadata本已按MAX_RESOURCES计量。
- 18：新增共享竞争测试8过3失败，测试晚登记工资/读取未启用Timeline导致；只修测试装配，生产程序不放宽。19重跑11/11通过。
- 20：最终release双CLI构建通过；21新增测试之后全workspace/all-target严格Clippy通过；fmt与diff检查通过。
- 22：保存的最终施工CLI实际完成Work、checkpoint和cold open，完整状态hash相同。3600 worker-seconds消耗电水各1000，采购费20/10分，材料basis1000分，spent4630分。小型CLI观察RSS56,623,104B、footprint52,707,712B、墙钟1.07秒；同期其他项目Cargo存在，**不是干净性能验收**。
- 23：capacity命令误传目录，仅usage exit2，无模型启动；24按正确无参接口运行exit0。失败收据保留。

19在同一实际Boot库存100000电上验证：施工先耗1000后Town产销99；Town先产销100耗尽电、Work持久失败且资源/钱/进度不变；跨城真实派电40000后施工1000、Town59000产销59。包含Applied失回复、WAL/checkpoint冷开及原票重放，无重复扣费或消耗。Town路由source77仍为fixture替身；这些测试不代表真实路网/全电网/公平并发调度完成。

## 资源与验收边界

Project实测1616B<2048B；Engine分配容量4,104,080B<4MiB，双槽EngineWorkspace16,424,120B，每份retained preparation1,516,368B，组件之间不重复相加。对独立v125基线，core Engine+584B、metadata+680B，所有潜在角色合计+3400B。generic potential由79,700,704升至79,704,104B，原预算67,108,864B，两版slots_fit/runtime_grant均false；这不是已准入运行拓扑或allocator lifetime峰。

M4/32GiB，work与共享target位于HP P900 USB/APFS外置SSD，旧内置盘结果不混用。新模型不继承v125性能结果。原normal仍失败，原8h未完成且保留失败，真实16GB为not_run；本阶段不签完整M0、完整0.1、持续盈利或经济平衡。仅一个有界Project；管网峰值、真实道路共同联调及完整家庭劳动市场仍未因此完成。

## 保留与回收

本阶段全部故障输入、当前CLI完整Store、已有最新M0完整恢复原件和190份受保护文件保留。共用原target，没有创建独立构建缓存。仅对终态、无占用且内容完全相同的本阶段源码ZIP进行硬链接去重；完整字节及每个历史路径仍可读，收据另列分配空间与可用空间观察，未删除恢复快照。
