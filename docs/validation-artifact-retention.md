# 验证产物回收规则

**work现行位置与保护身份**：[完整迁移记录](evidence/work-migration-20261022/STATUS.md)。work原路径链接外置HP P900 SSD，原件均在，190份新卷保护身份见该记录；历史same-inode收据不等于当前身份。后续记录实际存储路径/卷/空间，新IO基线注明外置USB/APFS SSD；源码卷df不能代替work所在卷空间。

**共享target现行位置**：原路径`/Users/frankqin/dev/MyCity/target`符号链接至`/Volumes/HP P900/dev/MyCity/target`；[迁移核验与退役记录](evidence/target-migration-20261022/STATUS.md)。继续复用原路径；外置卷离线先恢复挂载，不自动创建替代target。

**2026-10-22集中回收补充**：[本次清单与报告](evidence/disk-reclamation-focused-20261022/STATUS.md)精确删除joint已退役两代的50份逐字节匹配既有gzip的正常组件，401,145,856B；190完整原件保留，work25.427GiB。此前74GB及历史成果不计入本次。两代仍不可直接完整恢复，未匹配组件和故障证据保留。

**最新只读复查（v0.5.108）**：[当前盘点](evidence/disk-reclamation-live-20261022/STATUS.md)重新核验190原件，当前六份大型压力均仍有恢复/故障依赖，新增删除0B，历史释放不重复计入。随后[核心合并阶段](evidence/core-m0-integration-108/stage-retention.json)未生成大压力或独立target；固定验收exe使用硬链接保留，共享target复用。

**当前阶段回收（v0.5.106）**：[映射pool功能终态](evidence/m0-t13a-pool-mapped-proof/STATUS.md)及[当前可用性](evidence/m0-t13a-pool-mapped-proof/retirement/availability.json)：54份本阶段诊断pressure/reserve/重复合成组件精确退役6,748,618,752B；属于新增数据终态回收，不计入v105历史清理成果。190恢复/故障原件逐SHA保留，work剩25.337GiB，target2.080GiB复用。两代诊断不可直接恢复，raw/index/source/exe/故障保留；df另列，正常组件gzip不等于完整快照。

**当前集中清理（v0.5.105）**：[报告](evidence/disk-reclamation-v105/STATUS.md)及[逐快照可用性](evidence/disk-reclamation-v105/availability.json)覆盖本次六个旧正常快照的原路径承诺：154文件精确退役，分配空间7,550,910,464B；work降至25.322GiB，target2.080GiB复用。190完整恢复/故障原件逐SHA保留，业务/WIP/raw/固定源依赖/exe不变。已删快照不可直接完整恢复；gzip组件不是完整快照。df变化单列，不全算成果。阶段结束先回收或登记依赖再开启新大负载，正式测量不与清理或构建并发。

**当前回收与恢复可用性（v0.5.104）**：[R07功能终态](evidence/m0-t13a-r07-report-error/STATUS.md)及[availability](evidence/m0-t13a-r07-report-error/retirement/availability.json)覆盖本阶段旧原路径保留承诺。实际新PID冷开与继任后仅退役8份旧合成pressure/reserve，分配空间12,699,467,776B；新最终v3第二代及原五份完整/故障基线190文件全SHA保留，退役目录不能直接恢复。初次预检st_blocks变化在删除前止步，5,619,712B保留块减少单列排除；df另报，不归入本次成果。源码/WIP/raw/组件/index/固定源依赖/exe不变，共享target保留复用，其他项目Cargo未干预。

**当前回收与恢复可用性（v0.5.103）**：[用户指定集中回收](evidence/disk-reclamation-v103/STATUS.md)及[availability](evidence/disk-reclamation-v103/availability.json)覆盖本次相关原路径承诺：7份已无依赖prefix/已归档正常重复组件退役，分配空间617,242,624B；190份完整恢复/故障原件保留，当前R07第一代未冷恢复保留。共享target复用，源码/WIP/业务/raw/固定依赖/exe不变。三份旧正常布局继续不可直接完整恢复；已删prefix也不能直接复用。df变化单列，历史释放不重复计入。本轮不运行模型或构建。

