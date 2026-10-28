# #21 原 map walk allocation 消费：独立两日预期

2026-10-29，基线 v0.5.220，实际仓库 `/Users/frankqin/dev/MyCity`。仅阅读业务源码和冻结输入推导，没有运行编译、测试或脚本，没有抄运行输出。唯一交付为本说明，所有新场景运行验收 `not_run`。

## 固定输入与可声称的边界

源为 `tests/fixtures/finite-city/transport_opening.rs`，复用 `asset_opening.rs` 和原 Payroll/警方真实往返/Boot/Town场景。三种 Scenario 是**分别创建的初始道路输入**，不是同一世界运行时关路、改门或恢复通行。原警方图未改，也不能声称警方图和此walk adapter同源。此空间building入口的owner2映射不新增住房71的产权/场址认证。

资产owner7、公司7现金100000，真实工程资本28916（工资28800+stock15材料100+电水服务费16），投运61200，life3DAY/residual27716，whole量1000。Payroll21首日工作27300秒/工资75/remainder3000，次日28800秒/新增80，累计56100秒/155。警方08:00–09:25占用原成员，工程contractor4不冒充该成员。资产、Payroll、警方图不会因walk道路关闭自动停用。

walk profile：OD101从building入口11→商家入口12，OD102反向；每向4人，partition最多4往返×25q=100q，provider1每travel_second1分。每日RepeatFixedDailyDemand，同一个固定初始profile每日重用原allocation契约，不发明全球共享或跨市场的额外运力。

家庭2 reference_price_per_q改为1000，是**显式意愿输入**，不是资金/商品/道路增资。商家7商品价仍10；原初始现金、库存和股本均不变。所有金额为分，q沿Town既有Quantity整数坐标，不能自行换算成另一1000倍收费口径。

## 几何、拥堵与运费的手算

读取 `city-map/src/{geometry.rs,traffic_adapter.rs}`、`city-core/src/{transport.rs,market_access.rs,town_economy.rs}`：

1. connector为(3000,2000)→(3000,1000)→(5000,1000)，长度3000cm，速度100cm/s，free_flow30s。
2. road二次曲线为(1000,1000)、控制(3000,400)、终点(5000,1000)。源码细分判据为L1误差≤16cm（坐标使用SCALE）；初始1200、每层/4，depth3误差18.75仍需拆，depth4误差4.6875，得到16段。量化点每段dx250，前半dy绝对值70/61/52/42/33/23/14/5，后半镜像。
3. 每段长度取ceil(sqrt(dx²+dy²))：前半260/258/256/254/253/252/251/251，合2035；总保守离散长度4070cm。adapter `ceil(4070/100)` 得road free_flow41s。以上为源算法下的独立手算，不是原回归或执行证明。
4. Open两OD使用方向相反的独立边：101路径21+24，102路径23+22；各边流4/容量4。`transport::congested_cost`在flow/cap=1时为 `floor(free×1.15)`，故connector34、road47，每向mean_travel81s。
5. allocation取min(out served4,return served4,trip_count4)=4往返，accessible_quantity=4×25=100。`market_access::resolve_delivery_quotes` 把总旅行费 `(81+81)×1×4=648` 转成 `ceil(648/100)=7` 分/q。

实际市场运费是 **7×实际销量**，不是直接扣648，也不是按未售q扣700。首日94q运费658、次日100q运费700，合1358；取整后的商品费率不会变成另一笔独立648付款。provider1由家庭2支付，公司7仅收商品款，不把该运费记进公司7销货成本或再扣公司7现金。

Open送达价10+7=17，原utility_surcharge0。`demand_at_price` 为 `min(100,floor(100×1000/17))=100`；quote容量100，因此首日销量受已产94限制、次日100。家庭初始200000，原一次Police费用2000及原住房/工资路径维持，新增两日市场扣款1598+1700=3298不会构成此fixture现金瓶颈；实际仍由原protected/spendable检查执行，reference1000本身不加钱。

## 两种关闭输入的方向结果

| 初始场景 | outbound101 | return102 | round trips / accessible q | access status | 市场运费 |
| --- | --- | --- | --- | --- | --- |
| Open | served4 | served4 | 4 / 100 | Reachable | 7/实际售q |
| RoadClosed | served0 | served0 | 0 / 0 | NoOutboundService | 0 |
| DoorCannotEnter | served4（door仍可出） | served0（不能进入building） | 0 / 0 | NoReturnService | 0 |

DoorCannotEnter不是“单向可达就能卖”：constructor只保留door→anchor边21，不产生anchor→door边22，往返取两向最小值。对accessible0，resolved quote.reachable=false/capacity0/freight_per_q0；不收去程已分配人数的旅行费。具体unserved reason按原transport结果断言，不把NoReturnService改称RoadClosed。

## 关键生产顺序：关路仍先生产，但第二日不继续造货

`town_economy::plan_with_executions`先生产，再依据reachable quote找买家。`transaction::prepare_town_settlement`在配置recipe时遇到原output stock.quantity!=0直接跳过该recipe。两者必须合读：

