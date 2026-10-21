# 固定L1工作区与因果时钟：v0.5.91

当前源码66e7e2aee0464d302738c4a1d36cfe2b2e7d17de7ac6f8d088330a2d85629138；本有限阶段同源全仓release、strict clippy、fmt和build均实际退出0；全仓702.046139542秒，60份harness摘要/429条通过记录（含嵌套输出，非429个唯一用例）。固定源码zip和验收exe前后不变。详见[最终回归](regression-terminal.json)及各frozen命令原日志/JSON。原负载/预算/频率/快照和冷恢复不变，没有GPU/UI/AgentKit。

a702首个固定区域实验真实1MiB分配失败，完整pending原件、固定源/exe/raw保留。8d66fe后续COW直接转移原reserve Vec，原16扫描/29文件写入读回和冷恢复成功、全仓release704.906s退出0，但strict clippy发现测试current初值被覆盖未读，退出101；其原证据在相邻m0-t13a-layout-cow-reuse，不伪装通过。最终源码只将测试copy_page输入改成旧current以读取真实被共享的值，生产COW逻辑不变；增加有界启动因果往返，代码与原始证据分源保留。

本源码已实际验证：3个CLI错PID/source/nonce请求均在业务与pool启动前失败；15项Python反例通过；启动往返静态独立审查无阻断；原L1实际20.53997975s、83帧、16扫描、完整3,378,784,402B写回/冷恢复及原v50完整冷开外层4.070572s退出0，内部恢复4.030479416s，两个端点分别报告。新旧原29文件索引与manifest（除source身份）一致。

监督映射不确定5,560,709ns，联合L1映射16,655,084ns；扫描9到15实际取得28帧无snapshot在途的完整同相位捕获，inside规则未改。数据见actual-l1-analysis.json、l1-residency.stdout及原work/causal-clock-l1-smoke-01/observed。当前allocator requested3,257,932,070B、grant3,261,490,790B、free3,558,368B、allocation_failures0。只能证明这一次短成功路径，不能证明长时无碎片。

同相位群组RSS3,358,654,464B、footprint3,348,074,360B；后者超normal3,328,599,654B共19,474,706B。分角色footprint：L1 3,264,186,960B，pool68,518,248B，observer12,943,696B，supervisor1,360,184B，driver1,065,272B。不能将差额称为已证明的System缓存或直接扣pressure。2个退出附近帧有RSS/FP缺测，另1个末尾ESRCH ancestry未知；原缺测保留，不补0，均未混进28帧同相位证据。

全仓终态、[独立依赖裁定](advisor-result-review.md)及真实进程/占用核对后，[精确退役27文件](retention/deleted.json)：pressure/reserve及25份与已有完整gzip逐字节一致的合成组件，分配空间3,378,831,360B。该短测快照现在不能直接完整恢复，hash/index不替代已删原件。原raw/事件/manifest/index/源/exe/拒错/冷开报告保留，旧formal7/8、v50、smoke-L2和a702失败pending原件前后全SHA一致。其余118,816个work文件身份/长度/mtime/链接数不变。[回收结果](retention/result.json)：work从23,424,972降到20,125,332KiB（19.193012GiB）；共享target 1,400,172KiB不删、继续复用。df观察增量3,379,585,024B单列，不全归因；无新压力归档，不重复计v90及更早回收。

正式三场景各三轮、同源稀有补样、四速、实际28800秒未完成；真实16GB not_run，完整M0不签收。下一步见advisor-runtime-accounting.md，先核实元数据/owner归属，不能用已知超标结果缩负载制造通过。

下一步[首轮原正式L1](NEXT.md)保持300+1800秒、原全负载与逐帧测量，补真正多轮快照/COW/冷恢复及长期变化证据；单轮与已知normal失败不会被写成正式三轮整体通过。
