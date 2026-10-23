# v0.5.130：原满级内存现状与真实Engine耗时诊断

这是一份实际诊断结果，不修改冻结负载、性能门或Rust生产代码。0.1三日施工—交通—经营联合已在v129发布；本轮继续M0，不让其他模型开发停等。本次运行是冻结布局与恢复压力负载，不含完整30日真实业务；真实业务计时仍按v128独立报告，不能互相替代。

## 原满级同相位仍超限

固定来源db4577273a37fdbeeb95c092e20356ba2386bdb0dd311f2b6020de2bb89f0daf、exe83b7a28460c8d82ae1b5c5c17ca154359648d6d3937103f973a8fa988fcc373a，即v128，不把v129 CLI源码或后续候选指纹写成该次来源。完整ZIP、CRC、构建输入指纹、exe内嵌身份和冻结fixture启动前核验；脚本及依赖启动前冻结。原控制器保持361272 native/1232426 stub记录、64MiB pool与全部布局，新增两代快照由实际模型产生，不是手工缩小或复制替身输入。

[终态收据](native/terminal.json)、[独立同相位核对](native/native-diagnostic-summary.json)、[原始采样](native/case/host-samples.jsonl)与[完整恢复窗口](native/case/full-restored-window.json)显示：120秒安静准入后，控制器退出0，模型28.768227125秒、观察器29.35862575秒，两代快照/全部payload校验读回及第2代冷恢复通过。期间外部编译/重任务采样0、采样错误0。四个被跟踪进程身份及控制器均已确认退出。

暂停且完整恢复的3帧均为3336228080B，原normal上限3328599654B，仍超7628426B。此值与v125同相位相同，但来源不同，只能说明该配置诊断未观察到物理降幅；不能称same-source A/B或正式门通过。全程raw峰3336293616B，比该窗口高65536B；全程footprint/RSS各缺2个进程样本，不能把缺测当零或完整最大值。R07不授予新扣减，不能从raw再减已释放背板。观察器已计入raw，不重复加；外层Python控制器的资源另列，不能用不完整角色集证明shared8MiB全部通过。

该轮不是300+1800秒三轮L0/L1/L2、四档完整速度或真实8小时。物理normal仍未接受，原8小时失败证据不撤销，真实16GB为not_run，full_M0=not_accepted。

## 真实typed Engine与canonical构形

诊断代码仅保留在隔离分支，作为证据发布于 `typed/isolated-source`，未合入 `crates/`。使用既有真实Engine恢复同一2507418B检查点，三轮typed hash与独立Value字节/SHA oracle一致；此前1KiB小缓冲方案未合入，本轮没有增加缓存或省略摘要。

三轮总hash38.68–39.77ms：typed序列化5.74–5.97ms、输入校验7.67–7.83ms、canonical输出含SHA25.28–25.97ms。full-null完整序列化/校验/遍历34.44–34.73ms；相减只作粗估，不是精确SHA耗时。每次1020125写调用，216905节点，其中174240数字、21526对象和146340键；数组非字符串标量分隔符合并最多减少49413调用（4.84%），不代表同幅度耗时收益，暂不实施。

[诊断汇总](typed/typed-shape-diagnosis.json)与[实际运行](typed/runs/)保留：78构建、79三轮探针、80完整core lib135/135、81构形探针构建、82实际构形探针、83 core all-target严格Clippy均退出0，来源未在各命令期间改变。80发生在新增shape example之前；83覆盖两个新增example。它们属于隔离诊断来源，不计为v130生产源码完整回归。原样日志末尾空行保留，不能为了格式检查改写证据。

Status的business/full是不同投影，checkpoint字节SHA不能替代当前Engine摘要；正常窗口revision会变化，不采用无依据的摘要复用。advisor依据数字节点的新证据建议下一步只验证受限整数原字节快路径，保留全输入校验及其他数字回退；尚未验证的优化不列为通过。

## 存储与保留

32GiB Apple M4，HP P900 USB/APFS外置SSD，work和共享target均在该卷。不能转用旧内置盘IO结果或冒充16GB设备。

两代完整原件仍在 `/Volumes/HP P900/dev/MyCity/work/v128-native-01/combined-control/model/layout/snapshots/snapshot-000001` 与 `snapshot-000002`，各32文件，按st_blocks合计6770327552B（不含目录，不是APFS独占物理空间）。固定exe、ZIP、依赖及全部原始日志仍完整保留；此发布中的hash/报告不替代原件。原190份迁移保护原件未改动；本轮新增恢复基线仍有后续依赖，不删除。

本轮复用共享target，无新独立缓存；未清理业务输入或压力原件。前一v129源码ZIP去重的89395200B属于前阶段，不重复算成本轮释放。

## 2026-10-23恢复基线交接与退役

上述“完整原件仍在/仍有依赖”描述是v130发布时的历史状态。后续当前全额基线已由j3p第二代及受保护旧部分prefix完整原件承接，实际当前reader兼容恢复通过后，精确退役两代v128的pressure.bin和work-reserve.bin；它们现已不可直接完整恢复。其余组件、日志、source/exe保留，见[交接与精确清单](../m0-t13a-smoke-handoff-137/STATUS.md)。
