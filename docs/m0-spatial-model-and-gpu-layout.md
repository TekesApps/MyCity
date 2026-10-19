# M0-T07：空间／地图实验与跨后端缓冲布局

v0.5.11，2026-10-19。Rust headless实现子阶段；无UI、窗口、渲染或GPU运行。不重开已收口的产品功能设计，按[空间契约](spatial-contract.md)、[BC43 §1](city-baseline-completion-contract.md#1-cd-04高度图与物理资产的同一坐标)实现明确子集。T07完整验收仍有缺口，不能把本篇当作P-IMPORT／P-XMAP全部通过。

## 1. 实现归属、单位与安全边界

新增city-map crate独立拥有不可变Terrain与Scene，依赖city-core的身份／来源校验及hash，不将大数组塞进World128KiB，不复制经济或区域余额。所有资产带schema、来源摘要、terrain_revision／geometry_revision；Scene摘要包含两侧Terrain内容摘要，不能同revision换内容后仍冒充原依赖。初始化只能形成新的独立实验资产，不提供set_terrain或运行世界改地形入口。

world／branch／map、当地米制frame／垂直基准、地图origin显式保存。平面i64厘米限定±1,000,000,000cm、高程限定±1,000,000cm；先查范围再相减、叉积／平方／面积采用i128。该有限范围使所用中间值有界，不承诺任意i64安全。土地始终16m索引，小／中／大64／128／256格，地形4m顶点257²／513²／1025²；格子不裁断建筑。土地索引左闭右开，最外边界可作接缝但不再产生一格。

Scene只接受矩形用地及最多8个严格内部矩形孔、固定高度的矩形实体、最多64个本图和64个邻图障碍、最多32点步行接入；通用折线／曲线最多1025顶点。没有任意多边形布尔、旋转楼体、悬挑、桥隧、挖填、结构承重或矿权求解。Layer是用途标签，实体碰撞仍按真实xy及z，不能用U1／G改名免冲突。实体零净距时允许仅边界接触，保护孔洞不许1cm实质侵入；曲线走廊对相切也保守拒绝。

授权布尔与地图框架在本实验中是有限输入事实，不是法律真实性证明，更不是玩家可以自报的施工权限。headless结果明确construction_authority=none；接入真实建设事务前必须由原权限／产权／预算／工程流程提供可信事实。

## 2. 真正导入的地形与缺测／水域

[实际输入](../fixtures/m0-spatial.json)与[原生PNG](../fixtures/terrain/m0-flat16.png)为项目原创合成平地：257²、16位灰度、4m采样、row0北、有效黑色映射0cm；未做地理配准，不伪造测绘准确度。许可／原分辨率／准确度未知null与处理版本显式保留。源分辨率是来源声明，当前PNG样本对应物理跨度与4m目标采样分别可计算，不用重采样声称提高测量精度。

本轮只接受2—4097的正方形、非交错、8／16位原始灰度PNG，显式顶点语义、本地米制。格式白名单仅IHDR／连续IDAT／IEND；RGB、alpha、tRNS、调色板、APNG及包括gamma／文本／ICC在内的附属元数据都返回Unsupported，需先在外部明确转换并保留来源。没有自动颜色／gamma变换。位深／尺寸／摘要／manifest错配、损坏CRC、截断或超限不发布资产。

独立校验完整zlib流的解压字节数必须等于(每行灰度字节+1过滤字节)×行数，固定64KiB窗口，检测多余行和尾压缩输入；随后PNG库校验过滤／CRC。真实反例证明PNG帧读取会忽略多余解压数据，因此额外校验不可删。该防护和输入尺寸门不等于已做完整恶意文件模糊测试。

有效掩码支持AllValid或最多32个源像素矩形缺测区；有任何未补齐缺测即NoData，黑色不作默认缺测。水域独立支持AllDry／AllWet或最多32个源像素矩形湿区，不代表水文模拟或任意外部掩码格式。目标顶点水标记采用声明的最近源样本，安全放置还与保留的源湿区做保守相交，避免降采样遗漏窄水域。水区边界向源单元保守扩张可误拒，不会由目标顶点全干推导源范围无水。

高程按源灰度和z_min／z_max做受检有理双线性重采样，最后一次厘米量化、恰半远离零。内部row0南；发布后的查询只用固定西南—东北对角线的两三角面重心插值，绝不再用双线性曲面。四角平但内部山峰会阻放置：整块保守覆盖的地形单元顶点都必须与平台同高且无水。多查相邻顶点可误拒边缘地块，返回NeedsDetailedValidation；没有偷做土方或给不平地免费基础。

CLI只读取宿主明确给出的JSON和PNG两个文件，不接受manifest路径、URL、脚本或自动下载；JSON64KiB、PNG16MiB流式门。没有目录下载／provider缓存沙箱能力，不把缺少路径入口称为整个P-IMPORT路径安全已完成。

## 3. 曲线、占地、门与地图接缝

二次Bezier递归细分采用有界i128定点控制点。曲线与同参数弦差由控制点二阶差的L1界证明≤4cm，端点厘米量化欧氏误差<1cm，总预算5cm；最多16层／1025点，超限拒绝，不能暗降精度。冻结折线胶囊半径=向上取整半路宽＋5cm，覆盖曲线误差。对矩形保护区用整数线段／点距离精判该保守胶囊；胶囊冲突不宣称原连续曲线必然精确相撞。长度是冻结折线逐段厘米长度上取整之和，不冒称连续曲线精确长度或真实交通时间。

联合Scene检查整块道路包络、建筑基底和完整门接入走廊的地形支撑及权利／保护孔；道路与障碍相交、地面上方2m行走净空受阻均拒绝／需细验。当前道路联合场景限定平坦步行模式，不证明车辆最小半径、坡道、车行接纳、轨道纵断面或实际容量。道路几何函数可画候选，不提供投运许可。

门在正确外墙面，朝向只约束离门第一段；之后可转弯。净宽至少容纳整条接入走廊，门不能挤在外墙角部；接入从门面出发时仅裁去初始端帽与宿主墙体的相交，之后不得返回／穿越宿主或他人实体。锚点必须精确相同坐标、同z／模式／方向／授权；不会在0.5m候选半径内悄悄吸附。Scene中步行锚点还必须是冻结道路端点。

两图连接须同world／branch、不同map、同米制frame与垂直基准、全局内部不重叠、真实共享一段边界；两本地口变换后完全重合，z／模式／净宽／法向／方向和双方授权相容。角点相触不是接缝，净宽必须落在共同边界内。Scene只启用Walk几何验证，两侧整个净宽×2m接近区必须平坦、无水、无实体障碍。仅查中心高程不足；非步行仍Unsupported。

示例两图显式复用同一不可变合成高度资产，不复制一份Terrain；不同高程资产的侧峰／湿区反例另在测试构造。边界口和楼前道路是各自的局部几何验证，不代表已有完整楼门到邻城门的交通路径，更未替代R54实际有限通道与费用。

## 4. 现在固定的wgpu数据交换布局

CPU权威保持i64厘米、整数物量／钱／身份；不为GPU方便把判定改成全局f32。以后macOS走Metal、Windows目标走DX12，共享wgpu和WGSL模型，后端实际支持见[wgpu Backend文档](https://docs.rs/wgpu/latest/wgpu/enum.Backend.html)。本轮未选择／安装wgpu版本、未创建device，也未验证这两个后端。

GPU输入是可丢弃的只读投影和候选批：[布局声明](../crates/city-map/src/gpu_layout.wgsl)按[WGSL对齐与大小规则](https://www.w3.org/TR/WGSL/#alignment-and-size)设计，Rust逐字段写小端字节，不transmute Vec<Point>、Rust bool／enum／usize或依赖Rust默认struct布局。声明尚未经shader编译器／反射或GPU验证，当前测试是CPU字节契约。

| 缓冲 | 格式与字节契约 | 所有权／限制 |
| --- | --- | --- |
| SpatialHeader | 四组vec4<u32>，offset0／16／32／48，总64B；宽高／源顶点offset、4m间距与layout版本／map、地形及几何revision的lo/hi、job generation的lo/hi和有符号z原点位型 | GPU不使用裸i64；CPU另存world／branch／terrain内容hash及全局厘米原点 |
| TerrainHeights | 行主序array<f32>，stride4，y0南；高度为相对z原点的米 | 256m块＝64×64单元、65×65共享边顶点，不复制整城高精网格；只上传可见／脏块的策略后续接入 |
| TerrainFlags | 单独array<u32>，stride4，bit0为采样水标记 | SoA，CPU保留源水区精判，GPU采样全干不能证明可施工 |
| LocalVertices | array<vec4<f32>>，stride16，块内相对x/y/z米及w=1 | 超256m块须先显式裁分；禁止直接把很大的世界厘米原点转f32；三角拓扑以后仍按同一SW—NE规则 |

tile携带CPU Stamp(world/branch/map、地形／几何revision、内容hash、job generation)，matches_current必须与当前宿主快照完全匹配；重开／取消／失效需宿主换generation，GPU自行回传相同数字不是授权。GPU候选必须回到CPU做最终精确几何／权限／守恒与版本校验，尤其不得以f32误差容忍1cm侵入。device丢失只丢投影／候选，按既有CPU提交与数值策略处理，不重写历史。

缓冲内容偏移与动态binding offset是两回事；未来设备初始化须查询adapter/device limits，并按实际min_*_buffer_offset_alignment和copy对齐分配／上传，不凭64B header推导动态offset可64B对齐。并行／双缓冲、上传脏区／回读页、job背压、shader编译／布局反射、Metal／DX12读回一致性及device-loss仍未实现。现有测试只证明CPU打包的字段、大小、步长、lo/hi身份和转换误差；不能据此宣布跨后端兼容通过。

## 5. 内存划分与实际证据

完整产品常态候选仍为3.10GiB，尚无16GB实测。空间资产在既有R03几何／索引0.30GiB内划最多21MiB候选工作份额（两个Terrain各10MiB＋一份Scene1MiB）；不另开应用域，也不是每8城各一份。每个Terrain和Scene有实际owned capacity门，统一多资产缓存与21MiB联合准入尚待宿主实现；当前CLI只持一份复用Terrain和一份Scene。

导入峰值从既有运行时／保存0.45GiB内划128MiB高峰作业门；输入PNG＋连续IDAT保守各16MiB、解码器40MiB、实际解码输出、目标高程i64＋水标记u8、调用方传入的全部旧资产字节、两代几何2MiB及256KiB解析／临时余量共同预检。保留多个旧代须如实传old_owned_bytes，超限拒绝而不是免费保留；新旧并存／高峰作业调度及allocator与driver总峰值仍要联合实测。小／中／大高程单独有效载荷528392／2105352／8405000B，另有水数组及头；不把它当总占用。

GPU投影当前CPU单tile≤64KiB，另有最多1025×16B顶点，固定上界实际合计小于64KiB；未来最多2个同时在途投影的128KiB CPU额度从运行时域划出（与R54 dirty64KiB不同）。设备缓冲按用途归原GPU渲染域或求解域GPU计算32MiB，不能两域各领一份同一物理buffer。未接入GPU不计虚构显存／吞吐收益，128MiB导入与渲染／保存／预演峰值联合准入仍待T13。

[实际headless报告](evidence/m0-t07/spatial.json)：小地图66049顶点，Terrain owned594968B、Scene17617B，合计612585B；原始PNG207B，导入预检45029289B是保守额度之和，不是RSS测量。曲线17点，半径含误差355cm，冻结折线长上界4070cm；接入路径3000cm。GPU打包tile头64B、高度16900B、flags16900B，CPU owned34080B，顶点272B；平地高度转换误差0。非平坦正负高程测试另检查每样本误差≤0.5cm；这个误差只属于转换，未覆盖实际shader／GPU算术误差。

[Release测试](evidence/m0-t07/tests.txt)76项通过（35 core单元＋5区域＋7 CLI＋16空间＋12存储＋1编译失败），fmt和[clippy](evidence/m0-t07/clippy.txt)通过。实跑包含三档目标地图及4097²的16位源解码，后者33570818B解码输出；没有将计数当成触页／解码实测。包含1cm侵入、孔洞／路宽／穿层、内部山峰、曲线10001个独立取样误差点、前向门后续转弯、接缝错位／错z／侧峰／水域／邻方障碍、损坏／透明／元数据／超长解压／预算拒绝和确定性CLI两次一致。

单轮advisor设计提示及有限实现审查指出窄水遗漏和接缝只看中心两条阻断，已修并加入真实执行反例；解压输出超长由本地反例发现后加独立流校验。审查无运行通过权。测试记录的进程退出仍只属于既有Store模型，本轮不可变空间资产尚未接Store，不能转移成空间存档恢复已通过。

## 6. 复现与未完成门

```sh
cargo test --workspace --release --locked --offline
cargo clippy --workspace --all-targets --release --locked --offline -- -D warnings
cargo run --release --locked --offline -p mycity-headless -- spatial-probe fixtures/m0-spatial.json fixtures/terrain/m0-flat16.png
```

本机Apple M4／32GiB／macOS26.6.2。[短CLI资源观测](evidence/m0-t07/process.txt)包含构建调用／宿主探测，仅作本命令观测，不代表城市长跑或16GB性能。Mac14部署目标、Windows／DX12、Metal设备、UI均未实测。

T07后续：真实搜索provider／许可缓存与有界下载；像元中心／裁剪／投影／通用掩码；非平地及车辆曲率／坡度和正式道路能力；完整交通图／端口瓶颈身份；任意多边形及工程权限；不可变空间artifact接Store根及崩溃恢复；GPU编译／真实后端和T13联合峰值。先接T08有限经济装配与T07所需的正式空间引用／保存依赖，不把本轮平地步行场景扩写成所有交通已完成。
