# 常驻typed解码基础（WIP）

报告延后82a80787原参数实验仍在第二代repay63失败；不是充分修复。原native/stub/COW负载及128MiB工作预留不缩；尚未整合常驻工作区，更未开始下一轮压力测量。

已实现ExactSlice：逐条覆写已初始化固定T:Copy切片，长度固定，无集合reserve/替换；多/少项、EOF错误可在同一背板重试。相关5项初步测试通过。

新增诊断Arena/LayoutInto和NativePressure::restore_checkpoint_into，不改变一般World/Arena路径。Native完整29,625,968B目标图在字段反序、缺/重/未知字段、错误后重试及完整规范序列化检查下10份背板身份不变，实际全量测试3.41秒通过。尚非最终冻结来源全仓/性能通过。

下一步四类最大typed表背板、64MiB解码区+64MiB并发COW、真实控制/词法scratch/2MiB已收费IO/原生2MiB线程栈分别核账；剩余真实初始化/触页，不用历史峰值倒算。扫描时decode占用区的借用语义必须显式，不能让全量扫描被整段mutex阻塞。失败不发布目标，支持同背板重试。禁止新增unsafe。
