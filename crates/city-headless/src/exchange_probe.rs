//! End-to-end regional fulfilment, through one Store and immutable spatial dependency root.
use mycity_core::{
    assembly::assemble,
    boot::{Command as B, Facility},
    exchange::{Command as X, Error as XE, Status},
    sha256,
    transaction::{Action, Caller, Envelope, Resource, TxError},
    units::Money,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn require(ok: bool, msg: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(msg.into())
    }
}
fn resources() -> Vec<Resource> {
    [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
    ]
    .into_iter()
    .map(|(id, c)| Resource::new(id, c).expect("constant resource"))
    .collect()
}
fn start(facility: Facility) -> Action {
    Action::Boot {
        command: B::Start {
            facility,
            reverse_request: true,
        },
    }
}
fn dispatch(quote: u32) -> Action {
    Action::Exchange {
        command: X::Dispatch { quote },
    }
}
fn link(edge: u32, open: bool) -> Action {
    Action::Exchange {
        command: X::SetLink { edge, open },
    }
}
struct Case {
    store: Store,
    dir: PathBuf,
    rules: String,
    input_sha: String,
    checkpoints: Vec<Value>,
}
impl Case {
    fn new(dir: &Path, v: &Value, rules: &str) -> Result<Self, String> {
        let bytes = serde_json::to_vec(v).map_err(err)?;
        let store = Store::create_spatial(
            dir,
            &bytes,
            resources(),
            rules,
            include_bytes!("../../../fixtures/m0-exchange-spatial.json"),
            include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
        )
        .map_err(err)?;
        Ok(Self {
            store,
            dir: dir.into(),
            rules: rules.into(),
            input_sha: sha256(&bytes),
            checkpoints: vec![],
        })
    }
    fn envelope(&self, a: Vec<Action>) -> Result<Envelope, String> {
        let e = self.store.engine().map_err(err)?;
        let v = e.world().view();
        Ok(Envelope {
            world: v.scope.world(),
            branch: v.scope.branch(),
            operation_id: self.store.committed_sequence() + 1,
            expected_revision: v.revision,
            actions: a,
        })
    }
    fn execute(&mut self, a: Action) -> Result<Envelope, String> {
        let c = self.envelope(vec![a])?;
        let r = self.store.execute(Caller::System, c.clone()).map_err(err)?;
        r.model.map_err(err)?;
        require(r.has_persistent_receipt, "persistent_receipt_missing")?;
        Ok(c)
    }
    fn reject(&mut self, a: Action, expected: TxError) -> Result<Envelope, String> {
        let c = self.envelope(vec![a])?;
        let hash = self.store.state_hash().map_err(err)?;
        let r = self.store.execute(Caller::System, c.clone()).map_err(err)?;
        require(
            r.model == Err(expected)
                && r.has_persistent_receipt
                && self.store.state_hash().map_err(err)? == hash,
            "rejection_mismatch_or_partial_publication",
        )?;
        Ok(c)
    }
    fn advance(&mut self, seconds: u32) -> Result<Envelope, String> {
        self.execute(Action::RegionAdvance { seconds })
    }
    fn prepare(&mut self) -> Result<(), String> {
        self.execute(start(Facility::Pump))?;
        self.advance(60)?;
        self.execute(start(Facility::Generator))?;
        self.advance(60)?;
        Ok(())
    }
    fn quantity(&self, id: u32) -> Result<i64, String> {
        Ok(self
            .store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .stocks
            .iter()
            .find(|s| s.id == id)
            .ok_or("stock")?
            .quantity
            .value())
    }
    fn record(&mut self, label: &str) -> Result<(), String> {
        self.checkpoints.push(json!({"label":label,"state_sha256":self.store.state_hash().map_err(err)?,"sequence":self.store.committed_sequence(),"world":self.store.engine().map_err(err)?.world().view(),"resources":self.store.engine().map_err(err)?.resources()}));
        Ok(())
    }
    fn report(&self) -> Result<Value, String> {
        Ok(
            json!({"input_sha256":self.input_sha,"state_sha256":self.store.state_hash().map_err(err)?,"sequence":self.store.committed_sequence(),"world":self.store.engine().map_err(err)?.world().view(),"resources":self.store.engine().map_err(err)?.resources(),"world_layout":self.store.engine().map_err(err)?.world().layout(),"engine_owned_capacity_bytes":self.store.engine().map_err(err)?.owned_capacity_bytes(),"store_owned_capacity_bytes":self.store.owned_capacity_bytes().map_err(err)?,"spatial_reference":self.store.spatial_reference().map_err(err)?,"checkpoints":self.checkpoints}),
        )
    }
}
fn recover(mut c: Case, label: &str, env: Envelope) -> Result<Case, String> {
    let hash = c.store.state_hash().map_err(err)?;
    let seq = c.store.committed_sequence();
    let expected = c
        .store
        .engine()
        .map_err(err)?
        .journal()
        .iter()
        .find(|r| r.operation_id == env.operation_id)
        .ok_or("original_receipt")?
        .outcome;
    drop(c.store);
    let mut s = Store::open(&c.dir, &c.rules).map_err(err)?;
    let r = s.execute(Caller::System, env).map_err(err)?;
    require(
        r.has_persistent_receipt
            && r.model == expected
            && s.committed_sequence() == seq
            && s.state_hash().map_err(err)? == hash,
        "regional_recovery_or_retry_mismatch",
    )?;
    c.store = s;
    c.record(label)?;
    Ok(c)
}
pub fn run(input: &[u8], root: &Path, rules: &str) -> Result<Value, String> {
    let w = assemble(input)?;
    let view = w.view();
    let x = view.exchange.ok_or("exchange_fixture_required")?;
    require(
        view.scope.world() == 1
            && view.scope.branch() == 1
            && x.home_city == 1
            && view
                .boot
                .is_some_and(|b| b.initial.map(|q| q.value()) == [5000, 100000, 0, 6000000, 0, 0]),
        "canonical_regional_fixture_required",
    )?;
    let base: Value = serde_json::from_slice(input).map_err(err)?;
    std::fs::create_dir(root).map_err(err)?;
    let mut normal = Case::new(&root.join("normal"), &base, rules)?;
    normal.execute(start(Facility::Pump))?;
    normal.execute(Action::RegionActivate { city: 2 })?;
    let last = normal.advance(30)?;
    normal = recover(normal, "closed_city_accepted_pump_wip", last)?;
    normal.advance(30)?;
    normal.execute(Action::RegionActivate { city: 1 })?;
    normal.execute(start(Facility::Generator))?;
    normal.execute(Action::RegionActivate { city: 2 })?;
    let last = normal.advance(60)?;
    normal = recover(normal, "closed_generator_completed_once", last)?;
    normal.execute(dispatch(1))?;
    normal.execute(dispatch(6))?;
    let last = normal.execute(dispatch(7))?;
    normal = recover(
        normal,
        "three_typed_shipments_no_early_destination_stock",
        last,
    )?;
    require(
        normal.quantity(8)? == 0 && normal.quantity(13)? == 0 && normal.quantity(14)? == 0,
        "premature_stock",
    )?;
    let last = normal.advance(62)?;
    normal = recover(normal, "electric_invoice_and_loss", last)?;
    let last = normal.advance(58)?;
    normal = recover(normal, "freight_receipt_acquisition_and_original_lot", last)?;
    let last = normal.advance(964)?;
    normal = recover(normal, "water_invoice_and_loss", last)?;
    require(
        normal.quantity(8)? == 1000
            && normal.quantity(13)? == 98010
            && normal.quantity(14)? == 98010,
        "physical_delivery_oracle",
    )?;
    let before = normal.store.engine().map_err(err)?.world().view();
    for city in [1, 3, 2, 1, 2] {
        let last = normal.execute(Action::RegionActivate { city })?;
        normal = recover(normal, "switch_only_updates_projection_watermarks", last)?;
    }
    let after = normal.store.engine().map_err(err)?.world().view();
    require(
        before.accounts == after.accounts
            && before.stocks == after.stocks
            && before.region.map(|r| r.interaction_time)
                == after.region.map(|r| r.interaction_time),
        "switch_replayed_money_or_goods_or_time",
    )?;
    let mut population = Case::new(&root.join("population"), &base, rules)?;
    population.execute(Action::RegionMigrate {
        from: 1,
        to: 2,
        requested: 2000,
        tolerance: 2,
    })?;
    let last = population.reject(dispatch(1), TxError::Capacity)?;
    population = recover(population, "1998_people_block_real_freight_pool", last)?;
    population.advance(30)?;
    population.execute(link(2, false))?;
    let last = population.advance(200)?;
    population = recover(
        population,
        "closed_route_keeps_people_and_housing_in_transit",
        last,
    )?;
    require(
        population
            .store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .region
            .ok_or("region")?
            .city(2)
            .map_err(err)?
            .population
            == 100,
        "people_crossed_closed_link",
    )?;
    population.execute(link(2, true))?;
    let last = population.advance(90)?;
    population = recover(
        population,
        "people_resume_remaining_route_then_free_common_pool",
        last,
    )?;
    require(
        population
            .store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .region
            .ok_or("region")?
            .city(2)
            .map_err(err)?
            .population
            == 2098,
        "population_arrival_oracle",
    )?;
    population.execute(dispatch(1))?;
    population.advance(120)?;
    let mut competition = Case::new(&root.join("competition"), &base, rules)?;
    competition.prepare()?;
    competition.execute(start(Facility::Pump))?;
    let failure = competition.reject(dispatch(6), TxError::Capacity)?;
    competition.advance(60)?;
    let last = competition.execute(dispatch(6))?;
    competition = recover(
        competition,
        "new_id_after_boot_release_accepted_before_contract_deadline",
        last,
    )?;
    let last = competition.advance(62)?;
    competition = recover(competition, "accepted_power_delivered_after_expiry", last)?;
    competition = recover(
        competition,
        "old_failed_id_still_returns_original_capacity_failure",
        failure,
    )?;
    require(
        competition
            .store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .exchange
            .ok_or("exchange")?
            .quotes[5]
            .status
            == Status::Delivered,
        "expiry_erased_delivery",
    )?;
    let mut interruptions = Vec::new();
    for (id, edge, dest, duration) in [(6, 3, 13, 62), (7, 5, 14, 1084)] {
        let mut v = base.clone();
        v["exchange"]["quotes"][id - 1]["expires_at"] = json!(190);
        let mut c = Case::new(&root.join(format!("interrupted-{id}")), &v, rules)?;
        c.prepare()?;
        c.execute(dispatch(id as u32))?;
        c.advance(10)?;
        c.execute(link(edge, false))?;
        c.execute(Action::RegionActivate { city: 3 })?;
        let last = c.advance(200)?;
        c = recover(
            c,
            "utility_buffers_money_and_leases_retained_while_closed_and_expired",
            last,
        )?;
        let ex = c
            .store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .exchange
            .ok_or("exchange")?;
        require(
            c.quantity(dest)? == 0
                && ex.energy_loss.value() + ex.water_loss.value() == 0
                && ex.freight_income.value() == 20,
            "interruption_replayed_or_lost_inputs",
        )?;
        let last = c.execute(link(edge, true))?;
        c = recover(c, "repaired_link_uses_remaining_travel_only", last)?;
        let last = c.advance(duration - 10)?;
        c = recover(c, "actual_delayed_utility_delivery_settles_once", last)?;
        require(c.quantity(dest)? == 98010, "delayed_delivery_oracle")?;
        interruptions.push(c.report()?);
    }
    let mut capacity = Case::new(&root.join("capacity"), &base, rules)?;
    for id in 1..=4 {
        capacity.execute(dispatch(id))?;
    }
    let last = capacity.reject(dispatch(5), TxError::Exchange(XE::Slots))?;
    capacity = recover(
        capacity,
        "four_live_deliveries_reject_fifth_without_partial_fee_or_locks",
        last,
    )?;
    capacity.execute(link(1, false))?;
    let last = capacity.advance(60)?;
    capacity = recover(
        capacity,
        "capacity_full_keeps_already_accepted_shipments_and_expired_future_quote",
        last,
    )?;
    require(
        capacity.quantity(7)? == 1000,
        "capacity_failure_debited_source",
    )?;
    let mut v = base.clone();
    v["region"]["obligations"][0]["amount"] = json!(700000);
    let mut debt = Case::new(&root.join("debt"), &v, rules)?;
    debt.advance(90)?;
    debt.execute(Action::RegionActivate { city: 2 })?;
    debt.advance(60)?;
    let engine = debt.store.engine().map_err(err)?;
    debt.execute(Action::Transfer {
        from: engine.world().account_ref(1)?,
        to: engine.world().account_ref(3)?,
        amount: Money::new(100000).map_err(err)?,
    })?;
    let last = debt.execute(Action::RegionSettleDebt { obligation: 1 })?;
    debt = recover(
        debt,
        "expired_old_contract_partial_real_payment_retains_arrears",
        last,
    )?;
    require(
        debt.store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .region
            .ok_or("region")?
            .obligations[0]
            .is_some_and(|o| o.arrears.value() == 1400000 && o.next_due.is_none()),
        "arrears_or_expiry_oracle",
    )?;
    Ok(
        json!({"schema":"mycity-m0-exchange-evidence-v1","result":"finite_regional_fulfilment_executed_and_verified","normal":normal.report()?,"population":population.report()?,"competition":competition.report()?,"utility_interruptions":interruptions,"capacity":capacity.report()?,"debt":debt.report()?,"exchange_inline_bytes":std::mem::size_of::<mycity_core::exchange::Exchange>(),"boot_inline_bytes":std::mem::size_of::<mycity_core::boot::Boot>(),"spatial_model":"one_verified_flat_template_three_metric_frames_and_typed_terminal_links","P_XMAP":"typed_finite_fulfilment_subset_executed_not_vehicle_or_powerflow_hydraulics","P_CAP":"voluntary_backpressure_and_reserved_existing_liabilities_subset_only","memory_16gb_gate":"not_run","ui":"not_implemented"}),
    )
}
