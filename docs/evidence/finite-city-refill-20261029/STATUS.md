# #21 Town消费后的原Boot真实补货：准时与迟到

基线v0.5.223，本轮v0.5.224仅补专属联合验收。生产代码/原输入/模型策略未变，root唯一共享writer/串行验收，无新模型、provider桥、tree、target或timer。完整21/117/115保持Open，无PR。

## 实际发生的业务

同一住房/实际Payroll/警方已到返/Boot/道路市场/工程资产城市先完成原两日日结；电stock3002已实际消费至68000。随后调用原Boot Generator重复任务，原成功后Idle允许重开，不使用#144连续电厂，也没有补任何期初钱物。

Start实际supplier3→contractor4付300，扣fuel1000/basis400、冷却水60000/basis30，WIP730；fuel余3000、water462000，原Boot3101/3102两Lease占用。电在成功due前仍68000，或在迟到日结后已被旧业务用光仍0。实际60秒完工才增加400000电，同时新增waste60000/heat600000，释放原资源、completed由1→2、清WIP；原票重试不付款或生产第二遍。

|对照|准时：2DAY启动、+60秒完工|迟到：3DAY−30启动、3DAY+30才完工|
|---|---|---|
|第3日日结时可用电|已包含实际新批468000|仅旧68000，WIP尚未完成|
|第3日销量/家庭运费|100/700|68/476|
|第3日工资/商品资本化/idle|80/80/0|80/54/26|
|第3日累计税/retained/C7现金|600/601/73155|472/473/72867|
|第4日累计税/retained/C7现金|600/317/72995|472/189/72835|
|最终电/水/raw|368000/462000/606|400000/462000/638|

两分支第4日均无新工时/产销，不能因剩余电补造劳动或续雇；工资旧行累计235/84900秒/rem3000、ends_at3DAY、paid=billed=accrued和未来租金/原已完公服等责任沿原输入保留。累计资产折旧1200/残值27716。迟到完成不能回算第3日原销量、原税票或旧资产成本。全账户现金3100000守恒。

## 库存原坐标、basis与分类

独立预期先冻结（independent-expectations.md），没有把比例/旧basis猜值当实测。原day2已提交输入实际观测E=126/W=262，旧Boot exported_basis=612；该整数观测与后续新结果分开保存。

- fuel仍按原origin5000/basis2000，offset1000→2000，stock余3000/basis1200；不因重开任务重置燃料坐标。
- 新水内部投入basis30不计成对外export，水余462000/basis232；原水export18000、工程分类保持。
- 准时put_pool合并后electric qty/origin468000，basis/origin856（E+730），offset0；日3出口100000的basis K=floor(856×100000/468000)=182，最后qty/origin368000、basis/origin674、offset0。
- 迟到日3先完整消费旧电basis E，库存/成本暂为0而WIP730仍在；完工才得到400000/basis730，日4不再消耗。最终Boot exported_basis738；准时为794。新批内部费用不重复计入公司7工程资本。
- 最终Boot电exports：准时432000、迟到400000，water均18000；local Town分类只增加相应实际数量/basis，construction分类保持，残余公服分类不变。
- 真实最终原件交叉算：fuel/电/heat及exports按原模型能量坐标5100000守恒，water/raw/waste及exports为6000000，stocks+exports+WIP/failed的basis合2900（原2100+实际fees800）。这些是实测原件核算，不是新增来源或压力替身。

## 固定程序与恢复/资源

- store01实际两个时间分支通过，9个独立冷子进程通过：Running、due前1秒、迟到日3已结算而仍在WIP、完成checkpoint、day4 checkpoint。每次原market票重试保持hash/序号/业务事实；Start票在Running和完工后重试均无新扣款/产出。保存恢复后同一job身份/Lease/WIP、原税/资本、pool坐标和出口成本一致。
- role01新准时/迟到Sim角色及原v223耗尽角色共存通过。逐观察点实际Sim state_sha256和sequence与独立Store reference相同，IPC重试Start/market票、完整最终checkpoint相同；在真实批次due前1秒额外退出/冷重启，Job仍Running/cost730。
- 新分支因额外一次冷重启，workspace grants/returns均为3；旧耗尽对照仍各2，不改旧断言。每次原59MiB授权、总64MiB池全归还，allocation failures0。观测arena最大55798480B，Engine owned3690456B，Store owned42008791B含Engine、不相加；这是采样角色区域，不是native总峰/实际16GB。creator/reference是测试工具，在正式角色外。
- 最终源manifest匹配，两个测试文件之外生产/旧fixture/Cargo/策略不变，原24 observer/M0 WIP SHA不变。原v221共享/旧默认/18单元128KiB/Journal布局及v223耗尽证据有据复用；新补货没有借旧early-repeat案例代替。headless源码身份包含测试，当前固定可执行文件与旧指纹分开，不声称二进制相同。

本阶段没有生产失败或修复，不重复advisor。运行范围是固定小城真实业务/库存/账户和必要恢复，不是模型规模、正式倍率或压力测试。

## 交付与边界

全部ZIP/manifest/固定程序、before/running/finished/end观察、world/WAL/checkpoint/proof/原票位于外置HP P900 work/finite-city-refill-20261029；事前1GiB，实际约343MB见summary，无不明缓存删除，原共享target留后续验证。Mac16,1/32GiB/macOS26.6.2/Rust1.85.1、USB/APFS，未推导16GB/native总占用/产品人口容量/可持续速度/全M0长期/GPU/UI/Windows。原M0/Wait/96页/内存标准不改。

统计起止见summary（验证窗口，不是开发耗时）：新业务/历史/缺陷关闭均0，18完整源包不机械退休。没有接通自动补货、持续工厂#144、续雇或长期自给；原#151/#155/#157关闭不重复计数，父任务保持Open。

后续待核的是同一非fuel pool在没有新批混入时的分段/合批成本：当前实现按每次current pool floor、现有证据证明守恒/恢复，尚未证明分段等价；与新增产物时重建pool边界不同。先构造真实最小对照，不预先宣布缺陷或扩成新框架。实现/结果回填#21、直接main/tag v0.5.224，无PR，确切commit见发布回执。
