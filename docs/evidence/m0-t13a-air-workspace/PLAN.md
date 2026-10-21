# Worker类型化空气工作区

代码前冻结：原acceptance SHA876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508、原full数量/页/数值步/CPU/全部checkpoint字节和保护不变。复用原State算法，支持owned及固定背板+活动长度，fixed backing全部初始化并可安全扫描unused tail；不暴露Vec，不能克隆/替换fixed工作区。worker解码seed直接写背板，不先普通State再复制。原4MiBI/O槽另计，typed候选1MiB必须按实际容量/字符串/padding算足再创建，不足拒绝。broker spawn前释放对应真实页，实际Filled及Child wait归还，部分初始化/补页失败保持failclosed。

Source最多20000/字段最多128²；fixed State私有，恢复/算子/报告/输出不能间接clone Input/State。Input身份流式hash保持原摘要；只输出活动切片，普通CPU/Sim保留owned及Clone语义。核心代码禁止unsafe，全局透明allocator不采用。

控制范围为worker数组/Source/持久字符串/填充及其恢复直写；解析临时/栈和其余Engine/Prepared/helper未闭合前不得宣称完整worker或normal64/common8。即使本场景闭合，不免其余分配前控制。测试分配身份/容量不变、inactive tail/复用/失败回填、超量先于额外value解码拒绝、原字节/完整数值/分页恢复/真实进程死亡与原full联合；期间冻结源码，target复用，无自有benchmark和build同行。advisor审边界和真实结果，未实测不算通过，阶段验证后版本/tag/push。
