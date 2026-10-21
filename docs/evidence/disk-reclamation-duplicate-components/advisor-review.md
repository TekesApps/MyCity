# Advisor 独立审查

来源：现有 `/root/boot_advisor` 两次独立只读回复，由主线程保存。未由 advisor 执行删除、解压或 SHA 实测；后者由主线程脚本执行，不能混为独立实测。

第一轮：完整字节归档＋逐路径映射可解决旧 native／meter 证据保留疑义。必须排除 v70 两代、v50 旧格式完整恢复输入、全部 .pending、negative fixture、非单链接文件，以及业务 world／兼容副本。删除前须完整归档并持久化父目录，实际解压核对长度和 SHA；保留每个原路径、快照身份、原 index hash、文件长度／hash、归档及理由。不能据此恢复退役 pressure 或宣称完整快照可用。

第二轮实际读取 plan.json、完整 reclaim.py 和脚本引用清单，核对路径类别、排除项、映射及执行顺序：46 文件未发现恢复依赖阻断，删除分配空间 2,861,096,960 B，归档新增 26,718,208 B，预计净减少 2,834,378,752 B。执行前须将最终 plan／审查记录及 evidence 目录 fsync，再 unlink。主线程据此修正脚本：save 持久化文件和目录，执行前额外 fsync 全部证据文件及目录。

引用清单并非全部失效：verify_legacy.py／verify_codec.py 对旧 business fault-reference 仍有活跃依赖。这些业务文件不在删除清单中且完整保留。此次许可不扩展到其他组件。

## 终态独立复核

执行后 advisor 读取 result.json、deleted.json、protected-after.json、STATUS.md、source-wip-after.json，并与精确计划核对：46 文件／46 inode 完全一致且路径已不存在，净分配量计算正确，du／df 准备期与删除窗口口径分开；完整布局各 29 文件 SHA 核验记录、其余 92,533 文件元数据及源码 WIP 不变记录与报告一致。未发现实质遗漏或归因错误，可作为单独清理文档阶段发布 v0.5.78。

advisor 未重新读取全部大文件计算 SHA、未独立重算源码 WIP 指纹，这部分依据主线程核验记录；未构建、运行负载或修改文件。清理不构成模型／性能验收通过。
