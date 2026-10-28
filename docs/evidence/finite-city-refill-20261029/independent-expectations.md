# #21 已消费电库存后的原 Boot Generator 批次重开

2026-10-29；实际仓库 `/Users/frankqin/dev/MyCity`。仅按 `transport_opening::Scenario::Open`、原 `asset_opening`、`boot.rs`、原Town/Payroll/税及Boot ledger实现推导，未编译、测试或运行脚本，未看未来分支输出。唯一写入本说明。旧四日耗尽对照保留，本分支全部新验收 `not_run`。

本分支唯一新动作：在2DAY=172800原Town日结完成后，实际 Start Generator，推进到172860完成，再沿原第3/4日日结。没有改任何期初存量、现金、资产政策、Payroll ends_at3DAY、需求、M0或接入#144。是一次真实有限批次重开，不是连续电厂、自动补货或长期自给。

## 基线与确定性来源

day2原业务基线：公司7现金72495/raw15量basis706/product16空；Boot电3002=68000，水3003=522000；Payroll累计155分/56100秒/rem3000；资产basis28916、charged516；日税累计440、retained441。

`boot.rs::recipe(Generator)` 为fuel1000、cooling water60000、fee300，时长 `SECONDS=60`；成功 `output` 为electricity400000、waste_water60000、heat600000。完整job.cost只加到成功电产物basis，废水/热不另分摊该cost。`command(Start)`先核真实投入及成功/失败出口容量，claim两个原资源、付款并移动投入至WIP；`finish`到期后才put_pool产物并释放资源。前一批成功后phase Idle允许再Start，不需要ClearFailure。

stock3001原fuel5000/basis2000，首次Generator已耗1000/basis400，所以2DAY前fuel4000/basis1600，原origin_quantity5000/origin_basis2000/offset1000。第二次仍走 `take(..., frozen=true)` 的同原坐标：再耗1000成本400，剩 **quantity3000/basis1200/origin5000,2000/offset2000**。

## 冷却水实际basis与WIP成本

水输入用 `boot.rs::take(..., frozen=false)`，实际成本为 `floor(current_basis×60000/522000)`，不是用tariff、公司7残值或期初水价替代。可独立确定此次恰为**30**分：

1. 首次Pump费用200+原电basis100=300，全部进入水600000；首次Generator取水60000，其成本30，留下水540000/basis270。
2. 原警方随后耗水10000，逐次pool floor累计成本介于0和5，余530000水/basis265..270。随后工程一次取水8000，成本 `floor((265..270)×8000/530000)=4`；所以2DAY水basis **W∈[261,266]**。Town只用电，不再消费该水。
3. 新批取水成本 `floor(W×60000/522000)=floor(10W/87)=30` 对整个范围均成立。故新job成本 **fee300+fuel400+cooling30=730**，不需要猜W的精确值。

原水细分扣费的精确W以及电精确E仍见下节有限未知；以上范围只用于证明新冷却水cost30，不把范围选点冒充已验值。

## Start与60秒完工的原子边界

Start完成、尚未finish时（172800）：

- 原supplier/operator3→contractor4实际现金**300**；公司7现金仍72495，工程资本仍28916，retained仍441。Boot fees原500→**800**。
- fuel stock3001为3000/basis1200（原冻结坐标不重置）；水stock3003由522000→**462000**，basis W→W−30，非fuel pool重新置origin_quantity462000/origin_basis W−30/offset0。
- 电stock3002仍**68000**，不能预支将来400000；无新电basis、无新出口。WIP inputs=[1000,0,60000,0,0,0]、cost730、phase Running、started172800/due172860，持有原Boot3101/3102资源。
- 原Pump完成1次、Generator完成1次。按现有开局无别的批次时next_job为3，此次identity3；具体token值由原资源writer给出，不手造。
- fuel_used在finish才计入，因此Start时仍1000；不能因fuel库存已减就漏掉WIP中的1000或提前把输出当可售电。

172860成功finish后：

- 原电同stock3002从68000→**468000**；只在此时增加400000及job.cost730。不新增stock/source或把同名视作完成认证。
- 水仍462000；waste_water3005新增60000，原60000→**120000**；heat3006新增600000，原700000→**1300000**。fuel_used1000→**2000**，fuel库存3000。
- Generator completed1→2，phase Idle、inputs/cost清零、tokens归零并释放原资源；Pump不变。无第二次付款，failed_expense仍0。没有承诺第三个批次或自动续发电。

