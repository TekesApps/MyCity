# advisor独立审查

初次架构建议要求权威状态与工作区分离、唯一lease、quote／失败／展开保留背板、持久后不失败发布，以及Option逻辑缺席不能释放固定存储。该阶段先落实时间线及核心固定表，完整Background／Store仍继续实施。

代码只读检查确认：五类lease先取所有权再复制活动数据，部分获取失败已取得lease会归还；锁不覆盖准备过程，重入失败关闭。Timeline固定表、null／Enable／复位和发布交换背板路径成立，控制块仍须进入后续真实容量账。

第一次审查提醒append成功先解除poison到apply之间存在未来展开边界；主任务已将成功解除延后至Engine apply、journal和Applied observer完成。最终审查核实该空隙已消除。

最终advisor只读核对终态测试、验证器源码及结果，认为v0.5.68有限范围无阻断：全仓Release、clippy、fmt有终态0；兼容验证器确实调用Store::open再比较完整canonical Engine，不是只比较预存摘要；旧rules identity下sequence3013、2,507,418字节与旧fault-reference完全一致，原243文件未改且完整副本保留。

发布边界：固定核心表／lease、展开保护、旧完整存档兼容。不是完整ModelWorkspace或normal64。每Engine独立PreparationWorkspace，v67目标容量公式不能自动套用；Background、一般恢复、WAL／DOM、真实池授权等未闭合，没有新的性能／内存验收。advisor未运行或干扰负载，也未构建。
