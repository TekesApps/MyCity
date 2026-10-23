# 空间步行接入真实市场结算（v0.5.139有限阶段）

实际已验证Scene中的建筑门、连接走廊、二次曲线道路、精确端口授权与方向，转换为真实有向交通Opening；容量/速度来自显式设施，群体步行OD经既有Assignment、Store profile/proof及市场resolver影响真实往返和交易。没有用几何距离自动授予通行权，也没有把WALK冒称货运。

## 实测范围

最终source `5b3b290e8207cde98c46e63170c7d32ba73393ac88502b6ff00a9f682af70d86`，272源码/284固定输入；[实际测试exe](fixed-test.json) SHA `c72c2519b8a7229b8095bd15cfdff250e458404705525f9a6b195bb91334b83d`，完整源码ZIP `f542c75aa9a8986651db9f66fbadbd006883ec3e583ea719144495f2538c5506` 在外置原路径完整保留。没有调用test harness的嵌入来源查询；采用真实build/test日志、前后source不变、完整归档及固定exe绑定。

174全workspace/all-target strict、175 map5+Store1测试、176 fmt通过，三个命令固定输入清单相同。Store1测试包含三个独立counterfactual world：开放时往返4/4人、实际销量40；封路时0/0人、销量0；禁止进建筑时去程4/返程0人、销量0。每份world实际checkpoint、冷开和同操作精确重放，proof字节、状态hash、原Receipt及销量不变。

177再用实际固定测试exe新进程运行一次取得CPU/RSS；不是Cargo编译耗时。其[独立检查点/proof核对](process-observation/independent-oracle.json)从完整原件读取101/102 OD服务量、WALK mode、proof SHA、现金四账户和库存：销量40/0/0，商品剩60/100/100；原料不变，钱物守恒。该重测还是同一个测试，不重复计为第7个独立测试。完整三份恢复原件位于process-observation/test-artifacts及外置原路径，hash清单不替代原文件。

## 边界与失败

本阶段只有单图、地面、单建筑/单走廊/单道路/显式terminal，最多3节点4有向边；不支持汽车/货运/公交轨交、桥隧/异高路径、多路交叉、跨图连网和完整城市路网。异高、非WALK或不合法连接拒绝，不声称已实现这些方式。

三个world是不同期初空间/设施配置的因果对照，**没有**在同一运行Store里原子修改空间资产并更新交通revision/profile的事务。原始Scene作为冻结源码fixture中的转换输入，Store保存其转换图/proof；不能宣称Scene几何资产已和运行时改路一并版本化持久化。既有施工road_effect的运行时容量变化是另一有限机制，不能混为F全部完成。D/E和最终跨系统联合仍未完成。

156是helper变量遮蔽编译失败；167是端口夹具200cm不足道路700cm净宽；168是真实生产方向映射反了（端口进入地图看can_enter、离开看can_exit）。分别修测试夹具或生产映射，保留两个方向反例，没有放宽物理/授权门。171/172发现测试风格lint，最终等价修正后174/175通过。所有成功与失败原日志、命令和输入清单均保留，不把旧源码通过嫁接为最终同源结果。

## 新增资源观察与回收

work/target位于外置HP P900 USB/APFS SSD，M4/32GiB；此微型场景不是原M0负载，未做quiet性能准入。177三个world串行的测试进程0.83秒wall、0.05秒user、0.06秒system，最大RSS50036736B，包含test harness/Store/断言/IO。单次适配/分配/恢复分段计时和实际Vec容量均在stderr中保留，不作全城吞吐承诺。

Opening拥有容量400B；Assignment约20864—20912B含path边ID；Store总拥有容量约41.47MB，其中含既有元数据/Store背板，不能全算adapter增量或把父子容量相加。完整各world逻辑51122—51556B，文件分配73728B，非APFS独占物理量。现有Engine/16GB预算不会自动覆盖新增功能，城市规模仍需单独组合实测。

阶段结束仅对两份逐字节相同的源码ZIP硬链接去重，完整原路径和内容保留，分配空间减少8937472B；卷可用空间观察增加8912896B单列，不宣称APFS独占释放。目标缓存还供D/E验证，复用且不删除。所有业务/恢复原件及验收exe保留，[回收清单](stage-retention.json)。原M0冻结负载、normal/四速/8h及真实16GB门不变；8h尚未由此获得通过。
