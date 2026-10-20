# T13A-3d：真实worker的固定槽所有者

测量前冻结：原full期初、acceptance、720小时／723目标／724窗口／3011票、8192页及空气60秒步长不变。仅Rust模型／CLI。本次实际full单独30日，不冒充正式L1联合／稳态／8h。实际输入和候选只由两个固定2MiB槽承载，合计4MiB在整个Owner生命周期实际触页；这是局部真实工作区，不是跨进程64MiB总账。

两槽初始完整填充；借用只转所有权标签，不释放／重新分配，不产生realloc双份。回收后重新填充全槽，报告前全扫描核验；仅有效前缀参加输入校验、候选hash和磁盘发布。限定闭包不能返回存储所有权或切片；票据绑定process／owner instance、run epoch、job／generation／attempt及序号。错误票、重复归还、错误／panic均有反例，panic不完整trace不能当成功报告。constructor分配失败使任务失败，尚无全局额度返还承诺。

core checkpoint_to改为两遍借用序列化，第一遍规范payload hash、第二遍原字段顺序编码，保留8MiB总字节上限。原checkpoint委托同codec，restore hash流式计算。工作buffer边界是2MiB，原State／serde／IPC不包含在四MiB槽内。idempotent既有发布使用固定64KiB栈hash校验，保留原6MiB磁盘缓存与持久发布规则。栈开销另报，诊断metadata计入worker原1MiB限额。

先验证旧／新codec逐字节兼容、部分writer／错误输出、容量边界／超一字节、错误／旧票与真实重复任务既有候选路径；全仓回归含compile_fail必须实际结束。冻结来源后actualfull30日，独立核验全部724个worker的4条转换／4MiB余额、726报告与727退出、所有原票与完整Engine；与v42原worker逐任务输入／候选hash及完整CPU期末对比。advisor独立终审后提交／递增annotated tag／推送。其余64MiB所有者、诊断总账、四档与正式长时门仍按原依赖推进，不用本地槽冒充全局通过。
