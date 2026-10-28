# Issue #102 resource fact test preparation

源码准备基线：main `61267fb`（v211），读取 root 在同一 checkout 中的共同资源观察 WIP；本交付没有冻结或覆盖生产文件。唯一写入为 `crates/city-headless/tests/task_resource_observation.rs` 与本文件。主代理负责集成、Issue 回填及唯一串行构建/验证；本子任务未编译、运行、格式化、提交、推送或创建 PR。

## 已准备的验收入口

- `pooled_sim_and_air_observe_existing_credit_and_retirement_history`：真实 Pool 与 pooled Sim；Status 中 Sim 原 Authorize receipt、59MiB 原 arena、pool boot/generation。精确 Air 页暂停时 legacy current owner 与 broker lease、CPU execution allocation 一致，原固定合同容量为 `normal_pool::GRANT_BYTES`。取消经原退役路径后共同 Status current owner 为空而历史 CPU/容量保持；显式新 attempt 的票、进程 instance、lease identity 均更新。Sim Checkpoint/Stop 后原池 64MiB 完整归还。
- `standalone_sim_local_arena_and_unpooled_air_have_no_invented_pool_capacity`：Sim admission 明确为本地激活；未入池 Air current/historical 容量为 unknown 且无伪造 bytes=0。要求父测试进程未设置 `MYCITY_NORMAL_POOL`。
- `direct_air_original_prepare_attach_and_wait_return_contract_capacity`：父进程启动真实 Pool，随后用当前 integration-test executable 的 `--ignored --exact` 启动 helper。仅对子进程设置 `MYCITY_NORMAL_POOL`，移除 `MYCITY_BROKER_SESSION` / `MYCITY_BROKER_SUPERVISOR`；没有 `std::env::set_var`。helper 通过公开 `Worker::start_bounded/submit/next/retire/task_observation` 执行两次纯 Air 计算，`controlled=None`，核对 direct grant、历史保留与新 grant 身份；父核对 grants/returns 增量为2、字节全归还。
- ignored `direct_air_resource_helper` 仅由上述父测试启动。它不是独立整城验收。

共同 Status report 入口核对 schema、非 unavailable、序列化尺寸不超过 12KiB，并递归拒绝 `key/sim_key/worker_key/broker_key` 字段。暂停控制协议只公开 legacy task observation，本测试沿现有协议检查，不新增暂停查询能力。共同字段与 legacy current resources 在取消后比较一致。

## 原池账目解释

`Summary.granted_capacity_bytes` 现行实现包含 `workspace_bytes + active Air GRANT_BYTES`。测试按原 balance 合同断言：Sim 独占时总授额为59MiB；Air 暂停时总授额为59MiB加固定 Air 合同容量；取消 Air 后只余59MiB；Sim Stop 后全64MiB回到池。共同 Sim arena/requested heap 是同份角色容量的观察，不再加一次。`Worker.ready_to_submit` 是原保留字段，submit 后仍为 true；活动性由 `has_active_execution`/原 outstanding gate 单独表达，测试不改原接受条件。

## 保留与执行环境

root 串行执行前须设置三个显式路径：

- `MYCITY_RESOURCE_TEST_ROOT`：已存在、canonicalize 后位于 `/Volumes/HP P900/dev/MyCity/work` 的证据目录。
- `MYCITY_RESOURCE_EXE`：root 核对 source hash 后保留的同源 headless 可执行文件。
- `MYCITY_RESOURCE_POOL_ROOT`：同 SSD work 下已存在的短路径父目录；子目录只加 `p-PID-N/p` / `d-PID-N/p`（末级由原 Pool 创建），避免 Unix socket 路径超长。

测试保留每条 Sim/直接 Air event、helper stdout/stderr/exit、Status、原池 summary、实际 world 与 checkpoints。Pool 自身原日志留在 pool root。没有清理存档、日志或失败证据，没有建立 worktree/target。direct helper 60秒超时只终止自己持有的测试子进程并记录失败，不伪造成功或返额。

## 验证状态

- 静态核对：已读取现有 API、CPU request identity、原退役路径和 Pool balance，修正测试中的字段路径、总授额算术与 Pool spawn 要求末级目录不存在的前置条件；这是源码检查，非编译证明。
- `cargo check/test`：`not_run`。
- 真实 pooled / standalone / direct helper：`not_run`。
- 格式化：`not_run`（按唯一串行 writer 要求）。
- 全部13项 Air 历史回归：未复制、未运行；由 root 按实际影响复用或重跑。
- 新测试目前为待 root 编译/运行的准备源码；任何失败仍须保留并按实际结果回填 Issue #102，不能据本交付宣称通过。
