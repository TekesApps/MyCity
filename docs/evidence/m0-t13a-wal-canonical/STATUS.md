# WAL有界规范编码与旧存档兼容（v0.5.74）

仅Rust模型／CLI。acceptance SHA `876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508`、原负载／CPU步数／速率与门限不改。本阶段移除WAL record的Value／clone和校验和整段拷贝，不是整图池或性能签收。

## 实现

- `canonical::Workspace`用可信128帧、每对象64个去重key的固定初始化遍历表，迭代处理对象，不将递归对象表堆在栈上；decoded key≤64B用于排序／去重，数组逐项处理，无64行限制。serde的有界输入有效性访问仍使用其原深度限制，不宣称任意JSON兼容器。
- 先检查全部输入子树的旧Value解析有效性，再执行重复key最后值规则，不能因旧子树被覆盖而跳过越界数字或非法surrogate。字符串规范转义、Number负零／指数／边界以原Value为独立字节oracle。Key／token／depth约束先于不受信scratch增长；固定32KiB载荷上限不变。
- WAL外层保持原derive字段顺序，只有record按旧Value排序。借用Envelope，无clone；两份固定32KiB缓冲承载typed record、规范record和最终frame，复用时保持raw引用有效。长度前缀与payload分块SHA，不再复制拼接。
- 重放先规范原record，再typed解码，再完整比较实际执行生成的规范record，保留空白／键顺序／转义／重复key的旧语义，拒绝typed visitor忽略的未知字段。Envelope actions按可信16预留并在额外元素物化前拒绝；原TooManyActions在persist hook前返回，没有缩小合法WAL。
- 保持原Prepared／半帧写／fsync／Root／CURRENT／Applied边界。坏record在尾部截断或GC之前拒绝，原回执不能变成新交易。
- JsonReader修复关闭引号计入token内容的一字节边界问题，支持64个ASCII字符全部unicode转义；默认Native类型和布局、数值模型不变。

仅启用原固定serde_json1.0.149的raw_value feature；Cargo.lock和依赖版本未变，未走reader拥有型RawValue／Box<RawValue>。Workspace retained perStore；仍不是实际ModelWorkspace授权。

## 同源验证

| 证据 | 结果 |
|---|---|
| [源与exe](source.json) | source63c1642e45fef44374dddeddfa4aeb9f1a734929777267b213ea799e26dcf4ca，167文件；fixed binary460627cab5a6ac964ba014e79336223170022caceee4b023eeef224c42518d10 |
| [终态检查](checks-final.json) | 全仓release30935终态0，595.301695667秒；379 harness pass sum含nested，非去重测试数；clippy／fmt／build均0 |
| [旧完整恢复](compat-proof.json) | 当前exact rlibs打开旧rules完整world；原world与完整兼容副本各243文件前后hash不变；3013序列及2,507,418字节逐字节相符 |
| [实际新WAL往返](roundtrip-proof.json) | 独立旧world副本新增一笔零额成功转账及一笔stale失败事务；两份实际WAL帧按旧derive＋Value编码逐字节和checksum全等，实际重开／重试后序列3015不重复入账；再保存两次及重开，全部3015条回执相同，原3013条逐行指纹不变 |
| [源／容量核验](integrity-capacity.json) | 源zip逐文件相符、固定binary及v70／v71／v73既有验收exe SHA不变；runtime_grant_accepted=false |

全仓保留原经济、区域、后台、实际退出与检查点回归。新增旧Value数字／转义／重复键字节oracle、20,000行数组、100层对象、复用／溢出／坏输入拒绝及32组成功／失败帧对照；实际重签WAL覆盖70次转义同key、64字节全转义previous可重放，unknown顶层／嵌套字段及被覆盖越界数字拒绝且不截断未验证尾部。[开发记录](failure-notes.md)与最终验证分开。

原Engine SHA仍为 `f4c922bfc7a6a214e73d1c69c26386b3faa2465794dc9f77f99793a554246e02`；临时实验新增零额成功事务后revision变化，最终完整Engine SHA为 `41862152b83c650453e32f4d777f38a72bcde1f3b62837c7f86a7a1cc36ab7f2`，WAL及检查点恢复均与新增后的完整期望字节相符。零额交易只验证成功持久化路径，不是经济负载或性能替身。

## 预算与未完成门

基础目标44,939,296B，新增canonical表726,016B、双WAL缓冲65,536B及Store头24B均明确计入；Raw Background串行暂态仍补两个最大State共1,325,248B。合计46,264,544B＜67,108,864B，余20,844,320B。未扣除原五个Air槽，未用估算取代分配前授权或完整物理测量。

`business_comparison_checkpoint`、`Engine::state_hash`／WorldView的DOM与编码、legacy v1检查点及若干元数据／查询仍待收敛，然后接整图Reference／FaultReference／Sim／Compare／候选和恢复的实际授权。共同8MiB、正式分级、四档、三场景三轮稳态、完整新源联合和实际28800秒8h仍未签。当前32GiB主机没有真实16GB设备，该门not_run；M0 not_accepted，goal active。[advisor](advisor-review.md)只认可本有限实现／恢复阶段。

## 回收

复用现有target，无新增独立target或压力副本。成功临时`work/t13a74-wal-roundtrip`终态、无占用和归档后按[精确清单](roundtrip-retirement-plan.json)退役245文件；[结果](roundtrip-retirement.json)记录分配空间10,207,232B（9.734MiB），85,131个非选中work文件元数据不变。**临时fork已不可直接打开**；proof不是存档替代。完整原world、兼容副本及source／固定依赖／exe／日志／收据／故障证据保留。

删除窗口work18,049,544→18,039,576KiB；target1,019,624KiB保持未变。df可用空间观察增加16,805,888B，其中超出删除分配空间的6,598,656B不归因本次回收；APFS共享块及其他任务仍影响df。st_blocks不证明独占物理块。本轮不删除当前共享target；仍需当前库做后续接入，历史proof关联的旧feature库及验收测试程序用途尚需逐项解依赖，先保留，下一阶段结束再查。

v70两代完整布局、v50完整旧格式、v66业务原world和t13a68完整兼容副本继续保留。已退役旧v64／v65首轮／v66布局不能直接恢复。按[统一规则](../../validation-artifact-retention.md)阶段结束回收，下一主依赖见[NEXT](NEXT.md)。
