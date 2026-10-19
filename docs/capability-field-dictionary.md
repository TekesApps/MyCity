# 通用能力字段字典

设计契约：v0.4.24，2026-10-19。将既有P01—P34与C01—C16细化成有类型字段路径，供 [物件契约](common-object-contracts.md) 组合；不是Rust结构体、数据库迁移或已注册工具schema。所有字段服从 [粒度收敛](model-granularity-and-convergence.md)，共享profile字段不在每实例复制，关系／派生路径可投影原表，不为每个格子／物件分配以下全集。

## 0. 读表、精度、存储与更新的共同规则

S为冻结定义，A为必须保留的权威状态，R为持久关系或原来源引用，D为可重算结果。S内容可共享，但实例绑定哪个版本是R，不能随catalog更新变更。下表每个路径是一项语义字段；点号表示有限已注册类型的分面，不允许任意JSON键。可选字段返回适用原因，不是所有字段分配一个独立状态对象。

| 类型／精度标记 | 逻辑表示及准入 |
| --- | --- |
| Ref | 有world／branch／kind／稳定ID／generation校验的有类型引用；实际紧凑位宽／代际装不下先重布局，不截断身份。外引正文／索引／旧版本同样计费 |
| G | 原几何域，提交坐标0.01 m、角0.1°，中间计算防溢出；源地形／环境精度另列，0越界容忍不改变 |
| T | 整数游戏秒；时段[t0,T)、公历规则另引用，不用现实帧／墙钟。实际班组／劳动工时由秒换算，余数沿来源 |
| N | 非负整数人数／单元／件数／槽位，受相应全世界／对象限额；不能用平均人数代真实权利或独立自然人数 |
| Q | resource_code绑定的冻结单位／量化／范围；库存量、流率、累计量分别有单位。量化、范围或转换缺失为未配置；不默认f32、吨／件／m³通用相加 |
| Money | 世界游戏货币定点最小单位／有界范围，实例示例币分不擅自覆盖旧规则；计费余数归原来源。溢出／不可表示拒新承诺并保留旧事实，不能饱和截成0或无限 |
| Score | 原0—100游戏指数／有界权重，精度及平滑锚点须冻结；量化后不应每显示帧重复舍入。压缩／阈值精度未验证则表示待验，不能默认装入u8就算完成 |

更新仅由唯一writer在游戏事件／稳定区间提交；其他系统发意图或读原来源。路径按kind展开，例：住宅所有权由S02、场地几何／产权来源由S01，各元组只有一个writer，查询分面只读，不是两个系统可同时改同一字段。原事务S00负责原子提交／修订，不代替业务writer。

每段表头给出共同读取系统／指标，行内给writer和触发；完整系统反向关系沿 [P／S／M表](property-system-metric-matrix.md)。触发包括源版本变化／到期／阈值／真实交付，非每帧全城扫描。A／R不能被缓存淘汰；D缓存须有来源版本、时间、coverage／age。缺省统一区分absent、not_applicable、not_built、template_unconfigured、unknown、unsupported、inactive、shortfall和真实0；关闭不清库存／债权／占用。

存储代码：B＝原建筑／设施目标1 KiB及几何；O＝4096×128 B运营；H＝32768×64 B住房；F＝32768×64 B家庭及真实人口／劳动来源；E＝32768×32 B雇佣关系；J＝128×16 KiB工程；K＝4096×128 B关键设施；U＝65536×32 B端口；X＝8192×64 B占用组；I＝65536×64 B库存；C＝1024×1 KiB一般合同；M＝4096×128 B计量；A＝32768×96 B账户／65536×32 B义务；P＝4096×64 B植物；S＝2048服务来源／64池；L＝原规则／案件／权利和日志；D＝原稀疏路网／网络／OD／几何；T＝128×512 B教学。静态正文共用64×4 KiB热页及既有分页／磁盘，不是免费变长状态。候选复合新增另用0.125 MiB扩展，不在B／O后隐藏数组。

## 1. Root基础分面与通用依据（P01—P06、P33）

读取：所有适用系统；M23／M24。身份／绑定／版本是持久事实，template内容仍S。

