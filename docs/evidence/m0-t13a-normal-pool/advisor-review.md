# 独立审查

## 最小实现路径

advisor建议先交付实体64MiB余额和worker真实槽借还；普通Engine分配不释放对应填充，作为额外压力，不能用堆峰之差补洞。无法在普通分配不改的同时证明总量精确在原预算内且未用余额始终足额；本次不设计通用allocator。

## 首次源码阻断与修复

首次发现两项必须修：部分补页/事件写失败后不能继续当普通协议拒绝；Process.spawn不能接受旧ready。已增加ActionError::Fatal/Rejected，内部错误直接终止服务；启动前拒绝既存路径/symlink，先try_wait再匹配ready.pid与新child。真实CLI故障测试覆盖第二补页失败仅61MiB、Attach后事件失败，要求非0退出/无balanced报告/无returned信用；旧活/死服务目录拒绝。

advisor只读确认两阻断已修，targeted2原worker10项及实体池5项通过，stderr留证不改变状态机，无新增必须修项。可以在最终同源全仓通过、normal120后执行原600秒联合；没有用targeted2替代全仓，也没有另启动负载。

Prepare丢回复可以fail-closed但不能称自动恢复；Sim死亡后的旧grant不会凭PID消失接管。Filled绑定worker首次槽身份，后续job依靠同进程复用trace关联；转换期间不保证连续满压力。普通Engine是额外压力，池总堆门不反推出独立诊断费。精确normal-owned和完整T13A仍未证。

## 测量终审

待真实测量和原始工件完成后独立审查，不预写通过。

## Fragmented transport blocker (after failed functional attempt)

Independent advisor inspected the unchanged binary probes: a 17-byte prefix was rejected after 7.64ms before its suffix, and 3 of 2,000 fragmented requests were rejected. This proves premature rejection of incomplete frames; the old generic error does not identify the exact category of grant125. Approved repair scope: explicit blocking accepted socket, unchanged 2s timeout, bounded category-only rejection diagnostics. Require delayed suffix, actual timeout, truncated EOF with subsequent recovery, and distinct source/boot tests; rerun real 724-grant business separately. Joint pressure failure remains failed. No new performance acceptance granted.

## Independent final phase review

Advisor independently reran static_check.py, verify_functional.py, verify_full.py --pool-only, verify_pool.py and coverage.py successfully. Reviewed explicit blocking accepted socket, unchanged timeout, category diagnostics and per-grant actual slot/wait/return proof. Verified723targets/724workers/3011receipts, entire Engine including metadata and historical candidate hashes,731normal heap instances,357periodic scans,724 grants/Filled/returns and final64MiB with no active grant/rejection. Two short helpers41260/41263 had no RSS frames: no physical peak inferred from footers. Old functional lacks pool exit report and old joint failed before business; neither is merged into corrected-source joint success. Approved v0.5.47 as this phase functional completion, no new blocker. Shared8MiB/exact normal workspace/L1 joint/steady/four speeds/8h/real16GB remain unsigned; fullT13A not accepted.
