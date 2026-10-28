# 有限岗位群组实际工时与公服共存

#155有限合同和#156恢复缺陷已发布v0.5.217、回填并实际关闭；[回执](closure.json)。完整城市、#21/#5/#57及硬件/M0任务保持各自范围。本阶段不是整个劳动领域或产品容量验收。

原Payroll登记精确成员范围及不可变游戏来源，09–17实际持有原member_activity才计工。先占有者优先；已知冲突等待，释放后只从新claim累计。整组时薪不按人数相乘；资金、工资余数、欠薪、实际付款日收入和Town产量沿原权威路径。StopWork/Terminate/EndForExit及到期保留应计和必要退出责任。旧无登记输入仍是prototype，不能推断历史人员。

状态仅用原按需OperatingModels，未扩World/Payroll内联字段。P5每原Envelope最多Begin一组，原continuation同时计步骤与强制分票；保持96页/原磁盘和收据门。纯Idle/Waiting不申请成员序号；有界流式hash不分配编码Vec，两份成员候选/副本和恢复工作区显式计费。Store比对原genesis登记再查原票，不新增账、调度器或认证框架。

## 实际结果与固定版本

[完整摘要](summary.json)、[最终498构建输入](source-manifest.json)。完整原件位于`/Volumes/HP P900/dev/MyCity/work/payroll-members-runtime-20261028/`；每个build有独立完整source ZIP、manifest、固定程序/SHA和结果，不能说所有检查来自同一程序。最终shared14/storage15/role16生产源码及输入与发布工作树逐文件一致。

- shared14：当前core313通过、1原忽略、1独立full-native诊断过滤；岗位9、原Timeline16、设施13、工程16通过。原18单元`120568+8456+2048=131072`，World74224B；原票3488B≤4096。
- compat02/decode03：旧公服9、配置资产原预算原子拒绝、release当前结构解码/旧null/同槽copy3项通过。随后仅测试和新可选策略逻辑变化；旧缺省字段/镜像结构未变，原18/原票已用最终shared14重验。
- storage15：Store单元36、原Timeline7、欠薪退出3、紧急续办4（1显式冷探针忽略）、旧公服短回归4（1冷探针忽略、1长例过滤）及新增Store5（2显式探针忽略）通过。显式冷探针由父例实际启动，不用ignored数量代替执行结果。
- 新Store场景实际9个冷进程exit0，覆盖部分P0/P3/P5、第一组已Begin后同刻第二组续办、WAL/checkpoint、原票重复。两种写盘中断后实际工秒与工资只记一次；96页恰好预留/冷续消费，97页事前拒绝不改业务。
- 同成员警方从08:00实际占用至09:25，岗位首日实际27300秒、75分、余数3000；次日累计56100秒/155分。Town产量94+100、余数22800，原料余806、电余76000、累计税698/留存699。不重叠成员正常8小时/80分每日，两成员群组也只按整组费率。
- role16：旧完整V2存档默认栈恢复字节不变；真实Sim两次打开/冷重启/IPC与独立参考一致。原59MiB授权、allocation_failures=0，观察arena最大55798280B；Engine owned3690248B、Store owned42008586B（后者含Engine，不相加）。64MiB池两次实际workspace归还完整恢复。creator/reference在shipping角色之外；不据此宣称全进程RSS/native/整机峰值或16GB通过。

## 失败原件与证据复用

首次包名选择未启动编译，随后Restore trait/Rows API编译错误；groups04测试用了错误Cycle编码及复用拒绝票号，均修测试并保留失败源/程序，未改原票规则。

store09实际发现强制分票只留一票；store10发现#156：家庭无入住但Timeline Enable已有income_anchor，Engine错误提前empty finish。最小修复只让所有原票引用均空时自动empty恢复，其余交原Store。probe12对原存档失败；store13同原件只读probe通过，新通过场景genesis与checkpoint-13完整字节与原失败件相同，原件未覆盖。原advisor仅窄核具体架构/阻断与修复，无新明确阻断；不替代root实际执行。

**旧720日公服回收长例本次240秒超时，未完成，不算通过。**中断程序、日志、完成前缀及完整存档保留。该case原函数体、输入与v206历史通过一致，相关回收/公服域文件又与v216完整5任务实际Boot回收通过一致；新Payroll在此旧输入中缺省，新增步骤/强制票为0，已有入住使旧/新empty guard均不进入。仅复用回收业务语义，不复用当前内存/时延：详见摘要的逐文件SHA与具体分支依据、`work/payroll-members-runtime-20261028/legacy-reclaim-reuse.json`。不称本轮全库或完整长例重跑通过。

## 范围、统计和保留

统计窗口见summary：仅为本轮记录的验证窗口，不当开发耗时。可靠可实施/完整源码交付起点未知；本阶段业务验收1、历史补关0、缺陷1，实际Issue关闭见回执，验证窗口不当开发耗时。18份整包交付未机械核销，减少0；包数不代表业务任务完成度。

Mac16,1/32GiB、macOS26.6.2、Rust1.85.1、HP P900 USB/APFS，原唯一target/work串行。最初2GiB证据规划按固定debug程序和完整失败原件，在下一构建前修订3GiB；实际约2.00GiB。不是游戏预算放宽。24份无关WIP逐文件保持；未建新tree/target/timer，当前target仍供下一接线使用，恢复原件及当前/失败程序保留，未删除未知缓存或宣称APFS回收。

行数/编码上界不是城市或人口承载承诺。真实16GB、全进程native、全M0长期/压力、产品可持续倍率/规模、完整招聘/通勤/资格发放、GPU/UI/Windows仍未验。下一消费现有#151资产日结交付至#21共同场景，先核输入/直接差项，不新增模型批次、多档或通用框架。
