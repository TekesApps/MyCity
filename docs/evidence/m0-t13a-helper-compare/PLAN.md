# 固定期末文件核验工作区：测前冻结

目标为T13-A normal64依赖的一部分，非总门签收。继承v0.5.64原fixture、全30日、723目标/724窗口、20/40ms、完整L1和所有CPU算子，不缩负载。不做UI/GPU/AgentKit。

仅BeforeReopen、CompareSpeed、CompareEndurance采用固定4MiB比较背板。CURRENT→root→index→image绑定保留，原文件hash/长度验证；两个Compare模式完整流式逐字节比较实际image与已验证独立参考，不能仅比hash。真实Store冷开仍由原运行执行。普通Compare继续原Engine业务/空气比较，Reference/FaultReference、Sim/Prepared/Store仍EXTRA。

测前只读核对v64耐久实际image与fault-reference原始编码各2507418B且逐字节相同；旧恢复再导出相等不自动证明其他编码都相同。新模式明确要求受信规范writer工件；非规范但语义相同编码不声明兼容，必须失败且不发布结果。

复用broker监督协议，Register固定HelperCompare kind，子不能启动worker。父在分配前取得原64MiB内4MiB页面；helper从已填充Owner借真正缓冲，Filled后才读取模型。父持Child，实际wait后才Revoke/ACK；spawn失败Cancel，Bind失败也必须kill/wait后撤销。父SIGKILL时EOF只促退出，不作死亡证据，池保持fail-closed；未实现自动孤儿额度回收。

business_sha256须明确为完整字节相等后引用已验证独立参考报告；增加来源绑定，不能冒充重新计算。参考工件hash/长度和报告来源均验证；结果不发布前，复核root/index/image命名身份及文件描述符元数据，避免静默接受切换。主程序依旧单writer，恶意并发篡改不在安全边界内。

验证：固定缓冲空/分块/中尾差异/长短/IO错误；Owner错误/展开后归还；拒绝helper启动worker；实际helper生命周期及失败回收；原完整full功能/所有旧hash/冷恢复；最终同源全仓release、clippy/fmt、独立advisor。硬件/正常压力准入、原联合负载和归档沿用；未实际测的门保持未测。对比只报告原始来源和观察，不宣称正式性能或64MiB整体通过。
