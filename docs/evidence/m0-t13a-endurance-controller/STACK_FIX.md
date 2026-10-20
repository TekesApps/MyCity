# 实际栈溢出与最小修复

旧ce9efd全仓debug回归句柄63357终态101，scale_rare PID21280 SIGABRT；[脱敏符号栈](attempt1-source/stack-failure.json)与完整旧来源/失败日志保留。Claims/source两分支CLI及冷恢复已完成，失败发生在测试尾部默认线程栈的Store::open→v2 checkpoint→Engine serde→World→Option Region。

Engine.world改为Box<World>，只改变内存持有，不改serde字段/顺序/旧线格式。不调RUST_MIN_STACK，不增加测试线程栈。Engine本体已经计Box指针；owned_capacity_bytes再加一次World.layout内联及子容器，移除旧内联World的扣减，防止漏计/重复计。物件从栈移到堆仍占内存，恢复/候选多份World成本仍需实测，不签常规64MiB。

fixtures/regressions/t13a-legacy-stack-world.zip来自原失败的source锚点，97序号、2527389B期初Engine；保留原World/genesis/原规则，不重写业务。新增stack_restore默认线程栈测试打开原v2状态并逐字节比较重导出，在debug0.39s已通过。该兼容测试显式用旧规则；生产Sim源代隔离不变。最终原scale_rare debug在默认栈153.54秒通过；全仓release316通过/0失败/0忽略，clippy/fmt/build通过。完整同源smoke02也终态0且独立核验通过。原full debug失败仍保留，未声称新full debug全仓通过。

advisor认可最小尝试但强调Box反序列化仍可能有栈临时值，因此必须原故障路径实测，不凭Box推论。后续若仍失败，按新的实际栈定位，不无据堆Box。
