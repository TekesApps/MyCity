# 实际分配器边界与原生观测接线（v0.5.89）

只做Rust模型／CLI。已有Sim Status入口在报告分配前读取真实current／arena used/free及进程clock；L1每次原scan后读取全局分配器并标snapshot在途；日界大Status报告释放后追加小标记，至下一真实RPC形成空闲区间，没有增加等待或改业务释放时刻。固定64MiB与真实current、System模式和激活arena明确区分。

## 实测与限制

来源0418b0739385eaea81fe3ba299fab06085c754cdeee4630c82b447087a497314，198files；[freeze](freeze.json)记录原acceptance／opening／Cargo.lock未改。[同源全仓回归](regression-summary.json)718.6666秒退出0，60成功harness记录／427通过条目（含nested，不是唯一测试数），strict Clippy／fmt／build退出0。三规模实际Status断言验证真实59MiB grant、current与used/free及clock，业务只读不变；原完整L0／OOM／安全排空等回归通过。离线身份与空闲区间各6项反例通过。

公开本机SDK的proc_listpids／proc_pidinfo／host_statistics64替代短ps／vm_stat采样子进程。C与ctypes结构64／96／416及关键offsetof实际一致，宿主只返回248B VM数据，仅使用返回范围字段；自进程／三子进程／host实际探测成功。SDK导入纳入tracemalloc，另收费64KiB。读取父子关系前后绑定相同Mach出生身份；PID复用／关系变化／临时不可读均unknown。完整帧只表明本次枚举及采到实例完整，不证明采样间隙所有短命进程峰值。

新来源真实L1短诊断：先实际120.374s normal准入，之后observer20.541742s，原16次全规模扫描、脏写、完整快照及同源冷恢复（3.224846s）成功。使用exec替换预检进程，没有额外停驻的Python调度父进程。[实际覆盖](actual-l1-native-coverage.json)83帧中81完整；不完整帧66（16.509s）为布局退出／冷恢复交接时关系和资源缺测，帧80（20.007s）为监督退出时祖先未知，均在短测固定3～15秒窗外。RSS／footprint各1次缺测，不填0；这两帧不能描述成初始启动缺测。5个实际正常退出角色均有单一PID及单一观察Mach birth记录。没有sampler_ps成员。

[原始窗口审计](l1-smoke-window.stdout)与[资源窗口](l1-smoke-resource.stdout)各退出0，但脚本0不是全门通过。[同相位驻留](l1-smoke-residency.stdout)在无snapshot在途的扫描8～15后获得对应完整帧：L1存活Rust requested3,257,932,107B，RSS3,323,609,088B，footprint3,322,530,504B；同期组RSS3,416,621,056B、footprint3,406,139,328B。总组物理观察已高于3.10GiB候选预算。与requested的驻留差额不等于已证明System缓存量，不从pressure直接扣差额。

本轮仅签接线／有限短测：正式三场景各三轮、完整物理及common8闭合、同边界历史增长、稀有200补样、四速与实际28800秒仍未完成。新Sim日界计数和释放标记尚需实际正式L2匹配；旧e49读取30个Status但无释放标记，被离线审计正确判未测。无真实16GB设备，保持not_run，完整M0不签。

## 回收与下一步

[终态回收](retention/result.json)：真实模型及observer均退出、无work／target占用、保护原件及源码全核验后，精确删除短L1的pressure／reserve及25份与既有gzip逐字节一致的合成组件，共27文件，分配空间3,391,627,264B（约3.159GiB），无新work归档。df观察增加3,391,660,032B单列、不全部归因；work剩18,528,092KiB（17.670GiB），target/release约1.336GiB继续复用。118,720个其他work文件身份／长度／mtime／链接／分配块不变。

这个短测snapshot原路径已不完整，**不能直接完整恢复**；完整字节组件归档、原index、raw样本、业务／冷恢复报告、固定源码／依赖与exe保留。当前formal第7／8代、v50旧格式及smoke-L2失败原件完整保留，前后全SHA一致；没有为本轮永久多留一份压力副本。

[advisor复核](advisor-next-source-review.md)支持下一步先实验L1固定区域复用原128MiB reserve，覆盖快照解码及冷恢复，保持原记录、padding、scan／dirty、I/O与恢复步骤不变。不能额外留旧System背板；区域从原预算推导，计启动前缀／元数据／碎片，OOM失败。随后实测效果及native／栈／映射残差；同用途R07预算替换须有实际归属证据。本阶段没有做此优化或修改预算，不能预告它必然有效。
