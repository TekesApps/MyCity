# 首次原生整组观测接入：入口errno污染，不能验收

冻结来源3ec4015365804f2604e74a8ecc72026877248108e7b7437eba5fa647ea7f8d71、固定exe及全部原始样本保留。L1原规模短测实际22.215秒，87帧、6身份；模型退出0，原生wrapper退出0仅代表流程结束。87帧无完整帧，227项enumeration_failed、另4项瞬时身份不确定。不能将passed_measurement_wrapper误读为完整观测或性能通过。

实测peak footprint 3,338,424,088B仍高于原normal 3,328,599,654B。2MiB observer授权、累计请求峰550,728B、combined diagnostic 5,505,024B是分配账，不是整组物理预算通过。置换0，8h/16GB未测，完整M0未完成。

发现锁定libproc0.14.11空子列表返回0时检查旧errno，而入口未清零。正常leaf反复误报，但本轮不删去这些Unknown。后续来源通过safe errno清入口污染；不能宣称消除库内所有errno歧义。修复、重测和当前可用性见[后续记录](../m0-t13a-native-observer-errno/STATUS.md)。原frozen-source/lock/exe、preflight、SDK raw和失败均不覆盖。

本轮唯一短测pressure/reserve已在[v0.5.102终态回收](../m0-t13a-native-observer-errno/retirement/availability.json)逐文件退役，目录现在不能直接完整恢复。原组件、index、SDK raw、失败、源/lock/exe继续保留；先前实际冷恢复是历史运行证据，不表示现存原路径仍完整。