| 字段路径 | 类别／类型 | 必需条件、范围／约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| root.object_ref | A／Ref | 每可寻址实体必需，类型／分支／代际匹配 | S00创建／合法迁移；原typed表 |
| root.template_ref | R／Ref | 有模板实例必需，冻结版本，非空字符串冒充注册模板无效 | S00接纳绑定；B／原实体 |
| root.schema_revision | R／有界版本 | 必需，字段语义／单位兼容才可读写 | S00迁移；原实体／模板 |
| root.instance_revision | A／有界版本 | 必需，CAS／旧计划失效，不以模板版本代实例修订 | S00提交；原实体 |
| root.creation_receipt_ref | R／Ref | 需要可追溯来源，旧引用不能指新对象 | S00创建／移交；日志 |
| root.geometry_ref | R／Ref、G | 空间物件必需；节点／边引用原几何，不给每家庭建几何 | S01结构变更；B／D／P |
| root.frame_ref | R／Ref | 有坐标必需，含合法地形／高程基准 | S01冻结／变更；原几何 |
| root.host_ref | R／Ref | 从属对象必需，独立对象明确不适用 | S01空间归属迁移；原实体 |
| root.lifecycle_ref | R／Ref | 有工程／实物状态必需，不等于企业营业或房屋入住 | S01阶段／验收；J／B |
| root.capability_mask | S／有界集合 | 只解析已注册C01—C16，声明不授予权限 | 冻结模板、S00绑定；共享正文 |
| legal.rule_set_ref | R／Ref | 所有适用动作／持续运行必查，不可卸载C | S12法令生效；L |
| legal.authority_ref | R／Ref | 涉权操作必需，玩家授权、产权、许可分别存在 | 原合法授权来源、S12校验；L／C／H |
| legal.effective_at | A／T | 法律／裁判具体生效边界，不能倒改旧区间 | S12生效；L |
| legal.transition_ref | R／Ref | 已有在途／患者／工程受影响时需明确过渡 | S12合法变更；L／原来源 |

## 2. C01空间（P07）

读取S01／S02／S04／S05／S10—S12；M01／M16／M20／M23。面积为指定口径的几何推导，不在每实体另存一份可独立改写副本。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| spatial.footprint_ref | S／R、Ref／G | 实体包络，不是整片site锁 | S01建造／改造；B／几何 |
| spatial.site_ref | R／Ref／G | 合法土地／使用范围，不能由影响半径代替 | S01权利／几何变更；原几何／L |
| spatial.area_basis | S／枚举 | site／GIA／NIA／GEA等明确单位，不相加混口径 | 冻结模板；共享正文 |
| spatial.area_value | D／m² | 由对应area_basis和冻结几何计算；非额外可出租权益 | S01查询／几何失效；有界缓存 |
| spatial.envelope_ref | R／Ref／G | 含实际z、施工／维护／必要净空与误差预算 | S01结构变更；原几何 |
| spatial.subregions_ref | S／R、Ref | 总≤8、包含排他储位等真实分配，不无限嵌套 | S01提交；B／正文，后续复合扩展 |

## 3. C02出入口与接入（P08）

读取S01／S03—S10／S12；M04—M06／M15／M16。每建筑／设施总≤8口、每口一个外接锚点；角色共享同口不再复制物理容量。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| access.port_ref | A／Ref | 独立物理口身份，非跨图512边界口 | S01创建／改造；U |
| access.local_pose | S／R、G | 局部位置／高度随实际姿态变换，不吸到最近道路 | S01结构变更；U／几何 |
| access.outward_normal | S／G | 朝向／允许方向由冻结模板声明 | S01改造；U／模板 |
| access.mode_mask | S／有界集合 | 首版步行／道路／水电相应类型，不能启用铁路／航空 | 冻结模板；U引用 |
| access.role_mask | S／有界集合 | 居住、访客、货运、维护等，符合真实开口 | 冻结模板；U引用 |
| access.anchor_ref | R／Ref | 道路侧／边内距离或正确管网连接，合法走廊全段 | S01绑定；U／D |
| access.schedule_ref | S／R、Ref | 开放时段及权限，与到达／实际使用时刻一致 | S01接纳版本，S12规则另校验；U／L |
| access.traversal_state | A／枚举＋来源Ref | 仅人货开放／封闭／等待／退出，不删在途；非水电开关 | S04通行状态；U／原需求 |
| access.network_state_ref | R／Ref | 仅水电接入／运行／隔离状态，结构绑定仍归S01；不复制交付 | S05网络状态来源；D／K／M，U只引用 |

