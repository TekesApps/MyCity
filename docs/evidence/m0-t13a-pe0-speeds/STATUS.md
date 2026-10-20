# 实际session P-E0完整30日：无等待通过、60档失败

v0.5.53交付无等待30日实测；v0.5.54补同源真实60档完整30日，业务正确但严格性能失败。未签60秒/日性能、full L2四档或整个M0。只Rust模型和CLI。

## 冻结与实现

[测前冻结](freeze.json)为原P-E0输入与空间包、720整点加1800/34200共722递增目标。业务模型和原每日至多60秒/完整30日至多1800秒门不变；这是新切分，原probe从半点继续推进，不能称同一旧命令序列。full L2原负载和四档要求保留，不因本P-E0模型较小而被替代。[方案](PLAN.md)明确经营/证明和计时边界。

Rust监督持实际driver Child及3600秒超时，driver先生成独立直接Engine参考，释放参考后再从实际Store创建前开始经营计时。实际session有界JSONL逐请求推进，最终必要Checkpoint持久确认和经营日志fsync计入末日与总段；不在中途暂停/换速/重开重置节拍。模型Core hash与包含空间引用的Store hash分列，核验双方及同轨迹完整Engine含metadata；Checkpoint沿用needs_reopen/Store门，不清未知提交。

最终[来源](source.json)0866a0f2a4b37e5f22b9c3002198b42675d723787079cc1a01633d9aacbbea86，146文件，binarya03c219f11a2149b17addbe80b1512f50ca70927bf73a5bd149a6577b66603a6。[54项定向回归](tests.json)（37lib+1bin+16session）、workspace all-targets clippy、fmt与Release build均终态0；不是全仓测试。新增Checkpoint实际恢复/原票及节拍不变回归通过。

## 真实成功轨迹

work/t13a53-step-02实际19316终态0，120.257秒normal准入；无本项目测试或其他基准同行。观察器21.059078秒，经营18.513690167秒（含创建/启动/IPC/持久化/末Checkpoint），参考0.112895秒、后验证单列。30日邻接预算最大1.037899167秒，末日含Checkpoint0.640936708秒；本无等待分支原逐日60秒与总1800秒门满足。

[独立核验](verification-step.json)直接检查原始请求/回复、每target Core及联合hash、实际CURRENT/root/checkpoint索引/原票链和完整模型字节。722目标及722原票、30日、同轨迹整个Engine含metadata、最终冷开、最旧与末票无副作用重试均通过；四处旧中途恢复不冒充重测。原P-E0终值现金cents=808000/56000/816000/1320000、生产与原料使用300000、家庭/政府消费180000/120000、税24000、供方余300000、其余库存/在途为空，离线核验从真实checkpoint再次读取验证，未用报告中的通过字段代替。

采样RSS峰37814272B、已获得footprint峰23987472B，86次进程footprint缺测保留，不宣称绝对峰；全程采样normal、swap净增0。3份正常heap分别归属supervisor/driver/session；driver与session生命周期峰6567567/5453425B，不能相加冒充并发峰或全部诊断费用。观察器other_diagnostic_owned_bytes为0只表示本路径没有注册CpuLog外部owner，不能推导共同8MiB通过。没有L1/正常64MiB实体池同行，不能推广为完整城内存预算。

[工件清单](t13a53-step-02/manifest.json)归档全部28文件，[静态核验](static-verification.json)绑定源码、二进制、冻结输入及两次运行实际工件。脚本均实际执行通过，只支持明确范围。

## 原失败与分源

首轮step01实际99776终态1/model2，1.537秒，在首target1800比较失败，只完成1笔，完整30日未完成。原因是CPU Engine hash对Store空间联合hash，[失败核验](failure-step-01.json)用原CPUhash+genesis空间引用独立重算，恰好得到原实际Storehash。修复新增独立Core字段，保留联合hash与空间绑定，并完整重跑。原[来源及54项日志](initial-store-hash-mismatch/source.json)42a1122f和[21文件归档](t13a53-step-01/manifest.json)保留；被drop回收的session无正常heap footer，保持缺失。旧类型编译错误、布尔clippy告警及各自源码另保留，未拼成最终来源通过。

[advisor独立终审](advisor-final-review.md)实际重跑核验并认可上述有限阶段发布，未签其余门。

## v0.5.54：真实60秒/日完整30日

work/t13a53-paced-01实际65764终态0，之前120.222秒normal准入；没有本项目测试/build/其他基准同行，仅只读源码与流式离线时间戳分析。原源码0866a0f2/binarya03c219f/722目标及输入全程不变。观察器1804.365328秒，经营1800.240858542秒（原门1800，超0.240858542秒），参考0.119919秒及冷证明3.337199秒单列，始终计同组资源。

[完整独立核验](verification-paced.json)通过业务正确性，但daily/whole/speed三字段严格为false：第29日60.0806095秒，第30日60.173214708秒，末日含必要Checkpoint/日志fsync60.240858542秒，不能用60.09秒邻接间隔或扣IO转绿。全部722目标与原票、原P-E0终值、同轨迹完整Engine含metadata、真实冷开及首末重试正确。[跨档比较](cross-rate-engine.json)确认无等待与本60档实际最终Engine均绑定4333551a7ac773c5c8fa900242c8fce79ed36046e4ff1b6545cc4d2d673fbe95，facts逐字节相同。

[整数节拍核验](pacing-drift.json)显示721次后续请求都有正等待、0次迟到ready无等待重基；实际释放相对前次deadline累计2558500583ns，首末release减去全部既定interval与该累计量精确相等。这里是等待后释放迟到，包含唤醒调度及相邻代码开销，不能独占归因OS sleep。支持下一步锚定scheduled_release，不能预言新实现通过；旧运行未中途重置节拍或加容差。

本次采样RSS峰34979840B、已获得footprint峰21709760B，7220次进程footprint缺测保持缺测；全程采样normal、swap净增0。观察器最大采样间隔0.652秒/采集窗口0.411秒如实保留，不称每帧严格250ms。3份正常heap；无L1/64MiB池同行，不能签完整城市内存。全部28文件[归档](t13a53-paced-01/manifest.json)与[静态核验](static-verification-paced.json)通过，进程0/观察器完成/核验0均不等于性能通过。

[advisor真实结果终审](advisor-paced-review.md)实际重跑三核验，同意有限失败证据发布。后续[待验收清单](pending-gates.json)保留各原门；[联合内存只读定位](next-memory-work.md)只是下一缺口，没有新实现/联合通过。

## 未完成

60秒/日完整30日已实测失败，需修复实际释放锚点累计漂移并重测原轨迹；不能用无等待18.5秒代替。full L2四档、L1联合、共同8MiB、精确常规64MiB、正式三场景各三轮、实际8h仍未签；真实16GB not_run，M0 not_accepted。P-E0有限资金原型不因此证明经济长期平衡或完整产品可玩性。
