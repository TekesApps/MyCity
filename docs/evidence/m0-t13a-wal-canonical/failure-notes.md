# 开发期发现

- advisor静态发现直接覆盖RawValue Span会漏掉被覆盖子树里的越界数值，例如{"a":1e400,"a":1}；没有把未补验证的实现发布。增加Valid全输入访问，逐标量和子树执行旧Value等价有效性检查后才覆盖，独立旧Value oracle和新实现均拒绝越界数字及非法surrogate。定向与最终全仓包含此反例。
- 64字节ASCII全unicode转义需384字节内容；旧词法计数额外计入关闭引号。改为不计关闭分隔符，保留转义状态及深度／数值界限，真实WAL previous全转义用例通过。
- 一次定向cargo test命令重复使用--lib被CLI终态拒绝，没有执行测试；纠正为一次--lib并列两个package，不把失败命令计为验证。
- 最终freeze后未改源码；clippy-first为开发期检查，最终结论仅取checks-final同源终态。原始stdout尾部空行保持原样，不能为通过文本whitespace检查而改写日志。
