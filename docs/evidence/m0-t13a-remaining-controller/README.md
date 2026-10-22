# 剩余稳态轮次控制器

这里只完成代码和纯脚本检查，`plan.json`九轮均为`planned_not_run`，没有启动这些轮次。固定Rust仍为32c、固定程序59aa；不使用当前0.1源码替代M0冻结来源。

单次入口示例（仅获得root测量窗口后执行）：

```sh
python3 docs/evidence/m0-t13a-remaining-controller/run.py l2 smoke smoke01
python3 docs/evidence/m0-t13a-remaining-controller/run.py l2 formal formal01
```

formal分别对l0/l1/l2的01、02、03运行，均使用独立新目录；每轮原300秒预热、1800秒测量、10秒静止，smoke为3+12+1秒，不替代正式。这个控制器不执行四速度、稀有类补样或8小时，也不自动连跑或重启；它们仍是独立未完成门。

复用已发布03 sampler、无损codec、源／卷／socket预检和readiness。固定manifest本身及其全部16个文件逐SHA验证，启动前再核对；清除遗留MYCITY环境，由observer注入heap/clock。steady supervisor自身创建normal pool，L2的Sim／model／worker接同版broker；L1 layout仅为external charge reservation，不声称其通过broker借页。

原4Hz、256MiB合并日志、压力60秒、swap256MiB、world20GiB、空闲2GiB全部保留。完整120秒滚动准入允许更早的污染历史留存；最近120秒污染或原readiness失败会拒绝。外层exec observer，不停留额外调度父进程。原normal失败／缺测／外部编译不被本控制器消除，单轮运行不能自动签三轮或完整M0。

纯测试命令：

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s docs/evidence/m0-t13a-remaining-controller -p 'test_*.py' -v
```

必须按实际footer、全树角色、业务和布局结果、延迟样本及恢复原件独立检查每轮；没有聚合验收器宣称全通过。各轮终态后按现行保留规则交接，不能为了完成九轮保留九套无依赖大载荷，也不能以hash取代仍需恢复的快照。

新增`speed_run.py`与独立速度计划；四档及unpaced均未运行。静态检查确认原32c speed-run/speed-driver缺少nonce observer-clock握手，因此wrapper在准入及模型启动前拒绝；见speed-clock-compatibility.json。没有关闭时钟检查或把原来源标为兼容。修复候选独立编码中，不影响原32c八小时功能轨迹。16项纯脚本测试通过，不等于稳态或速度实际验收。
