# T13A-3k 联合耐久托管

继承原冻结负载与门限，先验证Rust进程监督及完整L1覆盖，不签共同8MiB/精确64MiB或真实16GB。原冻结acceptance SHA256 876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508不变。

Rust joint-endurance持实际normal pool/layout/business Child。布局初始化完成后才启动完整scale-endurance；业务完整退出后发送S，布局保持最少600秒(smoke)/28800秒(formal)。EOF/非法指令/子进程失败/35000秒布局或36000秒监督上限均失败。布局以固定publication end排空已有deadline，扫描1秒、快照300秒、两代保留、128固定摘要不变；完成后冷读真正最新快照。恢复120秒超时保留失败，不改原正式冷恢复门。

业务仍720小时/723目标、2880查询/96保存及实际指定故障，725借还；smoke只缩墙钟，不能签正式速度或8h。统一supervisor时轴记录L1 ready、business spawn、业务时钟标记、完整退出、stop和布局退出，证明完整覆盖，不靠两者各自时长猜测。

CpuLog首行发布有界持久身份，与目录/正常report/heap对齐；故障缺footer仍缺测。最初每实例单identity文件会越过4096保护，预先发现后改为现有日志首行，不提高文件上限；初版回归来源独立保留。父断开/截断身份不补成成功。

保持Rustsupervisor0.25MiB/driver1MiB/worker1MiB/Sim2.5MiB/layout0.75MiB/helper0.25MiB/pool0.25MiB/observer2MiB原份额。普通Engine/serde/helper动态模型仍EXTRA，触页池不抵扣。原observer源码不变。无并行自身测试的同源短联合完成后独立核验；任何压力/磁盘保护失败保留，不删别人的文件/停别人的应用或减负载。

advisor单轮认可实现边界，要求严格stop/EOF、时轴覆盖、初始化身份费用、最新完整快照和硬失败上限；身份首行方案单轮认可，不扩大文件数。
