"""Publish actual migration receipts and update future-run storage rules after retirement."""
import json
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent;P=D/'attempt03'
s=json.loads((P/'switch.json').read_text());r=json.loads((P/'retirement-result.json').read_text());assert r['original_removed']
(D/'STATUS.md').write_text(f'''# work完整迁移至外置SSD（2026-10-22）

原路径 `/Users/frankqin/dev/MyCity/work` 现为符号链接，指向 `/Volumes/HP P900/dev/MyCity/work`。源码仓库仍在内置盘；work和共享target在同一外置HP P900 USB/APFS SSD。后续性能结果必须记录此存储环境，不将旧内置盘IO/恢复耗时作为当前结果；本次恢复耗时仅是操作观察，不是性能门。

迁移前检查进程与完整lsof，无work占用。源目录完整保留，复制后校验 **{s['files']:,}个普通文件** 的完整SHA/长度、权限、所有者、纳秒mtime和源扩展属性；另核对symlink文本及内部硬链接组，保留111个无主历史socket的离线节点类型和metadata，不声称复制了连接状态。190恢复/故障原件SHA逐项一致。[原、目标清单](attempt03/source-manifest.json.gz)及[目标清单](attempt03/destination-manifest.json.gz)完整保留，清单压缩不替代任何恢复原件。

复制与校验遇到的问题均保留失败脚本/收据：Xcode Python缺少xattr API、历史socket不能当普通文件读、rsync扩展属性处理生成AppleDouble错误；修复均在切换前进行。常规文件同步返回0，扩展属性单独复制校验，只删除精确列出的新生成且源清单不存在的AppleDouble旁文件。唯一metadata差异为一份Finder `.DS_Store` 被系统新增com.apple.provenance，源属性值和完整文件SHA仍一致，见os-metadata-exception.json。补证核对work根metadata和全部BSD flags；不宣称独立验证了所有条目的ACL相等。

独立advisor要求的额外核对已完成：work外硬链接以[旧新inode映射](attempt03/external-hardlink-mapping.json)衔接，旧same-inode收据只代表历史；唯一绝对symlink指向的10个固定验收exe逐SHA与可执行权限检查通过。work外原硬链接不删除。新的190份保护清单为[protected-after.json](attempt03/protected-after.json)，今后的回收工具必须使用此份现行身份清单；旧dev/inode清单保留作历史证据，不能原样用于新卷身份断言。

切换后通过原路径，使用固定source32c CLI实际新进程冷读typed第8代、v50旧格式和v3完整原件，三次完整load/hash与恢复后scan退出0，命令/原始日志/报告见recovery-results.json。其余原件也全文件SHA验证，未用hash替代原件。历史已退役快照仍不可直接完整恢复，本次不恢复旧删除文件，也不删除故障pending。

三组实际恢复及独立释放审查后，重新检查两侧无句柄，源副本和目标原有文件全量校验，再删除精确回滚目录work-migration-original-20261022。该副本已不存在，外置SSD完整数据仍在。旧源唯一普通文件分配空间{r['unique_regular_file_allocation_bytes']:,}B，其中无外部硬链接部分{r['allocation_bytes_no_external_links']:,}B；这是迁移原目录的分配空间统计，不能称总数据删除。内置卷可用空间观察变化{r['observational_available_delta']['/Users/frankqin/dev/MyCity']:,}B，另列而不当作APFS独占释放。根目录/元数据及其他任务变化不混计。

本轮只新建少量迁移校验和恢复报告，不新造压力数据或target；保留验收程序、源码/依赖快照、原日志、业务存档、完整恢复输入及故障证据。下一步继续已有模型与CLI验证；16GB、真实8h及完整M0均未由此次迁移获得通过。
''')
p=R/'.gitignore';p.write_text(p.read_text().replace('/work/','/work'))
p=R/'AGENTS.md';t=p.read_text();k='## 构建产物与磁盘占用\n';t=t.replace(k,k+'\n- work原路径`/Users/frankqin/dev/MyCity/work`链接到`/Volumes/HP P900/dev/MyCity/work`。恢复原件/验收证据已完整迁移；现行190原件身份清单为`docs/evidence/work-migration-20261022/attempt03/protected-after.json`，历史dev/inode不能直接当新卷断言。所有新性能记录注明外置HP P900 USB/APFS SSD、work/target实际路径及可用空间；检查work所在卷的容量，不用源码所在卷代替；不能复用旧内置盘IO结论。外置卷未挂载时停止依赖该卷的验证，不自动新建本地work。\n',1);p.write_text(t)
p=R/'docs/validation-artifact-retention.md';t=p.read_text();t=t.replace('# 验证产物回收规则\n','# 验证产物回收规则\n\n**work现行位置与保护身份**：[完整迁移记录](evidence/work-migration-20261022/STATUS.md)。work原路径链接外置HP P900 SSD，原件均在，190份新卷保护身份见该记录；历史same-inode收据不等于当前身份。后续记录实际存储路径/卷/空间，新IO基线注明外置USB/APFS SSD；源码卷df不能代替work所在卷空间。\n',1);p.write_text(t)
p=R/'docs/evidence/m0-t13a-extents-joint-retry-01/run.py';t=p.read_text().replace('stat = os.statvfs(R)','stat = os.statvfs(W.parent)');k='frozen_preflight = verify_retry_directory(D, R)';t=t.replace(k,'''save("storage-environment.json", {
    "work_alias": str(R / "work"),
    "work_actual": str((R / "work").resolve(strict=True)),
    "target_actual": str((R / "target").resolve(strict=True)),
    "storage": "external HP P900 USB/APFS SSD",
    "available_bytes_before": stat.f_bavail * stat.f_frsize,
    "source_volume_not_used_for_work_capacity": True,
    "historical_internal_disk_io_comparison": "not_equivalent",
    "formal_performance": False,
})

'''+k,1);p.write_text(t)
p=R/'docs/evidence/m0-t13a-extents-joint-retry-01/README.md';p.write_text(p.read_text()+'\nAfter work migration, the next live retry checks free space on the resolved work volume, records storage-environment.json with actual work/target paths, and labels HP P900 external USB/APFS SSD IO. Old internal-disk timings are not the current IO baseline. This storage recording change alone does not execute or pass the retry.\n')
p=R/'README.md';t=p.read_text().replace('仓库发布版本：`v0.5.109`','仓库发布版本：`v0.5.110`',1);i=t.index('v0.5.109完成');t=t[:i]+'''v0.5.110完成[work完整外置迁移](docs/evidence/work-migration-20261022/STATUS.md)：全部普通文件逐SHA校验，190恢复/故障原件保留，三组真实新进程冷恢复通过后才删除内置盘副本；原路径符号链接保持，验收exe及旧新inode衔接另核验。后续性能明确使用外置HP P900 USB/APFS SSD，预检按work所在卷检查空间；迁移不等于M0性能通过。

'''+t[i:];p.write_text(t)
print('Migration report and future-run storage rules updated.')
