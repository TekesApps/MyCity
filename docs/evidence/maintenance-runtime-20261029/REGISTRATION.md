# #159 登记与实际投运起点检查点

v0.5.229为进行中检查点，**不是完整维护验收**。#159/#36/#117开放；root共享接线1项，准备0项，无新增模块派工。#151原合同已关闭，现行GitHub回读仍Closed，不重复收口或计历史补关。

## 本次真正接入的范围

- 新facility authority v3显式绑定project7/component1/contract22和原owner材料Stock；原生产v2和None保持原语义。按需Box位于原facility backing内，未扩大World常驻头或原预算。
- 新初始维修场地、四人容量及窗口/成员/用途/owner来源明确；用途159仅放行匹配该维修登记的合同，沿原Venue路径，不授予通用新用途。登记不是实际进入或工时。
- 原建设成功验收投运时，同一Prepared创建维护起点。真正的投运票、时间61200及初始condition10000与原资本/容量同提交；未投运时没有snapshot，不从支付/施工工秒推导保养完成。
- 当前仅实现维护初始起点。恢复必须与原资本的投运事实及该初始snapshot相容；后续工时/磨损推进尚未接线，不能把这个检查点当作可持续维护运行版本。公开窄摘要只给投运事实，不复制完整World。
- 维护专用candidate profile v2明确不支持替换，原v1替换配方保持。首次进入实际checkpoint路径的component Vec在解码分配前限制MAX8，恢复后的真实capacity计费；不只按逻辑len收费。

参数、明确期初来源和单资产剩余合同见[设计](../../asset-maintenance-runtime.md)。这些是游戏profile，不是校准或硬件上限。

## 固定源码与实际验证

外卷`work/maintenance-runtime-20261029/registration08-{source-manifest.json,full-source.zip,build-result.json,results.json}`；每项固定程序和SHA均在build-result。`regstore01`对应源码逐文件SHA与registration08完全相同，另保留整包源码/程序，不借旧程序替代当前运行。

- 先原18单元/128KiB门：120568+8456+2048=131072，World74224；原Journal3488≤4096。原负载、Wait及预算未改。
- 来源/接线4项：人员/现金/Stock与旧资格、维修场地版本与未绑定拒绝、无工单backing同槽/copy/缺省/null、原建设/Boot/旧公服实际到返→成功投运→原票重复→checkpoint读回。
- 候选12项（原10及明确不替换/有界恢复）、工资内核20、旧实际设施劳动13、原资产same-slot1通过；原18选择与旧设施组有重叠，不合并成不同业务数量。
- `regstore01`的原FacilityWork WAL/checkpoint/Stop原票与丢回复恢复通过；旧Water V2真实normal生产/Move/RunDue/保存冷重启/原票/归还通过。没有设置RUST_MIN_STACK，全部串行，未扩全产品验收。
- normal实际采样最大arena used55861112B、原59MiB grant、allocation_failures0；共同64MiB workspace授予3/归还3。这是旧业务兼容门，不是维护normal、全进程native/系统总内存或16GB实机证明。

原建设资本/公服语义在新场景中共存；没有声称30DAY后的保养现场、履约与停机已运行。新维护原票、Store冷恢复及资源门仍not_run。

## 失败与修复证据保留

registration01为用途u32/u16编译错误；02真实入口因未支持的维修用途拒绝；03误写测试target（未编译）；04测试误用私有入口。05/06的总owned排序断言失败：同槽copy/restore另有Arena/表容量差异，故不能假定切回旧场景后总量必然更小。原诊断值/布局/完整source/exe保留；现改核维护字段真实owned、缺省释放、根地址/状态一致与原Engine硬门，没有放宽游戏预算或隐去失败。07通过，08再加实际解码分配界限后完成最终限定门；08结果不倒写旧失败。

## 本阶段统计与下一步

固定最终程序验证窗口UTC **2026-10-28T19:55:47.191998至20:01:03.704770**，北京时间29日03:55:47至04:01:03。它是构建终态到本组验证完成窗口，不是开发周期；可靠全合同可实施/源码首交起点仍未知。

新验收业务0、历史补关0、缺陷关闭0；18整包交付积压不核销、变化0。**当前唯一共享阻断仍是#159实际维护执行链未闭合。**下一在同一root lane接原canonical容量的到期安全停机，再实际Venue/原成员与Funds/材料、P5进度与费用、复位及维护恢复门；原159完整合同全满足才关闭。材料在due时必须真正可用，不能以期初25毫Q配方推定30DAY后的库存或跳过既有城市义务。

不启动94等暂未有近期真实消费方的源码模块；替换、Bridge、自动事故、多档/通用框架及GPU暂缓。现有住房/劳动/企业/公服/交通/Boot共同有限场景是消费者，不扩大成全领域前置验收。

## 环境与保留

Mac16,1/32GiB/macOS26.6.2/Rust1.85.1，HP P900 USB/APFS，原唯一target/work。统计/源/命令与资源值见[summary](registration-summary.json)。留证目录原根、`work/m159r01`、旧`work/m159w02`合计du约1.82GiB<事前2GiB；包含完整失败源/程序/原World/WAL/checkpoint。24无关WIP逐文件SHA未变。共享target仍需后续接线、失败和恢复原件需保留，本阶段不清理/不声称APFS释放量。无新tree/timer，不唤醒停止旧主。
