# EXEC-1文档检查与收口

2026-10-23，发布v0.5.146。仅文档静态核对：231个本地Markdown目标／锚点有效；14行模型登记均覆盖激活、周期、依赖与执行位置；23份引用源码与固定main提交93be71b字节一致（见来源记录）；保留单空气worker、Town显式日结、P4必要屏障、实际wait和通用协商未实现边界。git diff --check通过。未修改游戏代码或运行测试／编译／基准，未联系主会话；不作为新功能或性能验收。

| 检查文件 | SHA-256 |
| --- | --- |
| README.md | `12b1d1ca1baae7a1545818989aaeee7fd9cd18438067a92378b78a720b245514` |
| docs/PRODUCT_BLUEPRINT.md | `eaa5757694dad882228af1d2a83b9f14177914bc7b7ca3303c271d68c6ebe432` |
| docs/SOFTWARE_ARCHITECTURE.md | `fdcc69d8b73c5e6efcb4ad4f4427b3c984df3e7a4b0e7551cc564f2356a898d6` |
| docs/city-scale-and-adaptive-budget.md | `ac3da1cff81f759be04022ddc52a8f041f912e77e806315d3324899239363fe1` |
| docs/simulation-budget-calibration-plan.md | `f9950902d03dcc25d1ed162e3835b91bff3a6053d1c6dc4e5d8bcb54c9d8c4e9` |
| docs/model-task-execution-register.md | `bce586a3a51a422356ede42dce8445ba97507a2bf2881d4fb5221f8491e02cac` |
| docs/evidence/city-simulation-budget-design/execution-register-sources.md | `60853f783efd81ba874050ae87355a6f3946759477c4d60a183f353af6ce2feb` |

后续按既有C0—C3实现登记与协调，GPU仍依C4，运行前按既定主会话所有权和测量窗口协调。本轮清单和接口到此收口，不重复审查已冻结规则。
