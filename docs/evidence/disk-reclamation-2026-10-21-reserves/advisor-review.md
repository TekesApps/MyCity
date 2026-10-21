# 独立依赖审查

审查者：既有 boot_advisor，2026-10-21，只读。审查不是性能验收。

确认 t13a64-joint-01、t13a65-joint-01、t13a65-joint-02 各 snapshot-000001/000002 中六份 work-reserve.bin 均为134,217,728B、单链接，对应 pressure.bin 已退役；未找到后续实际恢复输入依赖。当前兼容脚本使用v66 business完整world及fault-reference，不使用这些layout reserve。可按精确清单退役；补availability，不改原manifest或历史结果。

native／meter等本轮保留：仍承担组件核验及历史故障证据；尚未逐项确认替代归档和全部引用，不因“可重建”直接删除。
