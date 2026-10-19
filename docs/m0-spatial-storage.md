# M0：不可变空间资产接入同一存档根

v0.5.12，2026-10-19。接续[T07空间模型](m0-spatial-model-and-gpu-layout.md)，本轮只补真实CPU资产的保存依赖，不扩UI、运行时地形编辑或GPU后端工作。后端兼容交给wgpu；已有CPU／GPU数据边界保留。

## 1. 范围与唯一权威

Store可以创建带期初空间包的实验存档；空间包包括已验证的地形和Scene、world／branch／seed身份，没有第二份钱包／库存／人口账。city-map::SpatialAssets是不可变CPU资产，Terrain／Scene以只读引用暴露；同一装配函数也服务原spatial-probe，避免保存与观察使用不同几何语义。

目前空间schema仍只有一个PNG高度资产、两个地图frame显式复用它。独立邻图DEM、多包缓存、运行中工程／地形修改和地图切换加载器未实现，不把已保存的楼体几何当成已取得产权、建成楼或付过工程款。已有经济／区域Engine仍按原事务推进，空间数据本轮不会消费材料、创造住房或授权建设。

## 2. 持久文件与版本绑定

| 文件／记录 | 约束及绑定 |
| --- | --- |
| spatial-input.json | 固定文件名，原始空间输入≤64KiB，含原manifest／几何／版本／身份；不接受manifest路径或URL |
| spatial-height.png | 固定文件名，PNG≤16MiB，按原T07灰度和受限解码校验；与输入内源hash匹配 |
| genesis.json v2 | 空间引用schema、两文件长度及SHA256、派生terrain／geometry SHA256、world／branch／seed、原opening／资源和rules_identity |
| root-{0,1}.json／CURRENT.json | 沿原提交边界，root通过genesis_sha256间接绑定全部空间依赖；每笔WAL交易保留同一个genesis引用 |

非空间Genesis v1只允许spatial缺省／null；v2必须有有效空间引用。v1带空间或v2缺空间拒载，不降为无空间世界。CLI继续以完整Rust源码／锁／fixture fingerprint作rules_identity；不兼容版本照旧拒载，无历史规则自动迁移。schema格式兼容不等于不同源码规则可直接继续。

恢复时固定路径／类型／长度先验，拒绝符号链接、目录或超长依赖。每个文件只读取一次，检查该缓冲的精确长度与hash，再用同一缓冲装配。派生CPU Terrain与Scene的摘要还须匹配Genesis；Scene摘要覆盖两侧Terrain内容及几何解释规则，不只覆盖GPU上传字节。原scope／seed不匹配或无效来源在创建目录之前拒绝。

存档目录由单写宿主独占管理；这些检查不是对抗能并发修改全部文件／根的恶意本机进程的操作系统沙箱。正常运行不反复解码校验磁盘源文件；关闭后依赖被修改，重开会拒载而不是修补原状态。

## 3. 创建、提交与失败结果

创建顺序：装配并验证模型和空间→创建全新目录／锁→写空间JSON／PNG并分别fsync文件与目录→写并同步Genesis→新建并同步空WAL→写同步root0→原子发布CURRENT并同步目录→同步父目录→才返回Store。所有固定依赖先于CURRENT，空间与资本从同一根同时可恢复。原创建API仍可显式创建无空间v1实验世界。

创建只接受不存在的目录。任何失败都不自动删除或重建已有目录：无CURRENT的残留目录是未完成创建，须保留诊断；CURRENT已发布后发生I/O错误／响应丢失则结果未知，调用方只能重新打开核对，不能当作没成功再注资。初始化没有重试即再次发资本的API。

打开先验证CURRENT／根／Genesis／规则，装配原经济期初与空间依赖，完成全部WAL校验／精确重放后才开放访问。任一空间依赖失败，既不回退旧根，也不清理WAL尾部或改写资产。只有全部依赖与已提交WAL都验证通过，原T06规则才允许截掉已证明在提交边界以外的尾部。

