# v0.5.132：受限整数编码优化与来源核对

本阶段合入规范编码的短整数快路径：已完成完整JSON校验后，仅原样写出0及最多18位的非零整数；-0、小数、指数及19位以上数字沿用原serde Number路径。完整输入校验、重复键规则、排序、深度/容量上限和持久化格式保持。没有新缓存、常驻字段、逐数字计时或预算扩容。

## 实际验证与来源

| 来源/范围 | 实际结果 | 可签边界 |
| --- | --- | --- |
| 旧隔离候选d3d… | core 137、headless 93、storage 21库测试与严格检查通过 | 受限实现及兼容，不签联合性能 |
| ABBAAB真实Engine探针 | 每进程热身后7次完整Codec::hash，所有SHA一致；基线/候选中位38300125/36433083ns | 单份2507418字节Engine的局部观察约4.87%，不是全模型提速 |
| 旧d3d…完整30日功能 | 720小时、723目标、724窗口、64工资、20000债项、8交付、16周期义务及完整Engine/冷恢复对照通过 | 经营188.402441333秒，driver 227.258007042秒；没有证明快进改善 |
| 最终a629…集成 | 137+93+21库测试、20项Store建设/市场/停业/公用恢复、严格全workspace Clippy、格式及构建通过 | 各命令实测范围，未声称整仓所有测试执行 |
| 最终固定CLI | 五种原有lifelines模式、默认六组换岗经济场景退出0 | 有限交通/建设/Boot及独立就业例回归，尚非新换岗真实联合场景 |

最终正确来源为`a6290a562362b7ea21379726f901a61cf54d32e8d9fe2059bccf3fe44449d3d8`，264项源码和12项额外固定文档输入，共276项。最终headless exe SHA为`8b27415ce6dc6a07660b31dd1a31bdb549615347a32bb3675b6582bad57acd26`，完整ZIP SHA为`f083a1fe9521f390c16d4665c61ca4f6b2c5461cdf92d7dea52d5ff238391094`。见[最终二进制记录](integration/integer-integrated-headless.json)与[示例记录](integration/integer-integrated-examples.json)。112实际运行验证内嵌来源一致，不只依靠文件名。

旧隔离候选为`d3d463d45db7cb589a68c7d22ac7781debea6c75bad6d1225df5cb19c6aec17e`，固定exe `453c6215f1637458847e4882116e0046ab43a1da6b7fd0b0529550cadd7ba282`。其已有crate文件与最终源码逐字相同，最终多一个town_employment example，见[逐文件比较](integration/integer-integration-source-comparison.json)。来源仍不同，旧30日记录不改写成最终源码的运行。

[微测原始结果](micro/summary.json)、[完整功能独立核对](old-functional/verification.json)与[runs命令原日志](runs/)分别保留。102/103固定范围未含编译期文档，而106起完整固定12份额外文档；既有Rust字节未改变。固定ZIP、验收exe及完整恢复world仍在外置work，发布清单不是恢复原件。

## 真实失败与性能边界

104严格检查因sparse checkout缺已提交的reference-worlds.json停止；105补该文件后又发现旧v106兼容输入缺失。随后查全Rust include_str/include_bytes引用，恢复全部7份编译期文档，并用run-command-v2.py与5份专题文档一同冻结；106严格检查成功。两次失败原样保留，不称模型失败，也不称原命令通过。

111来源核对exit1揭露root的收据错误：初次按Pathlib组件顺序计算指纹，与build.rs路径字符串排序不同，错误写出b2ca…。源码、ZIP、exe字节没有变化；[修正收据](integration/integer-source-identity-correction.json)及111初始记录保留。112按正确a629…实际内嵌校验退出0。

旧候选完整30日功能运行没有执行quiet准入（not_run_for_functional_validation），22次外部编译采样、180个RSS和180个footprint进程缺测均保留。报告的unpaced target_met不是1秒/日性能门。此次188秒与旧结果接近，不能称整体性能提升；不签clean性能、物理内存、L1联合或速度门。

另一次int-full-01在quiet准入被外部编译持续阻断；root仅终止自己已核验身份的准入helper，控制器exit1、model_started=false。见[终态](blocked-full/terminal.json)与[中止意图](blocked-full/abort-intent.json)。没有启动模型，不作为模型失败或运行完成。原始外部进程argv仅留外置盘，未发布。

## 资源、原件与回收

测试机为32GiB Apple M4，work及共享target均在HP P900 USB/APFS外置SSD；不套用旧内置盘IO结论。此快路径无新增常驻容量，原64MiB/common8MiB限制不扩大，但不是完整角色物理预算通过。

[回收收据](stage132-retention.json)：106–112七份相同终态ZIP中六份替换为硬链接，原路径及完整字节全部保留。文件分配空间减少25460736B，SSD可用空间观察增加25395200B，不称APFS独占回收。未删除源码、依赖、验收exe、必要日志、压力/业务恢复原件，190份迁移保护原件未动；没有新建独立target。

完整原件保留于`/Volumes/HP P900/dev/MyCity/work/`，包括int-func-01的world/checkpoint/reference与固定source ZIP/exe。hash不替代仍需恢复的文件。发布映射见[publication-manifest.json](publication-manifest.json)，可用[make_manifest.py](make_manifest.py)重新生成。

M0保持not_accepted：原normal仍超限，正式三轮/四速和真正8小时未完成；真实16GB设备门为not_run。新来源的正式联合smoke/8h控制器单独准备，不能沿用旧smoke或预算当作新来源通过。
