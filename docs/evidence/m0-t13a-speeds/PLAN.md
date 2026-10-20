# T13A-2：完整30日速度实验

原冻结acceptance SHA256 876ea995ea7834bfcea59b2ef8fa56d502aed9006477bcc67bcaa3f5cd944508未改。原full账户64/库存128/工资64/Claims20000、8城、8单/4在途、2启动作业、16义务、128²空气、8192分页、720小时/723目标/724窗口、完整30日与day7/8关开城等全部保留。速度快不能改变数值步长、原票或输入。

CLI speed-run NEW_DIR unpaced|600|60|10|1：Rust supervisor实际持normal pool和speed-driver Child；supervisor0.25MiB、driver1MiB、observer2MiB及其他原组件cap不扩大。query每10秒/save每300秒按真实墙钟。L1在本速度分支缺席，不能拼成L1联合/8h/16GB。普通Engine/serde/helper模型仍EXTRA，名义小计不签共同8MiB或精确常规3.10GiB。

独立CPU Reference先完成并始终处于同一观测进程树/资源账；operating t0紧接实际Sim创建/打开请求前，真实Store初始化与所有启动动作进首日及总段。小时h release=(h−1)*day_wall/24、deadline=h*day_wall/24，按乘后除的整数纳秒计算；不提前发布后续周期，小时h完成后的外部动作等到边界h。额外60/180/15日+120目标按对应小时周期发布，业务/动作顺序不变。unpaced单列无等待完整轨迹。

末段不额外等满标称总时长：全部原业务/空气/时间线完成后，先服务当时到期观测，再执行必要最终业务Checkpoint；其持久完成时刻冻结publication_end，停止发布新的周期观测。所有deadline≤cutoff的观测恰一次完成，排空中新未来deadline不延长cutoff、不声称完成。operating_end在排空完成后记录，包含等待/IPC/fsync/最后Checkpoint和排空，不能扣开销加容差。停止/冷重开/重开后Checkpoint/独立CompareSpeed属于预先规定的后验证段；重开后Engine含metadata对正常CPU参考逐字节一致。

记录30日固定日周期start→business持久完成、相邻完成间隔；无等待按相邻业务完成间隔含首日初始化计。最后一天再含必要Checkpoint/排空耗时。full L2 60档局部目标逐日≤60及全段≤1800均按真实operating_end判定；unpaced同样验证30日/逐日60。快速可实际失败但仍完整算完，不通过缩短样本、跳事实或移动t0制造通过。进程0只表示实验完整，不代表速度达标，report明确区分。

四档600/60/10/1都需各实际完整运行；600预计约5小时，60预计约30分钟，不能用一分钟缩放替代。先full L2 unpaced和60档，再10/1/600；观察器保护失败原样保留，正常重测须新120秒normal准入。正式8h仍按原h*40秒末端发布/96保存/指定故障，不适用提前释放。

advisor单轮指出“等满1800后再保存”会必然失败，实施前已修正；随后认可固定最终Checkpoint publication cutoff，要求核验时逐项对齐实际请求/完成、不延长cutoff、未发布不补计。未有实测前不算通过。

验收范围裁定：冻结mandatory_pe0_*指定P-E0，当前full L2不能替代；report明确profile=L2_full_actual、mandatory_pe0_gate=not_evaluated_other_profile、legacy_session_speed=not_retested。P-E0必须另用真实fixture/业务完整30日、持久化/IPC及独立预期跑通；若声称旧T10 session 61秒问题解决还须重跑其原路径。不能用“负载更重”推另一条路径通过。
