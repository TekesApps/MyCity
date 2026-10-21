# v0.5.93：真实页回收、持久I/O与失败定位

本阶段修复了join前回收竞争及每文件I/O背板重建；**没有解决重复快照的全部分配失败**。原负载正式L1实际约310.293秒后第二次快照失败，不计1800秒窗口通过，不签正式三轮或M0。

冻结来源c0e4ef46b0c3c8c3d2908d8b7d79f4adf3a32967f92fda959e29094f27331bc5，200文件；固定exe及SHA见[source](source.json)。全仓release 698.440秒exit0，60份harness摘要共434条通过记录（含嵌套，不是434个唯一测试）；严格Clippy/fmt/构建均exit0。原fixture/lock未变，只增加reusable_io.rs并修改layout_load.rs，见[回归分析](regression-analysis.json)。实际formal前120.275秒资源准入通过；source/固定exe/observer原件未变。

## 实际结果

- 首次完整快照29索引文件共3,378,784,402B，完整typed解码、压力/预留读回通过；join后实际归还9份旧COW背板。未以新Vec补假额度。
- 第二次实际失败已由[固定阶段记录](failure-trace.json)定位到repay索引63，即第64页1MiB申请；申请前requested3,259,063,297B、总free2,424,536B、reserve119页、total loans9/decode debt1/COW8。该记录不是失败瞬间，也不证明连续空闲块。fallible申请返回错误，未再直接SIGABRT；失败的工作区没有被声明已归还。
- [实际工件](actual-failure.json)：observer310.292509秒、1242帧、exit2，RSS峰3,358,867,456B、footprint峰3,348,090,720B；本轮swap净增及非normal pressure持续均0。峰值不是同边界泄漏或normal通过证据。
- 1800秒测量未完成，第二份pending不是恢复点。初版13项单测/静态审查或首次快照成功均不能覆盖此失败。

新完整原件独立冷恢复3.528秒exit0，v50旧格式完整原件独立冷恢复3.867秒exit0；均实际载入并核验全部原文件、继续扫描，不创建压力副本。见cold-new/old.json及完整报告；冷恢复成功不覆盖第二次快照失败。

## 下一项具体修复

[advisor实际结果审查](advisor-result-review.md)确认解码闭包末尾构造的JSON报告在repay64期间仍存活；优先延后这个报告分配，返回栈上的RestoreBound，原工作页全部归还后再构造报告。保留错误String可能逃逸的限制；常驻typed工作区作为该具体修正实测不足时的后备，不先扩大重构。

## 留存与未完成

本轮所有模型/观察进程实际退出后才分析；未清理故障。work/snapshot-backing-formal-l1-01的首次完整快照、第二份完整pending及raw/阶段记录保留，用于下一步针对性复现与恢复；没有解除旧formal7/8、v50及其他故障原件依赖。唯一target/release复用；新增固定exe约13MB和本轮两份压力快照，无新独立target。终态work实测33,359,988KiB（约31.81GiB），共享target1,400,552KiB；占用检查为空，本阶段删除0B，文件系统可用读数不归因回收，详见retention-result.json。下一次故障诊断交接后优先退役已无依赖旧合成载荷，不因目录名或相同hash删除完整恢复输入。

真实业务不变；本轮主要是L1 typed布局/合成压力，不冒充完整城市业务。三场景各三轮、四档、稀有类、实际28800秒仍未签；真实16GB not_run，完整M0不通过。

原始frozen-workspace.stdout与unit.stdout末尾各保留空行，git diff --check有两项原始日志EOF空白提示；不修改原始字节。Rust/Markdown/Python/JSON变更的diff检查通过，不能把原始日志提示描述成全部diff零提示。
