# 单轮独立依赖审查

advisor `/root/boot_advisor` 本轮只读结论：

- r07-backing-preflight-01/r07-prefix.bin 已有同源实际prefix读回及同进程重建；未发现后续原路径输入，可独立退役。保留报告、长度/SHA、固定源/exe，不称完整L1恢复。
- r07-coupling-l1-01 第一代尚未实际冷恢复，新v3布局在开发，未有替代基线；保留pressure/reserve，不为清理新增模型运行。
- native-observer-l1-01/02成功短测及已退役pressure的旧smoke-L2正常首代native/meter，仅在与既有gzip实际解压逐字节一致、原index一致后可精确退役；保留index/raw/report。
- 所有pending/negative、native-observer-faults、五份旧保护基线及新R07快照不随之退役。旧t13a1b用途不明继续保留。

这是依赖判断；实际逐字节核验及删除结果以plan/result为准。未运行模型或构建。
