# #137 限定准备交付（root记录只读子代理结果）

来源：`/root/execution_register_audit`，只读完成，无代码/测试/Issue写入；原角色只读，未自行写文件。本记录由root据其交付正文保存，非实施完成或运行验收。基线31fb173/v195，#137 Open、#131 Closed。

## 可复用入口

- core `opening_loader.rs`：OpeningInput::from_json/read_json/read_named_json/initialize_into，原字节≤64KiB、SHA和来源描述，使用原EngineSlot，成功槽不二次初始化。
- headless `opening_source_provider.rs`：read_local_file/read_local_bytes/load_resources_file；本地JSON、mycity-resources-v1、≤4096B/16资源，只id/capacity。
- storage `opening_input.rs`（Issue旧卡只列lib.rs已过时）：封闭GenesisOpening、原Store::create_opening_input*，原字节与初始化同源。
- assembly Opening/assemble_into：旧mycity-m0-opening-v1及有限profiles，原重复资本/金额/owner检查；runtime typed Handle依原scope/domain/index/generation生成。
- 原#131同槽、loader六parent/十二cold、provider八项/CLI两项按原范围复用，不能称当前HEAD全量通过。

## 真正差项和最小建议

在原canonical Opening解码路径接显式版本化的有限typed契约，不只CLI改写为另一份v1 JSON。列明本地JSON/SuppliedBytes provider、schema/profile、单位、clock和引用种类；真正解析到账户/库存/资本源/显式资源，并在恢复重走相同合同。CSV/未来schema/无真实来源保持unsupported。#136不是此首门依赖，也无需等31/32整个领域。

root需先固定opening_loader、assembly/解码兼容、storage/opening_input及恢复、专属opening_typed_provider_store/fixture等确切所有权；只有实际扩provider时才改其模块。旧合法输入、失败不发布、新进程/重试不重复资本、布局/转换/恢复重叠都待实际验证。

## 独立正常输入与预期

新非空间小世界：world9/branch3/seed44，原实验开局；账户1 Treasury5000分、账户2 Company1000分，各独立CashCapital10/11；库存1 owner2、Raw数量3000、成本700分，独立InventoryCapital12。显式resources=[]。不创造许可/用途认证。独立预期现金6000、库存3000/basis700、资本各消费一次；原合法转账137分后4863/1137，原票、WAL、checkpoint冷恢复后保持，库存不变。

另复用原Boot合法fixture及显式资源验证非空资源/实际clock映射；没有时间consumer不能靠接受一个clock字段宣称接线。

拒绝集合：未知provider/版本/schema/profile；错/缺单位、非法换算或溢出；clock种类/城市/scope错；同ID错domain/缺目标/重复绑定/源版本错；重复资本、现金源当库存源、owner缺失/金额错；缺资源或带初始reserved/occupied；N/N+1和64KiB边界、重复键。失败不发CURRENT，成功槽重复初始化不变。修改外部源后加载存档不得重读源重新创设钱物。

保存正常原字节/契约、137分交易、WAL和checkpoint fresh child、CurrentRenamed丢回报及重复create拒绝的完整原件。最少影响门：新typed正常/拒绝/恢复、原loader/Opening兼容/同槽六项；仅当相应provider/CLI修改才重跑其八/二项；公共领域解码影响到的131 venue/continuous代表恢复。所有新实现/运行均未开始。
