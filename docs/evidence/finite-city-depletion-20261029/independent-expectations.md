# #21 Open 同存档第3/4日：有限供给与到期独立预期

2026-10-29，实际仓库 `/Users/frankqin/dev/MyCity`；读取 HEAD `8c1a997fbe2bb5b575f990d6c401198a6af98795` / v0.5.222。仅依源码与既有两日基线推导，未编译、运行测试或脚本，未读取未来运行输出。唯一写入本说明。root负责原存档连续运行、固定源及串行验收，本说明的结果均为独立预期，运行 `not_run`。

## 不变输入与两日基线

沿 `tests/fixtures/finite-city/transport_opening.rs::Scenario::Open`，不改期初存量、reference1000、商品价10、原walk allocation/7分每q运费、Payroll ends_at3DAY、资产或原M0规则。不启动新Pump/Generator，不续雇、不接新公服、不补库存。

day2=172800后的固定基线：公司7现金72495，raw15量/basis706，product16空，Boot电3002=68000/水3003=522000；Payroll21累计paid/accrued155、work_seconds56100、remainder3000；资产B28916、投运61200、life259200/residual27716、charged516；tax assessed440、retained441。day2新税160尚未支付，下一到期为3DAY。

## 源公式：电不足按数量上限部分生产

读 `transaction.rs::prepare_town_settlement`、`town_runtime.rs` 原utility binding、`town_economy.rs::{worker_output,split_labor_cost,plan_with_executions}`：

- 原Boot绑定先按 `stock.quantity / stock_quantity_per_utility_unit` 求可用utility整数单位。68000电 / 1000 = **68**单位；原recipe每q需1utility单位。
- Payroll第3日有完整28800实际工秒，潜在产量 `floor((100×28800+22800)/28800)=100`，output carry仍22800。
- 实际产量 `min(潜在100, raw容量706, utility容量68)=68`。源码只在output=0或现金不足等原门不满足时跳过，不要求整批100电量；因此此输入应**部分生产68**，不是Stock错误、全部拒绝或自动买电。
- 实际raw消耗68，utility费用68（7→3）、电消耗68000。rawbasis按不可变lot累计floor差，此lot原1000/basis1000，所以68份cost68。
- 当期真实工资80不会因缺电退回或少付。`split_labor_cost(total80,produced68,potential100)` 将 `floor(80×68/100)=54`资本化，余**26**为当期工资费用；不足1分的比例尾差也留费用端。
- 商品basis=raw68+utility68+capitalized_labor54=**190**。不能写成216（全工资入存货），也不能在销货basis190之外再减完整工资80，正确额外减idle_labor26。
- 只有潜在产量的余数22800留在原recipe游标；被utility限制的32q不变成现成存货、待交付或可在无工时第4日补产的权利。

原Town先生产后卖货。Open quote容量100，送达价17<reference1000且家庭现金充足，因此新68全部售出：商品款680→公司7，运费68×7=**476**由家庭2→provider1；家庭本次总扣1156。profile的4往返/100q是最大可达量，不保证卖100，也不直接扣固定648旅行费。

## 第3日：新增税32与资本末前一段

day3=259200，资产年龄259200−61200=198000秒；原累计折旧 `floor(1200×198000/259200)=916`，从516再计**400**，账面剩余28916−916=28000。缺电不自动停止已投运资产的发展钟。

日税原公式为 `margin=销售−sold_basis−idle_labor−资产当期费用`，本日 `680−190−26−400=64`。taxable64，50%税**32**，retained新增32，累计**473**，累计assessed**472**。这不是再扣完整工资80或把未售潜在32份认成损失。

公司7现金独立式：`72495−day2到期税160−工资80−utility68+商品款680=72867`。折旧400不扣现金，已资本化工资54不再付第二遍，家庭运费476不经过公司7。day3税32当日只生成4DAY到期Claim，未在本次日结付款。

第3日后：raw15量/basis **638/638**；product16量/basis **0/0**；电 **0**、水 **522000**。实际库存投入减少68，不能仍断言raw606或继续耗100000电。原剩余fuel、水、设施与道路容量不因电为0自动消失。

## 第4日：Payroll到期、无产销但资产仍完成折旧

原PayrollSpec.ends_at=3DAY=259200，不是到第4个工作日结束。`timeline.rs::Payroll::bill` 在at==ends_at时设置next_due=None和working=false；`accrue_with_work`最多计到ends_at，原payroll_members在结束后的班次不重新Begin。

- 第3日仍包含calendar day2的09:00–17:00完整8小时；累计工秒 **56100+28800=84900**，工资累计 **235**，numerator remainder仍 **3000**。准确恒等式 `235×3600+3000=10×84900`。
- ends_at当次按原Daily bill将billed=accrued235，并在现金足够时付清到235。既有现金完全够付，因此本路径day3/4期末 **accrued=billed=paid235**、arrears0、next_due=None、working=false。保留原行和原成员释放/工资回执，不删除账目。
- 第4日没有新的worked_seconds或工资；recipe选不到有新增工时的worker，所以不生产。原商品库存已售空，销量0、实际utility费用0、实际运费0。电仍0、水522000、raw638、product0。不能自动续Payroll，也不能靠recipe carry22800或可达需求100绕过工时/供给约束。
- 资产life终点是 **61200+259200=320400**，在第4日16:00，而不是3DAY。day4=345600累计折旧封顶**1200**，当期 **1200−916=284**，账面剩余 **27716**。继续保存原资本/残值/游标；寿命到不自动ZeroRecovery、停用、拆除或Close。
- day4 margin=0−0−0−284=**−284**，taxable0、日税0，**不新增零额Tax Claim**；累计retained=473−284=**189**，assessed仍472。
- 原Timeline先付day3到期税32，故公司7现金 **72867−32=72835**。没有工资/utility/运费或284折旧现金扣款。

