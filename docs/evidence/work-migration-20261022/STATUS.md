# work完整迁移至外置SSD（2026-10-22）

原路径 `/Users/frankqin/dev/MyCity/work` 现为符号链接，指向 `/Volumes/HP P900/dev/MyCity/work`。源码仓库仍在内置盘；work和共享target在同一外置HP P900 USB/APFS SSD。后续性能结果必须记录此存储环境，不将旧内置盘IO/恢复耗时作为当前结果；本次恢复耗时仅是操作观察，不是性能门。

迁移前检查进程与完整lsof，无work占用。源目录完整保留，复制后校验 **120,166个普通文件** 的完整SHA/长度、权限、所有者、纳秒mtime和源扩展属性；另核对symlink文本及内部硬链接组，保留111个无主历史socket的离线节点类型和metadata，不声称复制了连接状态。190恢复/故障原件SHA逐项一致。[原、目标清单](attempt03/source-manifest.json.gz)及[目标清单](attempt03/destination-manifest.json.gz)完整保留，清单压缩不替代任何恢复原件。

复制与校验遇到的问题均保留失败脚本/收据：Xcode Python缺少xattr API、历史socket不能当普通文件读、rsync扩展属性处理生成AppleDouble错误；修复均在切换前进行。常规文件同步返回0，扩展属性单独复制校验，只删除精确列出的新生成且源清单不存在的AppleDouble旁文件。唯一metadata差异为一份Finder `.DS_Store` 被系统新增com.apple.provenance，源属性值和完整文件SHA仍一致，见os-metadata-exception.json。补证核对work根metadata和全部BSD flags；不宣称独立验证了所有条目的ACL相等。

独立advisor要求的额外核对已完成：work外硬链接以[旧新inode映射](attempt03/external-hardlink-mapping.json)衔接，旧same-inode收据只代表历史；唯一绝对symlink指向的10个固定验收exe逐SHA与可执行权限检查通过。work外原硬链接不删除。新的190份保护清单为[protected-after.json](attempt03/protected-after.json)，今后的回收工具必须使用此份现行身份清单；旧dev/inode清单保留作历史证据，不能原样用于新卷身份断言。

切换后通过原路径，使用固定source32c CLI实际新进程冷读typed第8代、v50旧格式和v3完整原件，三次完整load/hash与恢复后scan退出0，命令/原始日志/报告见recovery-results.json。其余原件也全文件SHA验证，未用hash替代原件。历史已退役快照仍不可直接完整恢复，本次不恢复旧删除文件，也不删除故障pending。

三组实际恢复及独立释放审查后，重新检查两侧无句柄，源副本和目标原有文件全量校验，再删除精确回滚目录work-migration-original-20261022。该副本已不存在，外置SSD完整数据仍在。旧源唯一普通文件分配空间27,301,773,312B，其中无外部硬链接部分27,296,321,536B；这是迁移原目录的分配空间统计，不能称总数据删除。内置卷可用空间观察变化27,359,772,672B，另列而不当作APFS独占释放。根目录/元数据及其他任务变化不混计。

本轮只新建少量迁移校验和恢复报告，不新造压力数据或target；保留验收程序、源码/依赖快照、原日志、业务存档、完整恢复输入及故障证据。下一步继续已有模型与CLI验证；16GB、真实8h及完整M0均未由此次迁移获得通过。

最终删除守卫发现恢复CLI刷新了v3输入layout父目录的运行态r07-state.json（不在190快照文件内），删除前停止。原版和恢复新版均保留为historical-r07-state-before.json、recovery-r07-state-after.json；历史控制文件及目录mtime已恢复，再经两侧全内容和190原件校验后才删除内置副本。见attempt03/recovery-runtime-ack-transition.json与恢复收据，未忽略内容差异。

公开仓库的进程清单仅发布PID/PPID/elapsed/executable派生版，完整原始参数保存在外置work迁移证据中，逐份SHA映射见process-evidence-storage.json，避免无关应用参数进入公开仓库。
