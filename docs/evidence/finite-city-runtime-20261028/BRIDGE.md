# #21 实际公服供给消费桥（v0.5.216）

**本段接线通过，完整有限城市与#21/#115仍开放。** root本增量新验收关闭业务0／历史补关0／缺陷关闭0；18完整源码包不整包核销。统计起止与每次命令/来源见[summary](bridge-summary.json)，可实施/交付起点未知，不用验证窗口或关闭窗口充当开发周期。

## 发生了什么

显式PoliceBootV3复用原同城Boot电/水输出、原Stock、Timeline、成员/容量和付款路径。普通V1/RecoveryV2仍用原普通库存；受管Boot库存不能静默直接扣。新profile只同一LOCAL_PROFILE、operator和两个输出，不混用多源、不展开Recovery/跨城或通用网络，非饮用Police用水规则保留。

每次实际消费只调用一次Boot.export：源Stock扣量和basis，服务取同一返回成本，Book仅作历史出口分类，无第二库存/钱包/费用。Book version1及双向绑定在明确新开局初始化；保存恢复缺Book拒绝，不从旧余额补造。关闭/归档/任务回收不清零/再次归档此累计。原本地Boot出口总量及basis等于Town＋Construction＋Service；原Exchange规则未变。

新Book在既有boxed服务状态内按需保存，计入Runtime可选指针、payload、Prepared深clone和恢复重叠；本地临时Book副本计入candidate工作字节。没有增加World/Boot/JournalRecord常驻字段，未加独立调度器。策略版本表示实际源语义，不是新增精细档。

## 实际验证及复用边界

- direct20默认栈：原两profile形状仍拒绝不兼容输入；本地profile拒绝Region；真实电水不足明确返回Stock而非预算短缺，原状态不变；新Book缺失拒绝；原普通库存场景继续通过。
- 真实泵完成后再启动机组。Police往返600秒×50mWh/s＋现场100000mWh＝130000mWh，现场水10000mL。出车60秒实际量3000mWh，按当时400000/730成本池取分得basis5。两次Town各100000mWh后电池剩70000mWh、水530000mL，现金总量3100000分；Book与Boot分类量/basis闭合。使用独立常数及原整数取分公式，不从运行结果反填预期。
- bridge14/15：同Store正常两日日结、WAL与checkpoint实际冷恢复、原票不增加序号/hash。跨回收专用场景不含Town生产（两日共存另验），保持原180日自然周期、5次真实服务，回收1个已关闭任务后累计650000mWh/50000mL不丢；回收前后冷进程重放不重复出口。该专用场景约573秒，是固定5任务/固定容量的历史链验证，不是人口压力或游戏速度支持。
- atomic19：实际消费中原WAL半写未发布，重开/原票可重试；CURRENT已发布但回复失败时保留已提交结果，同票不重复出口。写入失败原件保留。
- compat16：旧18单元仍精确120568＋8456＋2048＝131072B，World74224B；原票3488B低于4096B；可选资产原负载超限仍原子拒绝；9项旧公服回归通过。原负载、Wait和预算未改。
- shared21当前Core库310通过、1忽略、1独立全native布局诊断未重跑；不称全库无过滤通过。shared22 Storage34通过，2个query因缺必需SSD证据环境变量失败；同一固定程序query23补正环境后两项通过，其余34结果复用。无代码修复/整库重跑来掩盖环境失败。
- role17真实原normal进程：59MiB授权，6次采样allocation_failures均0、arena最大观察55785328B；Engine owned3677304B、Store owned41995638B。两次进程授权/归还后原64MiB池完整恢复。creator/reference在正式角色之外，不能拼成全游戏进程内存结论。

每份程序有独立完整源码ZIP/逐文件manifest和SHA，最终[输入清单](bridge-source-manifest.json)与当前源逐文件一致。早期验证至最终只有列明的测试源码变化，生产文件及输入fixture字节一致，详见summary对照；role17固定CLI SHA `831de5b98eb74937add7be6f32829f6013b882eba783c5940a5e17e19deb66a8`，不能说所有结果来自一个程序。原advisor源码窄核无新明确阻断，结论不替代这些实际运行。

## 失败与保留

bridge11编译遗漏Money导入/owned计算误放Opening，未执行；atomic18测试重试完成后又以新票推进同一时刻被Cursor门拒，测试纠正额外推进，游戏门未改；shared22环境缺项失败均原样保留。oracle草稿漏算泵所耗初始电池basis100，由root核原配方发现并纠正，正确水basis300、产电basis730，不能把代理报告当独立事实。

完整源码/固定程序、日志和失败/恢复world在外置 `work/finite-city-runtime-20261028`；normal world为role17证据目录的normal-role-19357/world，pool诊断在work/fc216pool17。当前及失败原件仍有恢复用途，未删除；复用原target，未新建工作树/定时任务。原24份无关WIP逐文件未变且未混入提交。证据沿2GiB额度（先前512MiB估算超额记录保留），未改游戏预算。Mac16,1/32GiB/macOS26.6.2，HP P900 USB/APFS SSD。

## 下一实际差项

账户级Payroll21没有成员/群组范围输入，也未向原member_activity账记工作时窗，不能宣称与ServiceWork互斥；已有#129只是账本能力，#132设施工资有另一消费者，不替代此接线。下一限定原Payroll和公服的真实成员/时窗来源、冲突/09–17边界、工资和原票恢复，不扩为全劳动或交通领域。

尚未签收完整有限城市持续供需/融资、工程/全交通、长历史normal资源、完整typed provider、native footprint、真实16GB、GPU/UI/Windows或原M0长期。原M0标准和历史结果不变，不根据当前小场景承诺人口或快进倍率。
