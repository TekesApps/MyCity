# 中间失败与已修复阻断

core-first.txt中的空气超额行反例失败来自错误文本改变（air_rows变成model_rows_extra_record），并非接受了额外记录。已恢复原air_rows文本，随后core-second.txt全core通过。

advisor发现MAX free规则的实际语义／旧存档阻断：MAX-1移除后会递增到MAX并进入free，最后一代仍合法可用；不能在恢复中一律拒绝。已保留旧free成员关系，加入MAX-1 remove→实际序列化重开→MAX分配→移除→再重开不再复用的完整回归。未将错误中间版本发布，也不把第二轮core结果用作最终来源。

clippy-first.txt为测试构造转义字符串时format_collect警告，改为直接write追加。最终以冻结163文件的全仓同源终态为准；中间日志不是性能或16GB证据。
