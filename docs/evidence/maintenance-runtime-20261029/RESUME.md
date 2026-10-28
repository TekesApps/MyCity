# #159 停工续办的工资内核与旧业务兼容门

v0.5.228进行中检查点；159/36/117开放，不是维护业务验收。此前v227来源/准备结果保留在[STATUS](STATUS.md)，本次生产只改facility_work.rs；新现场、停机和实际材料/P5/费用/Store维护authority仍未接通。

## 必要变化

实际源码原约束 work_seconds=settled-started、Stop终态，不支持停工后再续。维护专属ResumeMaintenance现在保留原begin/started、已挣/已付工资/余数与实际工作量，新增空档累计秒和最近续办原来源（上次停止/新成员活动/当前资格读取）。时间总量=实际工作+空档，空档不产生工资；旧生产合同仍不可续办。

新摘要字段仅维护使用；旧记录为0/None、序列化省略，读取旧字段缺省。sizeof Runtime/candidate/restore自动包含新增物理字段，编码最大包络也计入最坏维护source，不以省略必要状态或增大8KiB换通过。结构验证仍不代替原Store回执认证，未知更早细节不补造。没有第二劳动/资金账。

## 固定程序实际结果

- resume01先原18/128KiB、原Journal4096通过，门和输入未改；内核20及旧实际设备劳动13通过。
- 独立例：4人先实际1800秒，停600秒后换新成员活动再工作1800秒，总工3600秒/工资8000分，不把600空档当工资；首次起点、已付1000与余数保留，后付7000清余额。续办中DTO恢复后继续一致；原生产contract拒绝同类续办，旧wire无新增默认key。这不是实际维护现场/Stock/Store证明。
- compat-store01原FacilityWork的WAL→两个实际冷进程、checkpoint、Stop余数与热原票重试通过；丢回复原提交一次恢复通过。原正常Water V2角色首次池启动exit2，stderr实际为`path must be shorter than SUN_LEN`，未执行模型。失败全目录/程序/源码留存。
- 仅改运行目录到原HP P900短路径`work/m159w02`，同一固定exe不重编译、不改负载/栈/预算，compat-store02正常角色通过：实际生产、库存Move、原RunDue、保存/冷重启、原票与池返回均沿原测试。

实际角色数值/源SHA/命令见[resume-summary](resume-summary.json)，59MiB原grant、allocation_failures0，64MiB共同池3次workspace授予/3次归还。它是原电/水运行兼容，不表示新维护已在normal角色执行。Store父子owned不能重复加计。

源码/固定程序/原世界/WAL/checkpoint/失败日志位于`work/maintenance-runtime-20261029`，正常角色完整目录另在同外卷`work/m159w02`；合计仍纳入事前2GiB留证预算。没有新target/tree/timer，不清原件或用途未定缓存，24无关WIP保持。Mac16,1/32GiB/macOS26.6.2/Rust1.85.1，HP P900 USB/APFS；实际16GB/native总峰/长期/速度/GPU/UI未验。

本阶段业务/历史/缺陷关闭0，18包未核销。起止是本次构建终态至验证窗口，不是可靠开发周期。下一同一root共享159接现场与临时停机工单，再实际工料/费用/恢复；不扩大派工，不用这些专项关闭Issue。