**v0.5.102原生整组观测终态**：[回收与恢复可用性](evidence/m0-t13a-native-observer-errno/STATUS.md)精确退役9份本轮短测/失主合成载荷、3份缺当前errno的旧静态库，分配空间12,756,258,816B；本轮新目录不可直接完整恢复。typed正式7/8、v50及report-lifetime完整故障原件158文件全SHA前后核验，raw/typed组件/partial targets/源锁exe/固定依赖保留，其他128,391元数据不变。df变化单列、不计历史成果，唯一target复用。

**当前权威（v0.5.101）**：[本次用户指定集中回收](evidence/disk-reclamation-v101/STATUS.md)及逐文件清单覆盖旧smoke-L2完整保留和指定10份正常组件原路径保留承诺。smoke-L2暖场超时的pressure/reserve已解除依赖；指定旧正常组件完整字节保存在既有gzip归档，相关快照均不能直接完整恢复。typed正式7/8、v50和report-lifetime首代/pending2完整原件继续保留。仅清理精确核验的旧静态库，当前共享target复用；本次不运行模型或构建。

**v0.5.100原生读取后端终态**：[缓存清单与结果](evidence/m0-t13a-native-reader/STATUS.md)退役28份缺少当前强制libproc依赖的旧rlib/rmeta，分配空间104.691MiB；固定源/lock/exe、SDK对照和故障raw保留。无压力副本或独立target；实际后续build未重新生成已删库，当前3库及54固定候选文件复核。原完整压力恢复基线不动，df变化独立报告。

**v0.5.99 VM诊断终态**：[本轮留存与精确清单](evidence/m0-t13a-runtime-vm/STATUS.md)仅退役runtime-vm-l1-01唯一snapshot的pressure/reserve，不可直接完整恢复；组件/index/VM原文/实际恢复报告与原current7/8保留。预检58数量与实际64全文件不符，在删除前停止并修正；后续保护全部64原文件。实际分配空间及独立df变化见该轮retirement/result，不重复计入v98回收。共享target复用，无构建。

**最新恢复可用性（v0.5.98）**：[原L1终态交接](evidence/m0-t13a-typed-workspace/FORMAL-STATUS.md)及[availability](evidence/m0-t13a-typed-workspace/formal-retirement/availability.json)覆盖旧steady-controller-formal-l2-01第7/8代完整保留承诺。新typed-workspace正式第7/8代实际完整冷恢复并继续扫描、独立释放依赖后，旧两代仅4份pressure/reserve退役，分配空间6,356,516,864B，work剩24.806GiB；旧两代不可直接完整恢复。新两代、v50、最新82故障原件完整保留，旧L2业务/WAL/组件/索引/raw/源/exe不删。回收后置检查遇未归属Cargo而保守退出，已用无删除finalizer复核收尾，原错误留存。df变化独立报告；target继续复用。

**v0.5.97功能阶段终态**：[typed工作区阶段留存](evidence/m0-t13a-typed-workspace/stage-retention.json)新增删除0B；完整冷恢复复用既有原件，无新压力副本，无独立target。当前workspace/all-target及clippy/CLI profile缓存尚有依赖，未以单一构建图武断删除其他有效profile；固定源/exe/日志/原完整恢复基线继续保留。原formal实验另立用途/峰值及退役条件，测量期间不清理。

**当前复查（v0.5.96更正）**：[集中回收后盘点](evidence/disk-reclamation-current-recheck/STATUS.md)实测work24.40GiB、共享target1.34GiB；无本项目work/target文件占用，另观察到短暂1.95工具链Cargo，未停止；125,304项文件元数据不变。本次新增删除0B，12份pressure/reserve仍为完整恢复及故障依赖，保留原件；v0.5.94历史回收不重复计入。旧退役状态不变，下一阶段继续复用唯一target，先完成依赖交接再退役新旧副本。

**最新可用性（v0.5.94）**：[旧故障依赖交接](evidence/disk-reclamation-fault-handoff/STATUS.md)及[逐文件availability](evidence/disk-reclamation-fault-handoff/availability.json)覆盖旧a702/v91/c0完整载荷保留承诺。最新82实际全冷恢复、输入逐字节等价与独立依赖解除后，精确退役9个旧合成pressure/reserve，分配空间14,710,177,792B，work剩24.42GiB。旧首代不能直接完整恢复，旧pending不再保留完整故障载荷；最新82完整首代/pending、formal7/8、v50及smoke-L2完整原件保留。源码/WIP/raw/typed组件/依赖/exe与共享target均未删。

