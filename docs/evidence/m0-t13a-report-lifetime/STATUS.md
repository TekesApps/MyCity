# 82a80787报告延后实验：实际仍失败

固定来源82a80787ede81ea6846d64cae956950a3dfaf2c54f8d07ef18426c28995c3a71，200文件；只将闭包成功返回值从JSON改为栈上RestoreBound。13项相关测试、严格all-target Clippy与fmt通过；完整同源全仓未跑，不能称全仓通过。

原formal全部参数实际运行310.533594秒后第二代仍在repay索引63（第64页）失败；[实际结果](actual-failure.json)/[阶段记录](failure-trace.json)。申请前free2,425,304B、reserve119页、total loans9/decode debt1/COW8；不是连续空闲证明或失败瞬间。报告逃逸不是充分根因，未完成1800秒测量，不签稳态门。保持原fixture/额度/记录/字节/频率，没有缩负载或自动重试。

最新首代及完整pending仍用于后续typed工作区验证；最新首代固定82 reader实际全冷恢复约3.650秒成功，见[交接证据](../disk-reclamation-fault-handoff/cold-report.json)。更旧故障合成载荷退役状态以[最新availability](../disk-reclamation-fault-handoff/availability.json)为准，旧恢复承诺不能沿用。

后续常驻typed基础是不同WIP来源，不能与本实验混成最终通过。8h/正式三轮/真实16GB未签，16GB not_run。