Store::state_hash将Engine状态摘要与固定空间引用共同绑定；原engine.state_hash仍只表示模型状态，不能混为完整存档摘要。存储不确定后engine／spatial／spatial_reference／联合hash均拒访问，重开恢复后再使用。空间引用在每笔交易前后不变，没有未持久化的几何变更混入已确认回执。

## 4. 预算与证据

继续使用原T07几何域21MiB候选份额、每Terrain10MiB／Scene1MiB对象门和128MiB导入高峰门，不增加完整产品3.10GiB常态候选。保存PNG二进制独立写入，不把16MiB PNG按JSON字节数组扩成几十MiB或每次追加到WAL；引用只占小元数据，磁盘源由当前根保留，不能当可丢缓存。

空间常驻归几何／索引0.30GiB；Engine账本归模拟／事件域。打开时还同时存在原PNG缓冲、IDAT临时、解码工作页、目标Terrain／Scene、Engine回执预留、单帧WAL及校验／serde／prepare工作页。已有单对象／帧上限限制输入，完整allocator／统一页／保存预演并发峰值仍待T13测量；不能因为Engine仍小于4MiB就声称整个Store及所有工作集都在4MiB内。

[实际CLI报告](evidence/m0-spatial-store/store.json)：本次Engine owned2078144B、空间资产owned612663B、完整Store owned2691560B；Store额外持有身份、空间引用、路径／根头和各动态容量。此统计排除allocator metadata、内核文件页及临时恢复工作集；[短进程观测](evidence/m0-spatial-store/process.txt)也不是城市长跑或16GB证明。本机M4／32GiB／macOS26.6.2。

实际输入空间JSON3679B、PNG207B、目标66049顶点。创建后执行家庭→公司10000分转账，保存／打开，再重试原operation_id：committed_sequence保持1、空间摘要不变、现金总量3000000分不变。区域联合测试另真实迁2000意向／1998实际并推进30秒，支付／旧债／换城与空间同根恢复。没有将该有限转账称为T08经济闭环通过。

[Release测试](evidence/m0-spatial-store/tests.txt)共86项通过（35 core单元＋5区域＋8 CLI＋16空间＋12原存储＋9空间存储＋1编译失败），fmt与[clippy](evidence/m0-spatial-store/clippy.txt)通过。新增实际覆盖：

- 创建13个位置的子进程exit(97)：校验后、空间文件半写／同步、Genesis／WAL／根同步、CURRENT重命名／同步、父目录同步、返回前与响应后；发布前拒开，发布后恢复同一空间与期初资本，重复创建拒绝。
- 带空间存档的11个交易位置exit(97)：空间引用保持不变，资金／区域钟／迁移／欠款／回执按实际提交边界全有或全无，已提交重试不重复执行。
- 文件损坏／缺失／非普通文件／符号链接、错误身份／种子、v1/v2错误组合、超字节引用／未知路径字段及派生摘要不符拒载；即使测试重算外层校验和，语义错配也不能通过。
- 空间失败时保留CURRENT／Genesis／WAL尾部；I/O错误注入后共同访问受阻，重开再查。errno28是测试注入，未做真实磁盘填满实验。

单轮advisor方案审查要求同缓冲验证／重建、严格v1/v2、发布失败视为未知和完整预算，本轮均按此实施。上述进程退出／I/O注入不等于物理断电、磁盘硬故障或完整检查点压缩通过。

## 5. 复现与接续任务

最后的存档目录必须不存在：

```sh
cargo test --workspace --release --locked --offline
cargo run --release --locked --offline -p mycity-headless -- spatial-store-probe fixtures/m0-e0-opening.json fixtures/m0-spatial.json fixtures/terrain/m0-flat16.png /tmp/mycity-spatial-store-fresh
```

本轮补齐T07不可变期初空间的持久依赖门，不签收T07全部、T08经济、真实建设流程或16GB。下一主线转T08：正式劳动／生产转换／消费、工资税债及P-E0／P-E1启动耗尽实验；其活动仍须走同一事务／持久回执，不在实验脚本里直接改终值。T07真实搜索／投影／非平地车行等缺项保留，不继续展开同一存档故障故事；后端兼容不另开Metal／DX12适配任务。
