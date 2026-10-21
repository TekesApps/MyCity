# 下一步：完成整Role准入，回到原联合负载

本轮固定词法器和原full30日兼容已终态验证并独立签收。controller15836／session10775、parser session2212和边界Store探针均已结束，不再等待或重启它们。主源码25326e509d9a64b818ab2b9b220d8fc21235cc327e4d03d50bf9513fbc484ba7／188文件，固定CLI与archive对应；不继续无新反例地扩展本解析专题。

1. **按真实同时存活关系完成Sim／Reference／Compare整包**：沿用v81的ROLE-ALLOCATION-NEXT，更新Meta为24,903,808B heap及296B header／1MiB词法栈。包括双root及各自prepare、Store候选和codec、空间及Meta、Sim IO双槽及air_input、索引／路径／根控制、IPC／诊断、worker等。有限字符串、索引Vec及错误／控制暂态仍须有确定上界、预建或归入明确normal／diagnostic收费。不能用单进程实测峰值差额当保证。
2. **实际分配前broker授权**：当前4MiB IO grant不是整个Sim。角色声明绑定source/profile／PID／instance／generation／实际背板及容量；全部构造触页并消费后才Filled；构造失败、迟到、撤销、丢回复和死亡均实际验证，只有真实child wait后才能返还页。按现有真实阶段顺序复用角色，不能额外复制模型槽。原normal64MiB／共同8MiB保持；历史保守3root拓扑不fit不等于可以直接增grant。
3. **同源码原L1+L2实测**：整Role通过后复用仍完整的v70两代与v50旧格式输入作恢复兼容；新联合负载使用冻结原尺寸、数据和操作。先终态短诊断证明完整角色／容量／恢复没有悬空，再分级三轮稳态／四档速度／计时样本及真实28800秒8h；原故障点和冷开不能删。经济平衡与真实16GB未测仍明确列出。

优先复用`work/t13a-role-lexical-business-01/world`作下轮完整业务兼容／恢复输入；原v81小型v1和v68完整旧输入继续保留。因为规则带源码身份，兼容helper应使用原规则身份，不能强行让新Sim把旧source当成自己；新业务运行创建必要world时写明原因及替代／退役条件。

`work/t13a-lexical-parser-inputs`已逐文件退役，**不能直接恢复**；生成器及原v81原件可用于以后有具体反例时重建并核验，hash不替代仍需恢复的原文件。唯一target继续复用，不新增无理由的L1副本或独立target；阶段结束先终态／归档／依赖，再查占用和清理。

本轮无L1／normal-pool联合，没有签normal64/common8、正式分级／四速／稳态／8h。32GiB设备证据不替代16GB；完整M0不验收，目标继续。
