# 集中回收与恢复基线交接（v0.5.87）

2026-10-22。本轮优先清理，未继续开发模型、构建或另开测量。开始时正式L2测量正在运行；等待observer PID46375及父runner PID44975真实退出、离线审核结束后，才执行du、完整哈希、lsof及删除。未停止其他任务。此前约74GB、v86的382.56MiB和先前debug缓存回收均不计本次成果。

## 当前实测及口径

|项目|本次结果|
|---|---:|
|首次只读盘点work|27,837,128 KiB，26.548 GiB|
|第一次删除紧邻前work|27,826,272 KiB，26.537 GiB|
|最终work|18,514,968 KiB，17.657 GiB|
|精确删除文件／独立单链接inode|6／6|
|删除文件分配空间（st_blocks×512）|9,534,775,296 B，8.879951 GiB|
|新建work归档|0 B|
|共享target前后|1,400,484 KiB，1.335606 GiB；删除0 B|
|整个删除阶段df可用空间|103,791,575,040 → 113,312,571,392 B|
|整个阶段df观察增量|9,520,996,352 B|
|两次紧邻删除区间df增量之和|9,534,550,016 B|

汇总见[combined-result.json](combined-result.json)，逐次原始读数见[result.json](result.json)及[v85-retirement/result.json](v85-retirement/result.json)。首次盘点至删除前已经发生的分配变化不算回收。第一段保留文件有21项仅st_blocks变化，合计−12,267,520B，身份／长度／mtime／链接数未变；第二段无此变化。它们不计入删除量。文件分配块不代表APFS独占物理块；df受共享块、快照及其他任务影响，两种df增量均只观察、不全量归因。清理凭据在docs中新增约4.70MiB证据文件（校验时的分配空间读数），未伪装成work净回收或新增压力归档。

## 逐轮清单

|所属轮次|删除范围|分配空间|理由与保留证据|
|---|---|---:|---|
|steady-controller-smoke-l1-01|snapshot-000001内pressure.bin、work-reserve.bin，共2文件|3,178,258,432 B|短诊断已终态，实际全载荷冷恢复及扫描passed，无后续完整恢复依赖；[计划](plan.json)、[删除凭据](deleted.json)、[冷恢复报告](retained-cold-restore-report.json)、原index、原日志／业务／组件保留|
|t13a85-joint-01|snapshot-000001、000002内各pressure.bin与work-reserve.bin，共4文件|6,356,516,864 B|由新formal第7／8代接替完整恢复基线；[独立交接审查](v85-replacement-review.md)、[计划](v85-retirement/plan.json)、[删除凭据](v85-retirement/deleted.json)、原index／业务／组件／源码／exe／日志保留|

所有删除文件均按原index逐字节SHA及长度核对，单链接、无占用；不是按t13或目录前缀批删。实际保存计划及删除凭据并fsync文件和目录。第二段准备曾因把“8条历史摘要”误认作“当前2代”而在删除前断言失败，修正为精确选取第7／8代；原脚本和失败说明保留，未修改原报告，也未因此重跑模型。

**上述三个已退役快照现在均不能直接完整恢复。** hash、index和日志只保留历史证据，不替代已删除数据；没有声称仍能直接恢复。历史v85的retire_stage.py／旧清理保护脚本也不能再按旧pressure原路径完整复跑；其历史结果不改写。

## 为什么保留剩余内容

- 当前完整恢复基线：`work/steady-controller-formal-l2-01/observed/model/layout/snapshots/snapshot-000007`和`000008`。最新第8代已实际冷恢复完整3,378,784,402B；第7／8代各有完整写入、读回和记录恢复证据。新原件全部完整保留，交接仅裁定恢复依赖，不证明性能门通过。
- 旧格式兼容输入：`work/t13a50-joint-01/model/layout/snapshots/snapshot-000001`，下次兼容／恢复仍需原件，完整保留。
- 故障证据：`work/steady-controller-smoke-l2-01/observed/model/layout/snapshots/snapshot-000001`及整个失败日志／业务现场。暖场超时尚需后续对照，保留完整原件，未将它视作正常可替代基线。
- 真实业务world、WAL、财务权益收据、独立CPU目标流／业务参考、故障pending、negative、旧兼容world及用途不明产物保留。它们不是纯构建缓存。其他t13目录目前主要由这些记录、保留组件及证据构成，不能按目录名删。
- 固定源码／依赖快照、Cargo.lock、验收exe和当前未提交Rust工作均保留。验收固定程序和源快照身份见[replacement-provenance.json](replacement-provenance.json)。唯一target/release仍供后续验证复用，无确认可退役的独立target或增量缓存；不删后马上重建，不动其他项目或全局Cargo。

保护原件在删除前后按全部原index逐文件校验SHA与长度，见[protected-before.json](protected-before.json)、[protected-after.json](protected-after.json)及第二段对应文件。第一段其余118,672个、第二段其余118,668个work文件身份／长度／mtime／链接数保持；源码和未提交工作哈希一致。完整目录分布见[盘点前](work-overview-before.json)及[盘点后](work-overview-after.json)。

## 后续每阶段回收

创建新验证目录前写明用途、精确下游输入、需要原件的恢复门及终态解除条件；优先复用输入和唯一target。终态后先保存固定源／依赖、exe、原始日志／收据／故障，再检查真实进程及父runner、cwd、打开文件和硬链接。测量期间不清理或做大文件盘点。正常合成副本无后续依赖即按精确文件清单退役；需替换完整基线时，须新原件实际写入／读回／冷恢复及独立依赖审查成立。随后核验保护文件、更新可恢复性、分别记录st_blocks、du与df，再开下一阶段。

新formal两代下次只在后继完整恢复交接成立后释放；v50须旧格式兼容依赖解除；smoke-L2现场须故障对照结束且必要原件明确归档，不能自动过期。用途不明先保留并登记解除条件。没有可安全回收项就报告0B，不默认每轮永久多留一份压力快照。

本次仅发布清理证据与回收规则，模型WIP不混入提交；经济、性能、三轮稳态、8小时和真实16GB的验收结论不因磁盘回收改变。
