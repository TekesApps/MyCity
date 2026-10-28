# #158：合法暂停后的同pool成本分摊

实现/验证版本v0.5.225；[#158实际关闭见closure回执](closure.json)，不把tag当验收。root唯一共享writer，专属worker只补一个Sim测试文件并冻结，没有新模型/字段/账本/调度器/target/worktree。父#8/#21及完整#117/#115开放。

## 已修复范围

真实原Store同一警方旅程只增加暂停点，旧v1公服电成本235→234分、余电basis495→496；数量/现金/实际工时相同，总basis2600守恒。probe01实际exit101，完整红例源码/程序/两世界/恢复原件留HP P900，不能改称通过。

新开局显式`pboot-local-development-v2`，全部Boot输入和出口复用原Stock origin/offset累计floor差，只有真实产物合池才重建origin。原micro/scale/local-v1和旧原票继续旧语义，不重新计算已发生事实。旧v1不再被外推为分段成本等价；未知成本版本拒绝。当前typed Opening provider不提供本城版本，使用显式JSON，未静默映射。

保证范围仅同consumer顺序和实际pool边界。跨产出时点/交换consumer不属等价承诺。设计同步[Boot运行契约](../../m0-boot-runtime.md)，[独立预期](independent-expectations.md)，原advisor[一次窄复核](advisor-narrow-review.md)无源码阻断，advisor未运行验证。

## 实际串行验证

|门|固定结果与范围|
|---|---|
|最小新策略/旧事实|shared01四父用例通过：新direct/paused完全一致，WAL及checkpoint中途新进程冷恢复续办，原票重放；8个实际冷子进程；真实v224红例原档副本精确恢复235/234；未知版本拒绝|
|独立成本|新同一旅程service[237,5]、export242、余电493；未按实现输出回填预期|
|廉价旧兼容前置|compat01原18/无资产/同槽资产owned拒绝与Journal门通过；World74224B，120568+8456+2048=131072，Journal3488B≤4096；Wait/原负载未改|
|受影响共享|原Core Boot13和core313通过，1 helper ignored、1不相关full-native大布局诊断未运行；Storage36唯一用例通过（初始34，缺MYCITY_QUERY_TEST_WORK两项失败保留，补齐环境仅重跑该两项）；旧Boot两父含88实际退出点通过|
|旧同城/恢复|原资产同城两父及消费后原Generator准时/迟到补货通过，沿原原票/WAL/checkpoint/冷恢复；对应日志分列，不与新策略混算|
|实际默认栈Sim|新ExactRefill、旧Refill、旧Original三分支通过；实际同城住房/工资/Police/工程资产/道路经营/第二次发电，原票重放、途中冷重启与Store对照；新profile另有独立basis/origin/offset断言|

exact01只是中间最小通过，不替代最终源码；其后新增new入口校验与测试，最终全部由shared01验证。compat01与最终差异仅专属Store测试，不影响兼容门；shared01和role01完整代码/fixture/test字节相同，最终manifest与当前文件逐一核对。未无依据重跑751/720日历史大例，也未声明被跳过的full-native当前通过。

新同城第二次发电：实际到期前WIP730=燃料400+冷却30+费300，无提前电产物；实际完工合池468000/basis855，消费后368000/basis673、origin468000/855 offset100000；水462000/231 origin600000/300 offset138000；燃料3000/1200 origin5000/2000 offset2000。总basis2900=1200+231+673+export796。日4公司7现金72995/累计税600/retained317，工资3DAY后无新工时，旧租金等未来责任仍在。

## 资源、环境、保留和未验

Mac16,1，32GiB，macOS26.6.2，Rust1.85.1，HP P900 USB/APFS；原外置SSD唯一target/work，默认栈、未设RUST_MIN_STACK。实际新role arena观察峰55798480B≤原59MiB授权，Engine owned3690456B，完整Store42008787B（包含Engine，不相加）。共同64MiB池3次workspace授予/3次归还，allocation_failures=0；旧Original2/2。

源码ZIP、固定程序、红例、新/旧完整世界/WAL/checkpoint、每次结果及失败日志在`work/boot-partition-20261029`。事前留证预算2GiB（因保留红例及兼容/shared/role固定debug程序从1GiB扩大，不改变游戏门）；当前逻辑文件约1.16GB，不冒充APFS释放空间。24份无关WIP逐文件SHA未变。没有删除源/恢复文件；本target后续验证仍需，不清理用途未定缓存。

实际16GB/native全进程峰值、产品人口/持续速度、长期M0完整压力、Windows/GPU/UI未验。原M0标准不改，新策略通过不等于全城长期自给。

统计起止见[summary](summary.json)，只是本次诊断/验证窗口。具体缺陷158已实际关闭；新业务0/历史补关0，既有18源码包不机械减项。完整开发/源码交付至验收起点未知，不以关闭窗口代替周期。下一按实际有限城市需要选择已有交付，不批量新增模块。
