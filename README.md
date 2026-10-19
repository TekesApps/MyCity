# MyCity

文档发布版本：`v0.5.1`；v0.5.0首期非代码设计基线已冻结，新增交通扩展需求仍为候选。Rust + wgpu／macOS城市治理游戏；仓库目前只有设计、配置和合成输入，没有游戏实现或实机性能通过记录。

**从[首版设计基线](docs/FIRST_RELEASE_BASELINE.md)开始。** 它统一当前有效规则、23类范围、延期内容、两套合成世界输入、16GB预算、开发顺序、21项交付与待验证假设；最终审查状态也在同一处。

玩家主要用Agent文本命令向City Team提出目标，未来语音复用。地图可以精确选每栋建筑、道路局部或区域；团队依法完成调查、居民／业主沟通、补偿安置、审批、预算及真实工程。所有受理需求在事项工作台显示责任、缺项和进度，快进只推进同一游戏日历。

- [交通设施扩展目录](docs/transport-facility-expansion-catalog.md)：公交／客货轨道／轮渡港口、小中大机场、独立停车楼与配套机制，未自动加入原23类。
- [游戏策划与玩家体验](docs/first-release-game-design.md)：循环、成长、议会压力、引导、UI与保存。
- [Agent选择与调整](docs/agent-command-targeting-and-editing.md)／[事项工作台](docs/civic-project-workflow.md#9-事项工作台与程序性推进-w50)：精准定位、多方办理与工程进度。
- [23类配置与数值](docs/first-release-data-and-content.md)／[生命周期](docs/first-release-lifecycle-inputs-and-exit.md)：属性、资源、成本、废物和退出。
- [统一合成输入](docs/first-release-unified-world-inputs.md)：小镇与中图的具体数据／确定构造，不是可运行存档。
- [技术实施蓝图](docs/first-release-implementation-blueprint.md)：模块／时钟／数据权威、CPU-GPU／保存／预算和验收。
- [历史资料索引](DOCUMENT_HISTORY.md)：早期设计、案例与批次审查，当前生效口径以基线为准。

仓库：[TekesApps/MyCity](https://github.com/TekesApps/MyCity)，公开、[MIT License](LICENSE)。文档提交递增附注tag并推送，见[协作规范](AGENTS.md)。
