# 原满级负载：合并pool进程的有限诊断（v0.5.125）

## 结论及范围

本阶段发布实际实现和诊断证据，不是formal性能通过。新增 `r07-layout-driver NEW_DIR --combined-pool [--vm-trace]`，将原64MiB pool服务并入现有256KiB supervisor，少一个实际pool-host进程。旧 `r07-layout-driver NEW_DIR [--vm-trace]` 保留作同源对照；未修改正式joint-driver的生产默认。全城业务并未由这个布局诊断代替。

固定最终源码 `ac02e7ccb7e2b9e842e5f8dd15903b14f7fff9b9426909449b36b33bbd08db6c`、exe `557b2e4d0bc89f429a07a270c9f388b508d595c8b4a1f233b795becdf75797e7`，见[binary-final.json](binary-final.json)。[源码/ZIP/exe独立完整hash](fixed-source-integrity.json)复刻build.rs长度前缀算法，不能把当前共享target中的可变exe当冻结exe。完整源/lock ZIP见[最终09](09-fixed-cli-build/source.zip)，初始04也保留。实机Apple M4、32GiB，work与唯一共享target在外置HP P900 USB/APFS SSD；该存储环境下的耗时不能替代旧内置盘结果。真实16GB仍not_run。

## 实现与预算

原pool的64MiB实际初始化映射、共同8MiB、256KiB supervisor和原CLI参数保持。`serve_controlled`在服务请求和扫描之间调用有界owner回调，轮询Job完成，不能在仍需要pool RPC的子进程上同步阻塞。旧serve保留stdin-watch；新入口复用现有root owner-pipe与私有进程组退出保护，不另开parent-watch线程。Job唯一所有权在错误路径kill/wait，成功路径等待实际退出后才继续冷读及最终pool balance。

实际combined-control root Rust请求峰37,231B，加原64KiB退出尾102,767B，低于原256KiB；这是请求峰范围，不是物理内存上限。L1/layout-load仍各使用原2,983,880,294B授权，原normal pool64MiB不扩大。单次服务调度/owner状态为固定可选Job，不新增独立模拟、账本、日历或持久化系统。正常pool报告完整64MiB可用、零未返授权；本布局诊断没有实际业务worker借还，不能称全源业务联合通过。

混合root同时持有正常pool，不再是可获R07替换信用的纯诊断进程。observer spawn后立即冻结实际root PID并在其整个生命周期排除所有R07组件选择，包括初始direct-root标签。记录birth与source，raw成员和footprint求和仍包含root；估算扣减为0。separate旧角色规则不改。见[排除记录](combined-control/r07-excluded-root.json)。不能估算扣掉64MiB，不能用RSS/地址或VM分类抵预算。

## 实际执行与独立核对

- [03 pool测试](03-controlled-pool-tests/exit.json)6/6、[07角色测试](07-r07-role-tests/exit.json)3/3、[08 headless lib](08-headless-lib-tests/exit.json)89/89、[18全workspace/all-target严格Clippy](18-strict/exit.json)及[20fmt](20-fmt/exit.json)退出0。首轮02因真实socket路径过长失败，原source/log/exit保留；修正只缩短测试socket夹具，不扩大OS限制。
- 同一最终exe的[separate-control](separate-control/receipt.json)与[combined-control](combined-control/receipt.json)均实际两代完整快照及独立cold子进程读取成功，分别26.441927750与26.121286959秒，仅操作观察。完整payload的逐文件长度/hash读取发生在模型报告中，本阶段独立verify脚本只核对报告/时轴/身份，不声称再次独立读payload。
- [完整冻结负载对照](original-load-comparison.json)：两个control、单次half-wire及cold-error的layout manifest与原full-r07-vm-diagnostic-03全部字段除source相同，原acceptance SHA `876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508`。真实native361,272条、压力stub1,232,426条分别报告，压力替身不是城市业务人口。原容量、COW、R07映射、人口及normal阈值不缩。
- [单次half-wire03](half-wire03/receipt.json)：只注入一次不完整JSON，实际timeout拒绝后pool继续工作，两代完整快照和cold成功。half-wire02曾47次EOF+1次timeout，保留为多请求诊断，不能冒称单次。10控制脚本clock身份比较错误及原收据保留；补录实际终态，不把原失败改成通过。
- [kill-layout](kill-layout/receipt.json)实际身份绑定SIGTERM；[kill-root](kill-root/receipt.json)实际身份绑定SIGKILL，由既有observer清理后代。全部记录身份终态确认消失。故障controller退出0表示预期故障验证成功，模型正常执行不成功；kill-root model_exit_code=null是信号终止，不是0。
- [cold-error](cold-error/receipt.json)在确认活owner后制造已存在的cold目标，实际cold子进程注册后报AlreadyExists；model/observer退出2，无exercise成功报告，全部4身份终态消失。短cold子有1项unpolled，保留缺测，不能据此验物理峰。
- [21独立核对](21-independent-verifier/stdout.log)实际退出0、无blocker。separate-control历史收据顶层controlled=true且10项检查全true，但nested outcome.passed=false：旧helper复用half-wire结果后未更新汇总。原件保持；独立脚本从各项记录核对，不能默默修写原收据。combined-control修正后汇总一致。控制专用字段和故障专用字段分开，不能统一把passed_fault_exercise当正常控制成功。

