# 实际session发布节奏修复

v0.5.52只交付原session Run节奏修复及一日实际回归，不签完整30日P-E0或正式速度门。负载和经济规则未变，不做UI/GPU/AgentKit。

## 实现与实际验证

[方案](PLAN.md)：首次发布立即，随后计算/IPC占用原时间间隔；成功后next=实际release+interval，迟到输入不积累追赶额度。原interval<=60秒权限上限不变。Store原actor/payload幂等入口验证已存在operation；原票与冲突不更新节拍，already_recorded不再仅凭ID存在。普通失败保留节拍、模型错误仍暂停，Resume/相同速度不重置；明确Pause/真实速度切换开启新时段。进程内下一release可观察，重开不恢复墙钟节拍、不自动补跑，模型时钟及原票仍持久化。

最终[源码](source.json)246a60c24824a6305ea12e4faccd63d07219782cb4788c8b9a4605ce9f4933bc，145文件；binary354c5e74ab0fa88fc5b8b9306c04efd331c2e9fdacf6357901756834926ea9af。[实际回归日志](session-tests.txt)为1个bin单测及15个session集成测试，共16通过/0失败/0忽略；workspace all-targets clippy、fmt、Release build通过，不称全仓测试。所有测试/build句柄实际终态后封存，无运行中的本项目基准。

原session路径24次小时Run一日实际58.215741291秒，24次持久票、终值与24次直接Store推进一致，state SHA e12c19954ca89e978f61c64e4ce74e58c4aa4b99b095a663613288fd1e99849d。首次release为0、末小时最早57.5秒，因此原“必须>=60秒”睡眠下限改为>=57.5；严格速度报告仍用<=60秒，没有加容差。耗时含实际子进程启动、IPC及执行持久化，但不包含测试父进程预先创建Store，因此不能代替从Store创建前计时的正式30日测量。同组其他集成测试并行，条件如实保留，既不宣称隔离性能测量，也不外推30日。

[日志核验](verification.json)及[静态完整性核验](static-verification.json)实际运行通过，分别绑定明确测试/一日期末和源码/二进制/冻结输入，不将静态通过算运行通过。

## 初版失败保留

[初版源码](initial-wait-assertion/source.json)a84507e2b8b07b77fa05750d4894940696f27fffa5355ae2dce8a23ce21007e1：[孤立新测试](initial-wait-assertion/test-isolated.txt)通过，随后[并行回归](initial-wait-assertion/tests-failed.txt)14通过/1失败。原因是测试强求后续有sleep；41.67ms间隔可能已被正常计算/IPC消耗，零sleep合法。修复为直接核对实际next_release在原票/冲突/Resume/同速设置前后不变，Pause后为空，而非缩小负载或改变经济结果。初版一日57.943秒与最终一日58.216秒分源保留，不拼成共同通过；最终16项全部重跑。

[advisor单轮独立代码审查](advisor-final-review.md)未发现此有限阶段发布阻断，认可原权限及错误/幂等节拍边界；未签正式性能门。

## 未完成

完整30日P-E0 unpaced/实际60秒日档、full四档、L1联合、共同8MiB、精确常规64MiB、正式三轮稳态、实际8小时仍未签；真实16GB not_run，M0 not_accepted。下一步用真实P-E0 fixture和空间包，从实际Store创建前计时，经原session IPC逐小时完整30日，核验原预期现金/库存/消费/税务及冷恢复，不用full L2或一日回归替代。
