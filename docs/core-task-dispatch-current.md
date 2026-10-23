# MyCity 当前实际派工与交接

2026-10-23初次核对主线main v0.5.140，root复用既有agents，没有重复启动同线。D随后验证并合入v0.5.144；M0、F、E的WIP仍留在各自原树。非本任务 `docs/evidence/m0-t13a-rare-control-01/` 保留。范围为Rust模型/CLI；没有UI/GPU/语音/AgentKit。本表是执行合同，不是全目标完成声明。

## 唯一共享核心负责人

`economy_scenarios` 是 Engine/World/assembly/Timeline/事务候选/Store提交及receipt decoder的唯一修改负责人。D树已冻结供root验证；该writer当前在 `/Users/frankqin/dev/MyCity-core-work/spatial-runtime-update-current` 接入F事务和版本根。D仅在真实回归时重开，F后再交接E。root维护集成与验证；其他agent不得同时修改这些共享结构，也不另造时钟、账本或持久化。

## 当前批次

| 任务/负责人 | 文件所有权 | 权威输入→输出 | 依赖与交接 | 完成条件 |
|---|---|---|---|---|
| M0 / equity_current | equity-current树的headless joint_driver.rs；独立fresh R07 control目录及说明 | 原采样的source/nonce/PIDbirth/footprint→既有Rust消费者的受限3槽reading→实际映射置换 | 保留冻结j3p控制器；root构建新exe后pin、advisor窄审、统一短测，再判断正式重跑条件 | 缺测撤销/TTL补回/物理退映射实测，原64MiB/负载不变，剩余超额不抹去；原分级/四速/故障/8h仍逐门实测 |
| D shared core / economy_scenarios | 上述共享核心；town_runtime/equity/model capacity | 实际现金、原成本池、工资债税及股权→真实出资/分红/资本返还和游标 | 已修复清算股东排序和统一尾差；root完成205—209最终同源实测；接口与持久字段冻结 | 可信来源、偿债优先、未知已付费用仍阻断利润；真实Store恢复/重放与资源预算通过 |
| D evidence / construction_finish | D树tests/equity_runtime.rs与examples/town_equity.rs | 实际Store命令与独立整数oracle→场景、反例、冷恢复证据 | 共享core只读；发现接口问题交唯一writer，root串行Cargo | 出资/税/分红/财政后支出/清算四账户真实守恒；lost-reply后的业务回执本身必须成功，不能只看重放相同 |
| F / joint_current_prep | spatial-runtime-update-current树独立spatial_transport_updates.rs、新测试/CLI；lib仅export | 验证Scene、原始profile bytes/hash和有效权限→版本候选/精确asset/按日proof解析 | 给共享writer精确Action、root字段和授权需求；共享提交窗口排在D后 | 同一Store open→close→reopen道路/门实际改变销量；历史proof原hash可解释；重复/冷恢复不重复激活；未支持模式拒绝 |
| E / scoped_index_calls | housing-current树housing.rs、独立tests/housing_runtime.rs和examples/town_housing.rs、接线文档；lib仅export | 源明确的期初存量住宅/实际交付、家庭账户与实际收入→容量/唯一住所/租金欠费/迁居等待 | 先有界状态/serde/copy/validate/金额计划；world/Action/Timeline月历到期由唯一writer分批接 | 实际租金和欠费入原账本、日历/恢复一体；同一Store入住/欠费/工资后支付/迁居等待；独立planner不算完成 |
| advisor / scoped_handle_advice | 只读具体争议及验收 | 实际源码/失败/测量证据→有限阻断和判定 | 当前F权限来源与R07协议；阶段最终证据才复核，不重复总体审查 | 对来源、守恒、权限、预算给可证伪结论，不因源码测试存在签通过 |
| root | 集成分支、证据/任务状态/发布；统一Cargo与测量 | 各线稳定小批次→联合世界、实测、完整证据与版本 | 一次一个共享writer；构建结束后才允许改固定源码；正式测量禁止本任务编译/重测试/盘点/清理 | 住所/收入租金消费→交通供给→企业工程→税/分红清算同一持久世界因果成立；未测不通过 |

## 测量、计算契约与交付

Cargo与重测试由root排程，统一 `/Volumes/HP P900/dev/MyCity/target`；work也在外置HP P900 USB/APFS SSD。正式性能运行期间全部本任务agent仅编码和轻量只读，不编译/重测/全盘盘点/清理，不停止其他项目进程。当前j3q-01已因pressure保护停止并在v140归档；没有修复/环境变化不重复长测。

各线分批交接源码路径、具体接口、反例、CLI/可执行场景、冷恢复原件、CPU/内存/盘增量和已知限制；补权威IO、关键事件、稳定区间、等价合批、非线性阈值及资源归属，保留参考事件路径。新动态粒度实验不替代原M0负载/门槛，真实16GB门保持not_run。大数量字段计入多份候选和恢复放大。

每批root实际验证后统一提交/合并、唯一递增annotated tag并原子推送核远端。阶段结束先查占用/后续依赖，再清理明确可重建文件；完整恢复原件和故障/验收证据不以hash替代。当前各线未验证部分均不得报告完成。
