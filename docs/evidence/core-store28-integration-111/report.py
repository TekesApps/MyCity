import json,subprocess
from pathlib import Path
R=Path('/Users/frankqin/dev/MyCity');D=Path(__file__).resolve().parent;t=json.loads((D/'terminal.json').read_text());assert t['functional_passed'];source=t['source_sha256']
(D/'STATUS.md').write_text(f'''# 迁移后继续验证：28日持久化与经济反例

主分支源流`{source}`，237源文件，源码zip与SHA见freeze.json。独立线原commit88fea5e经main cherry-pick96ac2b1集成，仅两处Rust测试/fixture修改；M0冻结fixture、既有28日CLI及业务模型标准未改。

主分支同源实测：core lib **107/107**、Store town_market **3/3**、core/storage all-target严格Clippy和全仓fmt通过。原始命令/输出/退出收据及终态在本目录；两份实际测试exe在外置盘acceptance-binaries与共享target同卷硬链接保留，work/candidate-binaries为稳定入口，SHA见fixed-executables.json。work、target与TMPDIR均为外置HP P900 USB/APFS SSD，没有新target。本轮只有功能验证，另有其他项目Cargo负载，不签性能或内存规模门。

实际Store用例覆盖：丢失Applied回复后的冷开及原operation重试；28游戏日内每7日checkpoint并以新Store冷开，连续703条收据和28个TownSettle操作ID、现金/库存成本/工资游标/余数及全状态hash一致；未售库存存在时的部分已付劳动保持idle，不凭空产生生产余数。测试中消费/生产/工资均通过已有Engine和Store执行，不另造账本或持久化系统。

专用恢复fixture用家庭期初1,000,000分与每日100单位需求，确保每天100单位批次售罄，最后60秒推进算术尾数：工作777,660秒、工资应计实付21,601分、工资余数2,400、产出余数6,000，成品未完成部分不当作成品入库。此fixture不是原经济平衡基准。原CLI家庭依靠期初资金的结论不变，未证明可持续经营。

Engine反例实际覆盖现金耗尽、缺料、封路报价、配送涨价、需求下降、生产能源涨价；交通/公用条件仍是显式测试输入，不声称真实网络联调。独立阶段失败attempt20/21/22/24与最终25/26源码及输出全部保留：[具体结果](../core-production-validation/attempt-26/RESULTS.md)。最终采用Config整理测试参数，不放宽lint；有库存阻挡与无阻挡的部分劳动分支均保留。

[work迁移](../work-migration-20261022/STATUS.md)与外置存储记录已发布v0.5.110，190原件保留；本阶段不删除任何恢复原件。M0正式性能、原normal物理门、真实8h和真实16GB仍未由这些测试通过；完整M0及0.1三线未完成。继续缺口仍含真实交通/公用联调、经营退出、建设拆除与运营状态联动。
''')
p=R/'README.md';s=p.read_text().replace('仓库发布版本：`v0.5.110`','仓库发布版本：`v0.5.111`',1);i=s.index('v0.5.110完成');s=s[:i]+'''v0.5.111在外置work/target上完成[28日持久化与经济反例集成验证](docs/evidence/core-store28-integration-111/STATUS.md)：主分支core107项、Store3项、严格检查与fmt通过；每7日冷开、收据连续及工资/产出余数对齐。高购买力恢复fixture与原经济基准分开，真实交通/公用联调及长期平衡未证明；失败来源保留。迁移完成不代表M0规模性能通过。

'''+s[i:];p.write_text(s)
root=R/'work/core-store28-integration-111';tmp=root/'tmp';files=list(tmp.rglob('*'));assert not any(p.is_file() for p in files)
# Only empty owned test staging directories can be removed; keep intent and receipts.
for p in sorted([p for p in files if p.is_dir()],key=lambda p:-len(p.parts)):p.rmdir()
tmp.rmdir();(D/'stage-retention.json').write_text(json.dumps(dict(removed_empty_test_staging=str(tmp),removed_pressure_bytes=0,new_target=False,shared_target=str((R/'target').resolve()),work=str((R/'work').resolve()),fixed_exes='external same-volume hardlinks; no payload duplication',restore_originals_deleted=0,existing_compile_graphs='reuse; no proven obsolete graphs removed'),indent=2)+'\n')
print('Functional report and version prepared.')
