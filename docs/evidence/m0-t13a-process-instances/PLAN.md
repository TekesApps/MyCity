# T13A-3c：长跑实例身份与驱动复制开销

实测前冻结：不改冻结fixture、720小时／723目标／724worker／3011原票、8192页或空气60秒步长。仅做Rust／CLI。正式联合压力、四速度、三轮、8h与16GB仍未通过。本次同原full的30日业务单独实测，不加L1，不冒充完整joint或正式稳态。

CpuLog每实例用原子create_dir创建kind-PID-sequence目录，冲突仅对AlreadyExists有界重试；部分初始化、旧进程和旧PID目录不清空。报告具名PID、process-instance、producer-instance及目录，heap与CPU按显式PID／process-instance配对；目录唯一由原子文件系统创建保证，不依赖时间戳唯一。共享预算文件不重置。反例覆盖同PID连续、重启计数归零、并发冲突、初始化中断留目录、非冲突I/O错误与计数耗尽。

驱动将Ok响应移出而非克隆，payload哈希流式计算，日志借用包装使用固定128KiB容量，编码前拒超过上限的追加；golden行使用固定4096B容量并在复制前拒超限／截断。及时释放冷开／检查响应及输入编码，不改消息序列、原票或对账内容。记录减少不必要的副本，但不据此自动签费用门；模型／通信／验证工作区与诊断仍需分账，现有64MiB未用占位缺口保留。

验证顺序：fmt／release lib与worker、heap、Sim、scale集成反例；固定release二进制；有界observer实际full30日及冷开原票／完整CPU比较。新核验器逐一检查全部诊断目录实例、全部退出记录、raw数量与3011票链；原核验器及旧工件不改。advisor单轮独立审查终态后提交／递增tag推送。吞吐差异只能报告当次，不从无L1与旧有L1计时推出优化收益。
