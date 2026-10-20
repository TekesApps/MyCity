# T13A：完整L2无等待30日及速度控制器

v0.5.51交付Rust速度控制器及一次完整L2无等待实测；不是P-E0、四档速度或完整M0验收。只做Rust模型与CLI。

## 负载与实现

[计划](PLAN.md)保留冻结输入、720小时、723目标、724工作窗口、8192分页及空气60秒数值步长。speed-run实际托管pool与driver Child；支持unpaced/600/60/10/1，但支持命令不等于各档实测通过。小时周期开始发布、周期末检查截止；初始化、IPC、等待、fsync、最后必要Checkpoint及已发布观测排空全部计入经营时间。最终Checkpoint完成锁定发布截止；冷重开和独立完整比较另列，但始终计入观察器资源账。

最终[来源](source.json)65d5e659074237a460a0cf29bd494c573830ed6cd798ad098ce81de467dcf3aa，145文件；[41项定向回归](tests.json)、clippy、fmt及Release build通过，不称全仓回归。两版未测实现及一次报告字段宏编译失败分别保留，最终采用分步构造报告，没有提高递归/内存限额。

## 实际结果

work/t13a51-unpaced-01实际81528终态0，观察器373.131590秒，经营337.904630875秒；无本项目测试或其他基准同行。新120秒normal准入后启动，全程采样normal，swap净增0。[独立业务核验](verification-unpaced.json)逐项验证723目标、3011原票、完整Engine含metadata、冷重开及724真实借还/Filled/余额；原参考与v48逐字节一致。33次周期查询、1次周期保存及最后Checkpoint按实际请求/完成对齐。该full无等待分支逐日60秒和总1800秒局部目标满足，不能代替指定P-E0路径。

采样RSS峰158482432B、已获得footprint峰114722856B，1569次进程footprint缺测保留，不能当绝对峰。732份正常退出heap、726份sealed生产者报告。原始CPU计时49825692B，[分层分位核验](latency-unpaced.json)不合并producer/role或部分报告；Claims仅20、source执行仅4样本；quote capture/admit p95约22.61ms超过20ms，不能签本次正式十类门。历史独立稀有样本不拼成此次同源联合验收。

[工件归档](t13a51-unpaced-01/manifest.json)及[静态核验](static-verification.json)绑定真实终态文件、源码、二进制、观察器和冻结输入。verify.py/latency_audit.py/collect.py/static_check.py均实际执行；脚本通过只支持其明确范围。

## 快进与计算粒度

速度决定发布节奏和执行批次，不改变经济规则或账物精度。债项保存不代表每小时重算全表；到期索引、无变化区间跳过、同类批处理及稳定区间累计可减少重复工作。合并必须在到货、发薪、资源耗尽、债务到期和城市开关等因果边界切段；连续场保持稳定数值步长，统计近似另有误差契约。优化须同输入跨速核验关键事件、账目、库存和最终状态，不能靠减少本次冻结负载通过。以上是后续优化契约，不声称这些优化已全部实现。

[advisor独立终审](advisor-final-review.md)实际重跑三项核验并认可本有限阶段发布，未签正式门。

## 未签收与下一依赖

L1缺席；P-E0 mandatory not_evaluated_other_profile，旧session限速尚未重测。共同8MiB、精确常规64MiB/3.10GiB、四档实际完整运行、三场景各三轮稳态、实际8小时均未签；当前32GiB M4不代替真实16GB，16GB not_run，M0 not_accepted。下一步修复真实session先睡完整周期再计算导致超期的问题，保留权限/幂等/错误边界，再用真实P-E0 fixture/原预期及持久化IPC完整30日实测，不用本full实验替代。
