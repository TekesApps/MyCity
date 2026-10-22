# 原生单PID观测读取后端

候选v0.5.100。仅Rust模型/CLI，无UI、GPU、AgentKit。原3.10GiB门、冻结记录数量/频率和128MiB工作区/64MiB池不变。此阶段不签正式性能或任何占位抵扣。

## 实现

`native_observer.rs`通过精确固定的[libproc 0.14.11 safe API](https://docs.rs/libproc/0.14.11/libproc/pid_rusage/fn.pidrusage.html)读取V4 rusage和BSDInfo。项目unsafe边界不变；依赖内部仍有FFI unsafe。Cargo.lock新增21个包，原包版本/校验和全部保留；绑定生成器、生成结果及实际SDK/关键依赖源SHA见dependency-audit。固定source/lock位于frozen-source.zip，source.json绑定验收exe。使用唯一target。

采样以usage→parent→parent→usage夹取，确认Mach出生/父进程、预期身份及非退出状态；不可读/身份变化作为unknown，无伪造零值。采样区间不是原子资源快照。CPU保留raw，不按字段名假设单位。

候选CLI `native-metrics-probe NEW_DIR`只接受256B固定输入、严格连续序号/已知字段、至多8192请求；64KiB输出缓冲在原2MiB observer固定arena内，无fallback，不重置峰值。输出包括RSS/footprint、CPU原始计数、I/O、身份及进程内采样区间。

## 实测证据

同冻结源码`8b2c2594…a7c91`，独立SDK ctypes与Rust libproc类型的size和关键offset一致。240次活体请求的出生/parent一致，CPU raw值均处于独立前后读数之间；RSS/footprint保留实际波动，不强迫逐次相等。

另实际验证错出生、非法PID、活子进程及父进程wait后的子退出：244请求，3项unknown，无资源假值；退出证明来自真实wait，resource_unavailable本身不等于退出。超长/截断/跳序号/未知字段/第8193条请求各实际exit2，原始stdout/stderr保留。

本机CPU校准raw按125/3转换后相对getrusage为1.000951倍，支持本机Mach ticks解释；不将未缩放raw称为ns，也不外推其他主机。[本机Apple接口来源](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/bsd_kern.c)与SDK、实际校准共同留存。候选进程此次footprint为3,096,864B，含实际2MiB arena，allocation_failures为0；它只完成单PID读取，不能外推完整observer也有同等占用。

probe-result、abi-oracle、crosscheck.jsonl、cpu-unit-calibration及五类失败raw为权威。原始CLI samples/report/heap保留在`work/native-reader-probe-01`，不生成L1压力副本。微秒级单读耗时只证明本探针，不签原20/40ms业务门。

## 未完成及下一依赖

尚未接正式observer：进程发现/出生身份退休/孤儿、压力和swap保护、系统VM数据、磁盘保护、完整时钟因果映射与超时/收尾仍须接齐。正式CLI继续使用既有观测器，不能将本probe取代全组验收，8192 CLI上限也不能拿去证明8小时采样。

下一实施围绕完整原生观测器，不实现rusage一次读数的长期信用租约；全量arena终身触页的问题已明确，drop内部Vec不会退还物理页。组件置换必须指明实际可控制背板/来源/已收费排除/生命周期和余额交接，不能按19MB差额猜扣。物理失败仍保留；三轮、四速、共同8MiB、8小时、真实16GB及完整M0均未通过。

[合同解释更正](advisor-contract-clarification.md)：原冻结采用离散采样，并无全时刻精确互补硬门；用途明确的真实组件可以替代同用途占位，不必仅为离散观测另请用户变更合同。下一阶段仍须实现真正可释放的独立R07背板/组件归属与采样关联、实测调整后全组，不批准预估信用。

## 冻结回归与终态回收

同源全workspace 717.246626秒exit0，严格all-target Clippy/fmt/build均exit0；60个harness汇总、445条通过记录（含nested，不称唯一测试数）。固定源码包/lock/exe未变，advisor核对终态后同意此有限阶段发布，回归耗时不是性能证据。

只清除28份已不含当前强制libproc依赖的旧headless rlib/rmeta缓存，逐文件依据fingerprint记录；当前三份库和54份固定候选文件复核，源/业务/压力原件/日志/exe均保留。分配空间109,776,896B（104.691MiB），同期df独立增加107,479,040B（102.500MiB），不全归因；共享target余1,665,528KiB，work余26,220,136KiB。后续实际workspace build0.266秒通过，没有重建被删旧库。清单见cache-retirement；没有新独立target，也没有压力副本。
