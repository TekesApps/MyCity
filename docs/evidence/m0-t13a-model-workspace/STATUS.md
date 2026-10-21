# v0.5.69 固定核心表、准备租约与展开恢复

候选最初按v68准备，v0.5.68已先发布独立清理阶段；本模型实际发布v0.5.69，原始测试、run id及审查时间不改写。

本阶段实际改造核心数据及事务准备路径，不是完整ModelWorkspace／normal64签收。仍仅Rust模型和CLI，原冻结验收fixture、业务算子与数值步、门限不改；不做UI/GPU/AgentKit。

## 已接入的运行代码

- 通用`Rows<T,N>`分开固定backing和活动长度；预先建立全容量有效记录，push／copy不扩容，只序列化活动部分。反序列化在解码第N+1条之前拒绝超限，固定表不按文件提供的size hint增长。
- Timeline Claims20000／Payroll64、Engine资源／租约／hot journal改为固定记录表。逻辑`null`仍常驻背板，Enable不新建表，旧线格式不变。
- 每个Engine显式建立PreparationWorkspace，含时间线、dirty账户64、dirty库存128、资源8及容量租约64的准备槽。旧的业务限制不扩大；例如普通动作的dirty stock限制仍32，物理预留128不是新的操作权限。
- 准备前取得唯一lease，活动记录复制到既有背板；quote、校验失败、持久hook错误、展开或部分取得失败均归还已取得槽。重入失败关闭，未偷偷分配第二份表。后台Background当前仍走旧深Clone，不冒称已迁移。
- 持久化成功后写已验证账户／库存句柄，交换Timeline／资源／租约背板；旧权威背板归还工作区。journal容量预先固定，发布不执行用户回调。
- Store在append前先poison；成功append后仍保持到Engine apply、journal和Applied observer结束，防止磁盘提交后展开却继续使用旧内存。只有已分类的写入前Capacity／FactBlocked能解除；不靠清理lease撤销磁盘提交。CurrentSynced／Applied真实panic反例验证重开找回原提交、重试不重复付款。

## 同源实测

来源cc1ca0fb3e32ff336b20367fe43870677f32ab8bb3d60b8266a532bceb629893，160文件，见[source.json](source.json)／source.zip。固定headless binary SHA2a830aef888b9c83a362f66502607685b150c56bfa23d9b709a3410644c90907保留于work/candidate-binaries/model-core-tables/mycity-headless。fixture SHA仍876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508。

最终全workspace Release实际session46538退出0、638.418秒；clippy workspace all-targets和fmt同源退出0，见[终态检查](checks-final.json)及[测试记录](tests-final.json)。harness通过数包含nested invocation，不冒称去重测试数。源码／固定binary在终态后重新对照封存内容，保持一致。

之前core-first、core-lease／13项storage-recovery和core-tables／storage-tables分别通过；后者source快照tables-pre-publish-poison-source.zip对应25533，通过后再延长poison至apply结束。最终发布只用46538同源全仓结果，不拼接不同版本。中间check-tables.txt的编译类型错误已修复，原日志保留。

## 旧完整存档兼容

独立Rust CLI链接当前确切rlib，实际执行`Store::open`并重新导出完整Engine。输入是旧v66实际联合运行world的完整副本243文件；保留原rules identity，没有修改旧存档或用新规则覆盖身份。恢复sequence3013，导出2,507,418字节与旧fault-reference原始文件逐字节相等，SHA f4c922bfc7a6a214e73d1c69c26386b3faa2465794dc9f77f99793a554246e02。

[compat-proof.json](compat-proof.json)绑定helper源码、编译命令、实际rlib／executable hash及输出；[输入manifest](legacy-input-manifest.json)记录原243文件，原目录前后hash一致。完整副本保留于`work/t13a68-legacy-reopen/world`，未用hash替代原文件。此helper是兼容性验证，普通Store路径仍非完整有界解析，不当作吞吐、配额或16GB通过。

[advisor最终独立审查](advisor-review.md)认可本有限实现发布范围。

## 明确未完成的部分

完整Background／pending／field的常驻存储及五空气scratch、World/Arena／Opening一般恢复seed、WAL／receipt／canonical DOM的流式迁移，以及进程pool真实分配前授权／触页／wait预算仍未闭合。

当前每Engine独立拥有PreparationWorkspace；v67的37.59MiB是未来目标拓扑，不能直接套作当前内存实测。旧`owned_capacity_bytes`仍主要计权威状态，不能拿它代替含准备槽、Arc／Mutex控制块、解析、原生栈及allocator的全图账。后续须共享工作区或按实际副本数重算，并兑现64MiB原预算；表记录初始化也不代替完整联合物理驻留证据。

本轮没有新原full L1+L2／正式性能实验，没有四档、三场景三轮稳态或真正28800秒；真实16GB仍not_run，normal64/common8未签，M0 not_accepted、goal active。下一依赖继续完整Background及Store图，不回头堆更多孤立案例或再做一份规划表。

## 阶段回收

复用现有target，未建独立target，未新增压力快照。保留必要源码／日志／固定exe和当前完整兼容存档；此前恢复依赖不动。共享target属于当前构建／下一验证依赖，未将用途不明文件批量删除。阶段实际盘点见disk-stage-end.json；遵循[回收规则](../../validation-artifact-retention.md)，后续已替代且无恢复依赖的产物再逐文件退役，测量中不清理。
