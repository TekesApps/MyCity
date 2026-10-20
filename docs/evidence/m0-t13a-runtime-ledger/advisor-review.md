# Advisor独立审查：T13A-3f实际full

2026-10-21。独立advisor实际运行verify_full.py work/t13a3f-full-01及static_check.py，均退出0，并读取原始采样、退出记录及核验代码。结论：**可以按“单次full诊断与驱动费用实测完成”提交v0.5.45；本限定阶段未发现新的正确性阻断，补齐v44缺少的真实full Sim路径。**

135份源码、3721份归档与hash绑定一致；723目标、724个worker和3011张连续原票均核实。实际完整Engine包含metadata等于CPU，CPU最终字节及全部724份输入／候选hash与v42一致。730份正常退出记录按角色闭合，724个槽owner全归还且触页余额恢复。

驱动整进程requested heap峰535289B，加65536B尾部为600825B。可以确认本次运行低于候选1MiB驱动费用，不能推广成所有输入的强制上限。

advisor另独立发现并要求具体披露：除1334次footprint缺测，两个短命helper PID81075／81078未进入任何RSS帧；它们有正常heap footer，仍不能补出完整RSS峰。参考helper有140帧，末帧仅观察器和采样工具，未见残留模型进程。[覆盖明细](process-coverage.json)由原始采样再次复算。

共同8MiB、helper256KiB、外部64MiB、L1联合压力、正式性能、8h和真实16GB均继续未签收。观察器既有固定诊断收费不能替代共同总账；v43压力失败继续保留，不改为成功。
