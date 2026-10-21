# P-E0计划释放节拍：完整30日无等待与60档通过

v0.5.55只签实际session P-E0小模型的本来源30日实验。原v0.5.54实际失败证据保持原样；本次没有缩减负载或增加容差，未签完整M0。仅Rust模型与CLI。

## 修复与冻结

旧节拍以每次实际唤醒为下一期限锚点，累计释放迟到导致原1800秒门失败。现在等待前固定scheduled=max(ready,saved_deadline)，成功后next=scheduled+interval；早ready但晚唤醒不再永久推移以后期限，真正晚ready仍重基，无闲置追赶额度。权限、原票/冲突、失败、暂停和变速边界保留；未知提交仍需重开查票，不由Checkpoint解除。scheduled/actual/lateness分别报告，实际迟到不从经营计时扣除。

[冻结](freeze.json)保留原P-E0输入、空间包、722目标（720整点加1800/34200）与每日至多60秒/完整30日至多1800秒门。[来源](source.json)为6a3b78e609022f4709ef43d4e113cc9f80a98a9c1e3c01ca85cb049293f959a2，146文件；binary853d1d79e30f9272f60b5e2dfa81fd97445b40fdea192ef52f1e025a26c06b74。[55项定向回归](tests.json)为37lib+2bin+16session，failed/ignored为0，clippy workspace all-targets、fmt、Release build均实际终态0；不是全仓测试。一日回归58.142秒不替代正式30日。

## 两次真实实验

两次分别fresh120秒normal准入，来源全程冻结，无本项目测试/build/其他基准同行。原参考CPU及后验证与模型计入同组资源，单列计时；经营从Store创建前至末Checkpoint及日志fsync，不只统计CPU。

| 实验 | 实际终态 | 观察器总秒 | 经营秒 | 最大日预算秒 | 末日含Checkpoint秒 |
| --- | --- | --- | --- | --- | --- |
| work/t13a55-step-01 | 69125: 0 | 21.0490335 | 18.510858625 | 0.8008235 | 0.652194291 |
| work/t13a55-paced-01 | 58933: 0 | 1799.818631916 | 1797.659120708 | 57.642627666 | 57.659120708 |

[无等待核验](verification-step.json)与[真实60档核验](verification-paced.json)直接读实际CURRENT/root/checkpoint索引、原票链及完整Engine。两次全部722目标、原始权限主体/载荷/修订、每target Core与空间联合hash、722原票、完整Engine含metadata、原P-E0终值和最终冷开/首末重试通过。旧半点命令及四处中途恢复不冒充已重测。

原现金cents=808000/56000/816000/1320000，raw_used/produced=300000，consumed_f/g=180000/120000，tax=24000，supplier_remaining=300000，其余库存/在途为空。[跨档比较](cross-rate-engine.json)及各运行实际checkpoint绑定确认新step/新60/旧v54 Engine字节均为4333551a7ac773c5c8fa900242c8fce79ed36046e4ff1b6545cc4d2d673fbe95，facts也逐字节一致。真实60档的逐日和全段严格门均true，没有给旧失败加容差。

[整数节拍核验](queued-pacing.json)读取完整722回复：721次后续请求正等待，0次期限重基，scheduled相位漂移0；实际释放累计迟到2443652884ns仍单列保留，最大13950625ns，不从真实耗时扣除。queued审计可处理部分轨迹且跳过坏行，单独退出0不能签收；必须联合要求完整数量、完整JSON及实际账本的verify.py。没有独立ready时间戳时，重基分类依代码与期限变化，不冒充独立ready测量。

## 资源、工件和边界

step采样RSS峰38027264B，已获得footprint峰24118544B，86次进程footprint缺测；60档RSS峰35143680B，已获得footprint峰21021968B，7202次缺测保留，不宣称绝对峰。两次均采样normal、swap净增0。60档最大采样间隔0.660秒/采集窗口0.439秒，不能声称每帧严格250ms。3份正常heap记录不代表精确共同8MiB或常规64MiB已闭合；L1不在此实验。

两次各28原始文件完整归档：[step](t13a55-step-01/manifest.json)、[60档](t13a55-paced-01/manifest.json)。[静态核验](static-verification-paced.json)实际检查来源、二进制、原冻结输入、observer及全部归档逐文件hash。[运行前advisor审查](advisor-preflight-review.md)与[实际结果独立终审](advisor-final-review.md)分别保留；后者重跑核验，不重跑模型。

## 下一依赖

P-E0小模型step+60门仅在上述范围通过。full L2四档、完整L1联合、共同8MiB、精确常规64MiB、正式三场景各三轮、真正8h仍未签。真实机器为32GiB M4，真实16GB not_run，完整M0 not_accepted。

下一实现回到完整L1联合的分配前容量约束与阶段归属，再重新准入原负载，不扩大本小模型结论。正式Steady控制器的advisor接入约束暂存ignored work/steady-controller-next.md，未实现/实测。当前磁盘可用约9.4GiB只是后续准入线索，需实算多份快照和2GiB余量，不沿用旧准入、不删除唯一失败证据。
