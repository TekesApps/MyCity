# v0.5.120：工资退出原语与持久恢复实测

主线基线为b4d6889（v0.5.119）；复用Timeline、唯一Engine事务及Store/WAL，不创建账本或时钟。集成C线f2b88dd、1e297a7、b2a4c55、6914aaf、1007cd9，并由root修复持久receipt decoder、续页测试预算和CLI Option处理。最终Rust来源见[attempt06/freeze.json](attempt06/freeze.json)，245文件，SHA256 `7b9e946b2f16fb370f45559cab34dc417bbd1142861a06327825245bd50ae8cf`；源码ZIP、固定CLI和binary.json完整保留。

## 实际通过范围

在macOS本机Rust1.85.1、release、locked/offline、默认线程栈环境运行，复用共享target；work与target均位于外置HP P900 USB/APFS SSD。不是16GB性能验收，也没有测此命令的独立OS进程kill。

[原命令及退出状态](attempt06/results.json)：16项core Timeline、3项真实Store退出测试、18项storage lib（含新命令与旧typed decoder的map/sequence/坏字段对照）、110项core lib均通过；实际CLI、core/storage全targets严格Clippy及全仓fmt通过。不是全workspace测试的声明。

CLI场景先支付7日工资392分，推进到第7日10时又累计7分。EndForExit只停止未来劳动并将399分累计额登记为已计费，paid仍392、双方现金不变、合同原结束日保留；检查点后重新打开，原operation重放状态hash不变，再显式结算7分，最终accrued=billed=paid=399，双边现金守恒。见[实际CLI输出](attempt06/cli.stdout)。

Store测试覆盖完整时点System权限、不完整Advance返回Busy、完成后非System拒绝、检查点冷开、同op重放，以及WalHeaderHalf和Applied注入失败后的重新打开/重试只记一次。测试故障注入在同OS进程内，不冒充进程终止故障。

## 失败证据与修复

- attempt01保留早期时间目标错误：8日10时实际新增9小时；改为7日10时，金额期望不改。
- attempt02保留真实Store invalid_json和Busy/Unauthorized失败、源码ZIP和失败可执行文件。原测试Temp的无条件Drop删除了该次测试临时world，不能声称仍保有该原件。
- attempt03-repro使用新的失败保留helper和原旧decoder，忠实重新触发invalid_json；完整故障world保留在`work/payroll-exit-main-120-repro03/`，并有独立源码、可执行文件和原始日志。这是新复现，不是attempt02原件。
- root在有界手工receipt decoder登记EndForExit，并加入typed语义对照，未换成无界反序列化。
- attempt04保留续页限制真实拒绝与完整故障world：初始Advance预留limit=1，测试错误地用256继续。测试按既有limit=1继续，不修改持久预算守卫。
- attempt05保留CLI三处Option误用编译错误；root修正为ok_or_else。attempt06全部实际通过，旧记录不覆盖。

## 边界、预算与后续

原语不授予法律批准，不处理遣散/罚金/未来义务，不允许据此宣称企业已清算或股东可分红。合同、欠薪、未来责任仍可追踪。未新增常驻对象字段/表或领域余额；用现有事务拷贝/有界journal，不放宽任何容量。未增加专门紧急磁盘权限；普通存储准入失败时可拒绝，不能宣称空间耗尽仍能退出。其CPU/整进程内存和16GB规模尚未单独测量。

道路工程影响真实交通的候选3f7bdb7已独立提交，仅静态检查，尚未联合验收。退出冻结、债权分类和同级公平清偿仍缺真实持久接入，CompanyClose不能因本原语而放行。

## 阶段磁盘交接

只复用原共享target，没有新建target/cache。保存各次源码/日志和必要失败exe，最新成功CLI及实际业务world、两个故障原件保留，不能用hash替代；成功临时测试world自动回收。共享target后续道路联调仍依赖，本阶段不删。全局历史回收见[smoke02交接](../disk-reclamation-smoke02-handoff/STATUS.md)，本阶段没有额外可确认无依赖的纯缓存退役，新增删除0B。190份既有受保护原件不动。
