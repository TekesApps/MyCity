# 独立审查

## 测量前方案审查

由现有 boot_advisor 独立审查，结论：没有新增阻断；须等同来源全仓测试终态、冻结来源及连续120秒 normal 后执行。

必须保存三种 helper 的 request/result、root 资产、Capacity 各项与 owner 峰和退出实例；独立核验版本、hash与额度，不只读 passed 字段。原 full 输入、完整 Engine/CPU、723目标、724worker、3011原票、L1两代及冷开均须同源。各角色逐实例收费；helper调用由父进程成功退出后再进行下一次，不能将三个并发helper共用一份额度。Sim/worker/L1实容量，driver/supervisor整堆峰加尾部，helper静态预留，observer实际记录分别报告。

RSS/footprint同帧数据、漏采、pressure/swap必须保留；heap footer不能补短命进程RSS。128KiB监听栈与其它native不属于helper 256KiB。即使本轮通过，也不签外部64MiB压力、完整joint-owned、正式稳态、8h或真实16GB。

## 结果审查

独立终审准许按v0.5.46提交“有限功能验证＋联合实测失败证据”，未发现必须新增修复的因果、容量或恢复阻断。

advisor实际执行static_check.py，核对137源码/55归档及binary绑定；独立读取功能原始工件，重算723目标、完整CPU字节和除source/wall外全部报告字段对v42一致，三模式成功及四拒绝事件一致。Capacity控制owner峰最多2、路径最多3、golden/hash各最多1，仅支持有限路径账，不称allocator硬限。独立汇总全仓293 passed/0 failed/0 ignored。

确认61.0508秒退出、非正常pressure60.2287秒、245RSS帧/swap净增0；事件只到layout_ready_observed、无业务/完整报告/正常heap footer。本次共同费用和新来源联合运行未完成。没有重跑启动负载的functional，而是独立核对既有原始工件；未做大负载重试。

继续下一项64MiB owner/grant实现。共同8MiB、完整联合压力、正式性能、8h和真实16GB仍未签收，不需为本有限提交重复联合尝试。
