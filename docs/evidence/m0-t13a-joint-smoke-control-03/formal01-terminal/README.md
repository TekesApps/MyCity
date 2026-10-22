# formal01 提前终止记录

冻结32c来源、59aa可执行文件的正式8小时尝试已启动，但被原60秒系统压力守卫中止，未完成耐久验收。`observer-report.json`为外置work原报告的逐字节副本，`receipt.json`绑定其SHA及原始完整证据位置。本目录不是新通过证据，也不修改原负载或阈值。

- 报告的observer计时为17,404.892秒；business最后验证记录为17,361.370秒，target=1,562,400、windows=438。不同进程的计时原点不能混用，也不把启动准入等待算作完成的业务时长。
- 原守卫以非正常压力保守持续时间60.254秒触发，模型被SIGTERM终止，退出码-15。不能凭此单独断言MyCity内存泄漏或外部项目为唯一原因。
- 已执行2h worker故障和4h已提交回复丢失后的恢复；6h冷重开、8h终态未到达。末态query=1736、save=57；缺少完整model/report.json及formal日志预算收据。
- 69,620个观察帧全部持久化，但有13,696帧记录外部编译活动，不能报告纯净性能通过。RSS及footprint各有7个缺测process-samples，不是7个已确认缺测帧，也不得按零计算。零swap净增长不抵消压力失败。
- 实机32GiB，真实16GB设备门保持not_run。eight_hour_gate=not_run意指本次已启动但未完成；full_M0=not_accepted。

advisor仅独立审查了失败归类、有限业务记录和MCF尾部66块264帧的长度/CRC及压力标志；未全量复核MCF和全部恢复状态，不能作为完整验收签字。主任务独立读取末态和故障记录，收据保留原行号。

完整原件仍在`/Volumes/HP P900/dev/MyCity/work/j3f-01`，使用外置HP P900 USB/APFS SSD。snapshot-000057及000058保留完整内容但尚未做本轮停机后冷恢复；000059.pending为中断中间状态，不能描述为完整可恢复快照。没有清理、重启或用hash代替恢复原件。
