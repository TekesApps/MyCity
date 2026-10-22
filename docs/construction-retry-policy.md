# 工程请求重试与旧WAL语义

本策略的construction retry目标路径已实测：20号输出中的construction retry policy suite为3/3通过，覆盖旧策略保留失败receipt、V2容量失败释放后新Envelope ID重试、资金保护释放后新ID重试。20号同一运行随后utility recovery suite 0/2失败（`exchange_spatial_bundle_required`）；该后段失败保留，不影响前一suite单独结果。23号core/storage全目标回归早于道路genesis修复；71号epoch测试3/3，72号修复后最终core/storage all-target release回归exit 0。原Engine为已处理请求保存成功或失败回执；相同operation ID必须返回原结果，不能因后来补钱或补料改变历史。

`Project.execution_policy`将这项行为明确绑定在期初配置：

- 缺省为`legacy_v1`，省略序列化，保留旧存档字节和旧WAL执行结果。旧效果ID限制、道路退出拒绝和旧发布类型约束不被重新解释。
- `Project::new`产生`recoverable_v2`。内部effect序列与外部Envelope ID分开；内部effect仍由Project生成并经精确receipt确认，调用者不能提交自报成功。所有权限、当前revision、事务原子性和Envelope幂等继续由原Engine/Store负责。

V2下，旧失败ID永远返回原失败。条件改善后必须使用新Envelope ID与当前revision重新请求同一内部effect；成功ID重放不二次占地、施工、扣料或收费。相同ID改payload仍是冲突，不能把原请求变成另一操作。普通Actor不能越过原background/inbox保留ID规则。

V2同时启用本轮道路退出、Retrofit发布及一次性废物接收修正；未投运清场仍需明确期初规格和参与方审批。Legacy只为旧数据兼容，不能据此宣称旧世界已升级到新退出机制。本阶段不自动迁移旧世界；迁移需有明确检查点及规则切换方案，不能就地改旧负回执。旧reader/writer来源、失败收据和完整恢复输入继续保留。

20号构造重试suite实测3/3通过，覆盖容量与资金条件恢复后使用新Envelope ID重试及legacy missing policy保留失败回执；同轮后续utility suite 0/2失败保留。23号全命令回归早于道路genesis修复。69号误用了旧共享缓存而编译失败；70只清理mycity-core release缓存；71号新增road-epoch测试3/3通过；72号最终core/storage all-target release测试exit 0、source_unchanged=true。旧writer兼容另外由68号实际冻结旧writer产生并seal的accepted WAL、accepted checkpoint和历史v119 transport world验证：新reader在完整副本上的三例均exit 0，原件未变。22号utility recovery suite 2/2通过。结构尺寸和共同工作区上限不提高，以上兼容性结果不代表64MiB、性能或16GB通过。完整实际状态入口为[core-road-town-integration-122](evidence/core-road-town-integration-122/STATUS.md)。

材料恢复反例的源码位于`crates/city-storage/tests/construction_material_retry.rs`。19号该用例1/1通过，验证恢复不创建库存或资金来源、批次身份和成本基础不变；新请求施工消耗1,000数量刻度和400成本单位，并向承包商支付28,800现金，工程累计成本为29,200。旧失败回执保持失败，新成功回执不重复扣料收费，WAL及检查点冷恢复均在用例内核对；23号和72号all-target回归也覆盖此用例。19号另一个construction retry policy用例失败，随后20号该policy suite 3/3通过；单项结果与整轮失败历史分别保留。

V2普通建材不能直接引用Boot治理的电、水、燃料等库存：期初装配以`construction_governed_stock`拒绝，恢复校验以`checkpoint_construction_governed_stock`拒绝，Work另保留Unauthorized防线。23号与72号中相应governed-stock/genesis和checkpoint恢复测试分别通过；Legacy沿用原执行及守恒失败结果，不静默升级。该隔离防止把专用库存扣减误当已完成的施工公用供应；这些拒绝测试不证明施工水电断供已经接入。真正接入仍需明确施工供应绑定、计量分类、容量和同事务付款，不能借用Town绑定冒充工程供应。
