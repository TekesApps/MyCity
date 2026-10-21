# 模型与空间预建工作区构造接入（v0.5.79）

本次有限发布完成模型＋空间预建工作区的真实构造接入基础，没有名义扩大grant；完整角色仍需受控暂存和真实broker接入及联合实测。冻结负载和性能标准未改，不做UI/GPU/AgentKit。Cargo.lock新增mycity-map对既已锁定miniz_oxide 0.8.9的直接依赖；包版本／校验值没有升级，旧固定lock仍随历史源码快照保留。此前v0.5.78为清理文档发布，模型比较基线仍取已发布v0.5.77。

## 已实现的构造接入

新增EngineWorkspace拥有两个EngineSlot、各自准备区、候选和codec。identity逐项计主要背板、两root/两prep与容量；into_parts转移已有所有权，不再在使用方另建同一批背板。它不是分配前授权或完整role证明。

World初始化共用assemble_into保留原资本来源核销/金额成本/来源不复用不遗漏等规则，只是写入已有World/Arena/Rows；一般assemble使用同一规则。EngineSlot::initialize_into先失效，写初始资本及有界资源，检查重复/占用/Boot/Exchange绑定，失败reset；成功后into_engine消费原root。绝不将此初始化用于活动城充值或重放。

Store::create_spatial_in_workspace/open_in_workspace消费已有包；genesis初始化、checkpoint恢复备用root、candidate及canonical codec不另建。Reference/from_workspace和scale实际Reference/普通Compare、Sim均接消费路径；未使用Compare candidate保留至比较结束。尚有普通独立诊断构造入口，不借此签normal角色。

## 当前开发验证（不是完整阶段验收）

当前证据源12f86a5e6acb7186261530945e3bc56b2fc7e46daca0977865782d846e6a397f，174文件，见construction-source.zip/json；源码并未按完整阶段冻结，后续可以继续修改，不能将这些结果冒充后续新源验证。固定CLI位于work/candidate-binaries/role-workspace-construction/mycity-headless。

engine_slot及满30日Reference定向5条harness记录通过（37.645秒，含编译）；checkpoint_v2和recovery共23条harness记录通过（39.732秒，含编译），覆盖初始化失败/跨scope、移交前后背板身份、实际冷开两root交换、旧恢复/真实进程退出边界。clippy all-targets退出0。

另使用已发布v77固定CLI与当前实际CLI，三档opening的完整world、state hash及layout逐字段相等，原输出及身份见initial-capital-old-binary-proof.json。该独立旧二进制对照避免只比较两个都已改为共用初始化的新API。没有运行新全仓/完整Sim联合/正式性能或8h。

advisor只读未发现构造接入的资本、失败、恢复或本次主背板计费具体阻断；明确Opening解析、空间、legacy及控制暂存和真正broker仍未在包中，不签normal64/common8。

## 空间工作区接入

旧NEXT的16×16源假设错误：m0-flat16.png实际为207B、257×257、16位灰度；v77存档绑定manifest也是257×257。现按原输入实现normal profile：PNG≤64KiB、边长≤257、8／16位灰度、原严格PNG结构／CRC／zlib完整性；通用16MiB／4097²／128MiB导入API独立保留。normal超限拒绝，不fallback、不缩原输出地图。

TerrainWorkspace始终真实初始化并保留Large的1,050,625点高度及水域背板（9,455,625B），固定InflaterState／IDAT64KiB／raw132,098B／filtered132,356B。DecoderReserve保留解码背板及完整控制头。SceneWorkspace保留13份背板：道路／通道各1025点、32输入连接点、8孔洞、两组64障碍物和7组64字节字符串；共用原几何验证／曲线内核。SpatialWorkspace消费已有背板，成功后仅发布完整不可变资产。

Store新增create_spatial_in_role_workspace／open_in_role_workspace，Sim实际入口均消费core及spatial；冷开在读取normal空间资产前检查长度上限，原始资产、哈希及派生状态继续校验。一般Store入口保持通用导入，不能借此授权超profile的normal输入。

字段级有界解析覆盖空间数组／字符串；水域和有效性掩膜使用普通结构直接解析，避免serde内部tag先缓存Content，兼容rects先于kind。normal解析前还有词法token上限，保留合法1024字节全转义license。**解析产生的Vec／String与预建Scene背板仍有同时存活窗口**，Manifest、genesis、legacy和控制对象暂态仍需整角色计费，不因末态地址稳定而忽略。

