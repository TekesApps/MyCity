# advisor独立容量审查

先只读检查完整生命周期，指出prepare的Timeline/Background深副本、Capture构造与frozen验证、Accept候选/reference/result、staged重读、checkpoint验证、genesis与当前恢复、WAL DOM及普通Compare双模型／双编码都必须计入。Region/Boot/Exchange固定本体不得再冒充深堆表，Engine内的Timeline不重复加，但Prepared副本须另计。

初稿实质缺项为Opening：64KiB文件不能证明解码对象容量。要求拆分类型化表／头／字符串或直接seed目标Engine。主任务已采用类型化表并复用codec。

最终advisor实际运行固定候选 `work/candidate-binaries/model-capacity/mycity-headless model-capacity-plan`，输出与归档plan.json完全一致，并独立重算各行和各场景。目标需求39,415,656B，原预算67,108,864B，代数余量27,693,208B。未发现仍需补充的实质容量项，可以作为后续完整ModelWorkspace实施依据。

结论仅确认目标背板拓扑可装；runtime_grant_accepted=false正确保留，余量不是当前动态实现的额外分配授权。五个Air槽、流式普通Compare和receipt、固定metadata限制与所有入口显式借用仍是实现前提。advisor未构建或运行模型负载。
