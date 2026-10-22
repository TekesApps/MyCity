# Advisor独立审查，完整回归终态待补

审查者 `/root/boot_advisor`，2026-10-22。只读实际检查smoke、逐帧审计、跨60s结果、owner故障摘要、两次冷恢复报告及专属组代码；未执行删除或额外负载。

未发现新功能发布阻断，**仍须等同冻结源码的完整回归终态**。可发布范围仅原生wrapper接入、errno修复、周期采集和失主终止，不能签正式性能门。

88帧85完整/3未知保留；248帧跨60s，周期项在第0/240帧。活跃计算证据是已输出65,528B targets且CPUraw增长的Reference helper，不能叫空气worker计算故障。初始化、partial pressure写入、已记录组成员退出和外部组存活有记录；缺footer不当正常归还，后代不得逃组的限制保留。

footprint超限9,758,898B继续失败观察，置换0；5,505,024B诊断账不自动升级为正式共同预算通过。safe入口清errno只修已发现的旧值污染，库内调用歧义仍需Unknown。

## 终态后的回收依赖释放

全部测试/探针终态、占用复查通过且有逐路径清单后，可以退役：

1. native-observer-l1-01/02的pressure/reserve：均实际冷恢复通过，不作下轮完整恢复输入。
2. 本阶段owner-loss产生的完整或partial合成pressure/reserve：仅证明失主时终止，不作为崩溃恢复输入。

保留触发长度/分配信息、身份及终止证据、raw、index、typed组件、partial CPU targets、业务及固定source/lock/exe。旧typed7/8、v50、report-lifetime首代/pending原件不动。删除后新目录须标不能直接完整恢复。此记录是依赖审查许可，不代表已经删除；当前回归仍在运行。
