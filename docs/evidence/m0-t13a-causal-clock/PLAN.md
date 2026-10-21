# 因果时钟及原L1工作区

保留a702失败pending和8d66fe全仓通过/clippy失败原来源。只修COW测试读取旧current（避免被覆盖未读的赋值），生产COW/固定arena不变。新增启动前observer/Rust监督者有界nonce/PID/source往返，不假定跨进程Instant同epoch。每次send/receive原整数边界及区间完整保存，选择inside规则不变；超时/错身份/空交集失败，不回退成伪精确数据。

原64MiB/共同8MiB、记录、padding、1秒扫描/1MiB脏写、300秒快照、300+1800正式窗和冷恢复不变。启动校准成本留在真实wall/物理/诊断中；无新常驻线程/unsafe/依赖。先反例、严格clippy/fmt，然后冻结source/exe；原完整L1实际短测及冷恢复、v50原件完整冷开、全仓回归分别记录。取得同相位完整帧后按角色归属native/stack残差，不直接按峰值扣pressure。

原正式3x3/四速/同源稀有补样/实际28800秒仍必需；真实16GB not_run。复用唯一target，正常短测终态后精确退役无依赖合成副本；formal7/8、v50旧格式、smoke-L2故障和a702原件保留，不用hash替代。
