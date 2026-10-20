# 后续联合内存缺口：当前运行期间的只读定位

本页不是新性能验收，不修改冻结负载，也不把P-E0小模型推广为完整城预算。当前真实60档65764运行期间未修改crates/fixtures、未启动本项目测试/build/其他基准。

## advisor对v50 L1失败的具体定位

advisor只读检查layout_load.rs及v50事件，没有启动负载：快照539–542只复制Arc索引，native/tables共享Arc；493–498 COW先释放1MiB reserve再复制1MiB，join后568–584补回。647–713读回先从128MiB reserve借64MiB，native候选释放后再逐表解码，压力比较缓冲1MiB，没有整份3GiB恢复副本同时存活的证据。

v50第一份快照native解码保守界56362928B低于64MiB，完整快照22.8458秒；最大meter表160000×128B，Vec扩容的新旧容量可能同时约48MiB，不能只算最终20MiB。冷恢复先native/tables，后压力页/reserve；v50没有进入最终冷恢复，不能将原失败归因于该路径。

第二快照在L1时轴300.0038秒接纳，保护失败约316秒且非正常压力已持续约60秒，因此压力早于第二快照接纳。第二份可能延续压力，但没有证明由它触发，更未排除其他应用或系统活动。

具体未闭合：native的64MiB检查在serde完成后，table同样无分配前额度控制；逻辑释放Vec不保证allocator即时交回物理页，文件缓存/2MiB线程栈也影响RSS。下次原联合应在同一时轴记录snapshot写入、restore loan、native/table decode、归还各阶段的reserve/COW页数与requested heap，再对RSS/footprint/host pressure，区分存活分配、物理留存及外部压力。不是重开大量故事场景。

## 主任务对普通工作区的定位

normal_pool.rs目前只把worker_slots::OWNER_BYTES的4MiB两槽变为真实grant；60..64四页在spawn前释放，Filled后确认，再由broker实际wait回收。普通Sim/Store/Engine、serde以及独立参考helper仍明确EXTRA；不能从完整64MiB填充余额或生命周期峰相减，反推它们已纳入64MiB。

sim_host.rs的candidate Vec读取、导入/接纳，core/background.rs的输入冻结/结果克隆，以及Store执行/Checkpoint的模型与序列化临时量，应按实际生命周期归属。下一实现须同时满足分配前预算、实际替换压力页、死亡归还与原票恢复；仅给进程贴32MiB额度、少放32MiB压力页而实际只使用几MiB，不足以证明原足额联合负载。全局heap峰只作整堆保守上界，不能减两个不同时峰值当精确诊断量。

本页仅记录具体下一缺口，尚无新代码/原联合实测。完整3.10GiB/共同8MiB/精确常规64MiB、正式稳态、8h与16GB状态不变。