## 4. C03权利、责任与共享（P09／P10）

读取S01／S02／S04／S06—S12；M14—M16／M23。权利与空间依赖不是同一种关系；8192共享边按真实建筑使用／责任登记。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| rights.owner_ref | R／Ref | 真实产权方，非经营者名称推导 | 原产权表按kind唯一writer，S02房屋／S01场地；H／L |
| rights.operator_ref | R／Ref | 适用经营者必需，不建重复O实体 | S07经营关联合法变更；O／C |
| rights.maintainer_ref | R／Ref | 有维护责任则必需，关闭不解除旧责任 | S10维护委托来源；C／B／K／P |
| rights.use_basis_ref | R／Ref | 对象、范围、用途、期限、优先级／关闭依据 | 原房屋S02／空间S01合法接纳；H／C／L |
| rights.shared_use_ref | R／Ref | ≤32使用建筑/空间、全世界8192，待移交也占位 | S01登记／移交；原共享关系 |
| rights.cost_share_ref | R／Ref | 同一真实费用一次、非负覆盖权重，不生成交付 | S08计费条款来源；C／H |

## 5. C04工程／拆除（P11）

读取S01／S04／S05／S08／S10—S12；M14／M18／M19／M22。静态阶段和实际阶段进度分开，最多16阶段／128档案沿原工程额度。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| project.stage_profile_ref | S／Ref | 有限依赖、工作量单位／输入／排放／拆除清场 | 冻结模板；J正文 |
| project.current_stage | A／有限状态 | 接受、施工、停工、未验收、投运等合法转换 | S01阶段提交；J |
| project.work_required | S／Q工时等 | 明确标准班组／工程量口径，不把钱当进度 | 冻结阶段；J正文 |
| project.work_done | A／Q | 实际可行投入与有效班次积分，≤该阶段工作量 | S01区间提交；J |
| project.min_calendar | S／T | 养护／最低等待不可用加班消除 | 冻结阶段；J正文 |
| project.input_source_ref | R／Ref | 真材料／机械／劳动、外包抽象包明确标记 | S01工程输入意图关联S05／S07等事实；J |
| project.safe_hold_ref | R／Ref | 停工／取消／半成品保全与费用／现场来源 | S01阶段变更；J／C |
| project.commission_receipt_ref | R／Ref | 当时权限／几何／网络／班组／启动条件共同通过 | S01验收；J／B／日志 |

## 6. C05设备／资源需求与交付（P12／P13）

读取S05—S11；M07—M10／M14／M17。设备优先共享类别／数量与工况曲线，后续组合≤8类/功能；不是每灯／插座新实体。计量行按resource和类型展开为具体字段，元组单写。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| resource.code | S／有限枚举 | 有单位／量化／合法转换，不扩原16商品或混水电与实体货物 | 冻结规则；共享正文 |
| resource.demand_profile_ref | S／Ref | 总量或待机＋增量、有效工况／时段／最小需求／短缺后果 | 冻结模板；正文 |
| equipment.profile_ref | S／Ref | 相容设备类别、额定／启动、寿命与适用服务 | 冻结模板；正文，不默认逐设备状态 |
| equipment.rated_power | S／Q W | 相同profile类别的单位额定功率，数量乘额定不当实际电量 | 冻结设备profile；共享正文 |
| equipment.startup_profile_ref | S／Ref | 启动／必要输入／峰值／失败后果，与稳定工况分开 | 冻结设备profile；共享正文 |
| equipment.duty_schedule_ref | S／Ref | 有限时段／总量或增量口径和实际活动关联，不能随意省电 | 冻结设备profile；共享正文 |
| equipment.count | S／R、N | 原模板定值；后续合法差异绑定原扩展，不能直接改已安装量 | S01真实安装／改造；B／候选扩展 |
| resource.requested | D／Q速率或区间量 | 由计划活动、profile、时间与当前状态算，不是交付事实 | S05水电／对应原适配器求解；临时缓冲 |
| resource.allocated | A／Q | 当前有效预留来源，不与实际用量重复统计 | S05水电、商品预留S07；M／I／原来源 |
| resource.delivered | A／Q累计 | 实际边界交付，不凭容量／发票生成 | S05水电、商品接受S07；M／I／C |
| resource.consumed | A／Q累计 | 同一实际活动只扣一次，启动失败已耗不退款 | S05水电／S07库存按原活动意图；M／I |
| resource.interval_ref | R／Ref／T | [t0,T)、规则／费率／来源版本与提交覆盖 | 原交付writer；M／来源日志 |
| resource.remainder_ref | R／Ref | 精度余数唯一且可迁移，不在分表复制父累计 | 原计量writer；M，工资／房租另归原字段 |