## 固定连续结果表

| 项 | day2基线 | day3=259200 | day4=345600 |
| --- | ---: | ---: | ---: |
| 当期生产 / 销量q | 100 / 100 | **68 / 68** | **0 / 0** |
| 商品收入 / 家庭运费 | 1000 / 700 | **680 / 476** | **0 / 0** |
| 当期raw / utility费用 | 100 / 100 | **68 / 68** | **0 / 0** |
| 当期工资 / 资本化 / idle_labor | 80 / 80 / 0 | **80 / 54 / 26** | **0 / 0 / 0** |
| sold_basis | 280 | **190** | **0** |
| 折旧delta / cumulative | 400 / 516 | **400 / 916** | **284 / 1200** |
| margin / taxable_basis | 320 / 320 | **64 / 64** | **−284 / 0** |
| 新税 / 累计assessed | 160 / 440 | **32 / 472** | **0 / 472** |
| retained累计 | 441 | **473** | **189** |
| 公司7现金 | 72495 | **72867** | **72835** |
| raw15余量 / basis | 706 / 706 | **638 / 638** | **638 / 638** |
| product16余量 / basis | 0 / 0 | **0 / 0** | **0 / 0** |
| Boot电 / 水 | 68000 / 522000 | **0 / 522000** | **0 / 522000** |
| Payroll累计work / paid / remainder | 56100 / 155 / 3000 | **84900 / 235 / 3000** | **84900 / 235 / 3000** |

独立总量交叉式：共生产/售出262q，商品收入2620，家庭运费658+700+476=1834，raw总耗工程100+商品262=362，期末638。销货basis合263+280+190=733，工资资本化75+80+54=209，工资当期费用26，实际总工资235。四日累计税前 `2620−733−26−1200=661`，减累计税472得retained189。公司7四日现金 `100000−工程现金28816−工资235−生产utility262+商品款2620−已付税472=72835`。全世界现金仍3100000，运费/税与公司3收入分别沿原责任方守恒。

## 自然到期和仍须保留的责任

**税**：day2税160在3DAY支付，day3税32在4DAY支付；day1税280此前已在2DAY支付。day4结算后，既有三张税Claim合principal472，均paid=principal、processed=true；原source、各期assessment和tax cursor保留，不能因已付清删除或让replay重复付款。日税为每日正margin评税，day4负284不抵退前三日税或回写旧Claim。

**工资**：Payroll21的自然结束不是人工StopWork、续雇或解除历史欠付。源码始终维护paid≤billed≤accrued和整数余数恒等式；如果其他输入导致付款不足，应保留billed−paid，并沿原SettlePayroll等授权入口处理，不能因ends_at到期清零。本固定现金路径算出的欠付为0，不能以此宣称所有到期工资都无欠付。remainder3000是不到一分的原累计分子，不擅自四舍五入成另一整分应付、退款或新合同余额。

**房租**：原HousingLease901，starts_at0、Monthly anchor1、每周期1000、deposit0。游戏历epoch为2001-01-01；`Cycle::Monthly::next`和`housing::next_monthly_due`将首次due设为2001-02-01，即 **31DAY=2678400**。所以第3/4日没有此房租付款或新月账期，不从公司7现金扣房租。原tenant2→landlord1的1000首期Claim/住房占用和due保留；未到期不等于免租。原初次报价/授权valid_until400DAY也不是在第4日重新签约或自动换房的许可。

**公服**：原警方任务已于33900完成往返；延长Timeline不会自动再Accept、再次收2000、再耗130000电/10000水，或再次占用[0,1)。原wage条件valid_until100000早在day2前已到期；qualification authority/baseline valid_until200000在day2与day3之间到期，不能宣称续证或用旧事实派新警务。Payroll21依据自己的既有成员注册与ends_at，不凭这些警务资格到期抹掉其第3日工作权/应付工资。

原natural needs按 `phase=(stable_token+seed+31×kind_index) mod period`、发生日08:00，deadline为发生次日08:00；Medical period90日，Police/Fire period180日。给定本固定源Police day0发生并被Accept，可推出(token+107) mod180=149，故Medical初相59、Fire初相31，Police下一期180；这三个新周期都不在day3/4。已绑定的首期Police自然截止观察/完成记录仍保留；不把“没有新接单”改写成对所有未来需求已履约。若root发现固定登记身份/seed并非这一源，先重核该phase，不能强套本推导。

**Boot/资产/道路**：原Pump/Generator是有限已完成作业；仅Advance不会新发400000电。供给耗尽不删除其成本历史、剩余water/fuel或容量，不推导公司7破产/资产损失/ZeroRecovery。4往返walk allocation仍是原重复日需求可达上限，day4无商品销售不产生旅行收费；不是运行时道路关闭，也不说明警方graph同源。维持原资产持续营业和必要日结责任，未执行retirement/拆除/Close。

## 待 root 实际验证，不能代替的证据

应从原同存档按顺序完成day3/day4的Timeline与transport日结，分别检查partial Produce68、capitalized54/idle26、真实Boot出口数量、3DAY工资bill结束、原税source和4DAY零税无新Claim。日结重票、WAL/checkpoint/normal角色重启必须保持上述量和责任一致；旧两日回归不能当新四日验收。

本说明未测host默认栈、角色/原M0预算或完整全域义务。其他主体（公司3原公服净收入/现金、全自然需求归档）未单独给完整数值；没有为其填默认成功或续办。交付后冻结，root唯一负责实施及串行运行。
