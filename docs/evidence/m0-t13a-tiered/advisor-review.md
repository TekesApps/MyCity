# T13A-2d独立advisor阶段审查

代码／输入检查与实际工件审查各一轮，未改代码、未运行竞争基准。后续无需再次复核本轮同主题；正式T13A最终审查仍另有要求。

方案和代码：Tier静态嵌入独立输入，small／medium工资与债权收款账户合法，full输入不变；独立生成器默认只核对模式已实际通过。Progress带完整游标和job／generation／incarnation，raw_at在当前单writer串行连接下复用同一状态，每目标完整Status及最终全部字段比较未减少。三档rules独立。revision仍构造WorldView，未称零分配。

实际归档：advisor执行去掉唯一写文件语句的verify.py，额外读transcript、采样及存档链。source 5de330d5…faa0e，三档输入及全部世界文件、根引用、唯一连续原票和现金／工资／债项核对通过；723目标／724窗口各档保留，原票2955／2979／3011，各23段。full新旧CPU及实际Engine分别直接全字段相等，未在版本对照额外归零。

独立配对full请求及响应，完整Status726次、累计约20.613秒，Progress约0.091秒，RunRequired约150.402秒。支持“减少查询开销而未减少业务”；342.132到227.993秒为两次单诊断的观测变化，不宣传稳定加速比。

advisor独立重算观察成员RSS和为约64.3／80.2／110.1MB；缺footprint1043／1058／1126，不能保证短命峰完整。无L1、launcher未计入子树、other_owned为0，不签联合内存／16GB。响应计时不含submit前端全部成本，完整经营和drive包含。transcript响应只有hash与长度，逐点比较依赖冻结程序当场断言，不能声称所有Status载荷均已归档；终局完整状态及原票可独立复核。

结论：本次三档真实30日诊断无交付阻断，主线程须确认全仓库Release最终通过后发布。10类CPU、四档实际速度、三轮稳态与8h均未签收。下一依赖是真实CPU分项：显式瞬时固定大小收集器不进入Engine／Prepared／Receipt／检查点；区分execution、quote、replay及admission_validation；同8192步页聚合样本，空扫描不填活跃门，零现金真实义务变更仍算活跃，持久失败不标成功提交，worker页与Sim复算页分开，预演／总CPU和编码／I/O成本不消失。Instant报告计算区间经过时间，不冒称线程CPU时钟。