## 7. C06有类型容量（P14／P15）

读取S02—S10／S12；M01／M03／M07／M15／M16。容量族是模型类型而非一个可互换number；每真实资源键只能有一个提供方writer。按kind解析：住房S02→H，教学座位／课时S03→T（独立物理空间另请求S04），工作位声明S06→O但雇佣／劳动份额S03→E，人货道路／独立空间S04→D／X，水电输配／交付S05→D／K／M，库存／装卸接受S07→I／原货运，服务床位／队伍S09→S。只投影已有键，不新增万能容量表；未注册kind不接受预留。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| capacity.kind | S／枚举 | 住房、工作位、并发、床位、队伍、库存、吞吐等单位固定 | 冻结资源；上述原记录／共享正文 |
| capacity.design_limit | S／N或Q | 受支持范围及窗口明确，模板升级不直接扩容 | 原资源提供方接纳版本；原记录／正文 |
| capacity.effective_limit | D／N或Q | 当前设备／班组／资源／权限可行上界 | 原提供方求解；有界缓存 |
| capacity.reservation_ref | R／Ref | 原有效承诺，同一预留转实际仅转换一次 | 按上述kind注册的唯一writer；原承诺表／来源 |
| capacity.actual_occupancy | A／N或Q | 故障可大于effective，保留真实超占用／安全退出 | 同上按资源键唯一writer；H／T／E／O／X／S／I／D／K／M |
| capacity.release_source_ref | R／Ref | 真实来源／释放回执，4桶只是摘要不等于到时离场 | 按kind对应提供方；上述原来源／回执 |

## 8. C07就业、到岗与工资（P16／P17／P22）

读取S02—S04／S06／S08／S09；M03／M04／M11／M12。背景、技能、资格是不同门，不能学历直接变工资；32K匹配边保留真实成员／劳动份额。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| employment.positions | S／N | 同时工作位≠已雇人数≠参考FTE | S06接受岗位profile；O／正文 |
| employment.shift_profile_ref | S／Ref | 最多4班次窗口，实际必要工种／资格与工资保障 | 冻结岗位；正文 |
| employment.member_source_ref | R／Ref | 唯一成员／劳动份额，不能两个雇主同时用满同工时 | S03匹配／退出；E／F |
| employment.matched_ref | R／Ref | 真实雇主、工种、有效日期，招聘意图不算雇佣 | S03匹配；E |
| employment.arrival_ref | R／Ref | 真实交通到岗／合法远程规则，无模型不能当已到 | S04到达来源；E引用OD |
| employment.payable_hours | A／Q工时 | 实际合法应付，非产量比例，不每帧重计 | S03封区间；E／来源 |
| wage.basis | S／枚举 | 按时／约定薪酬及比例条件明确，不混每月与每小时 | 冻结雇佣条款；O／正文 |
| wage.rate | S／Money/计薪单位 | 单位、范围／取整来源明确 | 冻结条款；正文 |
| wage.pay_calendar_ref | S／Ref | 日／周／公历月、锚点／首期／改约，不无权延期旧款 | 冻结条款，S08依规则到期；O／E |
| wage.accrued | A／Money | 劳动发生即应计，到账前不花应收 | S08根据合法工时；A／E来源 |
| wage.paid | A／Money／原票Ref | 清偿同一债权／预留只扣一次，欠薪不消失 | S08实际付款；A／日志 |

## 9. 经营共同分面与C08生产（P18）

