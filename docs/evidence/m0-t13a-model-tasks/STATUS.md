# T13A-3e：独立模型证明helper与报告兼容

发布v0.5.44。**实现与有限功能验证完成；8MiB诊断、64MiB实际所有者、正式性能和8h仍未通过。** 本轮没有完整新来源Sim／worker／真实Store冷开轨迹；不能用下面的CPU参考或构造image链替代。

## 已实现

scale-sim-probe把原CPU参考生成、冷开前image验证、期末双Engine完整比较移到三个受监督helper；父驱动保留原723个目标／724窗口要求、原票重试及原冷开Status检查，不持有完整Engine。helper没有派生子进程，仍属同一观测树中的实际模型／验证负载。大工作区不是诊断，但报告、golden写缓冲、root摘要、控制数据仍须收费；本轮没有把它们藏进64MiB预留后宣称8MiB通过。

请求绑定source／tier及三份输入hash。任务目录与工件拒绝覆盖；临时文件fsync后发布，result最后发布；父驱动确认成功退出、PID及全工件长度／hash后才继续。普通错误／超时路径kill+wait；父专有管道断开使监督路径helper停止，实际杀父测试已通过。裸内部CLI入口不带parent-pipe时没有这一承诺。全局额度不能仅凭EOF退款，仍需监督者确认退出。

Reference的流式出口不创建完整报告Value；固定19字段空气摘要保留旧Value的f32→f64表示。旧API用Value出口，两者共享同一模型运算，不经JSON往返重建旧Value。代码见[scale_model_task](../../../crates/city-headless/src/scale_model_task.rs)及[scale_reference](../../../crates/city-headless/src/scale_reference.rs)。

## 实测与范围

最终来源 `6d7abef05ed9cf08984b1ae263d7b63336e792d9d1a7a7d1f430102da04757d5`，二进制 `b70ba883a43fe2fc42bdf2a0b6920af50653dab779f7d2756e700200b1b5fa1c`，原acceptance hash不变。

- 最终来源6项目标回归通过：5项helper反例／正常流式参考及1项full旧API。坏来源、部分／旧结果、坏工件、容量边界、杀helper与杀父管道回收均有实际测试。全target release离线clippy及fmt通过。
- 独立full CPU功能运行完成，仍为64工资／2万债／16义务／8单／720小时。全部723个目标的business／air／windows、完整CPU期末Engine字节与v0.5.42归档完全一致；报告除source和wall元信息外逐字段一致。[核验](verification.json)直接重读归档原始字节，不只读取布尔结论。
- 最终功能脚本7个命令／8个独立实例footer齐全，失败命令为预期exit2。Reference正常退出、请求／工件发布及完整比较均真实执行。Before／Compare输入链由脚本使用实际CPU checkpoint构造，**属于构造image链反例，不是实际Store保存／冷开通过**。
- full CPU命令约34.24秒，仅供复现功能运行；它与最终目标回归同时执行，且系统pressure始终warning=2，没有RSS／footprint共同测量，不能用此时长签速度或性能门。reference-probe父进程峰也不能替代scale-sim-probe驱动费用。
- 初版来源完成全workspace回归；最终报告修复版仅重跑上述受影响路径与全target clippy，**未重跑最终来源全workspace**。不同来源结果不拼接为最终来源全仓通过。

## 失败保留

初版流式报告使maximum从旧`2.0000178813934326`变为`2.000018`，CPU完整Engine／723目标未变，但报告兼容核验失败，见[初版源码](source-initial.json)及[原失败包](initial-failed/manifest.json)。第二版保留了文件表示，完整CPU文件对照通过，但新增Rust测试显示不能把JSON解析后的f64和未解析Value直接等同，也暴露兼容API不应增加JSON往返；随后拆成两个报告出口，分别验证内存Value和旧文件边界。第二版源码／实际工件和失败测试均保留，不改成最终来源通过。预备clippy／Sized编译错误也在validation包，只有最终命名日志用于最终通过结论。

135份源码、source support及三份实际功能工件包共88项原始文件完整保存；validation另16项日志。[校验脚本](verify.py)复算源码、支持包、原acceptance及所有归档长度／hash。[方案](PLAN.md)和[下一依赖](NEXT.md)保持原冻结标准。

## 仍未通过

原v43系统压力失败继续保留。本轮结束时系统仍warning，未满足正式重试的连续120秒normal条件，未启动正式联合／稳态／8h，也未停其他应用。驱动与helper诊断费用、64MiB实际grant及触页余额、最终来源完整Sim轨迹、四倍率、正式三轮、实际8h与真实16GB仍未通过；M0 not_accepted。advisor审查见[独立结论](advisor-review.md)。
