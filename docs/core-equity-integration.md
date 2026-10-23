# 股权、经营留存与退出集成边界

状态：接线设计建议，尚未实现、集成或测试。本文依据当前T08 `TownSettle`、库存成本字段、Timeline工资与T08-C企业退出候选实现整理。不能据此宣称已有股权、利润分红、税核算或企业清算剩余分配能力。

## 1. 唯一账本和提交边界

股权持仓登记可以作为World/TownState的有界持久状态，但现金、库存成本、工资、债务和履约仍以已有Engine/Timeline/Store为权威。股权模块只处理持股权与整数金额的分摊，不建立平行总账，也不从调用方接收一个任意的“净利润”。

`TownSettle`在事务候选中实际验证并应用 `SettlementBatch.lines`、Timeline工资差额、账户转移和库存变更。未来的留存收益增量必须在这个同一候选事务内、由成功应用的真实行构造，再与账户和库存状态一并提交。失败回滚时留存收益也回滚。现有Store按操作ID处理重放；相同命令重放应返回原结果，不能再次增加留存或派息。

现有 `MarketSummary` 是World范围的单日汇总，只留下最后一次结算，不保留企业归属、实际售货成本或逐期工资差额。因此不能用它重建完整利润，也不能把它当作历史收益累计器。不得从规划中的 `SettlementBatch` 预先记账；只有事务中通过校验并实际执行的行才可影响留存。

## 2. T08实际字段能说明什么

### 已有权威来源

| 事项 | 当前可用数据 | 对账含义 |
| --- | --- | --- |
| 商品收入 | 成功应用的 `PlanLine::Sale.goods_payment`、`shop_account` | 该款从买方实际转到店主账户时，才是该店主的商品收入。买方支付的运费、电力附加额属于各自收款账户，不能计作店主商品收入。 |
| 已售库存成本 | 同一 `Sale.goods_basis` | 仅对真实售出的数量结转成本；调用时检查权威库存切片。 |
| 库存成本池 | Stock/InventorySnapshot 的 `quantity`、`basis`、`origin_quantity`、`origin_basis`、`offset` | `stock_slice_basis`按累计整数floor差额切片，拆单、恢复和连续售出保持总成本守恒。剩余库存的 `basis` 仍为资产成本，不进入本期已售成本。 |
| 生产后库存成本 | 成功应用的 `PlanLine::Produce.output_basis` 及对应最终Stock字段 | 现行公式是 `input_basis + utility_cost + capitalized_labor + freight_cost + disposal_cost`。上述投入成为该批库存成本，不能同时当期再扣一次费用。 |
| 已发生工资 | Timeline `Payroll.accrued` 相对持久 `PayrollCursor.accrued` 的差额 | 作为工资应计成本；不以实际付了多少代替已发生工资。未付金额仍是应付工资债务。 |
| 实际工资现金流 | Timeline `Payroll.paid` 差额与Timeline账户转移 | 只用于现金流和应付核对，不再次增加工资成本。 |
| 当期闲置工资 | MarketSummary中的 `idle_labor_expense = wages_accrued - labor_basis` | 当前生产规划把部分工资按产量资本化进 `output_basis`；不能将全部工资再作为当期费用。闲置部分已经是未资本化工资费用。 |
| 已发生的直接生产付款 | Produce行中的 utility、freight、disposal金额及收付账户 | 对生产企业，这些付款已经放入output_basis，待该库存售出时随goods_basis成为成本。对服务提供方，它们是现金收入，但提供方自己的供给、维护和固定成本尚未完整建模。 |

### 有限、无重复的商品经营收益口径

在某一完整封账区间、所有输入均可信且覆盖完整时，T08商品经营净额候选为：

```text
实际商品销售款
- 实际售出库存的 goods_basis
- 未资本化工资 idle_labor_expense
- 已实现、且有权威记账来源的其它期间费用和适用税
```

`goods_basis`已包括此前资本化到所售批次的原料、生产资源、生产物流/处置和资本化工资成本。未售商品相关成本继续留在库存资产。生产行的utility/freight/disposal以及 `labor_basis` 不可在本期重复扣除。

销售订单的freight与utility附加款直接由买方付给对应账户；它们不是店主的商品销售收入。若接收方是独立运输、电力或水务公司，这些收入必须与其自身真实供给、燃料/采购、工资、维护、税及其他成本一起核算。当前公共服务主体成本未闭合，不能只拿其收款作为可分利润。

## 3. 工资、税、利润及现金不可混为一项

Timeline工资在推进时按到期规则尝试付款。对利润应计使用 `accrued`，对现金变化使用 `paid`；若余额不足，未付款部分既不能消失，也不能算成已付现金。生产工资的资本化部分通过库存成本，在货品实际售出时归入goods_basis；闲置或未资本化工资由idle_labor_expense当期承担。任何集成都必须在“工资全额当期费用”与“部分资本化后随销售结转”的口径中选定与当前库存basis一致的一套，不能二者叠加。

现行TownSettle没有实际税种、税基、税额和缴纳记录。T08-C ExitPolicy的 `tax_assessment_complete` 只是固定profile里的完整性声明，不是税务机关算出的事实，不能用作分红许可凭证。税规则、税额计算和权威付款进入账本以前，相关企业的税覆盖为未知，分红应拒绝。

当前系统也没有覆盖折旧、资产减值、租赁、法律费用、全部长期维护或所有其他生产成本的企业总账。只要该企业存在这些未入账项目，销售收入减已知T08成本不是净利润，不能称可分配收益。首版只能为明确声明完整的狭窄主体开通派息；完整性由Engine核对权威来源，不能由玩家或profile布尔值自行断言。

## 4. 最小安全留存收益与派息门