读取S03／S05—S11；M04／M07／M09／M10／M17—M19／M25。common为所有经营者必需，不依赖C08；零售没有制造配方仍有真实经营状态。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| operator.operator_ref | A／Ref | 真实经营者身份，非楼名自动成企业；地点和状态分路径 | S00身份创建／恢复；O |
| operator.host_ref | R／Ref | 合法实际经营地点，换经营者不重造楼或洗掉旧义务 | S07合法经营关联；O／C |
| operator.profile_ref | R／Ref | 已注册O01—O12等有界行业profile，冻结版本 | S00合法绑定；O |
| operator.operating_status | A／枚举 | 营业／停业／收尾，停业不删库存／员工／债务 | S06事件提交；O |
| operator.activity_source_ref | R／Ref | 唯一真实交付adapter事实，不二次生产 | 对应活动writer，common只引用；O |
| production.recipe_ref | S／Ref | 有界输入／输出／废物同口径，静态工艺无隐式新商品 | 冻结生产profile；正文 |
| production.equipment_limit | S／Q活动/时 | 与班组、输入、需求／容量取共同瓶颈 | 冻结设备profile；正文 |
| production.actual_activity | A／Q累计 | 同期可行活动；制造S06、电水S05等唯一adapter | 指定adapter；原活动／计量来源 |

## 10. C09库存、订单与运输（P19／P20）

读取S02／S04—S10；M05／M06／M08／M17／M25。运输状态归S04，订单／权属／接受归S07，不能“货运系统／市场都可以改全部记录”。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| inventory.owner_ref | R／Ref | 同一批货只有真实所有者，托管地不自动变产权 | S07产权转移；I |
| inventory.location_ref | R／Ref | 真仓／在途／储位，不凭到期消失或传货 | S07接受位置，运输进度引用S04；I／货运 |
| inventory.stock | A／Q库存量 | 期初＋实入−实出−损失＝期末，不用采购现金替代 | S07库存提交；I |
| inventory.held | A／Q | 同一煤不可燃烧又发货；held≤支持库存，不重复锁份额 | S07预留／释放；I |
| trade.order_ref | R／Ref | 双方、物料、费率／交付／期限／收尾，≤8资源及价段 | S07订单接受；C |
| transport.phase | A／有界状态 | 去程／等待／到达／在场／返程等原来源持续存在 | S04交通提交；原货运／OD |
| trade.accepted_delivery | A／Q累计 | 卸货／处理／最终使用有真实依据，订单不是已交付 | S07核验接受；C／I／原票 |

## 11. C10财务绑定（P21）

读取所有有费用／交易的系统；M11—M14／M25。组件只绑定原账户，余额路径为查询原A表，不是每栋钱包。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| finance.account_ref | R／Ref | 真实付款／收款／受限主体，不能用agent身份改私人钱 | S08账户绑定／合法转移；A／原对象 |
| account.cash | A／Money | 非负，实际双边转账，报价／估值／应收不加现金 | S08提交；A |
| account.protected | A／Money | 工资／托管／退出等有来源，不是又一份cash | S08预留／清偿；A及来源 |
| account.available | D／Money | cash减有效保护／占用，不能拿同钱担保两次 | S08查询；原来源投影 |
| finance.obligation_ref | R／Ref | 同一本金配对应收／应付／原票，不重复发债 | S08应计／清偿；A义务／日志 |
| obligation.creditor_ref | R／Ref | 真实债权方，托管／代收不自动取得本金 | S08原义务／合法移交；A义务 |
| obligation.debtor_ref | R／Ref | 真实债务主体，不因企业更名／停业消失 | S08原义务／合法移交；A义务 |
| obligation.source_ref | R／Ref | 工资／租约／裁判／原发票及规则版本，重放同本金只一次 | S08义务产生；A／日志 |
| obligation.remaining | A／Money | 未清本金／依法费用按类型区分，清偿不复制退款／抵扣 | S08清偿／合法调整；A义务 |
| obligation.due_at | A／T | 同一到期债权集合依据，改新约不任意后移旧款 | S08依法到期条款；A义务／持久索引 |

## 12. C11住房、家庭与生活（P23／P24／P26／P34）

