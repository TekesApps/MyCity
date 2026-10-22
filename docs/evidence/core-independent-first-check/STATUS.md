# 三线首轮独立检查（不是联合验收）

所有命令顺序使用同一canonical target，无新增独立target；准确源码与基线、命令、退出码见各子目录source.zip/result.json。

- transport：实际编译exit101，重复模块注册；失败原样保留，agent之后修复且扩展代码，待新来源实测，不能把修复后的源码与这次结果混用。
- construction：命令exit0但实际匹配0个测试，interpretation明确not_run，不算通过。后续已新增真正Engine/Store原子集成与实际测试文件，尚待新来源验证。
- economy：该首轮来源实际5项纯planner单元测试通过，非结算、非28日经营、非交通联合；之后新增reconcile与共享容量反例，尚待新来源实测。

验收收据必须同时核对退出码、实际运行数量和准确来源，不用空过滤结果制造通过。各线源在各自隔离worktree，root负责共享接口和构建调度；完整0.1目标仍未完成。真实16GB/M0门不受此检查替代。