若开始有限实现，建议在既有持久Town/World状态中为有资格的公司保留有界、版本化的累计账务游标与签名金额（能够表示负数/亏损），而不是另建可独立写入的利润账。每次成功TownSettle按公司和权威交易行累计一次；游标与Store操作ID、世界修订及封账区间一致。需要拒绝重复、倒退、缺日、未封账、库存basis不匹配或覆盖缺失的结果。

派息上限应为以下值的最小值，且任一输入未知时为零：

```text
可分留存 = max(0,累计已实现净收益 - 历史累计亏损 - 已声明未付股息)
可用现金 = 现金 - protected - 未清债权/工资/税 - 已承诺的最低经营/安全资金
可派金额 = min(可分留存, 可用现金, 通过偿债能力与来源限制后的允许金额)
```

分红不是仅由现金余额决定。工资和Timeline Claims由Timeline原记录提供；未到期债务也应计入保护，不能因为Exit planner暂时只支付到期款就用于派息。当前C的 `build_exit_snapshot` 能从Timeline构造实际应付Claims/Payroll arrears，并带入公司的cash/protected；这适合作为债权输入源。其它未列入Timeline或未绑定的债权、税、损失或最低维护要求须阻断派息。

一次合法声明须在一个Engine事务内同时：核对当前完整账期和股东名册；冻结本次名册/份额和总额；扣减可分留存、建立各股东应付；以确定的最大余数和稳定身份ID规则分配厘；然后执行公司账户到股东账户的真实转账。任何一项失败，整个候选回滚。重复operation_id由Store返回原receipt，不重复扣现金或增加应付。转账后要核对公司余额不低于受保护现金、债权和最低运营预算，股东收款总和严格等于声明总额。

## 5. 初始资本、外来现金和股份

现有Assembly来源只有 `CashCapital` 与 `InventoryCapital`，没有资金是否为不可分补助、财政拨款、贷款或其他用途受限款项的字段；运行时一般账户余额也不按来源切片。`protected`是账户级金额下限，不是资金来源证明。现阶段不能把任意现金转入视为可分收入，也不能用余额扣掉protected后就得出资金可自由分配。

有限首版只能对已完整核对的出资/现金流开放派息。未分类转入、借款、附用途限制的拨款、尚未核准的补助、无法追溯的旧开局现金或其他未知来源，应标记为不可分/未知，或者让公司退出派息资格；不能将它们并入留存收益。股东出资在同一个Engine candidate中必须真实扣投资方账户、真实加公司账户、更新股权记录。出资增加实缴资本，不是营业收入，不得补正累计经营亏损或产生分红额。

初始权益当前没有进Opening或World登记：现有CashCapital没有投资方身份和股数。既有公司必须经过真实初始化股权程序，不能从账户余额反推谁持股。通用Transfer action若用于注资，必须与增股以及Store exactly-once一体提交；普通转账receipt只提供操作追溯标识，不会独自证明资金已经扣入/加到账户。

当前股权模块把 `commit_paid_contribution` 设为crate-private，`PaidContribution`也仅crate-private，所以外部CLI/应用无法直接调用它；但两个bool参数仍可被crate内部任何模块伪传为true。Engine集成建议删除这两个bool，改由 `transaction.rs` 账户candidate成功转账后构造一个不实现Deserialize、字段私有、外部无构造器的 `CommittedTransferProof`，携带operation ID、来源账户、收款账户和实付金额。股权更新消费这个proof并校验收款企业、出资股东、金额与发行价相符。Store原operation journal处理exactly-once；proof和receipt都只是绑定已发生资金变化的内部证据，不代替现金转账。

## 6. 企业退出与清算剩余分配

目前T08-C退出路径在Engine候选中读取真实现金/protected、Timeline Claims、工资和公司的实际库存切片。Exit planner先按优先级处理债权；不允许债务/未来债务未清、库存未处置或核算/法律覆盖不完整时分配股东剩余。

当前C候选尚未接股权登记：`build_exit_snapshot` 固定设置 `shareholders_complete=false` 并传空股东表。Planner因此会对正余额阻断，只有残值为零的关闭路径可能继续。即使planner产生 `DistributeResidual`，当前SettleExit Engine loop只执行 `SettleLiability`，没有执行股东付款；CloseExit重算计划时要求除stop/finalize外没有其它行动，并要求cash和protected都为零。故正余额清算分配尚未实现；存在未释放protected余额时也没有关闭路径。

接线必须以持久EquityRegister为唯一股东来源，并在清算候选中验证版本、完整性、股东账户关系和合计股份。债权行动执行并更新Timeline后，才允许把债清后的真实可用余额交给分摊器。`DistributeResidual`需与公司扣款、股东入账和关闭状态在同一候选提交，并经过Store operation ID去重。资金不足/保护余额冲突时不能超付或先付股东；有未支持负债、未核税、未核资产或未完成法定流程时停止清算。

股东尾差采用最大余数法，并以稳定 `EquityParty` ID打破并列；行按party排序后应用。当前C exit planner的残余分配是先floor、再按输入行顺序分配余数，并非最大余数，接股权后应共用一个算法，避免两种分红口径不一致。

## 7. 当前未完成边界

- 没有Engine持久股权登记、出资/增资action或账户事务接线。
- 没有从TownSettle实际交易行累计且持久化的企业留存收益；当前MarketSummary不足以重建。
- 没有真实企业税评估/记账；ExitPolicy布尔声明不能替代。
- 没有覆盖电力、运费及其他公共服务主体的完整收入成本。
- 没有外来受限现金来源标记，也无法从混合余额判断哪些钱不可分。
- 没有C清算股东注册输入或执行残余分配；正的protected残值处理也未定义。
- 此文档仅为只读集成方案，未运行编译、测试、恢复验证或经济平衡验证。
