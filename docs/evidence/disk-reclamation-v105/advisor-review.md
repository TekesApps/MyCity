Advisor /root/boot_advisor 独立只读释放：r07-vm-l1-01 第1/2代 pressure.bin 与 work-reserve.bin 已终态，独立完整冷恢复通过，不新增必需恢复基线。允许删除4文件，保留原index、原始VM/身份/epoch/测量、组件、固定源码及exe。

已退役pressure的 r07-coupling 第1代、r07-event-driven 第1/2代、r07-report-error 第1代：native/meter及既有归档覆盖的纯合成组件必须逐字节及原index比对后才可退役。保护v104 protected-after.json190原件、业务world/WAL/收据、故障pending/negative。共享target不清空。

追加独立释放：r07-vm 第1/2代的native/meter及纯合成stub，在pressure/reserve退役后可按既有gzip完整解压逐字节+原index长度/SHA核对退役。此组件不是VM归因独立原路径依赖。保留完整无损归档及映射，protected190不动；两代不可直接完整恢复。
