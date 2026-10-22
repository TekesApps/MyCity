# 首次完整联合smoke：路径错误，未完成

固定32c来源/59aa固定程序不变。首次连续normal、无编译干扰准入实际耗时258.313秒；此前外部Cargo样本保留。实际observer/model启动后0.799秒、4帧即失败，normal-pool.stderr为`path must be shorter than SUN_LEN`。没有完成600秒，更不是8小时或性能通过。

失败在pool启动，没有产生完整L1快照。保留整个小型失败目录、原始stderr、clock/heap/进程及准入记录、源/依赖/固定程序；此阶段删除0文件，没有为失败复制target或新造压力副本。

新[control02](../m0-t13a-joint-smoke-control-02/README.md)仅采用短的外置work路径及冻结源socket inventory提前验证，不改变业务量、采样频率、normal/压力/swap/日志/磁盘阈值。它是独立新尝试，不覆盖或重启本次PID。其进展与终态须另行观察，不能把前置纯测试/静态合同当成完整smoke通过。
