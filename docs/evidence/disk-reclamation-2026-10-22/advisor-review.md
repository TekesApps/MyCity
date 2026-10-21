# Advisor精确清单审查

实际核对529文件、529独立inode、23快照、14轮、23种合成组件，分配空间1,752,080,384B。未发现独立业务恢复依赖；v70／v50、共享inode及pending排除；业务world／fault-reference引用保留。执行时重核index、元数据、SHA、逐字节比较、归档解压及占用。

要求删除后fsync23个snapshot父目录，写最终结果前完成；已落实。完整原字节归档成本须从净回收扣除，df独立报告。完成该修正并更新availability可按精确清单执行。Advisor未重解压全部归档或重新散列三大布局；这些由执行脚本实测，不能替代为advisor测试。
