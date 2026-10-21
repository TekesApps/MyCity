# 原完整联合负载重测

下一实验复用已发布v0.5.59实际binary/源码，不新增代码。原joint-endurance smoke完整L1+L2负载重测：L1最少600s，scan1s/snapshot300s保留两代及新进程冷装；L2原720h/723target/725windows含实际Cells杀worker、committed丢回复、coldreopen、2880query/96save。原pressure>60s/swap净增256MiB/磁盘2GiB保护不变，continuous normal120准入。仅诊断性联合，不签formal8h/16GB，也不把Engine/serde/helper EXTRA当64MiB闭合。补顶层MYCITY_HEAP_LOG绝对路径。源码及fixture冻结到terminal，复用现有target，不同行构建/测试。实际fail保留所有已生成工件及实时phase，不改变负载。

验收权威fixtures/m0-t13a-acceptance.json原SHA876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508，原始方案继承../m0-t13a-joint-endurance/PLAN.md。此实验无代码改动，不重复旧定向测试；对应已验证来源来自v0.5.59，原失败v0.5.50保持不变。
