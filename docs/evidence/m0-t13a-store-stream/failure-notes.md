# 开发期反例与修复

最终freeze之前的定向检查不替代同源全仓验收。

- 第一次scale_assembly编译使用Engine.unwrap_err，需要Engine:Debug；改用err().unwrap，未为测试扩大模型API。
- 早期长token测试写入不存在的world.schema，serde先拒绝unknown field，断言未命中词法保护。改为已存在的boot.profile，实际覆盖长字符串进入serde scratch之前的限制；没有放松门限。
- clippy-first因测试模块后还有容量函数而失败（原stderr保留）；将函数移到测试前，clippy-second通过，仍需最终freeze同源重跑。
- 冻结元数据最初把opening fixture SHA记入fixture_sha256；在回归运行期间修正文档元数据为acceptance SHA876ea…，opening SHA04c113…单独记录。源码归档及source SHA未变；未更改任何负载。
