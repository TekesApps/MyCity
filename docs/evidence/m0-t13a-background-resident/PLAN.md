# Background常驻背板：测前范围

原冻结fixture不改（SHA876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508），保持全部CPU数值步、容量和规范字节。仅Rust模型／CLI，不做UI/GPU/AgentKit。v0.5.69核心表继续向完整模型迁移，本阶段不把局部实现当作normal64签收。

采纳advisor的显式Work方案：权威Background只存field/pending常驻背板及存在标志；Prepared从独立BackgroundLease复制活动数据，成功持久化后交换，失败／展开复位归还。单独计算Work由Engine工作区借出，capture、pending摘要验证、Accept独立CPU reference及Store staged验证串行复用同一槽，不递归取得锁。后台source仅按原SOURCE_LIMIT预留，128²场／scratch／积分完整初始化；通用20k源没有被删除或缩小。

原Background权威上限320KiB、Engine4MiB及整图64MiB保持。当前每Engine独立常驻准备槽／计算Work，容量CLI额外按Engine份数列账，并保守保留原活动Prepared和五个通用Air槽；控制块给具名预留。算术fit仍非实际broker授权／RSS／16GB通过。逻辑null的背板仍计费。

验证固定背板地址集合（数值核本身允许field/scratch交换）、失败／展开／重入、前后窗口完整checkpoint字节、旧存档、全仓release／clippy／fmt。旧完整world保留原rules identity、用当前库实际Store::open重导出并与独立fault-reference完整文件比，禁止靠预存hash或改变规则蒙混。

同源冻结后再运行原full L1+L2 smoke600秒，观测器与完整723目标／3013票／724候选／725借还／600扫描／两代快照及故障流程沿用，不缩负载。开始前正常压力120秒，实测不并行本任务编译／测试或清理。终态后逐项独立审计；短诊断不是正式性能／四档／3x3稳态／真正8h。故障日志和完整新恢复基线归档后，旧合成压力仅在明确替代、复查依赖后退役。

公共frozen_input()/validate()兼容入口及一般Raw恢复仍会分配；input.identity()仍有短字符串。不得宣称零分配或全角色固定恢复。一般World/Arena/Opening seed、Store WAL／receipt／DOM及真实池图仍是后续完整64MiB依赖。真实16GB设备缺席保持not_run，M0不签。
