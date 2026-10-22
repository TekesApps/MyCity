# 旧已验收工程恢复夹具准备

当前已实际执行：prepare.py核对冻结v119来源；67号只构建旧writer测试程序并保留固定exe，随后直接运行该程序一次，导出已验收工程WAL与checkpoint（exit 0）；seal.py实际封存两组完整原件，8项纯helper测试通过。68号新reader只打开三份完整副本（两种工程格式和历史v119交通world），实际状态hash一致且原件未变。详情见[本阶段证据](../core-road-town-integration-122/STATUS.md)、[旧writer真实收据](attempt01/writer-receipt.json)及两组seal。以下为原准备流程与安全约束，preparation.json中的prepared_not_run记录的是准备当时状态，不能当作最终执行状态。

现有v119交通world可以证明旧profile/proof恢复，但其中没有已验收工程，不能代替旧工程WAL/checkpoint兼容验证。prepare.py只会在新独立源目录中还原v119固定ZIP，逐文件校验原清单，补齐固定旧提交的6份测试依赖，并新增一个导出测试。旧业务Rust文件不修改。它不编译、不运行、不建立target；已存在的候选目录一律拒绝覆盖。

导出测试复用旧construction.rs的真实Store、审批、施工和验收调用，在验收后关闭Store，再分别保存完整WAL和实际checkpoint副本及独立预期hash。失败world也保留，不用hash替代完整输入。它生成的是未来使用旧业务实现新执行出的夹具，不能称为历史原件。导出后仍须用新候选的reopen-check在独立副本上真实打开、比对hash，并保留旧原件；Store::open可能截断失败尾部或回收临时文件，不能在受保护原件上做新候选实验。

仅在当前正式测量终态、root重新开放独占验证窗口后执行准备、构建和导出。环境指定共享target及新的外置SSD输出路径；不允许在候选内生成第二个target。future_build_command/future_writer_command/future_environment会记录在实际准备收据中。先用`cargo test --no-run`只构建，将该次确切测试程序复制至attempt01/writer-test-executable并保存harness.rs；然后只执行保留程序一次，留存真实writer-receipt.json、writer.stdout、writer.stderr及完整输出world。不要先由Cargo运行再伪装成执行保留程序，输出目录禁止覆盖。兼容模式为：

```text
town_transport reopen-check COPIED_WORLD EXPECTED_RULES EXPECTED_STATE_SHA256
```

工程fixture规则为construction-engine-v1，交通fixture为m0-town-transport-fixed-profile-v1（已直接核对v119冻结ZIP的example RULES常量），两者分开验证。原M0冻结源、负载、阈值和运行目录不受此准备代码修改。

seal.py仅在以上实际执行结束后使用，默认只打印未运行计划；`--seal`要求固定v119源包/清单身份、保留程序实际命令与退出0收据、全部日志和harness哈希、两份已验收工程输出。它检查进程和world占用，逐文件哈希完整原件并复核未变化，然后以排他创建方式写两份manifest/seal；不会运行Store、修改原件或替代新读者实验。writer收据需绑定executable_sha256、source_archive_sha256、freeze_manifest_sha256、business_source_sha256、exporter_harness_sha256、stdout_sha256、stderr_sha256和实际command/exit_code。任何缺失、身份不符、占用或已有seal均拒绝，失败部分保留。封存工具本身只证明输入和证据身份，兼容结果来自68号真实新reader执行，不能用工具测试代替Store恢复。