- 首日三个场景均可从原料/Boot/真实工资生产94。stock15先被工程耗100，再为生产耗94；商品stock16产生quantity94/basis263（原料94+utility94+工资75）。这笔工资只是原Timeline的已发生应计/支付，不另付一次。
- Open首日卖光94，stock16清空，次日能再生产100，商品basis280（100+100+80）并全部卖出。
- 两关闭场景首日卖不出，stock16保留94/basis263。次日**因output非空跳过生产**，不再耗原料、电或utility现金；仍实际发生工资80，未被任何Produce资本化，形成idle_labor80。首日75保留在未售存货basis，不能又当首日闲置工资减一次。
- 因而不能把关闭场景写成两天都产100、都产94，或道路关闭自动停Payroll/停资产折旧。单纯修改reference意愿也无法绕过不可达门。

## 固定两日日结 oracle

资产三个场景均持续投运：day1累计折旧116，day2累计516（当期400），原basis28916不变。期末账面剩余28800/28400。资产折旧不另付现金；材料工程basis100不另扣现金。日税使用 `margin=sales−sold_basis−idle_labor−asset_delta`，税 `floor(max(margin,0)×5000/10000)`；retained累计加margin−本期税，无跨期退款或虚构负税。

### Open

| 项目 | day1 | day2 |
| --- | ---: | ---: |
| 生产/销售q | 94 / 94 | 100 / 100 |
| 公司7商品收入 | 940 | 1000 |
| 家庭2→provider1运费 | 658 | 700 |
| 家庭市场总扣款 | 1598 | 1700 |
| sold_basis / idle_labor | 263 / 0 | 280 / 0 |
| 折旧delta / cumulative | 116 / 116 | 400 / 516 |
| taxable_basis / 新税债 | 561 / 280 | 320 / 160 |
| retained增量 / 累计 | 281 / 281 | 160 / 441 |
| 公司7现金 | 71955 | 72495 |
| raw stock15量/basis | 806 / 806 | 706 / 706 |
| Product stock16量/basis | 0 / 0 | 0 / 0 |
| Boot电3002 / 水3003 | 168000 / 522000 | 68000 / 522000 |

公司7现金：`100000−28800−16−75−94+940=71955`；第二日 `71955−首日税280−80−100+1000=72495`。day2新税160仍未付；day1税280由次日Timeline付。provider1运费1358直接来自家庭2，未流经公司7。只比较公司7现金会漏掉这条真实新付款，应另断言运费收款来源/家庭扣款及world总现金3100000守恒。

### RoadClosed 与 DoorCannotEnter（业务数值相同，拒绝原因不同）

| 项目 | day1 | day2 |
| --- | ---: | ---: |
| 生产/销售q | 94 / 0 | 0 / 0 |
| 公司7商品收入 / 实际运费 | 0 / 0 | 0 / 0 |
| sold_basis / idle_labor | 0 / 0 | 0 / 80 |
| 折旧delta / cumulative | 116 / 116 | 400 / 516 |
| margin / taxable_basis | −116 / 0 | −480 / 0 |
| 新税债 / 累计assessed | 0 / 0 | 0 / 0 |
| retained增量 / 累计 | −116 / −116 | −480 / −596 |
| 公司7现金 | 71015 | 70935 |
| raw stock15量/basis | 806 / 806 | 806 / 806 |
| Product stock16量/basis | 94 / 263 | 94 / 263 |
| Boot电3002 / 水3003 | 168000 / 522000 | 168000 / 522000 |

公司7现金：首日 `100000−28800−16−75−94=71015`；次日只新增原Payroll付款80，`71015−80=70935`。没有首日税债，因此次日也没有该税付款。两个零税期间不应新增零额Tax Claim；负retained必须保留。商品263仍是资产而非首日销货费用；第二日80为期间工资费用。

Boot核对：Pump后水600000，Generator耗水60000/产电400000，警方耗电130000/水10000，工程耗电水各8000。首日Town再耗电94000。关闭场景第二日不生产，不再耗100000；故电168000而非Open的68000。水均保持522000。raw806同理，不能再无故扣100变706。原Payroll两日累计155/56100秒仍成立，分红/其他成本完整性门不因零税或运输proof通过而解除。

## root 后续实际验收入口与不变量

1. 必须原Store加载每个固定allocation profile并核对hash，用原Transport市场日结入口；不能用无proof的普通TownSettle绕过 bound transport。每日service_day/RepeatFixedDailyDemand、商家7/家庭2入口及provider1身份均应从原proof消费。
2. 首日三个场景都断言真实投运资本28916、商品实际生产94、Payroll75/成员互斥及Boot同步；关闭场景第二日断言未生产和idle80，不能只断言销量0。
3. Open独立断言报价7/q、日运费658/700、实际商品价10与day税280/160。两个关闭场景分别断言方向服务、0 roundtrip/0容量，而非把两个理由合为同一“路坏”。
4. 原票重试/Store恢复须同时保持运输proof消费、日结游标、现金/商品basis/Boot exports和资产折旧；组件/旧空间回归不等于该新共同场景已验收。parent/root负责冻结源和串行验证。
5. 本阶段不宣称实际运行时改路、警方图同源、货运车辆逐趟物理运动、单行道路工程、几何场址到整个企业产能门控、全域税完整性、无限供给或native16GB/M0验收。

交付冻结：仅本说明；所有表格为源码契约下的独立预期，实际新场景结果 `not_run`。
