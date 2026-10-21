# v0.5.71 一般模型有界解码

仅Rust模型／CLI，原fixture SHA876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508、原全部记录数及CPU数值步／门限不改。实际迁移一般World／Arena／Opening与通用空气数组解码，不是完整ModelWorkspace／normal64签收。

## 当前运行代码

普通Arena visitor按可信Domain64／128建立容量，记录直接写入预留allocation；不由文件max或size_hint扩大。slots/free任意字段顺序均先定capacity，超可信N在解码额外值前拒绝，重复／缺失／未知字段失败关闭。declared max仍控制业务容量，64背板下max3不允许第4个槽；实际预算按64背板计，小预算不足就拒绝。

恢复保留稀疏槽、free栈顺序及句柄scope／generation；restore_capacity改为纯验证，不再建立第二份Vec。MAX free表示最后一代尚可分配，MAX空值且不在free表示耗尽退休，不能一律拒绝MAX free。已加入完整最后一代跨两次恢复回归。

Arena保持原私有Vec布局和L1原owned计量；初始化后实际观测再clear，push始终有逻辑max与真实capacity约束。不把clear后的尾部描述为可安全借用的typed活动记录。L1 40000／110000记录继续走独立可信shape入口，没有借普通解码文件max进入大表，也未改变原结构尺寸或削减压力字节。

World账户／库存索引及资本来源、Opening根及region动态行改固定Rows；通用空气source20000及场／scratch／integral16384直接写入全容量typed背板，保持原air_rows错误文本。解析后普通Input push／clear在既有容量内工作，不放宽逻辑源上限。World预算检查改直接算真实容量，不为检查构造JSON树；期初资本验证去掉临时ID Vec和bootstrap编码／重解码，语义不改。

所有期初标签／空气身份的decoded64B visitor与lexical384B、depth128限制配合，在一般模型入口先保护serde scratch；384B允许合法64 ASCII标签的unicode转义。Native默认128B词法限制不变。FrozenLabel借字符串匹配静态label，不先创建临时String。

## 同源实测与旧完整存档

[source.json](source.json)／source.zip固定源码87796b6e784d1f124b96d7abb45d3898ae9b712dbba8007ea17f76d92bd8d892，163文件；固定headless SHA3e20379f34e39a7f0c0f808c9df8cdbad18616f4e77db3bb798cd39e4cbf6588，保留work/candidate-binaries/general-decode/mycity-headless。v70固定验收binary也再次核验未变。

最终全workspace Release实际92234终态0，540.068秒；clippy workspace all-targets／fmt／build同源退出0，见[终态检查](checks-final.json)。[362条harness通过记录](tests-final.json)包含nested invocations，不称去重测试数。覆盖密集空气原数值与规范字节、真实30日经济、旧空间／财政／公服流程和实际进程故障恢复，以及新解码／最后代次回归；没有将中间core结果拼作最终来源，[失败说明](failure-notes.md)保留。

[旧完整存档验证](compat-proof.json)用当前确切库实际Store::open已保留的完整副本，保留原rules identity，不改原world；原及副本各243文件前后完整hash一致。实际重开sequence3013，导出2,507,418字节与原独立fault-reference完整文件逐字节相同，SHA仍f4c922bfc7a6a214e73d1c69c26386b3faa2465794dc9f77f99793a554246e02。helper源码／命令／库／exe hash均绑定，不以hash替代实际恢复或原文件。

## 增量容量与明确未签部分

[同源Rust类型容量CLI](capacity.stdout)目标43,965,008B，不是现有全部解析已接broker证明。一般Background仍经Raw后归一化；本阶段[解码重叠补项](decoder-overlap-capacity.json)按一Raw State加一Raw pending Input保守收费为两份最大完整typed State：2×662,624B。加原目标共45,290,256B，小于原67,108,864B；原五通用Air槽未抵扣，临时解码串行而非按三个Engine重复算同时发生。只作具名保守算术，runtime_grant_accepted仍false，allocator／栈／RSS仍需实际整图账。

[advisor最终独立审查](advisor-review.md)认可本有限发布及上述串行重叠边界。Raw归一化尚未完全直接写目标槽，公共拥有式入口仍存在；Store WAL／receipt／canonical和所有角色的预授权／填充／实际wait整图继续实施。本阶段未新跑完整联合／正式性能／四档／三场景三轮稳态／真正28800秒，不把v70短诊断当成本源码性能结果。真实16GB仍not_run，normal64／共同8MiB未签，M0 not_accepted，goal active。

## 阶段磁盘

复用现有target，不建独立缓存、不新增压力快照、不新复制旧world；固定当前／历史必要exe、source／依赖、收据／故障日志及完整恢复依赖保留。t13a70-joint-01两代与v50旧格式snapshot1仍完整；旧v66布局pressure已在v70明确退役，不能直接完整恢复，其business world继续保留。实际盘点见disk-stage-end.json；本次没有确认无用且可安全删除的新独立缓存，未批删用途不明的共享target文件。按[回收规则](../../validation-artifact-retention.md)在依赖解除后再退役，测量中不清理。
