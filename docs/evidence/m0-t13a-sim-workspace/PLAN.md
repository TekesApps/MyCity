# Sim具名固定编码/候选owner接入原正常容量池

基线v0.5.57。保持原64MiB池、原worker4MiB两槽、全部fullL2输入/723目标/724窗口/3011原票/60秒空气步/8192页不变，不签尚未覆盖的Engine/事务克隆/serde/helper工作区。advisor只读建议先接实际固定buffer，禁止先发估算Sim总额度再宣称闭合。

本阶段broker注册前从池释放具名SimCodec4MiB，实际Sim Child启动后分配/触页两个2MiB槽并Filled；worker原4MiB位于不同页，两owner同时存在时池56MiB+Sim4+worker4=64。未用容量一直实体填充，普通Engine仍EXTRA。Prepared/Filled/Retiring/Returned分列；Prepared转换未满压力且记时。Sim父Client真实Childwait后broker先回收其worker再归还Simowner；丢回复/旧代次/部分补页失败不发布虚假信用。控制表固定4项，全部余额受原64MiB硬界约束；未使用项仍在池中实体触页。

输入checkpoint_to直接写受控槽，候选read_asset_into直接读取并校验hash，在同一借用内交给Store导入；不先生成整份Vec再复制。State冻结副本/导入模型/事务临时量仍EXTRA，不从历史heap峰相减冒充精确64MiB。owner元数据及bounded事件加入原诊断份额，字节缓冲属工作区。槽按实际job/代次复用，失败和unwind补回fill。

验证：真实页与双owner并存、预分配/身份/容量边界、表满/重复代次、部分补页失败，候选直读失败归还，既有实际Sim进程死亡/冷恢复/丢回复矩阵；定向检查后测同原full无L1真实30日，核对所有业务/空气、原票/完整Engine/首末恢复/两类grant账。仅额外owner接入和独立full功能分支，不替代原完整L1联合、常规64MiB/共同8MiB、四档/正式稳态/8h或真实16GB。

复用现有target，不新建独立编译缓存。测试/build/实测期间冻结源码，不跑同行自有负载；各阶段结束核对运行进程和文件占用后清理确定无用缓存，验收binary/原日志与源码快照保留。