## 电pool的basis与坐标：确定公式及有限未知

令**E**为本分支启动前原day2 stock3002的实际basis，**W**为同一原day2 stock3003 basis；二者不是可选政策参数，也不从未来补货输出回填。原Boot非fuel `take` 每次按**当时pool**比例向下取整并重新置origin/offset，警方原consume还会分成实际Timeline页面；因此仅凭电68000/水522000不能唯一反推出E/W。禁止把E写成 `floor(730×68000/400000)` 冒充独立结果。

这些数值如需完全数字化，root应固定原day2已验checkpoint里的输入事实E/W或完整原消耗分段，再逐步应用源floor公式。读取既有day2事实与采用未来重开结果不同；本说明未读取该快照，也没有猜数。**新批成本730、所有数量以及公司7业务结果不依赖E精确值。**

`boot.rs::put_pool`授权在此有限pool追加时重设原坐标（不是沿用Town商品lot的消费offset）：

| 电stock3002阶段 | quantity / basis | origin_quantity / origin_basis | offset |
| --- | --- | --- | ---: |
| day2、Start后、finish前 | 68000 / E | 68000 / E | 0 |
| finish后 | **468000 / (E+730)** | **468000 / (E+730)** | **0** |
| day3消耗100000后 | **368000 / (E+730−K)** | **368000 / (E+730−K)** | **0** |
| day4 | 同上 | 同上 | 0 |

其中 `K=floor((E+730)×100000/468000)` 是day3真实电出口basis。保留先前出口历史，不用新basis改算旧94/100单位商品utility，更不能把company7实际电价100与provider3的K当同一成本重复入账。

fuel维持原origin5000/basis2000/offset2000；水因内部take在Start后已经origin462000/basis W−30/offset0，day3/4不再变化。stock本身的owner/commodity/source/lot字段沿原writer，不擅自换ID或宣称它成为独立不可混lot。

## Boot exports与成本分类必须闭合

2DAY原累计 exports=[**332000**,**18000**]：电=Town194000+工程8000+警方130000；水=工程8000+警方10000。新冷却水60000是Boot内部投入，**不是water export**。Start/finish均不改这些出口及已认成本。

day3后 exports=[**432000**,**18000**]，day4不变：

- local_electricity_exported194000→**294000**，local_electricity_basis只增加K；这是原Town实际购电出口。
- local_construction_exported=[8000,8000]及local_construction_basis保持，工程原资本28916不因新批改写。
- service.boot_use.quantity=[130000,10000]及其basis保持；不虚构新公服、收费、工资或共用成员。
- exported_basis若原为X，则变为**X+K**。`validate_local_exports`要求它始终等于Town电basis+construction两项basis+service两项basis之和；新内部cooling30不能另塞进export分类。

独立总basis核对：Boot初始basis为fuel2000+电100=2100；原两批fees500，原day2有 `E+W+X=1000`（其余fuelbasis1600，heat/wastebasis0）。新fee300后总basis应为**2900**。finish后及day3后分别满足：

`fuel1200 + water(W−30) + electricity(E+730) + exported X =2900`；

`fuel1200 + water(W−30) + electricity(E+730−K) + exported(X+K) =2900`。

数量/能量另核：最终fuel3000×1000 + 电368000 + 电exports432000 + heat1300000 =原5100000；水462000+rawwater5400000+wastewater120000+water exports18000=原6000000。新批为真实转化，不能只追加电而保留已经移入WIP的fuel/water。

## 第3/4日日结确定值

新批60秒在夜间，未改变任何班次、警方已完成状态、Payroll ends_at3DAY或资产时钟。day3潜在100，电468000支持468utility单位，原料706足够，所以实际生产100；工资80全部资本化，idle0。商品basis raw100+utility费用100+工资80=280，全部售出，收入1000、家庭运费700（provider1），公司7不承担这笔家庭运费。

资产与耗尽对照完全同源：day3累计折旧916/当期400；day4累计1200/当期284，残值27716，不自动停用或Close。

