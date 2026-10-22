# 独立映射pool及地址诊断功能终态（v0.5.106，非正式性能验收）

冻结源6b70d8bc08d00625b369ae92d58ae5ce9dceff9d31a733fe17732de3dd38c87d、固定exe及依赖见source.json/freeze.json。该源与更早8d7397中间freeze不同，不能混用。

normal_pool首次证据路径超macOS SUN_LEN导致pool未启动（6过1失败）；原失败及stderr保留。只改环境中的证据目录为短路径work/pm-proof-01，同源7项实际全过；未修改负载和断言。5份1MiB donor映射在grant前实际消失，子进程实际wait后补回新代次，partial refill fatal等反例仍通过。raw VM与地址/进程身份保留。

映射payload64MiB具有分配前硬界；Rust metadata256KiB是退出时堆峰值验收门，不是已实现独立allocator的分配前拒绝边界。VM地址诊断只保留标量地址，不持有页面引用。

原L1两代联合诊断、provider暂停TTL补回和独立新PID完整冷恢复实际完成：4份VM夹取同源身份/epoch一致。外部vmmap扰动，工具未取得新物理credit，不能当正式测量。整组raw峰值3,337,031,232B，仍超过原normal3,328,599,654B；本轮仍不签normal。同源完整workspace实际736.293秒exit0，strict Clippy 9.746秒、fmt/build均exit0；固定exe与源码包未变。实际终态及命令见regression-terminal.json与frozen-*.json。

正式三轮/四速/实际8h及真实16GB仍未测/未完成，M0仍未完成。0.1三线隔离编码，独立模块测试与真实联合分别验收，不改变M0冻结输入与标准。

旧v104完整v3第2代及v50旧格式原件已由本固定exe实际新PID冷恢复并继续扫描，两个命令均exit0，见compatibility.json及原始stdout/stderr。未复制任何压力输入。

终态后按retirement/reclaim.py精确回收完成：54文件，包括本诊断第1/2代pressure/reserve及原index+既有gzip完整解压逐字节吻合的纯合成组件，分配空间6,748,618,752B（6.285141GiB）。这是本阶段新增诊断数据的回收，**不并入v105历史清理成果**。全部190必需恢复/故障原件前后逐SHA保留，源码/WIP与其他129,034个work/target文件身份元数据不变；无新target/归档。work剩26,568,160KiB（25.337372GiB），共享target2,181,236KiB（2.080189GiB）。文件系统可用空间同期增加6,746,017,792B，独立观测不当作全部删除成果。当前完整清单与可用性见retirement/result.json、deleted.json与availability.json；两代不能直接完整恢复，完整组件gzip不是完整快照，原索引/VM/身份/epoch/raw/source/exe与借还故障证据保留。

advisor最终只读核对同源test/Clippy/fmt/build终态及fixed source/exe一致，无新增有限功能发布阻断；确认normal仍未通过和256KiB门口径。未重复整体设计审查。下一阶段仍复用唯一target；三条0.1隔离线由root串行发放构建窗口，不让M0验收阻塞其编码。
