# 开发期反例与修正

1. 新adapter crate增加serde依赖后首次--locked检查因lock需更新退出101；实际generate-lockfile --offline后Release检查通过。原固定版本没有升级，仅增加本地adapter与固定linked_list_allocator 0.10.6。
2. 首次cargo test -p mycity-role-allocator --release --locked --offline（session52977）局部4项通过，但bin测试harness在main前实际stack overflow／SIGABRT，整体101。保留当时adapter-before-spin.rs。macOS std::sync::Mutex可能为pthread lock延迟分配，置于global allocator内部导致递归；改成不依赖分配／OS调用的AtomicBool acquire/release锁。
3. Advisor指出激活后释放启动System对象会留下逻辑扣款但物理块已释放的缺口；改为逻辑free计数减少、物理原块隔离保留至进程退出，并分别报告live／quarantined／charged。不能把这类free回填为新授权。之后才重新验证，旧失败不覆盖。

Frozen source attempt01: full workspace exit101 after142.655s. Two broker tests retained obsolete4MiB pool grant assertions; actual59MiB expected under approved whole-role design. Updated only3 pool assertions, codec4MiB assertions retained. Failure logs/source zip preserved under failed-freeze-01; panic fixtures mc-broker-64420-4/-7 retained. No acceptance claimed.
