# v0.5.128：有界分段计时与共享材料完整生命周期

仅Rust模型与CLI。最终build-input source为`db4577273a37fdbeeb95c092e20356ba2386bdb0dd311f2b6020de2bb89f0daf`，固定exe为`83b7a28460c8d82ae1b5c5c17ca154359648d6d3937103f973a8fa988fcc373a`。见[完整输入文件清单](final-source-manifest.json)、[源码ZIP及可执行绑定](diagnostic-final-binary.json)、[公开原始文件映射](publication-manifest.json)。完整源码ZIP、exe、Store/WAL/checkpoint在外置SSD原路径保留，hash清单不替代恢复输入。

## 实现及定向验证

- `normal_pool`服务循环复用8KiB控制缓冲；若有活动role arena，检查完整分配容量实际落在该arena内。原帧上限与256KiB诊断界限不放宽；最大合法帧、溢出、尾随、长短复用、截断与IO错误测试通过。没有实测其对原满级L1的物理效果。
- `sim_host`新增10项固定计时（240B），累计count/total/max而不存逐请求向量，计入已有诊断拥有量。`scale_sim`只把成功Status的固定摘要写入既有有界轨迹，128KiB每行、128MiB整轨迹界限保持；不增加Status、worker窗口或账务时钟。
- 共享Exchange raw7现在实际覆盖半建清场和完整投运后拆除；材料累计数量、真实lot basis、容量撤回、废物费用、WAL/checkpoint/cold及原票重放均核对。25/26各12过1失败，分别是测试误用非道路撤回时间戳、试图夜间增加有效工时；修改测试后27通过，36最终13/13通过，没有放宽生产工作时间规则。35与40的headless lib均93/93通过；39全workspace/all-target严格Clippy、41固定release构建通过。不是整个workspace测试命令通过；v126的workspace15 exit101历史不改写。

36两份成功完整恢复输入共28个清单条目、576173内容字节、651264分配字节，全部保留在`work/construction-utilities-current-01/36-retained-raw7/test-artifacts/`；[完整清单](retained-raw7-inputs.json)列出两份CURRENT、根、genesis、checkpoint、index、WAL及空间输入。25/26失败原件保留。27普通成功临时目录当时已清理，不能称27原目录可直接恢复。

## 完整30日实际分段观察

[02独立校验](profile02/verification-final.json)实际exit0：full 30日、720小时、723目标、724个worker窗口，64工资、2万债项、16项周期义务、八单实际交付；CPU oracle、完整Engine比较、检查点/冷恢复通过。全部723份显式Status获得计时摘要，同一PID/instance，使用最后累计值；724次摘要计算包含启动ready，不能把各Status累计快照相加。[完整轨迹](profile02/full/transcript.jsonl.gz)为原文件完整gzip；CPU最终Engine、目标及原生采样也完整保留。

| 计时区间 | 次数 | 累计秒 |
|---|---:|---:|
| RunRequired父区间 | 724 | 111.849763 |
| 缓存恢复、重绑、重置 | 724 | 12.301172 |
| 输入编码、发布 | 724 | 7.229359 |
| worker创建提交至首个validated响应 | 724 | 14.846428 |
| 后续worker等待及验证 | 1448 | 17.884984 |
| 候选读取与导入 | 724 | 20.121429 |
| 持久Accept执行与结果验证 | 724 | 29.019096 |
| Status业务摘要 | 724 | 23.543316 |
| Status完整状态摘要 | 724 | 26.815854 |
| Status Air摘要 | 724 | 0.799805 |

RunRequired与子区间重叠，不可一起求和；首响包括worker工作与调度等待，不是纯spawn；Accept包括事务/验证/持久化，不是纯fsync。operating为188.456830375秒，reference39.085617292秒，whole driver228.159421375秒，observer228.867021709秒。unpaced报告中的目标满足只适用于本次unpaced评估，不代表1秒/日通过，更不是优化前后干净对照。

11次外部编译采样命中（不等于11个独立任务），活动采样异常0；916帧中RSS/footprint各有118个进程样本缺测，观测峰101007360B/80185216B只是已观测峰。独立verifier将环境分类为`polluted_or_sampling_incomplete`，不签clean性能。M4/32GiB，work和共享target使用HP P900 USB/APFS外置SSD，不混用内置盘IO结果。

## 失败与范围

01准入等待时发现旧候选虽然生成计时却只保存payload hash，故在模型尚未启动时核对PID/PPID/birth后只终止自有readiness helper，terminal exit1；[终止收据](profile01-aborted/root-abort-prelaunch.json)及全部原件保留，不计为业务运行。01脚本启动后发生空identity防护编辑，执行版副本是按准确逆补丁重建并与事先记录SHA匹配，不能称启动时已捕获；[版本来源](profile01-aborted/controller-code-provenance.json)明确区分。02在启动前冻结脚本、测试和manifest完整副本，运行中不修改。verifier一次参数用错的usage exit2另列，正确位置参数命令实际exit0。

advisor独立核对计时聚合与上述分类。没有L1，不替代原normal/common、三轮四速、8h和真实16GB门；原normal失败、原8h失败未完成、真实16GB not_run维持。没有签完整M0或完整0.1。下一候选是固定小缓冲合并canonical hash小写入，尚未实测，不先声称收益。交通/施工/经营的新增公用库存联合CLI仍在独立分支；两日批次结果不能证明连续供给或断供后的工期，正在复用已有Pause/Resume修正，未混入本发布。

## 回收

[回收收据](stage-retention.json)：终态、无文件句柄后，仅对8份内容相同的源码ZIP做硬链接去重，历史完整路径和字节保留；唯一文件分配空间减少32473088B，卷可用空间观察增加32452608B，不称APFS独占释放。未退役恢复快照、故障输入、依赖、验收exe或target；本轮继续复用同一外置共享target。
