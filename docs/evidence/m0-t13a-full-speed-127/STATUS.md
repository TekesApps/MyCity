# v0.5.127：满级30日两档速度实测

本阶段交付两档独立L2功能验证与受污染环境中的观测速率，不签原M0联合验收。[独立核验结果](verification-final.json)、[完整原件与发布映射](publication-manifest.json)及[固定二进制绑定](fixed-binary-record.json)可核对。advisor独立检查了30日期限、最终checkpoint、时钟nonce、来源和控制器修复，未发现改变下列分类的问题。

## 固定负载与实际结果

两次均使用 source `ac02e7ccb7e2b9e842e5f8dd15903b14f7fff9b9426909449b36b33bbd08db6c`、exe `557b2e4d0bc89f429a07a270c9f388b508d595c8b4a1f233b795becdf75797e7`。M4/32GiB；work与target位于HP P900 USB/APFS外置SSD。每档完整执行30游戏日、720小时、723个验证target及724个worker窗口；64账户、128库存、64工资、20000债项、8城/8交付单和峰值4在途不缩减。功能、独立完整状态比较与checkpoint/cold均通过；两份nonce分别绑定各自实际模型PID，没有混钟。

| 档位与尝试 | operating实测 | 固定整段预算 | 逐日及整段观察 | 观察器全段墙钟 |
|---|---:|---:|---|---:|
| 1秒/日，rate1-01 | 188.759230042秒 | 30秒 | 30日全未达；整段未达 | 227.079173秒 |
| 10秒/日，rate10-02 | 299.950156541秒 | 300秒 | 30日与含最终checkpoint整段均满足 | 337.391924084秒 |

预算按预设日历起点逐日重算，不以相邻完成间隔掩盖累计迟到。独立参照计算分别36.605244542与36.162966秒，属于对照开销，不能混入operating归因。进程退出0只表示实验完成。两份原始业务report与完整日志分别在各尝试子目录；transcript、CPU目标和最终Engine以完整gzip字节发布，未只留下hash。

## 污染、缺测和物理观察限制

1秒档记录到5个外部编译活动采样，另缺1个活动样本；10秒档记录到3个活动采样、该类缺测0。这是采样次数，不是任务数量或持续时长；零缺测不保证采样间没有短命任务。两档`clean_measurement_environment=false`，总体`clean_speed_acceptance=not_accepted`；10秒档只能称本次观测达标。

观察器分别采908/1350帧，已观测RSS峰100,712,448/100,696,064B，footprint峰80,037,784/79,988,608B；**另有98/41个进程RSS及footprint缺测**，不能据此签完整进程组峰值上限。采样swap净增与非normal压力时长均为0，仅为本轮观察。controller自身RSS单列，不重复加到观察器组。未将这些数据称作正常3.1GiB、16GB或城市性能验收。

无关进程argv不进入公开材料；原始收据完整保存在SSD，发布副本只移除PID/comm字典的args字段，逐份原/发布hash及变换在清单中公开。

## 控制器失败与修复

rate10-01在准入约76秒因两次ps快照之间的短命进程变化误判而exit1，未启动模型、无observed目录；跟进核查无残留准入helper。原目录与controller-terminal保留。修复使用单次ps的完整executable字段，保留含空格路径，用PID/启动身份/父进程约束自己的超时信号；准入采样异常记为阻断，不抹去缺测。

后续是明确的新rate10-02/speed02目录，未重启任何活模型，也未覆盖旧尝试。1秒档控制器SHA为`cea36abf1931b567916ce40a43ffa1c96233edcd288154e2a9bcc3fb9dd73bbc`，10秒档为`32311cb97680b55f58ae4f74f50ead3ab11106421a4e28d706cc9c927ddfb45d`；原脚本版本保留在controller-revisions。模型exe及冻结负载未变。最终13项纯Python守卫测试通过，未以纯测试冒充城市运行。

## 仍未证明的门与回收

L1缺席；原normal64MiB、common8MiB、三轮四速矩阵、真实8h及16GB不由本阶段证明，完整M0仍不接受。已有8h失败与原normal超额不被覆盖，真实16GB保持not_run。v126模型和后续源码候选不继承本固定v125来源的性能数据。

本阶段完整业务Store/WAL/checkpoint、源码ZIP、固定可执行文件、原始日志和故障证据均保留在`/Volumes/HP P900/dev/MyCity/work/speed-full-v125-01`及其引用的pool-control-current-01，未删除快照。复用共享target，当前构建缓存仍供排队验证使用，未新建独立target；本阶段回收0B，不把卷可用空间波动算作成果。
