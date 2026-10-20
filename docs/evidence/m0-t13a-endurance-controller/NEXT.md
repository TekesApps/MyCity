# 当前状态与下一依赖

已发布仍v0.5.48 main25fc6d1；本阶段未提交。原ce9efd full debug63357终态101，真实栈溢出，原来源/脱敏栈/111文件失败工件在attempt1-source，旧smoke01按该旧来源保留。

当前2b378e37683f390f5be24459b933c24b118e8d6071ca5ab116c1f3e8bb9ff885，144源码文件，Box<World>最小修复+精确owned计数，fixture原锚点旧线格式不变。默认栈旧锚点debug测试31224终态0(0.39s)，原scale_rare debug17638终态0(153.54s)；全仓release18188终态0，316通过/0失败/0忽略，clippy/fmt/build全部终态0。source.json绑定新binary1fdd71...；源码/fixtures保持不动直到测量结束。

最终59179已终态0：work/t13a49-smoke02包装432.112秒/业务358.181秒，原full/720h/723targets/2880query/96save、故障及完整World核验通过。verify/diagnostic/latency/coverage/归档和同源static核验均完成；只短墙钟功能、无L1，不签8h/正式性能。阶段收尾v0.5.49提交/附注tag/原子push。旧数据复核显式第二参数attempt1-source，不能用新source.json替旧缺测。

新核验器区分OS失败wait和已写heap footer：Cells故障必缺footer，提交后worker可能先写尾部再被终止，按实际事件核对。当前smoke如PID含糊复用则拒绝；正式8h前补初始化identity链（JOINT_PLAN），不能用短期无复用替代。

下一主线：Rust joint_driver去Python启动器，L1覆盖完整业务8h时轴，逐实例共同诊断账。普通Engine/serde仍额外压力，normal3.10GiB/常规64MiB未闭合，不减历史峰补账。参见JOINT_PLAN；速度门周期预算排程见SPEED_PLAN，8h末端释放保持不变。原三轮稳态/四档/实际8h/真实16GB仍未签；当前32GiB不能代替16GB，完整M0 not_accepted。只Rust模型与CLI。
