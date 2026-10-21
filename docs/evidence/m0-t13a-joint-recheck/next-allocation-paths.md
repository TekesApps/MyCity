# 下一实际分配缺口（只读定位，尚未实现）

advisor /root/boot_advisor沿已冻结e6dd来源定位；本页不是额度签收，也不改当前实验。

- 常驻Store/Engine容器与池未借出填充并存，仍EXTRA。需覆盖实际容量/增长，不发估算固定总额冒充控制。
- city-core/transaction.rs::prepare_diagnosed复制Timeline、Background、resources、leases，原Engine与Prepared同时存活，20k Claims和场数据不能漏记；报价/实际应用先后阶段不直接相加。
- city-core/background.rs::frozen_input的continue_from_boundary复制State后还checkpoint Vec；capture/Accept同样checkpoint，Accept还持有CPU reference、候选，最后复制为新field。
- city-storage/background.rs::import_candidate_diagnosed保留原Engine、解码候选、quote Prepared/CPU reference/编码，随后实际Accept再取候选。外层4MiB SimCodec不控制内部State或Vec。
- city-headless/scale_model_task.rs Compare同时恢复actual/expected两个Engine再生成两份export_checkpoint，business/air比较又编码；reference临时同样EXTRA。
- worker4MiB也只控制input/candidate字节槽，State/计算数组不在该4MiB中。

下一修复先用有界流式hash替代frozen_input/capture/Accept隐藏编码Vec，保持相同canonical字节、旧hash和CPU比对；helper双编码比较可流式化，但actual/expected Engine仍需真实预准入/固定容量解码和克隆，常驻Store另独立闭合后合并同刻总账。不得用历史峰相减，不提高原64MiB、8MiB或缩full负载。

另有5fd8957/v0.5.60的《属性驱动的概率机制通则》待代码对齐：作为跨层统计倾向的共享确定性抽样/参数/属性查询工具，领域选择后果，不限定医院/投资，不强迫所有计算随机。确定义务、旧profile和交易排序保留；实现要显式版本、窗口/身份、unknown/硬门、持久去重和有界预算。当前实验期间不接入新机制，不能让新增随机改变已冻结oracle。
