# 显式换岗与真实城市设施的有限联调

在既有town_lifelines上使用`--employment-control`或`--employment-change`，均指定新的`--store-dir`。这两个模式包含三日真实施工水电、道路验收扩容和市场，不再用交通/供给profile替身。OD和施工班次仍是显式群体输入。

```sh
cargo run --release --locked --offline -p mycity-storage --example town_lifelines -- --employment-control --store-dir work/own-fresh-control
cargo run --release --locked --offline -p mycity-storage --example town_lifelines -- --employment-change --store-dir work/own-fresh-change
```

共享target/work在外置SSD，运行前确认挂载，示例目录仅限尚不存在的新目录。正式性能测量期间禁止编译或运行这些功能例。

两组开局完全一致；家庭资本移至新公司而不增加现金总额，家庭只靠工资消费。控制组每日80分，换岗组首日80分、以后8分。真实路网到户运费3分加商品1分，销量分别20/20/20和20/2/2，即便第三日道路容量已扩到10，低收入仍抑制消费。单一批次100件产品不代表持续补货或自融资。

换岗由受信任输入指定，不是自动求职或审批通过。原合同在完成结算的日边界终止，新合同从同边界开始，无重叠工时；新雇主没有生产配方。工程、工资、交通proof和市场共享Store、时钟与账本，完整冷开/历史重放核对不重复落账。具体来源、失败记录、独立方程、完整恢复输入与预算限制见[阶段证据](evidence/core-employment-lifelines-133/STATUS.md)。

`market_payment_total`沿用旧CLI名称，表示Boot生产电力款；商品款和实际运输费须分别看day3_goods_payment/day3_freight及完整市场/账户记录，不得混作家庭总支出。
