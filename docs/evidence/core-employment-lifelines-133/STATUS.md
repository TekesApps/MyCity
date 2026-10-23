# v0.5.133：就业购买力接入真实道路、建设与Boot供给

本阶段只改town_lifelines CLI，复用现有Store、Timeline工资、Town市场、建设与Boot账物系统，没有新增核心、账本、时钟或持久化格式。新增`--employment-control`与`--employment-change`，保留原五个模式。

## 同开局的真实联合结果

两组完整genesis字节一致，SHA256为`56048a4bcece3e5c8c261c0fe28168d85690697d7303445b48762abcbedaee80`。将家庭5原有1000分资本转给新公司8：家庭现金及其source19归零，新增source25与公司8账户1000分；总现金3101000分、库存basis5100分均有来源，未注资增加总额。

| 三个游戏日 | 留任控制组 | 第1日末换岗组 |
| --- | --- | --- |
| 家庭每日实收工资（分） | 80 / 80 / 80 | 80 / 8 / 8 |
| 实际销量（件） | 20 / 20 / 20 | 20 / 2 / 2 |
| 三日商品款 / 运费（分） | 60 / 180 | 24 / 72 |
| 旧/新工资工时（秒） | 86400 / 0 | 28800 / 57600 |
| 新公司最终现金（分） | 1000 | 984 |
| 第3日货架结余（件） | 40 | 76 |

两组都有真实道路proof，容量按2/2/10生效，入口授权、方向、OD来源和流量可核对。往返各11秒，按1分/秒与每行程10件计算，运费向上取整3分/件；加货架价1分，家庭到户价为4分。按自然需求100、路径容量、实际库存及每日现金重算得上表销量，业务命令未预写每日成交。扩路后仍有购买力瓶颈。

共同工程实际消耗500原料、28800秒有效工时、电水各400000，电费8000、水费4000，总spent41300；验收边界172800秒发布道路容量。100件商品在第1日同一批生产，耗原料100与真实Boot电力100000；结余原料400，电力700000、水20000，现金与库存方程一致。此3日例不证明离职导致新增产量下降，单批产品尚未耗尽；该28日生产因果另见v131。

换岗由可信场景显式输入；旧合同在第1日完整结算后终止，同边界开始新工资合同，旧/新工时不重叠。原工资已结清，新雇主不承接旧债。新公司没有生产/服务配方，只用期初资本付工资；不宣称内生求职、劳动法律审批、持续盈利或经济平衡。

## 实际命令、反例与恢复

最终来源`04aaac50820e1403f1e762cdd6402750ae1af7e671d0c35da5f15edb7188c6f8`，264项源码+12份额外固定文档输入。固定exe SHA256 `c1cd739f6063ad5d0e5d3d7cdeb444a9db24084d92e9fe6a36d47c8ae890330b`；完整ZIP SHA256 `c90a08e3c8c981e6a8d4183f1a31acaf305125e91d62e387af218d73313e4636`，见[最终记录](employment-lifelines-freight-binary.json)与[完整输入清单](final-source-manifest.json)。example没有内嵌headless来源，使用实际构建/不变来源/逐文件ZIP绑定。

118严格Clippy、119构建、120全部7模式、121格式、122四项拒绝核对、123重复与进程观察均退出0/源码未变。122各子程序预期exit1；冲突选项不创建Store，重复使用现有Store被拒绝且18份文件hash未变，不能计作成功经营运行。

同一次真实Store经历停工、checkpoint冷开、恢复、实际施工和最终冷开；全部工程/工资/三日市场持久票据重放不增加sequence、不改state hash。123在新路径重复两组，完整最终state hash分别与120相同。原五模式关键字段与v132的108记录一致，完整最终state hash也一致。

113编译因跨checkpoint保留借用失败，114随后因函数参数数目触发严格Clippy；修为拥有的工资快照和传入已有Options后通过。117原五模式中的3个通过、4个三日模式因新增错误零运费断言失败；原始失败源/固定exe/完整WAL均留在外置work。修复只纠正费用归属，不调费率/需求/负载，120重新实际通过。见[runs](runs/)与[117失败日志](failed-cases/)。

[独立验证脚本](validate-employment-lifelines.py)从120完整genesis/checkpoint、真实profile/proof及固定fixture重新推算8类核对，[结果](employment-lifelines-independent-validation.json)为passed。脚本曾误把家庭来源标签写成18，账户逐项来源校验已覆盖实际source19；保留[初始报告](employment-lifelines-independent-validation-initial.json)，改为显式核对19并用整数取整后重跑通过。默认只读外置120原件，缺证据会失败，不启动模型。

CLI旧字段`market_payment_total`是Boot operator收到的生产电力款（此例100分），不是商品成交额或家庭总支出；独立核对没有混用。家庭、工程、生产各项费用按实际主体分别结算。

## 资源、完整原件与M0

共享Engine/World字段、4MiB Engine和原64MiB角色上限不扩容；新增一个账户/资本来源和第二个工资合同占已有64账户、192来源、64工资容量。CLI临时收集最多两份工资快照与历史票据，不增加逐人常驻模拟。尚未单独测量净增量分配峰值。

123小型单进程观察：control/change分别0.71/0.61秒，最大RSS57917440/54951936B、peak footprint53576064/50626968B，见[原始time日志](native-observation/)。这是32GiB M4上的整CLI观察，不是净增量、角色准入或16GB门。work/共享target均在HP P900 USB/APFS外置SSD；未运行GPU、UI或AgentKit。

control/change的[完整Store输入](cases/)随本证据发布，外置原件仍完整保留；最终checkpoint用于恢复，较早checkpoint不能脱离其当时root/WAL被宣称可独立恢复。原source ZIP、固定exe、失败WAL和重复运行目录继续留在外置work，hash不替代恢复原件。

[回收收据](stage133-retention.json)：118–123相同终态ZIP的五份重复副本替换硬链接，全部路径/字节保留；文件分配空间减少21299200B，SSD可用观察增加21250048B，不称APFS独占回收。恢复输入、日志、验收exe及190保护原件未删；复用既有target，无新缓存目录。

M0仍not_accepted，真实16GB门not_run。另准备[v132同源原负载联合控制器](m0-preparation/STATUS.md)：source a629…、exe8b274…与v132提交的276归档输入逐字节一致，16项纯测试/静态预检/support校验实测通过，但没有启动准入或模型。准备元数据旧HEAD纠错原件保留在外置preparation-v1-original；最终指向61587dc/v132。原120秒无编译normal准入、64MiB/common8MiB、28800秒、96 saves、2h/4h/6h故障及8h到期、4Hz和256MiB日志上限不变。formal须等该来源自己的成功smoke与日志预算；不能拿旧失败或旧smoke签新通过，也不把该v132准备赋给v133来源。
