当前WIP source ce9efd...；已发布v0.5.48 main25fc6d1。smoke60441终态0，独立verify2/advisor通过；全仓回归63357仍在跑（debug joint_sim原7日），不得提前声称315通过/提交。用write_stdin revalidate63357；代码与fixture保持冻结直到结束。核验器首轮正常heap假设计数错误保留并已按实际两个worker wait_failure修正。source.zip/bin/source.json/原工件已归档。

# 当前接入与后续依赖

本阶段只实现原长时控制器，正式8h未运行。源码与二进制见source.json；实际短墙钟满负载运行以原始报告为准，未结束不能签通过。全仓回归、实测句柄和终态应在STATUS中记实。

原下一依赖：将现有Rust joint_driver的常规池与L1布局托管接入该控制器，替换此阶段的Python功能runner，保持共同8MiB原分配。L1必须覆盖完整业务8h，不能因先初始化/先开钟而提前结束。布局现行最大28800s的保护要明确区分正式测量窗与前后初始化/收尾；若增加启动握手或额外保持时间，原1s扫描/300s快照与完整数量不可缩小，额外成本仍实测。先做同源短联合与预算检查，然后原三轮稳态/四档和实际8h；不把此smoke或过去600s联合诊断当正式门。

当前未闭合：普通Engine额外压力/常规64MiB精确所有权、共同8MiB（Python热工作区不能冒充0.25MiB Rust supervisor）、正式10类与排队/保存/恢复期限、正式三轮稳态与四档速度、实际8h、真实16GB。当前设备32GiB，16GB保持not_run。完整M0 not_accepted。

本次继续已确认63357仍真实运行，joint_sim PID751/Sim762的持久序号由370推进到610；未重启。joint_driver/L1覆盖与精确64MiB的下一接入点已具体记于JOINT_PLAN.md，仍待实施。

更新：63357仍在运行；joint_sim原7日回归已终态通过，后续normal_pool6项通过，已进入scale_model_task原Small30日流式参考测试（实际子进程R）。不得把当前整轮当终态。下一阶段新测量与回归优先使用release以贴合M0实测二进制；本轮已启动debug继续等待，不取消、不改输入。

更新：原Small30日流式参考测试已通过（683.90s），63357进入scale_rare，仍未整轮终态。新增latency_audit精确核对49.76MB原raw及summary子区域，未混producer/role/partial；捕获接纳和Region有越门，参考STATUS具体数值，不能宣称正式性能通过。并行debug回归影响不作扣除；后续规定环境重测。
