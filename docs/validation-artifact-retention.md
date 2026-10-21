# 验证产物回收规则

**当前可用性（v0.5.87）**：[集中回收与恢复基线交接](evidence/disk-reclamation-stage-followup/STATUS.md)覆盖旧v85完整保留承诺。新formal第7／8代经实际完整读回、最新冷恢复与独立依赖审查接替恢复基线；旧v85两代及已完成smoke-L1短诊断仅6份pressure／reserve按精确清单退役，均不再可直接完整恢复。新formal两代、v50旧格式和smoke-L2失败原件完整保留；业务／组件／原index／固定源码与exe／原始日志保留。本次删除分配空间9,534,775,296B，work剩17.657GiB；APFS及df变化另列，不重复计算历史清理。

**历史可用性（v0.5.86，v85保留状态已由上文替代）**：[用户指定集中回收](evidence/disk-reclamation-v70-components/STATUS.md)覆盖历史v70组件原路径保留承诺。v70两代的50份重复合成组件已按精确清单退役，完整字节继续保存在既有gzip证据归档；v70此前即缺pressure／reserve，仍不能直接完整恢复。当前完整恢复输入为v85两代及v50旧格式原件，不能以hash或归档替代它们。历史清理脚本的v70 CANON入口不再可原样复跑；当前审计只读其保留index。

[v0.5.85阶段回收及可用性](evidence/m0-t13a-role-arena/retention/result.json)更新完整基线：实际full联合和最新冷恢复通过后，新v85两代接替v70，旧v70两份pressure／reserve共4文件及9份临时合成输入已退役，不能直接完整恢复。v70原index／日志／业务／组件保留，v50完整旧格式和v85两代原件完整保留并全SHA核验。精确删除分配空间6,373,318,656B；保留文件APFS分配块变化另列，不计入回收。此次df前后差值缺可靠留存，保持未报告；唯一target继续复用。

每阶段结束先归档必要日志、收据、故障证据、原始索引、固定源码／依赖与验收可执行文件，再盘点 `work` 和共享 `target`。不为每轮测试新建 target；不清理其他项目或全局 Cargo 缓存。

分类和依赖优先于文件名：真实业务存档、WAL、财务权益记录默认保留；合成压力载荷可在旧轮次终止且无后续完整恢复依赖后退役。当前阶段、最新发布的完整恢复基线及仍被兼容测试引用的旧格式完整快照均保留；下一阶段确认替代关系后才转入退役候选。用途不明先保留，不默认无限积累每轮压力副本。

执行前复查进程、cwd、打开文件、后续脚本引用和硬链接；测量运行期间不清理，避免 I/O 污染结果。只删除逐文件确认的允许清单，不按 t13 前缀或目录名批删。先保存 inode、长度、mtime、分配块和所有硬链接，再执行并核验保留文件未变化。

删除压力载荷后，该快照立即标为“不能直接完整恢复”；保留 hash／index 只证明历史身份，不替代数据。仍需恢复验证的快照必须完整保留。若将来重建已退役载荷，须用对应版本生成器、种子／页布局／迭代次数重建，并核对完整原始索引后，才可重新声明可恢复。失败的 `.pending` 始终不是完整恢复点。

每次回收记录轮次、精确路径、理由、证据位置与当前恢复可用性。原始 manifest 和历史测量不改写；新的 availability 清单覆盖旧文档中的保留承诺。报告 inode 去重后的文件分配空间、前后 work/target 占用及文件系统可用空间变化；APFS 共享块、快照与其他任务会影响 df，不把其变化全算为本次释放。

本次记录见 [2026-10-21 清理报告](evidence/disk-reclamation-2026-10-21/STATUS.md)。后续阶段须在结束检查中明确“已回收”或“保留原因及下一次复查条件”，不得只启动下一轮而累积旧压力数据。

当前回收与恢复可用性以[v0.5.68后续报告](evidence/disk-reclamation-2026-10-21-followup/STATUS.md)为准：退役v64及失败v65第一轮压力，仍保留成功joint-02两代、v50旧格式依赖及当前业务兼容world。后续阶段不得将此精确允许清单泛化为批删权限。

[v0.5.70最新基线及阶段回收](evidence/m0-t13a-background-resident/STATUS.md)覆盖上述当时的保留承诺：新t13a70-joint-01两代及v50旧格式依赖完整；旧v66 layout压力已逐文件退役，其业务world和完整兼容副本保留。历史快照未重建前不可再宣称可直接完整恢复。

[v0.5.72旧轮次预留页回收](evidence/disk-reclamation-2026-10-21-reserves/STATUS.md)：仅退役v64、失败v65第一轮及v66旧布局共六份work-reserve.bin；这些布局此前即不完整，继续不可直接完整恢复。native／meter等历史组件、业务world、完整兼容副本、v70两代及v50完整旧格式快照继续保留。

阶段结束必须执行以下顺序：终态确认 → 保存日志／收据／故障及固定源码、依赖和可执行文件 → 核查下一阶段恢复输入 → 复查进程、cwd、占用与硬链接 → 精确清单清理 → 核验保护文件 → 记录分配空间和独立df变化。当前完整基线只有在替代基线真实通过且依赖解除后才可退役；用途不明确的产物写明保留理由和复查条件。共享target继续复用，禁止用删除后重建同样缓存制造回收成果。

[当前集中复查](evidence/disk-reclamation-current-audit/STATUS.md)：work约17.204GiB，三份完整布局及业务兼容输入保留；本阶段临时WAL fork退役9.734MiB，复查无追加删除。旧清理不重复计入，全部已退役副本不可直接恢复。