| 项 | day2基线 | day3 | day4 |
| --- | ---: | ---: | ---: |
| 电库存 | 68000 | **368000**（已真实补发400000） | **368000** |
| 水库存 | 522000 | **462000** | **462000** |
| raw15量/basis | 706 / 706 | **606 / 606** | **606 / 606** |
| product16量/basis | 0 / 0 | **0 / 0** | **0 / 0** |
| 当期生产 / 销量 | 100 / 100 | **100 / 100** | **0 / 0** |
| 商品收入 / 家庭运费 | 1000 / 700 | **1000 / 700** | **0 / 0** |
| 工资新增 / 资本化 / idle | 80 / 80 / 0 | **80 / 80 / 0** | **0 / 0 / 0** |
| sold_basis | 280 | **280** | **0** |
| 折旧delta / cumulative | 400 / 516 | **400 / 916** | **284 / 1200** |
| margin / taxable | 320 / 320 | **320 / 320** | **−284 / 0** |
| 新税 / 累计assessed | 160 / 440 | **160 / 600** | **0 / 600** |
| retained累计 | 441 | **601** | **317** |
| 公司7现金 | 72495 | **73155** | **72995** |

day3现金 `72495−原day2税160−工资80−utility100+商品款1000=73155`，只生成4DAY到期税160；day4现金只付该160，得72995。新Generator的300由3付4，不扣7，不计入7工程资本、费用或日税。公司3的fuel/cooling实际basis400/30随WIP进入新电成本，不能拿这些数直接认定公司3利润、税或可分配收益。

Payroll至3DAY自然bill结束：累计work84900、accrued/billed/paid235、remainder3000、next_due=None、working=false；day4虽有剩余电也不能无工时补产。无自动续雇或第4日新工资。其他月租/原公服来源有效期和责任沿耗尽对照不变，不因新批续办或提前结清。

公司7独立四日现金式 `100000−工程付款28816−工资235−Town utility294+商品收入2940−已付税600=72995`。商品销货basis263+280+280=823，工资idle0，累计资产成本1200，累计税前 `2940−823−1200=917`，减日税600得retained317。材料basis100只消费库存，不扣第二遍现金。

与原四日耗尽对照：新分支多售32，收入+320、原料basis多耗32、utility现金多付32、tax多付128；所以公司7现金多160、retained多128，不应把二者差异抹平。provider3另付批次费300给4；这不是公司7补资。全世界现金仍3100000。

## root验收边界与未定项

- 原2DAY已消费stock3002之后保存Start前、Running/WIP中、due前、成功finish后、day3/day4原件；不能只用旧early-repeat Generator回归证明该共同分支。
- due前电仍68000，投入/付款/WIP一致；finish才468000。原票重试不重复扣300/fuel/water或再加400000，实际Store恢复须保留同批身份、原clock/resources及Boot分类。
- E/W和旧basis分类分项精确整数在本说明未数字化，理由为原pool逐步floor分段；提供源公式、不假造独立常数。新job cost730及上表所有有限业务值已可独立确定。root如发现原day2来源/消耗顺序不同，应先核源，不倒填新运行输出。
- 不新增target、模型、连续发电、#144、招聘续期、道路/警方同源承诺；原M0/native16GB/长期自给及新分支实际性能均未验。本说明交付后冻结，root唯一生产writer及串行验证方。

## 追加迟到对照：3DAY日结先于新批完成

本节保留上述准时补货输入/公式，不回写其正文。迟到分支是同一Open输入的正常游戏时间对照：Start Generator在 **3DAY−30=259170**，due=**3DAY+30=259230**；先推进到259200、完成原第3日Town结算，再推进到259230真正完工，最后到4DAY结算。没有续办Payroll21（仍在259200自然结束），不新增需求、钱物、模型或安全/认证规则。本节独立推算，未运行。

### 时间边界、数量与库存basis

2DAY之前全部同基线。第3日09:00–17:00的Payroll实际工作已发生，但尚不意味着供给或商品已经提前生产；原Town仍在日末消费该工时。

