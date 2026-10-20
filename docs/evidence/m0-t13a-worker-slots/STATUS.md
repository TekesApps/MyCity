# T13A-3d：worker固定槽实现与失败实测

发布v0.5.43。实现及全仓回归完成；**完整30日／724窗口为not_completed，本次实测门failed**。本阶段没有通过全局64MiB、正式联合压力、四档、三轮、8h或16GB；失败工件保留，不从382个局部owner外推724。

## 实现及原门

实际worker bounded路径已接两个固定2MiB槽，共4MiB。初始化真实填充，借用只变标签，整个作用域不重新分配；读取／编码只写有效前缀，尾部填充不进入hash或文件。归还后全槽重新填充，报告前读取全部4MiB核验。切片／Vec／Arc不能逃逸限定闭包，票含process／owner instance、run epoch、job／generation／attempt及序号。错误／旧票与重复退款拒绝，正常／错误及unwind归还；panic不完整trace拒绝报告。

**available_touched_bytes是owner内仍持有的余额，不是全局可再次发出的额度。** 本地两槽闭合不代表跨进程池；进程终止后须由上层确认退出才可归还未来全局grant。constructor分配失败会使任务失败，本轮没有注入真实OOM，也未实现全局票回滚。

core新增checkpoint_to：先流式hash规范payload，再借用State编码原字段顺序，不再克隆整份State／保留完整payload副本；原checkpoint委托同codec，restore的hash也改为流式。8MiB检查点上限不变，bounded worker仍在2MiB槽内拒超限。既有候选校验用64KiB栈缓冲流式读取，不分配第二份候选Vec；原6MiB磁盘缓存、不可变发布和fsync顺序不变。State／serde／IPC／栈及其余验证工作区均不包含在4MiB数据槽内。

Owner固定metadata 1152B计入worker原1MiB诊断门，不把4MiB数据槽计为诊断，也不把元数据免单。原full期初、acceptance、8192页、空气60秒步长与全部目标不改，未缩负载。

## 验证与来源

- [全仓release回归](tests-workspace.txt)取得exit0，包含新旧codec逐字节／互相恢复、部分writer及错误前缀、容量边界／超一字节、旧票／错槽、unwind、真实重复任务复用既有候选、损坏文件拒覆盖，以及切片不能逃逸的compile_fail；最大20000空气源也覆盖。all-targets release [clippy](clippy.txt)与fmt通过。
- 早期[tests-scoped.txt](tests-scoped.txt)因不能混用--doc和--test被Cargo拒绝，没有执行测试；修正后的日志及最终workspace日志分开保留，不把拒绝算通过。
- Mac16,1／Apple M4／10核／32GiB、macOS26.6.2 arm64；不是16GB。source `53068b4764589bfdbe659c9f7a80523e70730da24dcd215249d7dd1ab5974b05`，binary `209c28f5b1a2053c286e4d9499f91ac15c0edde4faa55b96d26578d379371bc0`，见[source](source.json)。source.zip／source-support.zip及1998项完整失败工件均归档，无大文件省略。
- 实际原full单独30日命令使用[同一observer](observe_v3.py)及原scale-sim-probe full measure，未启动L1，不冒充联合压力。216.88965秒时系统非正常pressure连续60.197秒，按冻结保护终止自身树；未终止其他应用，原输入／源码／二进制未变。

## 实际失败及能证明的部分

[失败核验](failed-verification.json)／[脚本](failed_check.py)确认382条正常worker退出，分别对应完整4MiB余额、四转换、每槽一次借还及具名1152B metadata；这些382个worker的输入／检查点／候选hash与v0.5.42对应任务相同。383份封存报告中有1份缺正常退出记录，不补算完成；父进程／活动进程记录缺失也不记零。没有完整30日结束、期末冷开或完整Engine证据，本次均未通过。

868帧RSS成员求和通过，进程组RSS峰119,554,048B。footprint缺934个进程样本，不能称完整峰；系统swap净增0，磁盘逻辑峰71,588,723B。不能把系统压力失败写成模型自身内存超限，也不能凭本组RSS排除其对压力的影响。同期背景仅记录basename与RSS，未认定单一原因，见[pressure](pressure-after-failure.json)。

补充[副本冷读](partial-recovery.json)仅验证此次中断后的功能恢复：复制权威文件到独立目录，省略四个运行时锁／owner／缓存名，新Rust Sim进程冷开至sequence1648，查询最早原票hash与原transcript相同，前后Progress完全一致；仅执行progress／receipt_proof／progress／stop，未推进日历。68个原失败世界文件hash保持不变，原失败工件没有被打开恢复或改写。该补充不是性能重试，也不是完整30日期末恢复，事件与[脚本](recover_partial.py)单列，未重新请求全域审查。

停止后5分钟31次观察均为warning=2，未满足连续120秒normal=1，见[重试前观察](pre-retry-pressure.json)。本轮没有重试，未放宽保护、杀其他应用或改小负载。所有本轮模型进程已确认退出。

[静态核验](static-check.json)检查133份源码及全部1998项归档字节、原acceptance及源码支持包；[advisor独立审查](advisor-review.md)准许按“实现／回归完成，实测未完成”提交，不签满级运行。目标继续有效。

## 下一依赖

见[NEXT](NEXT.md)。压力恢复后再以相应冻结来源进行完整full验收，保留本次失败；同时推进驱动诊断账与其他真实所有者，不让系统压力等待阻止全部代码工作。局部4MiB不能抵销其余未核清的64MiB；四档、正式稳态、8h和真实16GB仍未通过，M0 not_accepted。
