# M0 smoke与完整恢复基线交接（v0.5.137）

本阶段只发布实际smoke、旧格式兼容恢复及精确载荷回收证据；不改冻结模型、负载或门槛。work与共享target位于外置HP P900 USB/APFS SSD，非旧内置盘IO环境。来源/原文件SHA见[evidence-manifest](evidence-manifest.json)。完整原件与原始采样仍在外置work，JSON摘要不替代恢复输入。

## 实际完成与未通过

固定source `a6290a562362b7ea21379726f901a61cf54d32e8d9fe2059bccf3fe44449d3d8`、exe `8b27415ce6dc6a07660b31dd1a31bdb549615347a32bb3675b6582bad57acd26`、source ZIP `f083a1fe9521f390c16d4665c61ca4f6b2c5461cdf92d7dea52d5ff238391094`，与j3p-control来源绑定一致；不是正在开发的企业清算源码。

`work/j3p-01`实际约608秒，2432帧、600布局扫描；真实业务720游戏小时/723驱动目标/2880查询/96存档完成，第二代全载荷冷恢复并继续扫描。真实业务墙时214.741秒，对应smoke预算72秒，不能说快进已达标。观察到1525个外部编译帧、14个RSS与footprint缺测、54个不完整进程组帧；**不算clean性能通过或normal内存通过**。外部项目未被停止。峰值RSS 3380166656B，footprint 3355251200B；不得以此宣称16GB设备门通过。

[日志容量筛查](log-budget.json)按保持4Hz全帧及固定业务量，formal保守投影162717695B，小于原268435456B硬上限。这只允许按同源控制器继续尝试，不替代实际8小时日志限额。[功能前置条件](functional-formal-eligibility.json)通过；真实8h、原四速/分级矩阵及normal门仍未完成，真实16GB设备为not_run。formal尚未启动。

## 新旧恢复原件交接

当前全额L1基线由完整 `work/j3p-01/observed/model/layout/snapshots/snapshot-000002` 承接，两代j3p完整原件保留给后续formal。旧部分R07 prefix兼容输入仍完整保留于 `work/r07-report-error-l1-01/observed/model/layout/snapshots/snapshot-000002`。

本轮用上述固定当前reader对旧输入实际新进程cold restore，3374262418B全载荷hash/load通过，mutation iteration 12→13。旧producer source不同，报告如实保留；旧运行信用未继承。6.760秒只为外置盘操作观察，不是性能门。恢复CLI刷新父目录运行态控制文件，已保存前后两版并恢复历史原字节和mtime，未修改快照原件。

advisor限定交接条件满足后，重新检查进程/四文件lsof为空及精确依赖，仅删除v128-native-01两代的pressure.bin与work-reserve.bin，共4份可重建合成载荷。其native/stub/index、故障/原始VM与RSS、源码、exe不变；190受保护原件不在删除范围。两代v128现已不能直接完整恢复，历史实测结论仍保留。[清单与理由](retirement-intent.json)、[回收收据](retirement-result.json)保留。

删除文件分配空间6349701120B（约5.914GiB）；卷可用空间观察增加6349680640B，分别报告，不称为APFS独占物理释放。没有清理其他项目或共享target，也没有新建独立target。每阶段继续按精确允许清单和恢复依赖回收，不能按目录名批量删除。

## 并行开发状态

C真实清算正在隔离分支实测，尚未通过最终Store冷恢复，不计入本发布的模型成果。新增D股权、E住宅、F空间交通分别隔离开发；仅模块或测试源码不计完成，之后须接真实事务、持久化和联合场景。正式性能测量期间只允许编码和只读审查，暂停本任务Cargo、重测试和清理。

回收后du分配空间实测：work 103753355264B（约96.628GiB），共享target 4295131136B（约4.000GiB）。work现包含后续多轮验证载荷，不能继续沿用迁移当日25.43GiB读数；本轮只处理有明确交接依据的4文件，其余按各自依赖保留，未以总目录名删除。
