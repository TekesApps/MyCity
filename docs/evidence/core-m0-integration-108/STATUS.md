# 有限核心模型与M0候选合并功能验证（v0.5.108）

精确源码流SHA：`32c6e61b9565545177c330f34455611b329fbe1cb38591ec5f6e7879731c4da2`，231文件；`freeze.json`另列源码ZIP自身SHA，二者不能混称。`check.py`在每条命令后复核源码流未变；原acceptance/full-opening/Cargo.lock与v106相同。

实际release/locked/offline：core lib97项、headless lib84项、工程纯模型7项、工程Engine8项、工程Store3项，共199项通过；core/storage/headless all-targets严格Clippy通过，headless和core examples构建通过。再运行单项目真实Engine建设CLI及六个纯经济fixture CLI，7次成功；现金/材料/工时通过现有Engine，经济CLI明确pureplan_not_settled。命令、耗时、原stdout/stderr及终态分别见对应JSON，CLI见`cli.json`。耗时是功能检查记录，非正式性能结果。7份对应测试/示例exe硬链接固定，见`fixed-executables.json`。

本阶段合入：有界经济planner及价格反例修复；单项目工程的真实Engine/Store施工和兼容修复；R07 extent与错误状态处理；八小时监督路径的R07角色映射。保留此前失败日志及相应源码，不将修复后的成功追溯到旧来源。

工程旧producer互操作明确覆盖真实v106 genesis/root/WAL和旧Engine、World、journal及receipt；旧probe的root checkpoint为null，不能扩称旧非空checkpoint互操作。另有合成复用槽位反例。通用ReleaseCapacity不能绕过项目租约绑定；实际lot成本权威，规划估价不再因高估或估价溢出阻断实际可支付施工。

R07定向独立来源9项测试和严格Clippy已经通过；本合并来源再次运行headless全84项。1MiB是单次新增extent映射的边界，不是OS footprint上界。新增八小时角色映射经advisor一次窄审；这里仅单元功能通过，驱动退出、旧layout退出、恢复新owner接管的真实生命周期尚待测试。provider reading与ack的认证范围不同，不把ack说成独立TTL/nonce认证。

## 尚未完成的原目标

- 动态经济仍是计划器；真实生产/销售日交易和28日经营、跨日WIP、亏损退出正在独立production线实现，未验收。
- 工程拆除/废物接收等纯状态机已定义，但完整Engine退出交易未接入；本阶段不能当B全范围完成。
- 新逐OD路径回执/实际交通→市场adapter仍在隔离线，不能把fixture路容或公用供应报告为真实联合。
- 原M0 normal/正式分级三轮/四速度/稀有事件样本/真实8h未因本次功能检查通过。上次v106原normal超限记录保留；新合并来源尚无正式性能结果。真实16GB为not_run，不宣称完整M0或完整0.1完成。

下一实际M0动作使用[冻结候选与原L1诊断脚本](../m0-t13a-extents-joint/README.md)。已固定精确headless exe并保存源码，尚未启动压力；新来源包含核心模型和角色接线，不能谎称单变量物理优化实验。所有实际原负载成本一并报告，不推算扣除新增代码开销。正式运行与编译/清理串行。

阶段回收见`stage-retention.json`；本阶段无新大压力、无独立target、无确认无用缓存，新增删除0B。保护完整恢复原件没有以hash替代。