开发期原257源、两位深×五滤波×三源边长×三输出尺寸、完整高度／水域／哈希、实际背板地址与负例已运行；历史失败原件、修复及准确命令保存在本目录。开发期map/storage回归退出0，clippy修正后0；其间仍有源修改，**不当作最终同源全仓验收**。最终冻结54dc5cf342e207a3808d6accbac14f82ac2f97eae3d3368087937818e7e23e4f／180文件，lock bbf3ec2da618221730acad2379495bac4f78077e14824f7c3f27e8cb62dbbf7a，见spatial-freeze.json／spatial-source.zip；后续原始证据全部取此来源。

## 同源实测结果

| 实测 | 结果／边界 |
|---|---|
| 全仓release locked offline | 退出0，566.121秒，53组／404条harness测试记录（含子进程，非404独立业务样本） |
| clippy all-targets／fmt／workspace build | 同源均退出0 |
| 旧v77三档opening对照 | 当前固定CLI的完整world、state hash及layout与旧CLI输出相等，见spatial-capital-old-binary-proof.json |
| 旧完整业务world冷开 | 正常role消费路径恢复seq3013，2,507,418字节完整Engine与旧参考逐字节相等；两root及core／spatial全部已声明背板身份保持；原件和兼容副本各243文件SHA不变，见spatial-compat-proof.json |
| 原full30日实际Sim／worker | 340.695秒；Sim含IPC／持久化301.420秒；720小时、723驱动、724真实worker窗口，64工资、20,000债项、8单交付，完整业务与空气均与独立oracle一致；seq3011，不混同有故障的旧seq3013联合实验 |
| 冷开／原始票重试 | 冷ready0.167秒、完整图像相等；最旧收据重试无副作用 |
| 全进程Rust heap记录 | 730条成功：model3 max peak＋64KiB tail为16,502,731B；Sim2为34,264,757B；worker724为5,980,225B；driver1为614,532B。不是同步物理峰值／完整grant证明 |

原始命令、退出和耗时见spatial-*-final.json、spatial-full-business.json；完整业务报告、heap摘要及原记录、终态见spatial-full-business-report.json、spatial-heap-summary.json、spatial-full-rust-heaps.jsonl、spatial-terminal.json。固定验收CLI为`work/candidate-binaries/role-workspace-spatial/mycity-headless`，SHA dc356fe87ecdff26525fade58e2dcdc0530a58d78b45967266c88d61a251896f。全仓运行与业务实测已经退出，不要重启旧controller48053／PID55767或session80733。

没有L1压力／normal pool，本次full30d只签功能诊断，**不签normal64/common8、正式分位／四档／稳态／8h或真实16GB**。advisor独立读取原始终态证据并结合前轮静态审查，认可有限的构造接入基础发布，未代替主线程重算全存档SHA或执行模型，详advisor-review.md。

## 磁盘

复用共享target，未建新target或复制L1。开发检查结束新增删除0B，完整world、原证据/固定源/依赖/exe保留；计量见construction-retention.json，不将df变化归于清理。另有cargo在TekesAgentKit运行，已核对cwd，未停止或清理其他项目。完整阶段结束仍须重新检查本项目进程/cwd/占用及恢复依赖再回收。

本次最终阶段已经检查进程／cwd／占用，全部验证终态，work/target无打开文件；新增删除0B，不归因df变化。唯一新完整业务world保留给下一阶段Meta／role恢复；继续保留旧格式完整快照、当前两代完整布局、旧业务／兼容原件及无损组件归档。work 15,632,808KiB、共享target 1,284,668KiB（含固定验收hardlink的inode统计影响），详spatial-retention-result.json。未为本轮新建target或复制大压力载荷；未来替代输入通过后按精确依赖回收。

## 未完成门

完整RoleWorkspace、全部受控解析暂存、真实分配前grant/Filled/Child::wait归还、共同8MiB、原分级/四速/三轮稳态、28800秒8h与最终独立审查均未完成。空间resident消费路径和原full30日功能已实测，不能冒充整角色通过。当前32GiB，真实16GB not_run，M0 not_accepted，goal active。