读取S02—S04／S08—S13；M01／M02／M05／M08／M12／M18—M21。住房、家庭、楼体各有身份，不给每栋复制居民档案；满意度8维取原家庭状态。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| housing.unit_ref | A／Ref | 可交付单元身份，实际验收后才发布，四户四权益 | S02登记；H |
| housing.usable_area | S／G m² | 由冻结合法几何／面积口径，不按site全额当套内 | S01几何来源，H只引用；H／正文 |
| housing.person_limit | S／N | 每单元人数上界，与实际入住分开 | 冻结合法模板；H／正文 |
| housing.occupancy_ref | R／Ref | 真家庭、实际接入／登记，封门不删户 | S02入住／移交；H／F |
| housing.rent_terms_ref | R／Ref | 真实双方、30日账期／费率／押金／救济，非造价保底 | S02合法租约，钱归S08；H正文 |
| household.members_ref | R／Ref | 真成员／劳动来源与账户，画像不是事实权威 | S02人口变化；F／E |
| household.purchase_intent | A／有界来源Ref | 有限目标、现金／时间、同一需求只满足一次 | S02行为意图；F／原市场来源 |
| household.actual_final_use | A／Q累计来源 | 真交付后消费／废物，未付或未达不算满足 | S07最终使用事实，F只引用；I／原来源 |
| household.satisfaction | A／Score[8] | 基本需求、住房、收入安全、通勤、空气、噪声、服务、程序8维；精度／锚点有界 | S13游戏时间更新；F，非8份建筑副本 |
| household.low_since | A／T或明确未触发 | 低值持续／冷却不可每帧重新起算 | S13状态更新；F |
| household.cooldown_until | A／T | 原30日行为冷却等有效时点，不因分组／重开清零 | S13满意度触发状态；F |
| household.satisfaction_anchor_at | A／T | 平滑起算／剩余精度的必要锚点，实际能否放入64B须验证 | S13时间更新；F／原来源不重复 |

## 13. C12有限成人教育（P25）

读取S02／S03／S06／S08／S09／S13；M03／M04／M26。≤3技能／8资格／128批次，不从成人课程自动扩完整学龄／大学／科研。背景／技能／资格的作用键是实际成员或相容劳动来源，不是整户共享一个值；同户不同资格不能平均，不新增逐人活动调度器或无预算身份表。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| education.background | A／有限枚举 | 与技能、资格不同，来源／迁入／合法变化可追溯 | S03验证写入，S02仅提供初始人口来源意图；F |
| education.skill_level | A／有限枚举 | 3层，上层不自动持所有执照 | S03考核／培训结果；F |
| education.qualification_mask | A／有界8标签 | 获得／失效有依据，不能用学历替代 | S03资格提交；F／课程来源 |
| course.profile_ref | S／Ref | 所需背景／教师／时段、120h等工量和最小90日历日示例 | 冻结课程；正文 |
| course.attendance_hours | A／Q工时 | 教学／到课真实发生，学习不与工作重复占时间 | S03区间提交；T／劳动来源 |
| course.assessment_ref | R／Ref | 工时／日历与考核共同完成，不承诺完成即有工作 | S03结果；T／F |

## 14. C13公共服务（P27／P28）

读取S02／S04／S05／S08—S10／S13；M04／M05／M15／M16。请求类型、物理空间、服务阶段和道路交通是不同权威。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| service.kind | S／枚举 | 支持的医疗／警务／消防请求，非完整疾病／刑案／火灾模型 | 冻结服务profile；正文 |
| service.staffing_profile_ref | S／Ref | 必需工种／资格／留守、班次与实际工时 | 冻结profile，岗位统一S06；S／O正文 |
| service.pool_ref | R／Ref | 原真实能力池，对外64子池不可重卖同设施 | S09池来源；S |
| service.request_ref | A／Ref | 真来源／责任／实际需服务量，不能订单=成功 | S09接纳；S |
| service.duration_profile_ref | S／Ref | 实际服务／停留／返回和失败／退出的有界时间规则，不由半径代替 | 冻结服务profile；共享正文 |
| service.phase | A／有限状态 | 派遣、服务、返程、未满足和完成，关闭保留患者／队伍 | S09服务阶段；S，通行引用S04 |
| service.occupied_capacity_ref | R／Ref | 床位／队伍等原提供方，跨用空间向S04请求 | S09服务资源／S04空间分别写各键；S／X |
| service.completed_quantity | A／Q | 真实有效交付、响应时间／失败来源，不按半径凭空服务 | S09封区间；S／原计量 |

## 15. C14维护／条件（P29）

