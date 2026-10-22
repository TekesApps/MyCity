# R07事件驱动两代实验（报告错误修复前来源）

来源7fd9e8d8，完整冻结源码、exe与实际数据保留。两代实际快照、TTL撤销补回和独立新PID partial prefix冷恢复通过；100帧74完整/26未知，8帧只停发布仍采样。raw审计原数量、每次1MiB脏写和owner/epoch/reading sequence；不是原子同时采样或正式性能通过。原normal仍失败。

本来源全workspace748.718s、严格clippy12.353s/fmt0.581s/build0.158s均exit0，共60成功harness摘要454条passed含nested，非唯一测试数。不能替代最终修复来源的回归。

独立审查随后发现metrics.report的Result可序列化Err却exit0。本轮实际为Ok、4085条9B记录完整；错误路径仍是发布阻断。最终1faa来源改为共享publisher传播错误，正常compute改为直接对象，并实际验证CLI exit2且无成功报告，见[最终阶段](../m0-t13a-r07-report-error/STATUS.md)。

旧两代纯合成载荷在最终新两代真实验证、完整继任基线和依赖解除后可精确退役；当前是否可恢复以[最新可用性](../m0-t13a-r07-report-error/retirement/availability.json)为准。原raw/index/组件/报告及固定源/exe保留；hash不替代已删载荷。原五份旧恢复/故障原件继续完整保留。

当前L1日志投影使用9B/record及1935B/扫描；短测8h投影不足以证明完整L2日志超限，也不证明8h通过。
