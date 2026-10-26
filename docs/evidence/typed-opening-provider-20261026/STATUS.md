# #137 typed 开局装配与 #150 默认栈回归：实际验收

v0.5.198固定本阶段游戏代码与证据；完整源码身份为[467项清单](active-source-manifest.json)，当前source05。基线v197；本页不把源码、advisor或tag当验收。原4项子合同已由以下实际结果覆盖，发布后回填关闭#137/#150，领域父#120保持开放。UI/完整M0/硬件不随之通过。

## 有限支持目录及真实接线

新schema `mycity-typed-opening-v1`，provider为内嵌实验数据V1，传输为SuppliedBytes／真实本地JSON文件。profile明确base V1、原Boot micro V1；其它新typed profile/provider/schema/version unsupported，旧v1所有合法输入继续原路径。不是全产品开局或全外部来源认证。

新字段的同scope/domain/catalog version引用直接解析到原账户、库存、资本来源和资源ID，再移动到原Opening builder；没有另一份数字引用清单、转换后JSON、World、钱包或资源池。CashCapital／InventoryCapital分开，账户source、stock owner/source、Boot账户／六库存／两资源端口都实际参与解析。金额仅分、数量依commodity固定milli-Q／milli-Wh／ml，无隐式换算；Resource为原整数并发槽，与调用方完整原表ID/容量相等，初始reserved/occupied为0。

带原resources的EngineSlot才可接新schema；裸assemble明确拒绝缺上下文，不能先建World后跳过资源门。Base没有时间consumer，clock必须none；Boot clock只认同scope／V1／秒／零起点的原离散操作钟，真实由Boot::Advance推进，不称Development／Interaction／墙钟。元数据不进World常驻字段，完整原输入仍存原genesis；恢复用相同Slot解码，不重读外部文件重发资本。

## 四项原合同→证据

|原合同|实际证据|
|---|---|
|typed引用／重复／缺项／单位／版本错误拒绝；缺配置不造事实|当前typed8覆盖三类引用的scope/domain/目标/version、资本种类与复用、未知provider/profile/clock、错／缺单位、金额总和溢出、重复键、64KiB/+1；64账户、128库存、16资源分别N有效／N+1拒绝。Boot端口/clock反例及原builder的用途/金额门保留。N是配置边界，不是人口承载或真实业务吞吐|
|workspace／临时恢复峰有界，同槽身份|固定typed＋原Rows／转换／解析逻辑工作区152464B，原slot owned3601880B，合计3754344B<=4194304B。输入Reader实际owned131238B而length65536B，单独归loader/caller，不把借用切片重复计费。真实normal角色冷恢复/实际操作/归还通过。独立slot proof两profile的world、7world表、3engine表、2timeline表、8background表、28preparation backing地址不变；成功槽二次初始化拒绝且状态不变|
|旧合法兼容、失败不发布、新进程／重试不二次初始化|source03旧Opening wire/错误/状态镜像、原loader、同槽Store6与场所/连续供给开局通过。新Store5父场景＋6冷进程覆盖6000分/3000库存700成本独立oracle、137分真实转账后4863/1137、WAL/checkpoint/原票、外部源变化、GenesisSynced/RootSynced中断不发CURRENT、CurrentRenamed丢回报原目录open／拒重复create|
|每个新支持provider实际装配与恢复|SuppliedBytes和真实reader都进入原Store；当前新typed CLI父场景分别用真实base/Boot文件经未改的本地provider/CLI装配，原字节SHA/完整genesis相等，改外部源后原存档不重初始化；资源不匹配不发布。旧CLI2同时通过。真实normal Boot Start→30秒→checkpoint/stop/cold原票→再30秒/完成，完整Engine字节及hash与完整直接参考一致，实际出水600000，原资源释放|

## 测试范围、复用及失败修复

- source03 release9组共40个主测试通过；ignored helpers／历史14 diagnostics不算通过。新Store6、原同槽12以及场所／供给实际冷进程结果分别保留，不把helper自身空跑当业务。
- source03默认资格原冻结测试出现真实stack overflow（exit -6）；前2默认门通过，后续停。单独缺陷#150，不重开旧122/123/148。
- source05只改assembly.rs的函数分帧：typed Opening/Result大临时值移入私有`#[inline(never)]`，旧路径不承担未使用的typed帧。typed预算／decode／builder尾部、原assemble_into及其它全部源／fixture／测试字节不变，见[精确复用边界](source05-reuse-analysis.json)。原advisor对三个具体接缝及该修复均窄核无源码阻断。
- source05原失败默认资格门实际40.5秒通过；随后其余7道默认门通过，合为原6＋新增2的8门，无RUST_MIN_STACK覆盖或断言／负载／表容量降低。新增typed8、Store5及实际冷进程均包含在当前默认门。
- 当前修复后的release新CLI1＋旧CLI2、真实normal角色、完整reference／冷原票、同槽地址probe另实测通过。无语义变化的source03功能／wire结果按精确源码差异复用，不伪称当前版本再次全量运行。
- source01测试对非Debug World误用unwrap_err、default04构建清单把资格测试归错crate，均为工具／测试输入错误，单独保留，不混作产品缺陷。实际source03 stack失败、固定源码、程序、世界和stderr保留；没有把失败改写为通过。

## 资源与完整原件

Mac16,1/32GiB、Rust1.85.1、HP P900 USB/APFS；原唯一target/work串行。Engine4MiB／World128KiB／normal64MiB／共同诊断8MiB／原默认栈与M0冻结负载标准不变。

当前shipping Sim实际两次59MiB授额：arena used最高56597224B<=usable61865544B，requested lifetime peak最高56684940B，allocation_failures均0；完整Store owned最高52137650B，包含于上述角色占用，不能再与角色相加。最终原64MiB pool完整归还，workspace grants/returns均2。原输入65536B，实际Reader owned131241B；parent首建Store owned42886764B／直接reference属于测试工具开销，未藏在shipping角色账内；Rust arena／逻辑owned不等于native进程峰。

完整原件在本机`work/typed-opening-provider-20261026`及`work/r45`，约302MB，低于事前512MiB；精确大小及每文件见[冻结原件索引](retained-originals.json)。包括全部版本源ZIP、固定程序、原请求/结果、失败、World/WAL/checkpoint、正常reference和冷读配置。31份固定程序以完整原字节或完整ZIP成员再次核验，见[程序身份](verified-programs.json)；若工作副本已退役，按条目解压+chmod755恢复，hash不代数据。

动态coordinator-state不作为不可变游戏原件索引；冻结时完整副本在coordinator-at-acceptance-freeze.json。后续Issue／发布回执独立追加，避免控制状态更新破坏原件身份。所有writer已停止后才能按明确依赖回收；本阶段只在终态/lsof无占用/ZIP逐字节对照后退役列明程序工作副本，没有删除源码、世界、恢复证据、target或工作树。24项无关WIP逐字节未变，无新tree/target/timer。

真实16GB、native/全游戏峰值、可持续速度、长期、完整M0、GPU/UI、所有未来provider/profile均未据此通过。#149是独立排队的源码审计缺口，尚未实测，不能拿本阶段结果覆盖其pending Timeline目标／普通额度满组合。当前非UI目标继续，父120及其它领域按其范围开放；无PR。
