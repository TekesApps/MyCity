# Advisor本轮聚焦方向（根代理记录）

依据03同源full-R07四角色VMmap与原normal_pool合同，先选C减少真实重复开销，不直接用A账面“正常池借8MiB给native”，也不先用B改变压力页驻留性质。

A只有真实backing/容量替换、身份/生命周期/归还明确、未双重承诺、原完整并发授予仍可满足、common8MiB不取消时才成立。Native/stack在原合同中单列，现有SimCodec需要59MiB及worker/air grant，不能把诊断grants=0视作永久业务余量。内核页表不属于用户arena。

B可研究为存储优化，但相同字节、扫描、1MiB dirty不证明同等内存压力；文件映射可能把成本移到可回收cache和重读IO。必须保留完整内容、COW/snapshot/冷恢复，同时计文件驻留/cache、缺页、读取、系统压力/swap与磁盘；未完整计费前不能用于签本轮normal通过。

最窄C候选：driver控制逻辑并入现有pool-host，保留observer/layout独立、64MiB真实backing与授予原样；不阻塞pool服务/扫描，不改变时钟、身份、失败传播/回收责任，不把driver拥有量藏到pool角色，原两代/冷开和故障必须重测。约1.31MB driver footprint未必全部可省，远不足8.45MB超额；若仍超则保留失败，不用A补账。此意见为只读方向，不是实现或性能通过。
