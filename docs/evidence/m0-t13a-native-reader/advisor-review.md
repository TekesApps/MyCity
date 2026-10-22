# advisor独立审查

实现预审未见阻断独立实测：固定RUsageInfoV4/BSDInfo safe调用，无本地unsafe/裸指针/自定义FFI类型。要求ABI独立核验、CPU单位保留raw实测、身份夹取不当原子快照，并保留超长/截断及8192边界实际失败。

结果审查实际读取ABI/CPU校准/244请求结果及协议失败日志，并独立逐行断言240份夹取：出生身份/父PID一致、CPU原始计数处于独立前后值之间。额外四次为错出生/非法PID/活子/退出子，3次unknown未伪造数值。有限“原生单PID采样后端已实测验证”无阻断；须等冻结全仓终态再决定发布。

布局/返回尺寸与CPU校准仅针对本机；3,096,864B含2MiB arena，不外推完整observer。resource_unavailable单独不证明退出，本例另有真实wait。未做枚举/全树/压力/swap/时钟映射/正式收尾，不能签共同8MiB、normal、8h、16GB或置换信用。advisor未构建或重跑探针。

新增终态复核：advisor实际读取四份命令结果及regression-terminal，同一8b2c2594来源全部exit0，固定exe/source包不变；无新增发布阻断，同意有限v0.5.100。正常物理失败/共同预算/8h/16GB/完整M0均不签收。
