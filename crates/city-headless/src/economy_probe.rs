//! Executes the declared P-E0/P-E1 inputs through Store, never assigns terminal states.
use mycity_core::{
    assembly::{assemble, WorldView},
    economy::{Command, Economy, Reason, DAY, HOUR},
    sha256,
    transaction::{Action, Caller, Envelope},
    units::{Money, Quantity},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
struct Case {
    dir: PathBuf,
    store: Store,
    rules: String,
    input_sha: String,
    checkpoints: Vec<Value>,
    diag: Option<mycity_headless::cpu_log::CpuLog>,
}
impl Case {
    fn create(dir: &Path, input: &Value, rules: &str, with_space: bool) -> Result<Self, String> {
        let bytes = serde_json::to_vec(input).map_err(err)?;
        let store = if with_space {
            Store::create_spatial(
                dir,
                &bytes,
                vec![],
                rules,
                include_bytes!("../../../fixtures/m0-spatial.json"),
                include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
            )
        } else {
            Store::create(dir, &bytes, vec![], rules)
        }
        .map_err(err)?;
        let mut diag = if mycity_headless::cpu_log::enabled() {
            Some(mycity_headless::cpu_log::CpuLog::create(
                &dir.join("diagnostics"),
                "economy",
            )?)
        } else {
            None
        };
        if let Some(d) = diag.as_mut() {
            d.set_l0_class(if dir.file_name().is_some_and(|s| s == "pe0") {
                "L0_pe0_model_transaction"
            } else {
                "L0_pe1_model_transaction"
            })?;
        }
        Ok(Self {
            diag,
            dir: dir.into(),
            store,
            rules: rules.into(),
            input_sha: sha256(&bytes),
            checkpoints: Vec::new(),
        })
    }
    fn state(&self) -> Result<Economy, String> {
        self.store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .economy
            .ok_or("missing_economy".into())
    }
    fn envelope(&self, c: Command) -> Result<Envelope, String> {
        let e = self.store.engine().map_err(err)?;
        let v = e.world().view();
        Ok(Envelope {
            world: v.scope.world(),
            branch: v.scope.branch(),
            operation_id: self.store.committed_sequence() + 1,
            expected_revision: v.revision,
            actions: vec![Action::Economy { command: c }],
        })
    }
    fn execute(&mut self, c: Command) -> Result<Envelope, String> {
        let env = self.envelope(c)?;
        let result = mycity_headless::cpu_log::execute(
            &mut self.store,
            Caller::System,
            env.clone(),
            &mut self.diag,
        )?;
        result.model.map_err(err)?;
        if !result.has_persistent_receipt {
            return Err("not_persistent".into());
        }
        Ok(env)
    }
    fn run_to(&mut self, time: u64) -> Result<Envelope, String> {
        let mut last = None;
        while self.state()?.time < time {
            let dt = (time - self.state()?.time).min(HOUR);
            last = Some(self.execute(Command::Advance { seconds: dt as u32 })?);
        }
        last.ok_or("no_time_advance".into())
    }
    fn report(&mut self) -> Result<Value, String> {
        let compute = self.diag.as_mut().map(|d| d.finish()).transpose()?;
        Ok(
            json!({"compute_diagnostics":compute,"input_sha256":self.input_sha,"state_sha256":self.store.state_hash().map_err(err)?,"committed_sequence":self.store.committed_sequence(),
            "world":self.store.engine().map_err(err)?.world().view(),"store_owned_capacity_bytes":self.store.owned_capacity_bytes().map_err(err)?,
            "engine_owned_capacity_bytes":self.store.engine().map_err(err)?.owned_capacity_bytes(),"world_layout":self.store.engine().map_err(err)?.world().layout(),
            "economy_layout":{"inline_bytes":std::mem::size_of::<Economy>(),"option_bytes":std::mem::size_of::<Option<Economy>>(),"allocation":"fixed_inline_once_per_laboratory_world_not_per_resident_or_building","days":30,"max_sale_lines":32,"max_claims":3},
            "spatial_reference":self.store.spatial_reference().map_err(err)?,"checkpoints":self.checkpoints}),
        )
    }
}
// Ownership is consumed for reopening; callers cannot accidentally retain the old file lock.
fn recover(mut c: Case, label: &str, last: Envelope) -> Result<Case, String> {
    let before = c.store.state_hash().map_err(err)?;
    let sequence = c.store.committed_sequence();
    let snapshot = c.store.engine().map_err(err)?.world().view();
    drop(c.store);
    let mut store = Store::open(&c.dir, &c.rules).map_err(err)?;
    let retry = store.execute(Caller::System, last).map_err(err)?;
    retry.model.map_err(err)?;
    if before != store.state_hash().map_err(err)?
        || sequence != store.committed_sequence()
        || !retry.has_persistent_receipt
    {
        return Err("economic_recovery_retry_mismatch".into());
    }
    c.checkpoints.push(json!({"label":label,"committed_sequence":sequence,"state_sha256":before,"economy":snapshot.economy,"accounts":snapshot.accounts,"retry_persistent":true}));
    c.store = store;
    Ok(c)
}
fn mutate_money(v: &mut Value, id: u64, cents: i64) -> Result<(), String> {
    let a = v["accounts"]
        .as_array_mut()
        .ok_or("accounts")?
        .iter_mut()
        .find(|a| a["id"] == id)
        .ok_or("account")?;
    let source = a["source"].as_u64().ok_or("source")?;
    a["cash"] = json!(cents);
    v["sources"]
        .as_array_mut()
        .ok_or("sources")?
        .iter_mut()
        .find(|s| s["id"] == source)
        .ok_or("source")?["amount"] = json!(cents);
    Ok(())
}
fn money(v: &WorldView, id: u32) -> Result<i64, String> {
    v.accounts
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.cash.value())
        .ok_or("account".into())
}
fn require(test: bool, why: &str) -> Result<(), String> {
    if test {
        Ok(())
    } else {
        Err(why.into())
    }
}
pub fn run(input: &[u8], root: &Path, rules: &str) -> Result<Value, String> {
    let base_world = assemble(input)?;
    let base = base_world.view();
    let profile = base.economy.ok_or("economy_required")?;
    require(
        profile.accounts == [1, 2, 3, 4]
            && profile.supplier_stock == 1
            && profile.raw_stock == 2
            && base.scope.world() == 1
            && base.scope.branch() == 1
            && base.seed == 7,
        "canonical_pe0_fixture_identity_required",
    )?;
    require(
        [
            money(&base, 1)?,
            money(&base, 2)?,
            money(&base, 3)?,
            money(&base, 4)?,
        ] == [1000000, 200000, 600000, 1200000]
            && profile.initial_raw.value() == 600000
            && profile.claims.iter().all(Option::is_none),
        "canonical_pe0_start_required",
    )?;
    let base: Value = serde_json::from_slice(input).map_err(err)?;
    std::fs::create_dir(root).map_err(err)?;
    let mut pe0 = Case::create(&root.join("pe0"), &base, rules, true)?;
    let last = pe0.run_to(1800)?;
    require(pe0.state()?.raw_transit.is_some(), "raw_not_in_transit")?;
    pe0 = recover(pe0, "raw_in_transit", last)?;
    let last = pe0.run_to(9 * HOUR + 1800)?;
    require(
        pe0.state()?.sales_transit.iter().any(Option::is_some)
            && pe0.state()?.consumed_f == Quantity::ZERO,
        "goods_not_in_transit",
    )?;
    pe0 = recover(pe0, "products_in_transit", last)?;
    let last = pe0.run_to(30 * DAY - HOUR)?;
    pe0 = recover(pe0, "before_day30_tax", last)?;
    let last = pe0.run_to(30 * DAY)?;
    pe0 = recover(pe0, "after_day30_tax", last)?;
    let v = pe0.store.engine().map_err(err)?.world().view();
    let e = pe0.state()?;
    require(
        [money(&v, 1)?, money(&v, 2)?, money(&v, 3)?, money(&v, 4)?]
            == [808000, 56000, 816000, 1320000],
        "pe0_cash_endpoints",
    )?;
    require(
        e.closed_days == 30
            && e.raw_used.value() == 300000
            && e.produced.value() == 300000
            && e.consumed_f.value() == 180000
            && e.consumed_g.value() == 120000
            && e.claims[2].is_some_and(|c| c.outstanding() == Money::ZERO)
            && e.tax_accrued.value() == 24000
            && e.sales_transit.iter().all(Option::is_none)
            && e.raw_transit.is_none()
            && v.stocks
                .iter()
                .filter(|s| s.id != 1)
                .all(|s| s.quantity == Quantity::ZERO)
            && v.stocks
                .iter()
                .find(|s| s.id == 1)
                .is_some_and(|s| s.quantity.value() == 300000),
        "pe0_physical_and_tax_endpoints",
    )?;
    let mut low = base.clone();
    low["bootstrap"]["branch"] = json!(2);
    mutate_money(&mut low, 2, 10000)?;
    let mut low = Case::create(&root.join("pe1-low-cash"), &low, rules, false)?;
    low.run_to(3 * DAY)?;
    let row = low.state()?.records[2];
    require(
        row.f_result == Reason::NoFunds
            && row.g_result == Reason::Accepted
            && row.sales.value() == 8000
            && row.cogs.value() == 4800
            && money(&low.store.engine().map_err(err)?.world().view(), 2)? == 7600,
        "pe1_low_cash_actual_sales",
    )?;
    let row3 = low.report()?;
    let last = low.run_to(30 * DAY)?;
    low = recover(low, "day30_with_unsold_batches", last)?;
    let mut supply = base.clone();
    supply["bootstrap"]["branch"] = json!(3);
    let stock = supply["stocks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|v| v["id"] == 1)
        .unwrap();
    stock["quantity"] = json!(20000);
    stock["basis"] = json!(8000);
    supply["sources"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|v| v["id"] == 5)
        .unwrap()["amount"] = json!(8000);
    let mut supply = Case::create(&root.join("pe1-supply"), &supply, rules, false)?;
    supply.run_to(3 * DAY)?;
    require(
        supply.state()?.records[2].reason == Reason::NoInput
            && !supply.state()?.records[2].accepted
            && supply.state()?.records[2].gross_paid == Money::ZERO,
        "pe1_exhaustion_must_not_hire",
    )?;
    let day3_supply = supply.report()?;
    supply.run_to(30 * DAY)?;
    require(
        supply.state()?.produced.value() == 20000,
        "pe1_exhaustion_no_free_production",
    )?;
    let mut debt = base.clone();
    debt["bootstrap"]["branch"] = json!(4);
    debt["economy"]["debt"] = json!({"source":42,"payer":3,"payee":1,"amount":700000,"due":0});
    let mut debt = Case::create(&root.join("pe1-debt"), &debt, rules, false)?;
    debt.execute(Command::SettleDebt { claim: 1 })?;
    debt.execute(Command::BeginExit)?;
    let reject = debt.envelope(Command::Close)?;
    require(
        debt.store
            .execute(Caller::System, reject)
            .map_err(err)?
            .model
            .is_err(),
        "exit_must_refuse_untransferred_debt",
    )?;
    debt.execute(Command::HandoffDebt { claim: 1 })?;
    let last = debt.execute(Command::Close)?;
    debt = recover(debt, "debt_registry_after_activity_exit", last)?;
    require(
        !debt.state()?.active
            && debt.state()?.claims[0].is_some_and(|c| {
                c.registry && c.outstanding().value() == 100000 && c.payer == 3 && c.payee == 1
            }),
        "pe1_debt_not_forgiven",
    )?;
    let mut prepay_input = base.clone();
    prepay_input["bootstrap"]["branch"] = json!(5);
    let mut prepay = Case::create(&root.join("pe1-prepay"), &prepay_input, rules, false)?;
    prepay.execute(Command::Prepay {
        quantity: Quantity::new(20000).map_err(err)?,
    })?;
    prepay.run_to(DAY)?;
    let last = prepay.execute(Command::CancelFuture)?;
    prepay = recover(prepay, "partial_order_canceled", last)?;
    prepay.execute(Command::CancelFuture)?;
    require(
        prepay.state()?.claims[1]
            .is_some_and(|c| c.principal.value() == 4000 && c.paid.value() == 4000)
            && prepay.state()?.direct_wages.value() == 8000
            && prepay.state()?.produced.value() == 10000,
        "pe1_cancel_does_not_undo_delivery",
    )?;
    let mut refund = base.clone();
    refund["bootstrap"]["branch"] = json!(6);
    refund["economy"]["debt"] = json!({"source":43,"payer":4,"payee":1,"amount":1207000,"due":0});
    let mut refund = Case::create(&root.join("pe1-refund"), &refund, rules, false)?;
    refund.execute(Command::Prepay {
        quantity: Quantity::new(20000).map_err(err)?,
    })?;
    refund.run_to(DAY)?;
    refund.execute(Command::SettleDebt { claim: 1 })?;
    let last = refund.execute(Command::CancelFuture)?;
    refund = recover(refund, "supplier_partial_refund_debt", last)?;
    refund.execute(Command::CancelFuture)?;
    require(
        refund.state()?.claims[1].is_some_and(|c| {
            c.principal.value() == 4000 && c.paid.value() == 1000 && c.outstanding().value() == 3000
        }),
        "pe1_refund_requires_cash",
    )?;
    let mut suspend = base.clone();
    suspend["bootstrap"]["branch"] = json!(7);
    let mut suspend = Case::create(&root.join("pe1-suspend"), &suspend, rules, false)?;
    suspend.run_to(DAY + HOUR)?;
    suspend.execute(Command::SuspendShift)?;
    let last = suspend.run_to(2 * DAY)?;
    suspend = recover(suspend, "nonproductive_wage_and_tax_reduction", last)?;
    require(
        suspend.state()?.expenses.value() == 8000
            && suspend.state()?.tax_accrued == Money::ZERO
            && suspend.state()?.records[1].tax_delta_cents == -800,
        "cumulative_profit_tax_must_fall",
    )?;
    Ok(
        json!({"schema":"mycity-m0-economy-evidence-v1","result":"P-E0_P-E1_executed_and_verified","pe0":pe0.report()?,
        "pe1":{"low_cash_day3":row3,"low_cash_day30":low.report()?,"supply_day3":day3_supply,"supply_day30":supply.report()?,"debt_exit":debt.report()?,"prepaid_cancel":prepay.report()?,"refund_shortfall":refund.report()?,"suspended_shift":suspend.report()?},
        "days":30,"profile":"pe0-30d-v1","economic_balance":"not_tested_not_sustainable_pe0","P_BOOT":"not_run","memory_16gb_gate":"not_run","ui":"not_implemented","gpu":"not_used"}),
    )
}
