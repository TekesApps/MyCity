# T13A-1a：真实满级输入与native布局子阶段

本子阶段为T13A-1的前半部分，不能签收整个T13A-1或T13-A。v0.5.30的冻结标准与数量未削减；当前保留32GiB设备、真实16GB未测。

满级实际共享Engine装配：64账户／128库存／8城／8 metric frame／16周期义务／64工资／20000 Claims，8份1000mQ货运，4份在day0第60秒发运、4份在day15发运。泵在0秒、机组在60秒启动，第一批到180秒结算；此顺序使4槽Inbox背压与新承诺互锁合法，不绕背压。完整具名输入、政策和独立算术在fixtures/m0-t13a-*-candidate.json；candidate文件名保留，实际hash随本子阶段冻结，性能结论仍未签。

[实际CLI结果](reference-result.json)和[压缩原始检查点](reference.zip)记录30日／720小时目标、724空气窗口。完整现金3000000分；工资118784、Claims20000、周期税48000、启动费用500、货价16000、运费160、收货费40。源库存12000mQ／4800分；四个目的池各2000mQ／4040分；114额外库存完整不变。所有资源及租约释放，8单交付／原始源碎片、Inbox8次归档及16到期义务逐项可审计。约32.76秒只是这台32GiB机器直接CPU参考的单次耗时，**不含Sim IPC／Store持久化，不算P-E0、四档速度、分位或长跑性能通过**。

[独立审计器](verify.py)从原始检查点重算全部64账户、工资、2万债项、目的池与源批次区间，不只相信报告中的passed字符串。构建源码hash包括crates、fixtures、工具链及锁文件，oracle也在fixtures中参与指纹；压缩工件每文件hash见[manifest](reference-manifest.json)。后续版本应在本tag checkout核验本冻结证据，不能拿变更后源码冒用旧结果。

原v0.5.29完整输入首次实际拒绝见[baseline-preflight](baseline-preflight.json)与[当时四份原始输入](baseline-inputs.zip)；开发时的其他阻断及处理见[development-failures](development-failures.json)。修复包含有界8图scale profile、保留旧profile限制、成本池合并与实际排放源覆盖检查。仓库不因此获得8城完整道路网络：仍是6条交易边、4交易方和共享平地模板；其他frame没有完整空气模拟。

真实native-layout诊断容器使用实际Account／Stock Arena、Handle索引、Claim／Payroll／Resource／Lease；40k／110k／180k／31200／8／64条。owned29625968B，原生JSON快照65299383B，实际文件写入／fsync／恢复及完整重新编码hash一致。它没有绕过World上限执行4万账户业务，也不是完整L1。完整回归243个测试通过，fmt／clippy及静态文档／预算检查通过。全部JPROD缺失域typed stubs、七域padding、COW仲裁及长时观测器仍待后续子阶段实施。

额外库存实际WAL／冷重开／检查点回执及7个真实进程退出点已测试；合池测试保留不同采购价、容量拒绝及第二次入库提交失败完整回滚。第一次全量测试跨源码重建造成worker指纹不一致，失败原日志保留为tests-source-race-failed.txt；固定构建后全量回归另存tests-release.txt，不覆盖失败。独立定向意见见[advisor-review](advisor-review.md)。

未完成：small／medium诊断，完整L1及有界组观测，10类计算分位／四档实际速度／P-E0实际经营30日，磁盘96保存增长预检，三正式场景各三轮稳态，真实8小时及最终advisor审查。所有这些门仍为not_run；完整M0为not_accepted。

重跑：`cargo test --workspace --release --locked --offline`；`cargo run -p mycity-headless --release --locked --offline -- scale-reference NEW_DIR`；`python3 docs/evidence/m0-t13a-assembly/verify.py`核验本发布的冻结工件。避免验证期间重建不同来源CLI。
