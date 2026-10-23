# BUDGET-1文档与配置检查

2026-10-23，基线 main 58845df／v0.5.140；独立 codex/city-simulation-budget-design 分支。

交付入口：[城市规模／局部预算](../../city-scale-and-adaptive-budget.md)、[实验与实施计划](../../simulation-budget-calibration-plan.md)、[JSON配置草案](../../inputs/simulation-budget/plan-v1.json)。既有总体入口、地图、性能预算、系统粒度和CPU/GPU专题同步接入，未改游戏源码、冻结输入、阈值或验收。

[静态检查收据](document-validation.json)覆盖15场景／12基底数值行、面积与住房区间公式、各行世界候选数量门、P53/A55域内算术、JSON语法、相对链接及锚点。git diff --check通过。候选数量门通过不等于实际字节／关系合法、可装配或能运行；各行A/I/V保持pending/not_run，当前交通实现上限不足明确阻塞。

advisor先对P53/A55/R54与Timeline实际代码边界只读评估，再完成同一轮设计稿核对，未发现守恒／执行／预算／范围阻断。已采纳精确不变量说明：每次运行内守恒和合法性为零容差；不同近似策略可有不同合法业务路径，只有同模型等价优化要求跨运行状态一致。

未启动编译、游戏、基准、压力或GPU任务；没有16GB设备验收。当前M0失败与未测项不改判。本文检查不构成功能或性能通过，本轮设计到此结束，参数和支持范围按C0—C6与E1—E9实测填写。
