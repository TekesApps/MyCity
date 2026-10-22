# R07独立背板、TTL补回与冷恢复（v0.5.104）

有限功能已完成同源实测、全仓回归与阶段回收；[advisor最终审查](advisor-final.md)确认可发布。本项不签normal或完整M0。真实业务与L1布局压力替身仍分开。正式三轮、四速度、稀有层、8小时与真实16GB各门继续按原证据验收。

## 最终来源及回归

来源`1faaf8d56d924e699bb72f0e897d4e3894faca02c45609871471cbbaf47b6793`，219文件完整[冻结](freeze.json)/[源码包](frozen-source.zip)，固定exe SHA`4857353b64e99a614f05de695d0b6a25e37f5b672b35cfb130c10d4e3fd30c46`，原始路径见[source](source.json)。原验收与满级输入checksum保持。唯一新增memmap2 0.9.9固定crate及checksum、既有依赖不变见[依赖核验](dependency-audit.json)；没有新增本地unsafe，不做UI/GPU/AgentKit。

[同源终态](regression-terminal.json)：全workspace release/locked/offline720.238秒exit0，strict all-workspace/all-target clippy4.606秒、fmt0.485秒、build0.128秒均exit0。61份成功harness摘要共455条passed含nested，非唯一测试数，0failed；[汇总](regression-summary.json)及stdout/stderr完整保留。固定exe、源码包与冻结来源前后不变。

## 实际功能与错误反例

- [错误CLI反例](error-probe.json)：显式诊断--report-fault在大模型分配前触发metrics_unknown_class，实际exit2，无成功report。正常与故障共用publisher，metrics.report()?成功后才创建报告，正常compute为直接对象。新增真实CLI测试在全仓通过。
- [原满级两代实验](exercise-result.json)实际30.328秒，21次361,272 native＋1,232,426 stub记录扫描，每次精确1MiB脏写；实际64MiB池终态全额平衡。事件驱动两代仅为诊断机制，正式300秒快照周期未改。
- 同owner实际释放4,521,984B纯R07同用途映射，第二代pin期间只停provider发布，TTL过期实际补回全额，再按新输入恢复置换；10帧持续采样，pressure/swap与owner保护未停。固定prefix、状态、长度/hash一致，独立新PID完整校验后重建全额且历史信用为0。
- [三份实际兼容冷开](compatibility-result.json)：v50旧格式、typed-v98及前轮v3原件均由最终exe完整读取、继续扫描、exit0；没有复制压力数据。
- [raw审计](raw-audit.json)：106帧78完整/28明确未知，所有置换ack均可按owner/epoch/reading sequence关联raw Reading；转换区间保留，不声称原子同时帧。R07控制容量203,328B包含在原4MiB metadata；最大请求残差3,033,260B，无分配失败。这是本次观察，不是任意异常路径静态证明。

完整机制见[实现契约](../../m0-r07-runtime-replacement.md)。64KiB段全额容量从原arena等额扣除，尾段仍在原arena；snapshot pin不领取虚假信用，过期实际初始化补页后才ack，冷开不继承旧信用。新v3明确改变R07generator及非R07 dirty选择，不声称旧pressure hash/轨迹不变；原数量、域配额、128MiB工作能力、64MiB池及共同8MiB不扩大。

## 原物理门仍失败

全raw峰 **3,337,309,712B**（第97帧）；完整帧峰 **3,337,276,944B**（第58帧），比原normal3,328,599,654B超 **8,677,290B**（约8.28MiB）。置换生效的稳定帧也仍超限，不能只展示信用或将旧峰减信用当新实测。专用诊断含故意撤销及冷恢复，不能签正式性能；L1扫描不是实际商业/空气算法吞吐。

旧5c机制探针、12e的8秒重复保存snapshot_busy、7fd错误修复前的实际功能与完整回归均保留独立来源：[初测](../m0-t13a-r07-mapping/STATUS.md)、[失败](../m0-t13a-r07-coupling/STATUS.md)、[前轮](../m0-t13a-r07-event-driven/STATUS.md)。原失败未被重写为通过，旧回归不替代最终来源。

## 阶段回收与可用性

全部验证终态后，按[条件许可](retirement/advisor-review.md)及[精确清单](retirement/deleted.json)，退役8份旧合成pressure/reserve：coupling首代、7fd两代及最终来源首代。新最终第二代完整保留作下一VM/恢复输入，原五份v1/v2完整/故障基线保留。190份保护原件全SHA前后核验，其他129,019项work/target的身份、长度、mtime和nlink不变，源码/WIP全SHA不变；没有新增压力归档或独立target。

|项目|删除前|删除后|
|---|---:|---:|
|work|39,708,468KiB|27,306,644KiB（26.042GiB）|
|共享target|2,180,708KiB|2,180,708KiB（2.080GiB）|
|文件系统可用空间|81,893,683,200B|94,593,994,752B|

按unlink前唯一inode的st_blocks移除分配空间 **12,699,467,776B（11.827GiB）**；df可用空间独立观察增加 **12,700,311,552B**，差额不归因本次删除，也不与历史v103回收重复计算。详见[result](retirement/result.json)。共享target保留后续所需build/test/clippy图。

首次预检因三份保留文件st_blocks变化在删除前停止，删除0；其inode/长度/mtime/nlink不变，190保护原件重新全SHA匹配。保留原[差异](retirement/preflight-inventory-difference.json)，重盘点时5,619,712B保留文件分配减少单独排除，不计成果。修正为身份/内容校验与分配块分别核验后执行；[后置分配变化](retirement/retained-block-changes.json)单列。观察到其他项目Cargo时按cwd和本项目lsof判断，不停止其他任务、不清理其target。

[最新恢复可用性](retirement/availability.json)覆盖本阶段旧原路径承诺：上述退役快照**不能直接完整恢复**；原index/组件/raw、必要报告/失败证据、固定源/lock/exe/依赖继续保留。旧prefix已于v103删除，不能直接复用。仍需恢复的原件没有用hash代替，pending仍非有效恢复点。

## 下一主线

[实际VM归因](VM-NEXT.md)：同birth/epoch/映射量夹取idle/inflight，定位真实地址及具名用途；现有wholefoot减arena/pool只是总量核对，不授权layout/pool残差信用。若没有可独立验证的OS分项，优先优化实际线程/映射/进程成本，不放宽原门或减少冻结负载。日志容量要按实际9B记录和完整L2/其他日志核对，当前L1投影不证明8h日志通过或超限。

原正式三轮/四速度/稀有层/28,800秒联合运行仍未完成，真实16GB保持not_run，full_M0 not_accepted。