- **259170 Start**：operator3→contractor4实际付款300；fuel再耗1000/basis400、水再耗60000/basis30，水522000→462000，形成WIP730。电仍68000/basis E；Generator为Running、inputs=[1000,0,60000,0,0,0]、due259230。公司7现金不因这300改变。
- **259200日结前**：新批仍Running，future400000不是可用库存。原68000电仅支持68单位utility；因此原Town实际生产68、原料消耗68、utility付款68，商品basis190（raw68+utility68+资本化工资54），idle工资26；售68/商品收入680/家庭运费476。
- **259200日结后、finish前**：旧电被完整消费，`take(false)`成本为 `floor(E×68000/68000)=E`。stock3002为 **quantity0/basis0/origin_quantity0/origin_basis0/offset0**。不能保留非零旧basis在空pool，也不能把尚在WIP的730记进此库存。
- **259230 finish**：此时才向空pool追加400000，`put_pool`得到 **quantity400000/basis730/origin_quantity400000/origin_basis730/offset0**。这比准时分支更明确：旧E已全部进入历史出口，新产物basis恰为730，不再是E+730。Generator成功回Idle、释放原资源，WIP inputs/cost清零，completed变2、fuel_used变2000。
- **4DAY**：Payroll21无新增工时，产量/销量/utility付款/实际运费均0；不能用已到货400000或recipe carry补做第3日缺失的32份。最终电 **400000/basis730**，水 **462000/basis W−30**，fuel **3000/basis1200**，raw15 **638/basis638**，product16空。资产仍持续投运，basis28916不变，累计折旧1200/剩余27716。

本对照只规定原业务时间顺序，不把“相同stock名/相同数量”当作相同basis证明。准时分支先合池再消费；迟到分支先耗空旧池、再以新WIP成本入池，两个时序各有原坐标和真实成本来源。

### 公司7税、现金与retained：沿原耗尽结果

| 项 | day3 | day4 |
| --- | ---: | ---: |
| 实际生产 / 销量 | **68 / 68** | **0 / 0** |
| 商品款 / 家庭运费 | **680 / 476** | **0 / 0** |
| 工资新增 / 资本化 / idle | **80 / 54 / 26** | **0 / 0 / 0** |
| sold_basis | **190** | **0** |
| 折旧delta / cumulative | **400 / 916** | **284 / 1200** |
| margin / taxable_basis | **64 / 64** | **−284 / 0** |
| 新税 / 累计assessed | **32 / 472** | **0 / 472** |
| retained累计 | **473** | **189** |
| 公司7现金 | **72867** | **72835** |

day3现金为 `72495−原day2到期税160−Payroll80−utility68+商品款680=72867`；day4只付day3税32，得72835。300批次费由**公司3支付给4**，不是公司7支出、资本或费用，不改变上述公司7税/现金/retained。day3税原票不能在259230电到货后重算成准时分支的税160；day4也不能因收到电而回写此前生产、税债或增加商品收入。全世界现金仍3100000，公司3与4分别真实−300/+300，不能只看公司7现金不变便称重开没有付款。

### Boot出口分类与WIP守恒

Start时原exports仍[332000,18000]。day3旧电全部出口后变为 **[400000,18000]**，finish和day4都不再变化：

- local_electricity_exported194000→**262000**，其basis增加 **E**；此前Town费用和旧出口不回算。
- local_construction_exported=[8000,8000]及其basis不变；service.boot_use.quantity=[130000,10000]及其basis不变。
- 总exported_basis **X→X+E**。新冷却水60000是内部WIP，不是water export；完工新电730仍在库存，不得同时加入任何已出口basis分类。
- finish前的basis核对：`fuel1200 + water(W−30) + WIP730 + exported(X+E)=2900`；finish后把WIP730换成新电basis730，仍为2900。原`E+W+X=1000`保证两式闭合。
- 最终物理能量：fuel3000×1000 + 电400000 + 电exports400000 + heat1300000 =原5100000；水462000+rawwater5400000+wastewater120000+exports18000=原6000000。

### 需要原票恢复保留的真实阶段

root应保留Start后Running、259200日结成功但尚未finish、259230成功finish，以及4DAY日结的原件。在259200恢复点，必须同时恢复：Generator原job/due/token/inputs/WIP730，电空pool的零basis坐标，原Town日3完成游标/税32原票/retained473，以及Payroll21已bill结束且累计235分/84900秒/rem3000；不能因日结已完成就丢WIP，或因WIP存在提前确认新电。

恢复后先重放原3DAY税/日结票只能返回原结果；再沿原Timeline到259230才发布400000/basis730。原Start重试不得再付300或多耗fuel/water，完工与其原驱动票重试不得再加400000；4DAY重试不得重复支付税32。仍由原Store/Timeline/Boot完成这些职责，本节不新增控制或认证机制。

追加对照交付冻结；准时和迟到两条都是待root串行实际验证的独立预期，旧早期重复Generator回归不能代替本共同场景证据。
