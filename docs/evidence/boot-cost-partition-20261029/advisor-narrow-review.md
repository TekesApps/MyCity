# #158 原advisor单次修复窄复核

角色 /root/three_module_delivery_advisor，read-only、not_run。原因是已实际复现正常暂停成本差异，并核对其明确版本修复；不是循环全产品审查。

结论：生产diff符合Issue158有限合同，未见新的源码阻断。新内部输入及电水出口统一frozen-lot，旧fuel和其他旧分支未改；put_pool仅真实产物合池重建origin；new/shape/validate检查六pool，Service两binding允许新version，Town/工程沿原export。

测试新[237,5]/export242/余电493与旧235/234、原档兼容明确区分。Sim测试oracle已将冷却水30与总job.cost730分清（400+30+300）；这项源码审查纠正不表示运行通过。

尚需root实际ExactRefill共同城市/默认栈/原18及Journal/normal资源终态，以及原票和冷恢复。本修复只承诺同consumer顺序及pool边界，不外推跨生产时点或交换consumer。
