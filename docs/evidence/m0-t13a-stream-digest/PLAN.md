# 接纳路径的固定缓冲流式摘要

改动前冻结：保持原acceptance SHA876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508和全L1/L2负载/CPU参考不变。以v61实际joint的execution capture/admit p99=45.111834ms及quote p95=29.304506/p99=60.467261ms为真实瓶颈证据。只消除State校验时的完整checkpoint Vec：固定8KiB摘要缓冲，规范编码/原hash不变；payload哈希同样批量摘要；捕获/冻结/候选接纳/恢复长度检查接新API。

State/reference/Prepared/Store/helper和worker数组仍EXTRA，不宣称normal64/common8闭合。摘要缓冲在嵌套payload+checkpoint时最多两份8KiB固定栈，不能忽略栈/原生成本。确定性校验不加随机，旧profile和oracle不改。

验证覆盖摘要分块/flush/空输入、原legacy字节/摘要/长度、完整及中间Source/Cells状态、原后台身份/限额/恢复/真实故障，复用target运行Release全仓/Clippy/Fmt/build。最终源码冻结后重跑原joint smoke fullL1全600scan/两快照/冷装+L2 723target/724逻辑/725worker/3013原票和实际故障。连续normal120及原pressure/swap/磁盘保护不改；原source/baseline结果单列，不凭局部优化签正式十类/steady/8h/16GB。advisor独立审源码及真实结果，阶段tag递增/push。
