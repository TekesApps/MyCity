# 迁移后继续验证：28日持久化与经济反例

主分支源流`2eb155719ad33e152a2354e43b01cb63bf4a99eb2286db1ba1e4d92ef7cfca9f`，237源文件，源码zip与SHA见freeze.json。独立线原commit88fea5e经main cherry-pick96ac2b1集成，仅两处Rust测试/fixture修改；M0冻结fixture、既有28日CLI及业务模型标准未改。

主分支同源实测：core lib **107/107**、Store town_market **3/3**、core/storage all-target严格Clippy和全仓fmt通过。原始命令/输出/退出收据及终态在本目录；两份实际测试exe在外置盘acceptance-binaries与共享target同卷硬链接保留，work/candidate-binaries为稳定入口，SHA见fixed-executables.json。work、target与TMPDIR均为外置HP P900 USB/APFS SSD，没有新target。本轮只有功能验证，另有其他项目Cargo负载，不签性能或内存规模门。

实际Store用例覆盖：丢失Applied回复后的冷开及原operation重试；28游戏日内每7日checkpoint并以新Store冷开，连续703条收据和28个TownSettle操作ID、现金/库存成本/工资游标/余数及全状态hash一致；未售库存存在时的部分已付劳动保持idle，不凭空产生生产余数。测试中消费/生产/工资均通过已有Engine和Store执行，不另造账本或持久化系统。

专用恢复fixture用家庭期初1,000,000分与每日100单位需求，确保每天100单位批次售罄，最后60秒推进算术尾数：工作777,660秒、工资应计实付21,601分、工资余数2,400、产出余数6,000，成品未完成部分不当作成品入库。此fixture不是原经济平衡基准。原CLI家庭依靠期初资金的结论不变，未证明可持续经营。

Engine反例实际覆盖现金耗尽、缺料、封路报价、配送涨价、需求下降、生产能源涨价；交通/公用条件仍是显式测试输入，不声称真实网络联调。独立阶段失败attempt20/21/22/24与最终25/26源码及输出全部保留：[具体结果](../core-production-validation/attempt-26/RESULTS.md)。最终采用Config整理测试参数，不放宽lint；有库存阻挡与无阻挡的部分劳动分支均保留。

[work迁移](../work-migration-20261022/STATUS.md)与外置存储记录已发布v0.5.110，190原件保留；本阶段不删除任何恢复原件。M0正式性能、原normal物理门、真实8h和真实16GB仍未由这些测试通过；完整M0及0.1三线未完成。继续缺口仍含真实交通/公用联调、经营退出、建设拆除与运营状态联动。
