# 固定空气模型背板的范围

普通CPU/Sim继续owned State。bounded worker持有一个私有AirWorkspace，数值内核共用State::page，不复制算法。Rows仅暴露活动切片；owned push/clear遇fixed拒绝，不能替换/扩容Vec。fixed State/Input不能从workspace取得或clone；TaskState只借workspace，禁止生成owned checkpoint。

20000 Source×20B＋16384格×(field4＋scratch4＋integral8)＋原schema/两个64字符摘要字符串＋padding总计1048576B真实heap capacity，全部初始化；活动长度独立。字段与scratch交换背板，检查排序后的分配身份集合不变。metadata/原生栈/解析临时/控制消息及其余Engine、Prepared、Store、helper仍EXTRA，不用减去进程峰值证明预算。

恢复先固定栈token/depth预检，再seed直接写固定背板；字段次序自由、必填/重复/未知/超量先于下一value解析拒绝。身份hash流式保持原JSON/SHA。失败reset活动区与unused tail；数值范围及固定填充/身份扫描不能替代独立数值oracle。

原64MiB池中，worker启动前释放真实5页：4MiB IO＋1MiB typed，页59..64；第一个Sim4页55..59，双owner后池55MiB。分别创建并扫描后Filled携同child IOidentity、typedinstance/cap/scan绑定；重复换typed身份拒绝。实际Child wait后补齐5页才发布返还，初始化/补页失败保持failclosed；取消无child时仍沿原取消责任路径。没有凭日志或名义额度归还。

同进程连续job复用同一kernel/slots，发布门完成后扫描loaded状态并reset再记录，然后Complete。故障可能没有末尾reset记录，实际wait是回收依据；不能把缺日志视为已reset或0内存。原事件1536B行、8MiB日志及worker1MiB诊断门不扩大。

本轮没有透明全局allocator/unsafe；不是整个worker零动态分配，也不是所有模型normal64MiB或common8MiB闭合。冻结原负载、CPU reference、规范字节、页限额及16GB/正式稳态/8h门不变。
