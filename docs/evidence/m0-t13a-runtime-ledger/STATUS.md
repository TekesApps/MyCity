# T13A-3f：原full实际轨迹与驱动费用实测

发布v0.5.45，Rust源码／二进制与已发布v0.5.44完全相同。本次在32GiB Apple M4上完成一次原full standalone，补齐v44仅CPU／构造image链的缺口。**真实业务／worker／真实Store冷开及驱动费用依据已测；共同8MiB、全局64MiB、L1联合、正式速度／三轮／8h和真实16GB仍未通过。**

## 入场与来源

[入场观察](readiness.json)25次采样覆盖120.186秒，全部pressure normal=1，最大采样间隔约5.02秒；随后新建world运行，无重用旧失败世界。原v43压力失败保留，不改为成功，不杀其他应用。原full的64工资／2万债／16周期义务／8单、720小时、723目标、724空气窗口、8192页和60秒空气步长均不变。

最终来源`6d7abef05ed9cf08984b1ae263d7b63336e792d9d1a7a7d1f430102da04757d5`，二进制`b70ba883a43fe2fc42bdf2a0b6920af50653dab779f7d2756e700200b1b5fa1c`。observer与v43原脚本同字节，原保护门继续执行。135份源码和外部测试支持包完整保存，实际进程均绑定同一source；源码和binary在运行中未变。

## 真实结果

[独立完整核验](full-verification.json)重读原始报告／raw／worker日志／存档及原票：

- 730个正常实例footer：驱动1、Sim2、worker724、模型证明helper3；没有把helper从共同进程树或费用排除。726个CpuLog producer目录与process instance逐一对应。
- 720小时、723目标、724个实际worker均完成，无失败attempt。每个owner的两份2MiB槽／四次借还及完整触页余额闭合，metadata1152B计费；全部724组input／checkpoint／candidate hash与v42相同。
- 最终sequence3011，全部3011张原票链的范围／操作ID／hash／genesis逐项核验；最早原票重试无副作用。真实Store checkpoint／新Sim冷开后，完整实际Engine（含metadata）等于CPU；完整CPU字节也与v42相同。不是构造image链替代。
- 5,536,188条原始计时／49,825,692B与每个执行／quote／replay分层报告数及最大值一致。此次只核对数据完整性，不宣称所有10类样本门或分位门通过。

实际全程316.365秒，业务段279.960秒，冷ready约0.08952秒。它是一次无等待full诊断；未跑四档实际paced节奏和逐日deadline，不把平均速度换算成某档已通过，也不从有限L2引擎外推完整产品城市。

## 内存与费用证据

1266帧RSS加和均核验。共同采样RSS峰76,546,048B，已取得footprint的共同峰63,685,976B；**1334次process-footprint缺测保留，短命进程绝对峰未证明**，不能把RSS改名footprint。[进程覆盖](process-coverage.json)还确认两个短命helper（PID81075、81078）没有进入任何RSS帧，只有正常heap footer；参考helper有140帧。没有把footer当作完整RSS峰，末帧只剩观察器／采样工具。全程pressure采样均normal，swap净增0；最大世界逻辑文件108,946,183B，无L1布局替身，也没有满级3.10GiB联合压力。

| 角色 | 本次费用或峰 | 结论 |
| --- | --- | --- |
| 业务驱动 | 整进程requested Rust heap终身峰535,289B＋64KiB尾部＝600,825B | 此次低于候选1MiB；当前代码未据此建立共同硬限额，不能外推任意输入 |
| Sim诊断 | 最大已收费2,446,904B，含256KiB临时预留 | 低于候选2.5MiB；现代码仍允许3MiB，未来须一致 |
| worker诊断 | 最大已收费848,016B，含槽metadata | 低于原1MiB；4MiB工作槽另属模型工作区 |
| observer | 已记录热数据上界581,415B | 原2MiB门内，native／栈及采集偏差仍单列 |
| 模型helper | 整进程requested heap终身峰29,425,301B | 包含真实参考／比较工作，不能当诊断费，也不能用两个峰相减求诊断 |

不同角色终身峰不是同刻峰，不相加冒充实测共同物理峰。helper仍有动态JSON树费用缺口，因此不加总上表宣布8MiB通过；64MiB尚无完整grant／未用触页余额证据。[方案](PLAN.md)记录advisor认可的待实现共同8MiB分配及具体收敛条件。

## 归档与后续

[3721项完整归档](t13a3f-full-01/manifest.json)没有省略payload；[静态核验](static-check.json)复算每项字节／hash、源码／支持包、原acceptance、binary／observer及入场观察。原始work已终止，原始world不再打开写入。运行方法与检查逻辑见[observer](observe.py)、[完整核验](verify_full.py)和[静态核验脚本](static_check.py)；[advisor独立结论](advisor-review.md)只签此有限范围。

下一源码阶段先落实helper有界具名控制数据／root校验资产和诊断容量账，再统一驱动／Sim／worker／L1／observer／监督的共同限额；原full再实测。随后仍须完成全局64MiB真实所有者／余额、冻结分支、四档、三场景各3×35min、实际8h／96保存／三类故障及最终独立审查。无真实16GB则not_run；M0 not_accepted，目标继续有效。