**v0.5.93阶段留存**：[重复快照实际失败](evidence/m0-t13a-snapshot-backing/STATUS.md)终态后分析，新增删除0B。本轮首代完整快照及第二代pending仍用于报告逃逸修复的下一步诊断；旧恢复输入/故障仍未解除依赖。复用唯一target；先完成下一次具体因果验证和依赖交接，再按精确文件退役，不默认永久保存每轮压力副本。

**最新集中盘点（v0.5.92）**：[当前用户指定审计](evidence/disk-reclamation-latest-request/STATUS.md)实测work约25.49GiB、共享target约1.34GiB；本轮新增删除0B。剩余7份pressure为恢复基线、旧格式输入或仍在诊断的故障现场，含新formal第1代及第2代pending。新formal重复快照失败，未解除旧基线依赖；pending不是恢复点。原始性能样本、业务数据、固定源／exe及共享target保留。历史退役状态不变，旧删除不计入本轮成果。

**最新可用性（v0.5.91）**：[阶段终态回收](evidence/m0-t13a-causal-clock/STATUS.md)及[可用性清单](evidence/m0-t13a-causal-clock/retention/availability.json)覆盖该短测的原路径保留承诺。原全仓终态后，27份成功短测pressure/reserve/重复组件已按精确清单退役，分配空间3,378,831,360B；不可直接完整恢复。formal7/8、v50、smoke-L2及a702失败pending完整保留，其他既有退役状态不变；共享target继续复用。下文旧“最新”仅代表当时状态，以本项为准。

**最新可用性（v0.5.90）**：[本次集中清理](evidence/disk-reclamation-current-stage/STATUS.md)及[逐快照可用性](evidence/disk-reclamation-current-stage/availability.json)为当前权威。新增退役102份终态短测压力／预留及已逐字节归档的旧重复组件，分配空间3,989,151,744B，work剩19.180706GiB。formal7/8、v50和smoke-L2完整原件、整个新失败pending、业务／源码／exe／raw保持；成功新短测不可直接恢复。下文所有旧“当前”保留承诺只代表当时状态，冲突以最新清单为准。

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

阶段开始的保留意图还须写明预期磁盘新增量；阶段结束必须明确每个新增大副本的下游依赖及解除条件，先回收或登记保留理由，再开下一轮。完整恢复输入不能用hash替代；可退役的重复组件须已有逐字节核对的完整无损归档。不为每轮复制target，其他项目及全局缓存不在本项目清理范围。

## 阶段回收执行门（v0.5.101补充）

- 开始前登记每个新目录的目的、预计新增分配空间、下游原路径依赖和解除条件；同时检查磁盘可用空间能否容纳本轮峰值。优先复用既有输入，禁止仅为验证轮次复制target。
- 同一阶段结束后先完成留存/回收决策，再创建下一轮大负载。登记本阶段实增、累计work/target、已删除清单、保留项及下一次解除事件；失败或中断也必须执行。
- 恢复基线交接必须实际完整读回/冷恢复并解除依赖；故障现场须区分内容故障与无关暖场失败，不能无期限把所有失败载荷都保留。用途不明仍保留并列入待决清单，不能仅凭超期删除。
- 固定源/lock/exe、日志/收据/原始时轴和故障证据保留。正常重复组件仅在无需原路径、现有完整归档逐字节一致且原index吻合时回收；不能给仍需恢复的快照换成hash。
- 每次删除前检查进程、cwd/文件占用、硬链接及实际后续脚本依赖，保存并核对精确允许清单；删除后核验保护原件和WIP，更新当前恢复可用性。正式测量期间不做清理或重盘点。
- 共享target保留当前build/test/clippy图及所需profile；仅退役已证实过时且不会立即重建的缓存，不清理其他项目或全局依赖。分别报告inode去重st_blocks、du和df，不把APFS或其他任务释放计作成果。
