# 实际session发布节奏修复范围

原P-E0与速度冻结标准不变。本阶段只修CLI Run“先等完整间隔再计算”的路径，完成其原一天回归；不是完整30日mandatory测量。

首次新Run立即发布；后续实际release=max(now,已保存release)，成功后next=实际release+本次游戏时长换算。计算、IPC和状态响应占用区间，迟到不积累追赶额度。每次原换算interval<=60秒的拒绝门保留，不能因首次等待0放大请求权限。Step不计墙钟，不改业务规则。

任何已有operation进入Store原actor/payload校验；原票、冲突均不改变release，already_recorded只对匹配持久票为真。普通参数/模型/存储失败不清release；模型错误仍自动暂停。Resume、相同速度重设不清release；明确Pause或真实速度切换开启新时段。未知提交依旧reopen_required，原票查询/重开后再对齐。

墙钟release只在当前进程有效，不持久化、不补离线发展；Store模型时钟/原票/游标仍持久化。状态显示进程内next_release纳秒用于观察，不是跨重开绝对时刻。

验收：原权限/暂停/幂等/存储压力/源协调/日历回归不变；测试实际release状态而非强求有sleep；原24次小时Run一天对照24次Store直接推进，包含实际子进程IPC和持久化。一天实测不得外推30日、不能代替P-E0 mandatory或全L2四档。后续真正30日必须从实际Store创建前计时，保留同fixture/空间包/原终值、按日与全段时间、不扣必要IO，逐条核验原票和完整恢复。
