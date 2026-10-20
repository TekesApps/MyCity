# T13A-3c：实例身份与驱动复制收敛

实际full30日与有限反例通过。发布v0.5.42，advisor独立终审及共享调用者回归均取得通过终态；本阶段不是正式联合压力、四档、三轮、8h或16GB签收。

## 实际来源与输入

Mac16,1／Apple M4／10核／32GiB，macOS 26.6.2 arm64；不是16GB。source `ea9b274cfc56659fafb7aaf0dace09355eb572eb8a1ba5521d95376504d62b5b`，binary `1a34833577186cd2f6cb70b66d38eeef8a4bef3c358ed8cd8573316a51b1070c`；[源码信息](source.json)及source.zip／source-support.zip具名归档。

未修改冻结acceptance或full期初：64账户、128库存、64工资、20000 Claims、8城、16周期义务、8单／4在途、2泵机作业、128²空气；720小时、723实际目标、724窗口、8192页、空气60秒步长、关城重开与原票不变。

实际命令为 `MYCITY_HEAP_LOG=ABS_ROOT/model/heaps.jsonl python3 observe_v3.py NEW_RUN_DIR -- target/release/mycity-headless scale-sim-probe NEW_RUN_DIR/model full measure`。本次无L1同行，只验证原full业务及本轮改动；不能与有L1同行的旧285秒作提速对比，也不能把无L1 RSS当完整城市内存。

## 修改及反例

- CpuLog使用原子create_dir创建kind-PID-sequence；冲突只重试AlreadyExists，最多4096次，其他I/O错误和u64计数耗尽立即拒绝。已有／部分初始化目录不删除、不覆盖，旧共享预算不重置。
- 报告显式记录PID、进程instance、producer instance与目录。heap与CPU使用同进程OnceLock身份；唯一目录由原子创建保证，时钟不充当绝对唯一依据。核验按PID＋instance配对，重复／歧义即拒绝，不把同PID多实例折叠。
- 驱动移动取出Ok，流式哈希同一serde编码，去掉完整payload编码副本；日志借用包装，固定128KiB容量，golden行固定4096B容量并在复制前拒超限／截断。及时释放冷开响应和最终恢复输入。消息和原票时序不改。
- advisor发现初稿仅限len时Vec可能扩到140000B；已修固定容量，并测试70000＋60000＋逐字节追加及3／17／2051／4000／7000读取分块。早期通过日志保留，但最终以tests-capacity-fixed.txt为准。
- 反例覆盖同PID连续创建、重启计数归零、并发冲突、初始化中断留目录、原文件及原预算不变、非AlreadyExists错误、计数溢出；还验证流式hash与旧serde字节hash／长度完全一致及超限不改变旧缓冲。

## 真实终态

[独立核验结果](verification.json)确认：

- 完成30日，723目标／724窗口、3011原票完整链；完整Engine（含元数据）与CPU全字段一致，逐目标业务／空气对照、冷开图像不变、最早原票重试无副作用。
- 727条正常退出堆记录：724 worker＋2 Sim＋1驱动；726诊断目录全部按显式实例匹配，无歧义。5,536,188条raw样本共49,825,692B，所有角色的计数和最大纳秒与报告一致。与v41联合包相差的是本次没有L1样本，业务样本未删。
- transcript共6028请求／8924响应，最大行786B；所有请求连续、响应关联及单一在途规则通过。流式hash保持原编码语义；原始消息无新增截断。
- observer总368.378秒，实际Sim业务推进312.604秒（含IPC与持久化），CPU参考54.370秒；冷开ready 0.0854秒。只是单次诊断，不签四档实际速度或正式分位／积压门。
- requested Rust堆终身峰：驱动29,358,552B、Sim29,442,259B、worker3,753,772B，不能相加作同刻总占用；不包括栈、native或allocator元数据。没有同条件旧版重测，不据此下提速结论。
- 1474帧RSS成员求和通过，进程组RSS峰125,517,824B；footprint缺1570个进程样本，不能称完整峰。observer热上界632,514B，系统非正常pressure时段0、swap净增0、磁盘逻辑峰105,092,334B。

完整工件保留在[t13a3c-instances-01](t13a3c-instances-01/manifest.json)，无大文件省略，归档逐字节hash见[静态核验](static-check.json)。[verify.py](verify.py)检查真实工作根；[collect.py](collect.py)和[static_check.py](static_check.py)可核对来源及全部归档。

## 回归与独立审查

最终[tests-capacity-fixed.txt](tests-capacity-fixed.txt)共32项通过（lib15、worker8、heap1、scale2、Sim6），[共享调用者回归](tests-shared-callers.txt)2项通过（7日joint 73.84秒、full稀有分支14.40秒），release all-targets clippy与fmt通过。早期tests-final.txt不是容量修复后的来源，以最终日志为准；本阶段未另宣称完整workspace回归。

advisor实际重跑只读核验，额外核对3703项归档hash、原723目标顺序和逐目标摘要，确认有限阶段可提交，见[review](advisor-review.md)。实际长时间PID复用尚未发生／验证；同PID重启与并发冲突由定向反例证明，不能把它写成8h实测。

## 未通过门

Sim诊断收费2,446,916B、worker846,876B各在原cap内；驱动诊断份额仍需闭合，不能称完整8MiB通过。64MiB外部预留／未用占位仍缺受控借还，不签3.10GiB满压力。当前单次没有覆盖v41 quote p95 20.93ms及历史474.96ms长尾的销项，不能从本次完成推成热点已消失。

下一依赖见[NEXT](NEXT.md)：先补驱动费用与受控owner容量／触页余额、失败回滚及回收，再扩全部外部所有者；冻结其余分支、四档、正式三轮与8h继续待测。真实16GB not_run，M0 not_accepted。
