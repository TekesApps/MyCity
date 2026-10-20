# T13A-2g：P-E1短场景实际采样

固定71个独立真实Store运行：debt50、prepay8、refund8、suspend5。共885个成功事务、50个拒绝Close失败原票；剔除16个末次AlreadyCanceled后869活跃事务。恢复重试不填样本。四类分别报告，未把成功幂等命令冒充真实取消／退款。

|case|活跃样本|p95 ms|p99 ms|max ms|
|---|---:|---:|---:|---:|
|debt|200|0.024291|0.028667|0.045374|
|prepay|208|0.026042|0.033833|0.053749|
|refund|216|0.023958|0.037417|0.038291|
|suspend|245|0.025666|0.031459|0.063750|

达到本阶段每case200样本及20/40ms分位门；计时是Prepare+Apply区域Instant，包含抢占，排除encode/持久化，不能当作完整事务或快进壁钟。原小时步长、命令次序、branch与输入保持不变；完整CLI与短CLI共享函数。每个最终World逐字段一致于v0.5.37原完整CLI，原始参考档及提取文件hash见reference-provenance.json。不是经济平衡已校准的证明。

所有71份世界／WAL原票／diagnostics完整归档并核hash，ZIP与父目录同步后实际解压Rust冷开，World／state hash／sequence一致才回收临时目录。verify.py独立解析CURRENT/root/genesis/WAL长度、checksum和前序链，对应成功或失败原票及operation绑定raw区间，核末次CancelFuture及AlreadyCanceled。计数不信CLI硬编码常量。source.zip/source.json与static-check.json绑定最终Rust来源93994291ef5c7afcb22ccb926f6f7602b58baa83427392f7150708eeb45d44f0及binary；全部尝试保留，没有择优重跑。

32GB M4上的正式组壁钟32.070806125秒（含归档/冷开/观测退出尾部），峰值同时RSS40,091,648B；observer热量556,974B、Rust诊断预留6,291,456B、合计6,848,430B。原始时延7965B、分类日志7,457,054B。缺失footprint145个进程样本，swap净增0。Python runner/zlib/JSON临时owned未完整计入，250ms采样不证明所有短进程峰值；不签联合内存门。observer监测的binary是Python解释器，runner在每次Rust业务调用前后绑定同一Rust可执行文件hash；不称持续观测Rustbinary。跨trial预算为固定次数与事后上限检查，不是producer全局写前硬限额。

失败及边界：pilot-01因误把整个历史work作为扫描范围触发filecap，SIGTERM原档保留；pilot-02缩回本实验目录、四case功能通过，但observer尚未接6MiB charge，仅作功能pilot。正式full-01接入charge、全部71次完成。离线verifier首次把Rust枚举AlreadyCanceled拼成小写而失败，修正字面值后pilot/full重核通过；未改变业务或选择计时。

测试：最终来源四case历史完整World／cold open／禁止覆盖既有世界集成测试通过；原完整30日CLI回归在提取后通过，并在最终来源再跑（结果见tests-full-regression-final.txt）；all-target release clippy和fmt通过。没有声称全workspace测试本次重跑。

仍未完成：正式10类联合、其余冻结明确压力分支、四个实际运行速度、L1同行、3×35分钟稳态、8小时运行与故障恢复。474.96ms背景接纳历史长尾仍未解释，短case通过不抹去它。真实16GB设备not_run，完整M0 not_accepted。下一步进入其余性能分支及联合调度负载，不再扩充经济故事。