读取S01／S05／S06／S08／S09／S11；M07／M09／M10／M14—M16／M20。条件值在原K／B／P或明确候选扩展选一，不存在两份可独立改的健康值。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| maintenance.profile_ref | S／Ref | 时间／实际活动磨损、维护量、保全／减容／更换规则 | 冻结模板；正文 |
| maintenance.installation_at | A／T | 真安装／整套更换锚点，改名／付费不重置 | S01安装移交、S10维护只读锚点；K／B／候选扩展 |
| maintenance.condition | A／Score或Q条件 | 真实支持状态，不平均异龄，不以状态名伪造细故障 | S10区间／作业提交；K／B／P选定唯一来源 |
| maintenance.actual_work_ref | R／Ref | 真实劳动／材料／清运／停机与费用来源 | S10核验交付；C／原来源 |
| maintenance.backlog_ref | R／Ref | 未完成维护／责任、合法到期，停业仍保留 | S10作业／到期；K／B／C |
| maintenance.last_tick | A／T | 当前条件的最后有效锚点／原来源，不每帧重复累计磨损 | S10区间提交；同一K／B／P来源 |
| maintenance.derating | D／Q比例／能力 | 由条件／规则推导，不能直接改来绕安全门 | S10提供条件、原资源提供方求有效量；有界缓存 |

## 16. C15自然／管理植被（P32）

读取S01／S02／S04／S10—S13；M05／M20／M21／M23。最多4096斑块／重要树，不逐棵视觉树建状态，受管理不等于不维护也永远成熟如新。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| plant.profile_ref | S／Ref | 适用成长／衰退、管理／残余物规则，无生物模型不假装农林产业 | 冻结模板；正文 |
| plant.growth_bound_ref | R／Ref／G | 最大保守生长包络预验，摘要不能扩大权利／几何 | S01登记／改造；P／几何 |
| plant.maturity | A／Score或Q阶段 | 基于时间／条件推进，不因换视觉模型满龄 | S10事件／积分；P |
| plant.coverage | A／Q m²或有界比例 | 实体覆盖／几何并集口径，不能多楼引用重复加面积 | S10成长／作业；P／几何 |
| plant.management_quality | A／Score | 与成熟分开，真实养护才改善，死亡不等于清场 | S10作业／衰退；P |
| plant.last_tick | A／T | 成长／衰退积分锚点，切图／改LOD不重置 | S10区间提交；P |

## 17. C16外部性／环境（P30／P31）

读取S11—S13及受规则影响系统；M18／M19／M21／M23／M24。源活动是原事实，贡献是有来源推导；环境场具有自己的积分状态，不每个物件各有全图场。

| 字段路径 | 类别／类型 | 约束 | 唯一来源／更新；落点 |
| --- | --- | --- | --- |
| effect.profile_ref | S／Ref | ≤8支持通道、单位／系数／作用范围，未知不当零 | 冻结模板；正文 |
| effect.activity_ref | R／Ref | 原真实运行／通行／施工等唯一来源键 | 原adapter写活动，S11只引用；原来源 |
| effect.contribution | D／Q源强 | 每来源一次、同时间窗，车尾气不在工程再加一次 | S11有界源计算；场／原缓存 |
| environment.field_state | A／Q场 | 原32m网格／8通道，稳定积分，非任意乘大步 | S11时间／边界提交；原环境域 |
| environment.exposure | D或原A累计／Q | 同来源与实际居住／活动时间，峰值证据不能由日均补造 | S11定义的唯一家庭／场来源；F／场，存储不足不得启用新累计 |
| environment.coverage | D／有界元数据 | 模型／近似／窗口／版本／age，游戏指数不叫真实测量 | S11查询；统计／报告缓存 |

## 18. 字段完整度与不能省略的落点

以上是字段语义契约。每个Ref指向的有限profile还须具备单位／范围／版本／参数／适用性及正文大小；未配置不投运，缺新机制仍B／C，不因为能引用一段文字就算支持。数组仅允许上表明确有界集合，不构造人口×行业×资格×收入×地块全组合。

量化值、可表示最大／最小、舍入方向和溢出处置应作为启用profile的schema元数据；尚未决定具体位宽的记录标“表示待验”，不得凭Money、Q或Score标签断言128 B运营／64 B家庭已装得下。旧版本、到期源、未清义务／在途、索引和转换依据仍按原持久方案保留；实际字段／索引超预算先收窄新接纳量或重预算。

不新增组件／P组／系统／指标编号，也不新增通用动态字段表。道路、管线、水源、端口等特定结构字段在 [常用物件契约](common-object-contracts.md) 注册，复用相应原typed状态；不存在注册writer／单位／表示／参数的字段明确阻止相关操作。Coverage／source_mode等查询元数据按请求生成，不每个字段额外常驻一份。
