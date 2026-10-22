# 首轮R07实际耦合：诊断重复保存失败

来源12e17ab，原冻结源码和exe保留。原满级L1、真实64MiB池实际启动，首代快照仍持有时，第8秒再次请求保存，产生layout_snapshot_busy；root报告r07_exercise_layout_failed，完整证据见initial-failure.json及原work日志。不是正式300秒保存周期的失败或性能通过。

历史exercise.py包含原拟SIGSTOP方案，但本次在抵达该步骤前已失败；未实际暂停observer。不能原样复跑。后续改为仅在显式诊断中停provider发布，采样/压力/失主保护始终运行。

首代原件后来分别被7fd及最终1faa来源实际新PID全冷恢复，报告位于相应compatibility-result及work/r07-report-error-compat-01/v3-predecessor。后续完成独立partial-v3继任基线后，可依[最终阶段逐文件可用性](../m0-t13a-r07-report-error/retirement/availability.json)退役纯合成pressure/reserve；是否已删只以该清单为准，原index/组件/raw/失败证据和固定源/exe保留。不可因历史冷恢复报告而继续声称已删载荷可直接恢复。
