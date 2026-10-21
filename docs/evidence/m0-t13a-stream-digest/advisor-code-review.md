# Advisor只读代码审查

既有授权advisor /root/boot_advisor未发现生产逻辑、限额或分块哈希新阻断。检查HashWriter大块直通/缓冲补满/flush/finish字节顺序及空输入；checkpoint_digest复用checkpoint_to，长度及摘要来自同一次规范编码；capture/Accept仍有ASSET_LIMIT、身份/输入/CPU reference，storage仍做完整checkpoint验证及原资产哈希校验。

测试使用独立Sha256::digest(raw)，不为测试开放私有hash；原编译失败等待真实终态后修复，新来源独立封存。随后另修测试格页误用8192超过原1024上限，仅修fixture，不提高原上限。两次失败各自来源/日志保留，不借旧源通过结果。

嵌套两份8KiB逻辑摘要缓冲属原生栈开销，实际栈/foreign仍单列；State/reference/Prepared等EXTRA保留。尚不签性能改善/normal64/common8/完整联合。
