# Advisor 独立依赖审查

2026-10-22，复用 boot_advisor 只读核查，未运行模型或改变文件。结论：未发现所列25种组件仍承担后续实际恢复输入的依赖；可在逐文件确认旧index长度／SHA、现存gzip完整解压字节、无占用及单链接后退役两代共50文件。最终逐字节核验由主线程执行，本记录不将其描述为advisor实测。

- 当前role-arena的diagnostic_audit.py只读取v70 index；crates没有v70原路径依赖。
- 历史清理脚本以v70作CANON并检查原文件。删除后这些脚本不能按历史原路径完整复跑；必须说明归档映射，不宣称无影响。
- 旧NEXT.md中的v70完整保留承诺须由当前availability覆盖；v85两代已实际通过并替代，v70压力此前已退役。
- index、models、preview、native-restore-bound、业务／故障及v85两代、v50完整原件继续保留。

补充直接检查：旧background-resident/verify.py读取历史日志和目录集合，其打印的latest_full_layout_cold_restore=true是历史报告事实；不是现在重新从完整原件执行恢复的证明。不得将该静态脚本的旧输出解释为v70现在可恢复。

## 实际结果独立审查

advisor只读核对plan与deleted完全相同、50个唯一原路径均已不存在、分配块401,145,856B与work du下降一致、df单列、fsync及可用性声明正确，未发现发布阻断。未运行负载／构建或大文件重哈希。

非阻断建议为脚本直接断言protected-before／after相等；本次documentation-check.json已实际比较通过，随后也将断言加入脚本供后续检查。修改不改变已执行清单，不重复执行删除。
