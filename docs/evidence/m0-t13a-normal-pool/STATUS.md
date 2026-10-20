# T13A-3h：实体余额、真实借还与传输修复

版本v0.5.47。只签本阶段实现／回归及**无L1的原full30日功能装配**；不签联合性能、共同8MiB、精确64MiB常规工作区、正式三轮稳态、四档、8h或真实16GB。原负载和保护阈值未改。

## 改动与真实边界

独立Rust池实际持有64份已填值、核对capacity的1MiB页；创建worker前释放4MiB，worker真实持有并复用两个2MiB槽，父端实际Child wait后才归还。源／boot／单调grant／父实例／子PID与两种独立capability绑定，日志不含capability。内部变更／日志／补页错误终止服务，不伪报信用；旧目录／旧ready不得接管。Prepare丢回复仍fail-closed；Sim死亡后旧grant接管尚未实现。

普通Engine／serde分配尚未转换，不扣减池填充，属于额外压力。4MiB转换间隙有记录，不宣称持续64MiB实占。正常槽借还不是完整常规模型内存预算闭合。池每秒全量扫描／事件的当前8MiB日志上限尚不能支撑8h，需后续有界处理。

## 同一阶段保留的失败

| 实验 | 来源 | 实际结果 |
| --- | --- | --- |
| 原L1联合 [归档](t13a3h-joint-01/manifest.json) | dc7115，修复前 | 81.061s因非normal持续60.009s保护失败；325RSS帧、swap净增0；业务尚未启动，池0grant；没有正常退出报告，不能视为0内存或功能通过 |
| 首次无L1 full [归档](t13a3h-functional-01/manifest.json) | 同上 | 96.129s模型失败，generic request_identity；125grant/125return/124Filled/2拒绝；末次扫描64MiB，但无池正常退出报告，不能签完整生命周期 |
| 修复前分片 [探针](prefix-before-fix/result.json)／[2k碎片](codec-before-fix/artifacts/result.json) | 同上 | 17字节前缀发出7.64ms即拒绝，后缀尚未发送；2,000次碎片请求误拒3次。证明提前拒绝，不足以给旧grant125分配具体错误类别 |

accepted UnixStream显式清除继承的非阻塞模式，保留原2s读取／写入超时；固定错误类别区分timeout、EOF、IO、decode、bound、trailing、source及boot。事件只记类别，不保存请求或私钥。新[探针](prefix-after-fix/result.json)等待58ms无提前回复、补齐后返回预期未授权拒绝；[2k碎片](codec-after-fix/result.json)误拒0。真实进程回归另验证超时、截断后恢复、错误source／boot与无额外信用；这些是传输探针，不是业务替身。

初次全仓在storage测试的time-only临时目录遇AlreadyExists，旧日志和源码保留；改用原子create_dir及PID／serial／时间防冲突，不删现有目录，不改原业务或7故障点。初始编译／协议u128／clippy错误也保留，不混入最终来源。

## 最终来源及实测

[源码／binary绑定](source.json)：source `5e6c4c2fa7f4edf34ea52d4ddc616676451d40d130bcaa1d9fb45dc8cf9b7d7b`，binary `5e1fa4984e32244ffdf7568ccfaaa3f858875fccdbe6c265183e8eba3c05b928`；139文件及支持文件随zip保存。Rust1.85.1、release／locked／offline，全仓[303项通过](tests-verification.json)，[clippy](clippy-transport.txt)／[fmt](fmt-transport.txt)／[build](build-transport.txt)终态0。

连续[最近120秒normal](functional-readiness-after-fix.json)准入后，以同一来源执行原full30日＋池、无L1：[完整工件](t13a3i-functional-01/manifest.json)、[归档](t13a3i-functional-01/artifacts.zip)。Python只是保持池stdin及拥有的子进程等待；解释器／startup／native开销未闭合，不能拿该分支签共同8MiB。observer入口hash为launcher脚本，真实Rust binary单独绑定。

- [完整业务核验](full-verification.json)：720小时／723目标／724worker窗口／3011原票；731正常Rust实例，726生产者实例目录；完整Engine含metadata与CPU一致，CPU编码及724输入／候选hash与v42相同。原票链逐条核验，重复最旧原票无副作用。
- [池核验](pool-verification.json)：724grant==Filled==return、拒绝0、最终64MiB／无活动grant，真实worker实例与父wait逐一对应；357次周期全页扫描。池整堆峰＋64KiB退出尾部67,191,738B低于64MiB＋0.25MiB，**不能用这个总量相减证明所有诊断内存**。
- [运行绑定](functional-verification.json)：总365.872895s，业务329.286026s，实际Store冷ready0.094916s；不是同负载联合比较或四档速度签收。
- 全程1464采样帧，采样RSS峰168,935,424B，已得footprint峰123,553,736B；1538次进程footprint缺测，swap净增0、非normal时长0。见[逐实例覆盖](coverage.json)，短进程缺测／PID复用不当0，采样峰不当绝对峰。

[静态归档核验](static-check.txt)验证当前源／二进制、两次失败独立来源、最终完整业务与逐文件hash；[核验脚本](static_check.py)可重跑。原始大L1快照正文保留ignored work，索引hash随失败归档，不冒充新来源冷装通过。observer自身和整个主机分别观察，不外推16GB。

## 未完成门与下一依赖

| 门 | 状态／下一动作 |
| --- | --- |
| 本阶段真实借还及原full功能 | passed，范围如上 |
| 原L1联合 | failed，旧来源原样保留；新来源未重跑，不能补成通过 |
| 精确常规64MiB及共同8MiB | not_accepted，Engine普通分配、完整外部开销仍待闭合 |
| 四档快进／正式三轮稳态 | not_run，不从365.9s推断通过 |
| 实际8h | not_run；先接通Sim故障下broker拥有Child及代次撤销回收，处理8h日志增长，再按原720目标／96保存及故障时点实际运行 |
| 真实16GB | not_run，当前32GiB Apple M4不能代替 |
| 完整M0 | not_accepted |

advisor方案／结果独立审查见[记录](advisor-review.md)。下阶段推进未实现依赖，不重复扩本阶段案例，不通过缩负载或改冻结门制造通过。
