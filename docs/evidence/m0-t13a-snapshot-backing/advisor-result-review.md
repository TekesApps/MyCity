# 实际失败后的advisor独立审查

boot_advisor只读检查当前代码和failure-trace.json，未执行负载或修改文件。确认第64页还款申请失败，不能用总free大于1MiB证明连续空间，也不能宣称已有COW/缓冲修复解决了重复快照。

先提出常驻typed工作区候选，并按Native29,625,968 + W64 1,920,000 + W128 20,480,000 + W256 6,144,000 + PathRefs 3,600,000 = 61,769,968B计算；加原6MiB泛化余量超过64MiB约952,560B，若采用必须重新闭合控制/stack/已收费I/O实际重叠，不能少留COW页或添加unsafe。

随后对具体报告逃逸独立确认：serde_json::to_value(native_restore_bound)在loan64后分配Value、对象节点和字段名，作为restore成功值跨repay64存活；RestoreBound本身全整数不需堆。应优先用闭包返回RestoreBound，结束候选/临时变量作用域，完成原repay64后才转Value/构造最终报告。外部预先存在entries不属于此项新逃逸，错误分支String仍可能跨还款存活，不能宣称所有路径不碎片化。

同意先以原正式负载验证这项最小生命周期修正，不能因为已有大型typed方案就跳过具体小逃逸；即使通过第二及后续快照，也不推导全局分配器永不碎片化。
