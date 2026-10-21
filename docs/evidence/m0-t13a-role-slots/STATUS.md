# 角色恢复槽与实际调用接入（v0.5.76）

仅Rust模型／CLI。源920a75746d3919b9ea4e6edc976c261cb6f97f119f3292267928bea0dc6e33a1，172文件，固定源见source.zip；固定CLI SHA为5e0efbdc2d66fe776cc30657fa8d984b8fe199fbb5db0b29d89f9c1962909721。以下是有限阶段实测，正式性能与完整M0仍未通过。

## 实现范围

EngineSlot一次构造完整World/Rows/Timeline/Background及准备背板，直接Map／Seq seed目标，不先反序列化新Engine再复制；checkpoint copy复用原背板；reset使候选不可观察，失败不发布；publish_swap仅交换已有root。48个主要背板身份含World、三张Engine表、Timeline、Background和全部准备工作区及控制块。身份稳定只证明这些背板复用，不证明serde scratch、哈希／错误字符串等零临时分配。

共同校验由原validate_restored拆为validate_resident，slot不再次reserve准备区；原普通API仍保留。未知／重复／缺必需字段、超量及EOF均拒绝，可省略Option字段由reset取得空值。一般Arena保留free栈、稀疏槽、MAX最后代次和声明上限，固定容量来自本地类型而非文件。

Store创建/打开时保留备用EngineSlot。CPv2实际tmp文件在该槽中完整恢复并验证字节长度／hash；冷开全部验证后才交换到genesis root；v1也seed备用槽。失败及外层hash错误保持备用槽失效，原权威状态及poison／fsync／CURRENT／GC顺序保留。新增常驻root进入owned统计，目标Store头扣两Engine inline，避免和既有多root拓扑重复。

Reference预建恢复槽，用同一4MiB codec写checkpoint并seed，终局不再新建Engine或完整编码Vec。ordinary Compare预建双槽及codec，实际读取的原文件经长度／SHA验证后seed；business与air均spool完整字节比较并核对EOF，不改成仅hash。Endurance／Speed仍走既有独立stream完整Engine比较。codec输入slice不能逃逸，失败后的调用长度从零开始。

copy_from_into明确是checkpoint语义，hot journal由Store永久回执链负责；不得当成保留热重试凭证的通用Engine clone。

## 已有开发期实测及最终验证

冻结前定向EngineSlot满级20k债／64工资／pending任务、copy/null/截断/超量/重试及48背板身份测试通过；v2保存/冷开反复复用及故障恢复7项、recovery15项通过；模型子进程／满30日Reference／comparison／slot共13项通过，另有稀疏Arena seed测试。之后补充缺省Option与重复／未知字段反例并冻结，最终全部以[接续说明](NEXT.md)中同源全仓结果为准，不拿开发期结果替代。

开发期clippy发现int_plus_one与实现位于测试模块之后的布局问题，已修并定向clippy通过。最终同源全仓release退出0，563.767秒，388条harness通过记录含nested；clippy all-targets、fmt、build均0，见checks-final.json。完整旧world实际重开3013seq／2507418B与旧fault-reference全字节相等，原243文件及兼容副本前后逐SHA不变。旧Value业务1818613B、状态2480116B完整字节及Store联合hash一致，见compat-proof.json、codec-proof.json。

同源固定CLI原full30d真实Sim／worker功能装配退出0，端到端327.077秒、Sim含IPC与持久化288.214秒；720小时／723驱动／724实际worker窗口，64工资、20000债全付、8单交付，完整business＋air独立对照、冷开0.151秒及最旧票无副作用重试通过。序列3011（本轮未注入联合故障；不能与旧joint3013混淆），原opening及policy不改。无L1与normal pool，仅功能诊断；单轮耗时不作为正式分位或快进证明。见full-business-result.json与full-business-report.json。

730条退出heap记录均同源且success：三次scale-model-task lifetime Rust requested peak加64KiB尾部最大18169159B，两次Sim最大24884587B，724worker最大5980153B，driver614398B。这不是同步全进程物理峰值、实际分配准入或共同8MiB通过；见heap-summary.json。

## 预算、磁盘和剩余目标

没有扩大broker授权给旧动态分配贴标签。完整角色包、候选/空气工作区、所有小分配和实际分配前准入仍要闭合，实际64MiB及common8MiB未签；实际最新容量CLI基础54078840B，加仍计入的Raw暂态1325248B共55404088B，小于原67108864B、余11704776B；这是目标容量算术，runtimeGrantfalse，不是当前物理或实际授权证明。正式分级、四速、三轮稳态、28,800秒8h及真实16GB未通过；当前32GiB主机，16GB not_run、M0 not_accepted。

共享target复用，无新独立编译缓存或压力快照。当前完整业务、v70两代和v50旧格式恢复输入保留；验证期间未清理。终态检查79957／84121／84226均已退出、work/target无占用。新完整world作为下一阶段导入／重放恢复输入，完整保留；原始性能与故障证据、固定源/exe及共享target依赖未解除，追加删除0B。阶段末work18172168KiB（17.330GiB）、target1034288KiB（1010.047MiB），df可用115490279424B；没有把其变化算作回收成果。见retention-result.json与[集中核查](../disk-reclamation-live-recheck/STATUS.md)。先完成回收检查再推进下一阶段，没有新独立target。
