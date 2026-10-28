# #21 同一有限城市跨电量耗尽、工资到期与日费用尾差

基线v0.5.222（游戏v221），本轮v0.5.223仅新增验收，没有生产代码/开局输入/策略/预算改动。root唯一共享writer和串行验收；原独立预期已冻结。完整21/117/115开放，无PR，不因此有限增量关闭父任务。

## 实际共同业务

沿原transport_opening::Open，保留住房/实际Payroll/警方已完成到返/Boot/工程资产/Town及真实道路proof。仅将同一世界由2日继续到4日，不补电/库存/现金，不改Payroll ends_at=3DAY或寿命/残值。独立源码算术先于执行冻结，见independent-expectations.md。

|结果|第3日|第4日|
|---|---|---|
|生产/销量|68/68，电量允许部分生产|0/0，原电库存为0且原工资合同已自然结束|
|商品款/家庭运费|680/476|0/0|
|实际工资/计入商品basis/闲置人工|80/54/26|0/0/0|
|公司7现金|72867|72835，仍支付前日已发生税32|
|累计税/retained|472/473|472/189，无新增零税Claim/退款|
|累计资产折旧|916|1200（当期284尾差），残值27716|
|raw/product/电/水|638/0/0/522000|638/0/0/522000|

四日总销售262、收入2620、家庭运费1834。工资累计235、84900秒、余数3000；paid=billed=accrued=235，working=false、next_due=None，原行/余数保留。商品basis733、闲置26、折旧1200，累计税前661、税472、retained189；现金由原100000减工程28816、工资235、生产电费262，加商品款2620、减实际税472=72835。全账户现金3100000守恒。

第4日reference_need仍100，实际销售0；summary的market_demand/unmet_market_demand为0不能写成生活需求已满足。Town日聚合分配提供市场容量，不宣称实际逐车旅行或与警方图统一。

## 必要责任与恢复证据

- 三期税Claim（period1/2/3，280/160/32）及来源仍在，4DAY时全部paid/processed；第4日无新零税Claim，不回写旧税。
- 租金Claim901本金1000/已付0（未来待付1000），tenant2→landlord1、lease901/unit71来源保持，due=31DAY、未processed；尚未到期不等于免租。本轮没有声称实际31日收租已测。
- 原警方任务早已结束。固定自然需求下Medical59日、Fire31日、下期Police180日，均未在3/4日发生；旧警务工资/资格已到期不自动续证、接单或再次收费。不能把本轮无新单当成未来已履约。
- 固定store02默认栈实际4日通过，新增显式核验partial labor54/idle26、工资行与全部三税/未来租金来源；第3日WAL冷开、第4日checkpoint冷开，共2个实际独立子进程通过。热/冷重放原market票不增序/重复generation、折旧或费用，完整hash和业务摘要一致。
- 固定role01：原两日道路角色共存检查与新增四日角色均通过。实际Sim IPC推进/Store管理proof日结、重复原operation、暂停退出/重启、最终checkpoint与独立Store reference及独立数值预期一致。原59MiB授权、64MiB池、2次workspace授予/归还平衡，allocation failures0。观测arena最大55798488B；Engine owned3690456B，Store owned42008794B含Engine、不相加；creator/reference在正式角色外，均不是native总峰或真实16GB证明。

## 有依据的复用及原件保留

对照HEAD v222的502个tracked构建输入，仅两个专属测试文件变化；生产/core/Store/Sim/旧fixture/Cargo及规则字节不变，24无关observer/M0 WIP SHA均相同。由此复用v221的shared、旧无配置/default/原18预算/Journal布局和已测原票兼容证据；新增四日业务/冷恢复/normal已分别实跑，不拿两日专项代替。无新增架构疑点，不重复advisor。

headless/build.rs会将crates内测试也计入MYCITY_SOURCE_SHA256；因此本轮shipping二进制身份与v221不同，并未称二进制逐字节相同。生产源未变、旧输入/合同/布局未变，新的固定source manifest与程序分别保留，旧两日角色同新程序补验。

原始4日程序store01通过后，按执行前独立预期补强了工资/租金/税来源和idle人工断言，再构建store02重验；两份源/程序/世界都保留，没有已知生产失败。本轮全部固定ZIP/manifest/程序/world/WAL/checkpoint/proof/claims在HP P900 work/finite-city-depletion-20261029，约524MB<事前1GiB，实际大小见summary。原唯一target/work复用；当前target后续仍需，未删除不明缓存，无新worktree/timer。Mac16,1/32GiB/macOS26.6.2/Rust1.85.1，HP P900 USB/APFS，不从本结果推产品人口/速度。

## 结果和差项

统计起止见summary（验证窗口，不是开发周期）；本阶段新关闭业务0、历史0、缺陷0，18完整源码包积压未整包减少。原#151/#155/#157关闭有效，不重复计数。四日证明有限投入不足后的合法停产/结算/恢复，未证明长期自给、持续补给/续雇、低现金欠薪/欠税叠加、31日房租/后续自然需求、完整父合同、实际16GB/native/容量倍率/M0长期/GPU/UI/Windows。

下一只读准备核对现有#144/#133/#132连续产出能否共享当前Boot库存，找实际provider/consumer断点；不假设已有桥、不批量新建能源模型。提交/tag/PR状态回填#21，发布版本v0.5.223，无PR；确切commit以发布回执为准。
