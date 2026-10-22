# 共享target迁移（2026-10-22）

`/Users/frankqin/dev/MyCity/target` 现为符号链接，目标 `/Volumes/HP P900/dev/MyCity/target`。未修改Cargo target配置；所有使用原共享路径的构建继续复用这一个target。外置卷不可用时先恢复挂载，不另建本地缓存或改成多套target。

迁移前无本项目进程或target打开句柄，目标目录不存在。复制后逐文件核对 **9,167个文件** 的完整SHA、长度、权限和纳秒mtime，并核对内部硬链接组。首次rsync严格检查因纳秒mtime被截断而在切换前停止；保留原错误记录，补回精确mtime后全量重核，再将原目录改名、创建链接。复制时原目录完整保留。

通过原路径实际运行 `cargo +1.85.1 test --release --locked --offline -p mycity-core --lib --verbose`，106/106通过。首次检测dependency info变化并仅重编core；第二次106/106再次通过，core及依赖为Fresh、无编译，确认缓存可复用。这是迁移回归，不是16GB或M0性能验收。

验证后再次检查源副本/目标目录打开句柄、重新核验完整源副本，才删除精确回滚目录 `target-migration-original-20261022`，现在该副本已不存在。原源码、WIP、work内存档/恢复原件、固定依赖和验收exe未删除；外置target继续保留。已删除源副本的唯一inode分配空间为 **2,326,003,712B（2.166GiB）**，但同等缓存已迁入外置盘，属于迁移，不是总数据回收。内置卷可用空间观察增加2,172,719,104B；不把其等同APFS独占释放。本轮独立历史组件回收见[报告](../disk-reclamation-focused-20261022/STATUS.md)，不混算两项。

逐文件迁移清单、复制/切换/占用收据、回归原始日志及源副本退役结果均在本目录。后续阶段复用此target，每次终态按[回收规则](../../validation-artifact-retention.md)处理无依赖中间产物；用途未确认或恢复仍需的完整原件保留。

阶段结束后两份逐文件JSON清单已逐字节无损gzip归档，原执行路径及SHA映射见manifest-storage.json；保留原执行脚本与收据，不能原样重跑已完成的迁移/删除脚本。此处压缩的是校验清单，不涉及任何恢复快照。
