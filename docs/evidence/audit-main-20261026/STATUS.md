# v0.5.200 审计候选与证据边界

基线main v199 / 5fe412b531af4abcd60675cc03e25ec5cfcf3ebf。此次仅固定最新审计状态，不运行编译、游戏测试、压力或GPU，不关闭Issue，不创建PR。

## #149 当前事实

原v198/v199游戏程序在HP P900原work实际复现：已完成3600边界、phase=None、target=7200，used=10507、normal_limit=5255、normal_blocked=true；Exit、CancelAdvance、继续Advance分别返回Capacity拒绝，状态未变。baseline01预期取消成功的断言以101失败，未改写为通过。完整请求／状态、固定程序SHA和实际日志在本目录，完整world/WAL/checkpoint／源和程序在work/timeline-target-emergency-20261026。

当前transaction.rs候选沿原CancelAdvance，仅在完成边界清除未执行目标，核原义务、资金、占用等不变后给应急资格；原Busy门、driver、Store、预算与存档格式未改。新增Store测试源码覆盖热冷、错误请求、原票、写中断及与设施供给场所共存的预期，**全部尚未编译与运行**。两份共享opening fixture被上一轮rustfmt连带格式化，本次按实际现状入库；不据此宣称语义／兼容性已验证。原版本可从v199精确恢复。

原advisor曾就取消目标的具体方案给出只读建议；没有复核当前候选的实际运行结果，不是本候选验收。后续先编译与专属检查，再按影响做共享、默认栈、恢复和必要资源门，失败保存原件并修复。

## 已验与未验分界

#136有限时窗合同v196/197、#137有限typed开局及#150栈修复v198/199已有对应验收与Issue关闭。其原固定程序结果不延伸到本次候选。累计窗口2026-10-25T02:39:00Z至2026-10-26T03:11:16Z：新验收业务5、历史补关0、缺陷2；待集成18／待验证0（历史交付口径，149缺陷在制另计）。本检查点新增验收／关闭均0。

24份保护WIP逐文件核对原保留清单后完整ZIP存储，仅供审计，未装配到原路径。主目录仍保留未提交原件；不能将此补充包视为M0通过。旧v196快照与失败证据保持不变。verify_snapshot.py只验证字节身份，不执行游戏或宣称正确性。

所有进程／原型CPU与真实业务资源，仍按原World128KiB、Engine4MiB、normal64MiB、共同诊断8MiB等预算；Mac16,1/32GiB不能替代真实16GB、native、持续推进速度、长时M0、GPU或UI验收。既有HP P900唯一target/work不变。
