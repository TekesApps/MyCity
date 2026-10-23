# EXEC-1只读源码定位与范围

2026-10-23，直接读取 `/Users/frankqin/dev/MyCity` 的已提交代码，固定main `93be71bb4a0fcd2f531af61f2d902b2a984e37ee`（v0.5.144）。下列相关文件读取时与该提交字节一致；未触碰主仓库未跟踪证据／其它工作树，未向主会话发送消息。表格用于定位代码，不是功能或性能复测。文档分支不合入这些代码。

| 固定源码位置 | SHA-256 |
| --- | --- |
| [crates/city-core/src/lib.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/lib.rs) | `0fe4f40d913284484cc2a713dcabfd431357759c06e7fe447dab1cb419dd546d` |
| [crates/city-core/src/timeline_driver.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/timeline_driver.rs) | `387c3badcf41b2b60d11c1fafe2aa2e78e9365de4fcf7574a8b56b1acb1bbc72` |
| [crates/city-core/src/timeline.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/timeline.rs) | `bb21808312e7dcaeddec0d500f8ee8dc7d94407d05a09ba26cf7935786c502c8` |
| [crates/city-core/src/schedule.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/schedule.rs) | `169aa624ee8a91f7869ab8d3010a693e05937298b5aa039bc36232da1e153825` |
| [crates/city-core/src/transaction.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/transaction.rs) | `bf57b80a55c13c001c1b314d6b10ff4bd88eebd5990bc75825fb006f5b77cfe5` |
| [crates/city-core/src/town_runtime.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/town_runtime.rs) | `2cfc7b73e13d02f246015b43d321b9cec3d08a1bc3113ab54c9da88ee1f41999` |
| [crates/city-core/src/town_economy.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/town_economy.rs) | `48ad6a65473a5f99846eab64cb73324d613919a3a433e1cd018d9adf35de53f6` |
| [crates/city-core/src/town_utility.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/town_utility.rs) | `5642c21d18f8ee0b41e7a555d753bc6dc239d776ce8c86eb8b024cf07094c370` |
| [crates/city-core/src/construction/mod.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/construction/mod.rs) | `731736809953fc14a4dd35c74a7bb0e7f1df45fc339af3de30e912c846ed878e` |
| [crates/city-core/src/transport.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/transport.rs) | `fe9cae4ab20eb3d94f8e88b1ac3f531cbb03622effbedc1437b933d5954a888d` |
| [crates/city-core/src/economy.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/economy.rs) | `123ff28924407a71012fa88156c97190b2388bf99685bd744862b5a45b902e2b` |
| [crates/city-core/src/boot.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/boot.rs) | `56c2061b036e6592b9ef907279d3fde343588a9eef6c71afab4b2bdd915e84be` |
| [crates/city-core/src/exchange.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/exchange.rs) | `f14e3bc12072bc45d61c3dc8850270863c7b7365cee03b8a0a8af662c68ae123` |
| [crates/city-core/src/region.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/region.rs) | `caf62765568958889c426c6f0a45875049ea1ab44743dc1bbb0cb1e8013be63a` |
| [crates/city-core/src/air.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/air.rs) | `1b5f76f5adc77e2a93072dbfc028e96b55783b9dd3cf18c68b1449963a0da7ed` |
| [crates/city-core/src/background.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-core/src/background.rs) | `57e19b584f5196cbbfdd68f20fb32dd9280c8f6ade473379a2b2fe49832d9d4d` |
| [crates/city-storage/src/lib.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-storage/src/lib.rs) | `485aeecbd1b8cbd804f0b57ab6f223aa50aed217f610ebec55ccf2e48f22d5cb` |
| [crates/city-storage/src/town_transport.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-storage/src/town_transport.rs) | `3028ec658e8ea0188162b8ea244d8b442ee324ab6d58e9d2c0cbb02231427a25` |
| [crates/city-map/src/traffic_adapter.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-map/src/traffic_adapter.rs) | `7271de4a3ceafd0e36ca0a48e52f781de2bc580748256209e5c96e34e796e07f` |
| [crates/city-headless/src/compute_worker.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-headless/src/compute_worker.rs) | `ad1c460e56bbab849deea1525d19e7526d06a866fe69aabf2529718613fa39a4` |
| [crates/city-headless/src/sim_host.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-headless/src/sim_host.rs) | `f1607b643ce927a290713e972a347cb339270ab230a5078eb90176caced3f22a` |
| [crates/city-headless/src/normal_pool/broker.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-headless/src/normal_pool/broker.rs) | `75b82d43b117949ea62a116e048c1e3c3bfab5445719d61041e208468e79471b` |
| [crates/city-headless/src/steady/supervisor.rs](https://github.com/TekesApps/MyCity/blob/93be71bb4a0fcd2f531af61f2d902b2a984e37ee/crates/city-headless/src/steady/supervisor.rs) | `234ade30a66d64ba177395b3162196acfda99f083ba9dee0a275d4f12e5c8c22` |

核对用途：Timeline真实due集合／完整边界／空气屏障；Town显式整日结算与proof；施工和股权等显式事务；有限Boot/Exchange/Region；空气数值切点、Ticket/Request/Event；Sim唯一writer、broker实际租约回收和测试supervisor。仅记录与执行清单有关的代码；未审计所有业务函数或运行其测试。
