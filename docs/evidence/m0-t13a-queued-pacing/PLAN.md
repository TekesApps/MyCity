# 计划释放时刻锚定：原轨迹重测

原真实60档已完整1800.240858542秒、严格失败，v0.5.54留证；本阶段修复不能借旧30日当新来源通过。冻结722目标、输入、每日至多60/总段至多1800秒及P-E0终值全部不变，freeze.json逐字节复制旧文件。不是降低模型精度/负载，也不是增加时间容差。

ready在原权限/shape/receipt/interval门之后；scheduled=max(ready,saved_deadline)，等待后actual记录真实开始，仅新请求成功时next=scheduled+interval。晚ready重基，不积累闲置credit；原票/冲突/失败/Resume/同速不改时钟，明确Pause/变速规则不变，未知提交先查票。scheduled/actual/lateness分列，计时仍实际Store创建前至末Checkpoint/日志fsync，绝不扣迟到。

纯时轴反例覆盖早ready但晚唤醒不累计、计算跨期/长空闲无credit；原session及P-E0控制器回归后新来源封存。先真实完整step30日，再fresh120normal后真实约30min的60档，全部722原票/同轨迹整个Engine含metadata/原终值/冷开仍核验。新增queued节拍审计按正等待时next_deadline恰等于前deadline+interval、晚ready才重基逐笔检查，旧actual-release整数审计限定v1，不能套新源掩盖差异。

advisor已在旧实验运行中对条件性方案认可，不预言新性能一定通过。新完整实际结果仍需独立终审，未测不算通过。full L2四档/L1/共同8MiB/普通64MiB/三轮/真正8h/16GB门不变，M0不宣称完成。
