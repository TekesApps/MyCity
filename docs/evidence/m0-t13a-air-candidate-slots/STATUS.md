# 固定空气候选槽及实际导入／重放（v0.5.77）

有限阶段实测通过，正式性能与完整M0未通过。最终源冻结为68d0247291c48fc595ccd1604761e32651f4c1dcd00f116c175239a4b81f7a6c，173文件，source.zip保存完整源和固定依赖；acceptance、full opening及Cargo.lock SHA未变。固定CLI SHA为4aaa1927b21161bb53dbe13fe025ace828b4107300676617581a598cf3456f10。

## 实现及边界

BackgroundStateSlot仅是内部128²／background::SOURCE_LIMIT（当前4源）profile。一般Air的20k源/其他side API保留；未提高或缩小冻结负载。槽一次预建field/scratch/integral、Input sources及固定字符串，restore直接seed并校验schema、payload hash、State、2MiB附件上限及完整EOF；失败清空有效标志，8个主要背板身份保持。受控page只推进既有State，失败也失效；没有借出可替换根的可变State引用。通用State::restore仍按原通用限额工作。

Background::frozen_input_into复用候选并核对原冻结checkpoint digest；无pending或hash不符均不可读。Reference复用独立候选；Sim持有独立冻结输入槽，与Store候选同时常驻，不能相互抵扣。Sim不再clone包含Input的整个Job，Ticket构造后复制代际／尝试两个标量，不借用Job跨越写事务。

Store创建/打开时预建自己的候选槽，import/staged检查/附件接纳/WAL重放串行复用。load_asset_into利用原stream::verified的8KiB页逐块读，不先拥有完整Vec；内部payload hash和内容地址的完整文件hash分别核对。外层失败或部分候选重置槽，失败WAL回执仍不需要附件。完整CPU重算、job/generation/field/input身份、唯一writer、持久后发布／poison／CURRENT／GC顺序保留，候选与验证scratch分开。

State::checkpoint_sha256以既有checkpoint_digest输出同一wire的hex；实际Sim status、Reference目标轨迹与rare预期不再先造完整checkpoint Vec。普通EngineSlot未使用的engine_mut同样删除，不能通过公共可变根接口替换背板。普通Compare和流式Endurance/Speed比较契约不变。

## 验证安排及已知开发期记录

开发测试和编译失败保留在development-failure.md及原stdout/stderr：两次错误fixture误认为内部16源（实际4）；修成常量满级4/通用5。headless借用跨Store可变事务编译错误改为只复制标量；clippy四处冗余字段改为简写。冻结前core及storage定向通过只代表其当时源，不能替代最终全仓结果。

最终同源全仓release退出0，594.769秒，392条harness通过记录含nested；clippy all-targets、fmt、build均0。旧完整world重开仍3013seq／2507418B，与旧fault-reference完全相等，原243文件及兼容副本前后不变；旧Value业务1818613B、状态2480116B完整字节及Store联合hash相等。见checks-final、compat-proof、codec-proof。没有因观察超时重跑。

原full30日实际Sim/worker总程339.326秒，Sim含IPC及持久化290.266秒；720小时／723目标／724窗口、64工资／20000债全付／8实际交付，完整business+air独立对照、冷开0.163秒及最旧票无副作用重试通过，最终seq3011。本轮未注入联合故障，不能冒充旧joint3013seq；无L1/normal pool，不以耗时或最大边界1.088秒签正式性能。见full-business-result/report。

730条同源success heap记录：三次model task生命周期Rust请求峰加64KiB尾最大16502713B，两个Sim最大24997873B，724 worker最大5980167B，driver614484B；不是同步全进程物理峰、共同8MiB或实际grant证明。见heap-summary.json。

测试包括分片reader、满/空/部分/完成/超量、好坏好复用、内部hash/schema/重复未知/EOF/字节词法限额、推进失败失效和稳定背板；真实staged checkpoint/重开/WAL/旧代拒绝/原票幂等；另直接测试内层合法但外层hash错误时失效。沿用已有真实进程退出故障矩阵。

## 容量与未通过项

Store和Reference各自owned报告包含新增候选；Sim status独立列air_input owned。现有五个1MiB保守额外Air槽覆盖Store candidate、Sim air_input、Reference candidate及两个余量；每Engine Background Work在原prepare单列，不借此抵扣。Store头保守含新增inline，继续保留Raw暂态收费。advisor指出Reference场景旧表只列1 Engine，现已改2 root及2份retained preparation，并增加测试约束。最新CLI基础54079296B，加Raw暂态1325248B共55404544B，小于原67108864B、余11704320B。Reference和ordinaryCompare各31860248B、Sim打开54079296B；仍仅目标算术、runtimeGrantfalse。

这些不是实际RoleWorkspace授权或同步物理峰值。normal64/common8、正式分级/四档/三轮稳态、28,800秒8h未通过，当前32GiB、真实16GB not_run，M0 not_accepted。

## 磁盘

阶段开始前已完成v76终态回收检查；复用共享target，未建独立缓存。v70两完整布局/v50旧格式、旧业务/兼容完整world及v76完整world保留。新full业务目录是原负载必需的独立实验，不复制L1压力。最终验证期间未清理；终态后work/target无占用，新增删除0B。work18282896KiB（17.436GiB），共享target1084960KiB（1059.531MiB），df可用116257927168B；波动不归因本次回收。原收据/故障/诊断及固定源/exe保留，最新完整业务world作为后续角色包恢复/重放输入保留。没有已证明无依赖的新缓存，不删后重建制造成果。见retention-result.json。