[v0.5.76终态回收](evidence/m0-t13a-role-slots/retention-result.json)：当前验证进程已退出、work/target无占用，新增删除0B；新完整业务world保留为下一阶段恢复输入，旧完整布局及必要证据不变。运行中的[集中只读核查](evidence/disk-reclamation-live-recheck/STATUS.md)不计历史回收或df增量为本次成果。

[v0.5.77终态回收](evidence/m0-t13a-air-candidate-slots/retention-result.json)：原验证进程已退出、work/target无占用，新增删除0B；保留最新完整业务world用于角色包接入后的恢复／重放验证，无新增L1复制或独立target。仍需完整旧恢复输入，不因缺少可删除项而删除必要证据。

## 重复组件的证据归档与阶段收口

[重复组件集中回收](evidence/disk-reclamation-duplicate-components/STATUS.md)覆盖旧报告中“全部历史 native／meter 原路径继续保留”的承诺，仅限其精确清单。已退役 pressure 的旧正常快照可以移除经逐字节确认重复的组件原路径，但须先将完整组件无损归档、实际解压核对长度及 SHA，持久化归档及父目录、逐路径映射／原 index hash／审查记录，再删除。完整原字节归档与单纯保存 hash 不同；已退役目录仍不能直接完整恢复。当前完整恢复基线、旧格式兼容输入、故障 .pending、negative fixture、共享 inode 及业务文件不随此规则连带退役。历史脚本中仍有效的 business／fault-reference 依赖继续保留。

阶段结束的回收记录必须列出：本阶段新增目录和用途、下一阶段精确依赖、立即退役项、保留项的解除条件、唯一共享 target，以及扣除新增归档后的净分配空间。重复载荷先检查是否能直接复用已有输入；必须复制时记录复制理由和终态回收条件。留存必要的故障／验收数据，不让每次验证默认永久保留一个完整工作目录。用途不明先保留并记录，不能只依前缀或 mtime 判断。不得清理其他项目、全局 Cargo 缓存，也不删除马上需重建的共享 target 来制造成果。

[v0.5.79构造基础终态回收](evidence/m0-t13a-role-workspace/spatial-retention-result.json)：全部验证终态、work/target无占用；本次新增删除0B。唯一新完整业务world保留给Meta／role下一阶段恢复；未复制L1／未新建target。现存完整布局、旧业务兼容及已无损归档的证据继续保留；df变化不归于清理。

[最新集中复查（v0.5.80）](evidence/disk-reclamation-final-recheck/STATUS.md)：work实测14.907GiB，剩余三份pressure均属完整恢复依赖；无新可确认退役项，删除0B。逐原文件校验三份布局及两份旧业务输入，保留当前Meta未提交工作；原始延时样本、故障现场及共享target不作为缓存删除。新增测试目录创建前应写明用途、下游依赖及终态回收条件，不以轮次默认永久保留。

[v0.5.81 Meta／legacy阶段回收](evidence/m0-t13a-metadata-workspace/retention-result.json)：全部实际验证终态、work／target无占用；新增删除0B，无独立target或L1副本。新增完整业务恢复输入、小型v1兼容原件和固定exe目录按[预先用途与解除条件](evidence/m0-t13a-metadata-workspace/stage-retention-intent.json)保留。不是每轮默认永久留存；下次全Role验证优先复用这些输入，仍需原完整布局和旧兼容原件。df变化独立记录，不计回收成果。

[本次用户指定集中复查](evidence/disk-reclamation-current-request/STATUS.md)：当前work实测15.013GiB，完整恢复输入逐文件复核；无新增可确认安全退役项，删除0B。固定依赖／exe／故障及延时原样保留，当前词法解析阶段终态后、创建新负载目录前再按精确依赖复查。

[v0.5.83固定词法终态回收](evidence/m0-t13a-fixed-lexical/retention-result.json)：验证及独立审查终态、占用和完整原件复核后，按精确清单删除11份合成JSON及patched world内9份文件，分配空间101,556,224B。临时输入不能直接恢复；原v81完整v1、原布局、新完整30日world与源码／exe／原始日志保留。无新独立target或L1副本，df变化单独报告。


## 2026-10-22集中回收补充

本次精确清单及恢复可用性见[清理报告](evidence/disk-reclamation-2026-10-22/STATUS.md)。仅对已经退役压力的旧正常布局，将逐字节一致的纯合成stub去重无损归档；不是业务人口／住宅／旅客存档。旧index及测量不修改，完整原字节归档保留，退役布局仍不能直接完整恢复。正在使用的v70两代、v50旧格式完整布局和业务兼容输入不随此清单退役。

后续每阶段开始记录新增目录用途、精确下游输入及解除条件；结束时必须完成回收决策后再开下一轮。优先直接复用既有输入和唯一target；需要复制时预先确定终态回收条件。对未发布／失败轮次也执行同样规则。保留日志、收据、故障证据、固定源／依赖及exe，不把它们混作临时缓存。无可安全回收项时报告0B及原因，不重复计算历史成果。

[v0.5.89 native观测阶段终态回收](evidence/m0-t13a-native-boundaries/retention/result.json)：短L1实际冷恢复及独立依赖审查后，精确退役27份无后续依赖压力／预留及已逐字节归档的组件，分配空间3,391,627,264B；短测snapshot不可直接完整恢复。formal第7／8代、v50及smoke-L2故障原件仍完整，源码／raw／index／exe保留。无新target或压力归档，df变化另列。
