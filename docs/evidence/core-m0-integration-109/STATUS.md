# 有限核心模型集成与迁移后收尾

当前源流 `5ac7c8eb74893b17864411e8018b7a0cb60767150223a0523688aac00c83cfc5`（237文件），固定源码zip及SHA见fmt-corrected/freeze.json。221项实测通过：core106、headless84、交通市场12、建设纯模型7与Engine8、Store建设3与市场1；严格core/storage/headless all-target Clippy、构建和fmt通过。原第一次fmt失败及源码保留，随后仅两处测试断言格式修正并完整重跑，见format-only.json与两轮原始输出。不能将初次fmt失败改写成通过。

迁移共享target之后，按相同源流再次固定10个实际test/CLI/headless文件，放在外置盘acceptance-binaries并由work/candidate-binaries原路径链接访问，和target为同卷硬链接，避免复制验收exe。逐文件SHA与位置见fmt-corrected/fixed-executables.json。迁移回归core106两次通过；其第二次为Fresh无编译。

随后实际运行固定exe的7项CLI：Town28日与normal/road_closed/bottleneck/capacity_expanded/cross_city/unrelated_cross_city，全部退出0。完整原文及命令见fmt-corrected/cli-results.json和*.cli.stdout。交通到市场为未落账提案；实际Town Engine28日仍使用固定交通/水电约束输入，不能称A到C完整实网联合。

Town28日现金总额3,000,000守恒；家庭现金净减6,160，工资22,400不足支付28,560消费和配送开支，依靠期初现金，不能称可持续平衡。生产300、售280、余20、耗原料300；库存成本和工资资本化/闲置费用分列。Store本轮只有有限检查点/重试验证，不把尚未合入/实测的28日Store或新增负面场景当通过。后续仍需真实交通接入、长期财政/企业退出及工程拆除运营联动。

[M0原负载诊断](../m0-t13a-extents-joint/STATUS.md)因控制器缺测失败，部分采样也仍超原normal；两代独立冷恢复通过不等于性能通过。修正retry与2MiB候选仅准备，尚未运行。真实16GB、8小时及正式性能门仍未通过，完整M0未完成。没有UI/GPU/AgentKit。

[集中回收](../disk-reclamation-focused-20261022/STATUS.md)只删已逐字节归档且解除依赖的50组件，190完整原件保留。[target迁移](../target-migration-20261022/STATUS.md)逐文件核对后通过原路径回归、移除内置盘副本；复用一个target，不以迁移量当总数据回收。该有限集成及磁盘治理阶段已验证，后续继续原目标。