初始04源码4a818…的combined归因把整个混合root计为诊断，产生最高71,958,528B的错误R07 replaced值。05/06原证据保留，但两者不参与最终物理改善比较。最终ac02已修正、重新构建及真实执行，不能跨源码拼接旧峰值。

## 同条件物理结果：仍然超限

两个最终control均在provider pause后，连续3帧同时满足：full mapped277,610,496B、replaced=0、permitted=0，group/coverage完整、注册root/layout/owner PID+birth一致，所有成员footprint之和等于group。窗口完整原帧和时点见[separate](separate-control/full-restored-window.json)、[combined](combined-control/full-restored-window.json)。

| 同条件三帧窗口 | 原始footprint峰B | 比原阈值3,328,599,654B超额 |
| --- | ---: | ---: |
| separate | 3,337,555,520 | 8,955,866 |
| combined | 3,336,228,080 | 7,628,426 |

本轮观测差1,327,440B，只能归属这次同源受控诊断。两次全局序列各有2项process采样缺测，所以没有拿全局峰作严格峰比较；不将缺测填零。控制器自身约17MiB单列，observer已含raw不重复加。采集窗口非同时原子测量，原始窗口宽和采样gap保留。尚未直接实测“pool内部scan/event写失败且子进程活着”的错误路径；受控owner失败单测、实际子进程故障及root失效不能替代该特定故障。

下一步需要继续减少真实开销并将验证过的机制接入正式联合驱动，完成原分级/四速/轮次/恢复故障和真实8小时。原formal01仍失败；本阶段不是吞吐门、不是normal预算通过、不是16GB门，完整M0保持not_accepted。advisor已只读复核源码及新增三项control/fault收据，无功能诊断发布新阻断，未替本阶段签formal验收。

## 留存和回收

完整最终separate/combined两代、half-wire03、cold-error故障输入及已有190保护原件均留在 `/Volumes/HP P900/dev/MyCity/work/pool-control-current-01` 与原保护路径。公开目录保留必要source ZIP、日志、收据、时轴和独立核对脚本；完整恢复输入及固定exe在SSD，hash清单不替代它们。其他项目的raw进程参数不发布。

初始05combined/06separate已由最终同源control替代。确认所有进程终态、两侧lsof无句柄、后续恢复无旧路径依赖，最终control实际cold通过且两代全部文件逐SHA重新核验后，仅精确删除旧两组各4份pressure.bin/work-reserve.bin。旧两组当前**不可直接完整恢复**，原source/exe、语义组件、index、日志及R07归因失败证据保留；没有删除真实业务存档或故障pending。见两份[combined退役收据](retirement-initial-combined/receipt.json)、[separate退役收据](retirement-initial-separate/receipt.json)。

本轮删除8个纯合成文件，按st_blocks统计分配空间合计12559417344B；两段SSD可用空间变化合计12559376384B另列，不宣称APFS独占释放。本次未新建target，也未清理仍需用的构建缓存。当前完整control保留到后续正式驱动与恢复验收完成交接；half-wire02多请求诊断载荷暂保留，尚未解除故障输入依赖。建设候选仍在独立工作目录：最新126/129通过，剩余Exchange材料sink缺口未修，不混成本阶段完成。

阶段末du分配空间：work94,860,623,872B（约88.35GiB），共享target3,550,670,848B；本阶段目录34,760,708,096B。此为当前总占用，不是本轮新增。保留最新两种正常对照、单次半帧恢复、多请求诊断和cold-error故障输入等完整数据；用途未解除者不删。git diff --check对源码/版本文档通过；五份Cargo原始stdout末尾空行产生EOF提示，原日志逐字节保留，不以格式化改写证据。
