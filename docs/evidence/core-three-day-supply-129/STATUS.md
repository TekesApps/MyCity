# v0.5.129：三日施工供给、交通与经营真实联调

本阶段只扩展既有 Rust `town_lifelines` CLI，复用同一个 Store、Engine、Boot、公用库存、工程、Timeline、交通 proof 和账务，不另造时钟或账本。新增 `--construction-utilities`，可与 `--hold-road-inspection` 组合；拒绝与 `--generator-failure` 组合。默认三个历史模式仍是两日场景。阶段功能通过不代表完整0.1、M0或性能验收通过。

## 固定来源与实际验证

最终262项构建输入指纹 `7935a500c00687b6dccf2cac54ee75f5b4d3b809f983d2631c8fd0b1a4e93ba6`；固定CLI SHA256 `5002a35a7ec2ff492689342079d5740755877f8aef225ee15498bce9b79c7ef7`。本CLI不内嵌headless来源指纹；来源绑定来自完整归档和逐文件清单。源码ZIP位于外置work的 `70-three-day-full-balance-bin/source.zip`，SHA256 `cedc6e95a43adeeee4f36010d93f443d6206c94f655250964aab6a748a0e0455`。

[最终来源清单](final-source-manifest.json)、[固定可执行文件记录](joint-three-day-final-binary.json)、[五场景实际收据](three-day-final-cases.json)、[独立结果比较](three-day-independent-comparison.json)及 [逐文件发布映射](publication-manifest.json)共同绑定证据。

- 69：全workspace、all-targets严格Clippy，退出0，来源未变；不是全workspace测试命令。
- 70：release CLI构建退出0，来源未变。
- 71/72：三日正常与扣留验收场景，退出0，来源未变。
- 73/74/75：原默认、机组失败、扣留验收三个两日回归，退出0，来源未变。
- 57首次编译的可变引用错误、64格式严格检查错误、67/68对缺少PartialEq的Resource直接比较错误及原日志完整保留；随后改为显式比较资源id/capacity/reserved/occupied。59/60是较早候选，不替代最终71/72。

完整命令、原始stdout/stderr及子进程退出码见 [runs](runs/)。包装器本身退出0不能代替每个exit.json中的实际子进程退出码。

## 实际三日因果链

第0日17时首次工程Work因真实Boot供给不足得到 `Boot(Input)`。负收据持久化，但现金、受保护余额、完整库存lot/basis、资源占用、工程与Boot效果均不变化。承包人显式Pause，检查点和冷恢复确认；实际机组启动并交付电水后，显式Resume，重新设置有效计时起点61260。恢复后立即申请28800 worker-seconds、elapsed为0，实际得到 `Construction(WorkLimit)`，再次验证所有效果不变。

第1日市场结算仍使用旧路容量2。后续机组真实供给；第1日17时，距离恢复86340日历秒，申请一个09–17班次的28800 worker-seconds成功，实际消耗各400000单位电和水。第2日市场在验收之前结算，仍使用容量2；正常组随后验收，新增容量从第3日服务窗口生效。扣留组始终不能使用新增能力。

| 实际结果 | 第1日 | 第2日 | 第3日 |
| --- | ---: | ---: | ---: |
| 正常道路容量 | 2 | 2 | 10 |
| 扣留道路容量 | 2 | 2 | 2 |
| 两组生产数量 | 100 | 0 | 0 |
| 正常销量 | 20 | 20 | 60 |
| 扣留销量 | 20 | 20 | 20 |

销量由实际需求、购买力、价格、库存、真实道路proof共同约束，不在模型中预写日销量。当前生产规则是已有产成品lot未清空时不再追加同类生产，因此后两日生产为0；不能描述为每天100或持续经营已证明。

施工电费8000、水费4000、劳动28800，加已拥有材料500单位的basis500，工程累计spent为41300。材料basis是存量成本转入，不再支付第二笔材料现金。机组三次供给后电库存700000、水20000；工厂实际用电100000、basis182。三次工资共240，现金总量3101000守恒。最终检查点冷开相同，三笔历史市场及三笔工程结果收据重放均不重复收费、用料或施工。

## 恢复输入与范围

[完整最终输入清单](three-day-retained-inputs.json)记录五场景完整Store。正常与扣留两组最终Store的原文件逐字节发布于 `stores/`，包含CURRENT、root、checkpoint、WAL、空间输入及交通proof；另外三组完整Store及固定exe、源码/依赖归档保留在外置work。实际中间Pause/Resume检查点冷开已执行，但随着后续root/WAL推进，不承诺任意历史checkpoint单独即可恢复整组Store。

本场景使用固定群体OD和显式工程班次；没有实现自动断供停工、Resume供给互锁或连续公用供应保证。有效时间反例只证明本显式流程，不代表所有调用路径均由新策略统一管控。没有建立稳态盈利，也没有校准经济平衡。完整28日经济压力矩阵仍见既有v122证据，本次不是将三日替代28日。

## 资源与M0边界

本次生产源码只改CLI，不新增持久模型字段或扩容。既有Engine 4104080B、Project 1616B只是已有结构尺寸，不代替联合heap/stack/native预算。71的时间工具观测1.27秒、最大RSS 61571072B、footprint 57295232B，仅为小型CLI运行观察，不是clean性能或真实16GB验收。

机器为32GiB Mac，work与共享target均在HP P900 USB/APFS外置SSD；不能继承旧内置盘IO数据或其他来源性能。原normal物理门仍未接受，三轮分级/四速矩阵、真实8小时未完成，真实16GB为not_run；完整M0仍not_accepted。

另保留 [未合入的小缓冲实验](withheld-hash-experiment/CANDIDATE-STATUS.json)：其138项core测试属于独立实验来源，不能计入本发布源码测试。Value探针哈希一致但未显示稳定收益，不合入1KiB缓冲，也未执行该候选完整30日或授予内存收益。

## 阶段回收

[回收收据](stage129-retention.json)：22份终态、字节完全相同的源码ZIP通过硬链接去重，保留所有原路径和完整内容；唯一文件分配空间减少89395200B，可用空间观察增加89182208B，不能宣称APFS独占回收。未退役任何业务快照、恢复原件、故障证据或验收程序；继续复用共享target，无新增独立构建缓存。
