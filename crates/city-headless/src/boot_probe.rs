//! Actual finite P-BOOT activity and persistent retry driver.
use mycity_core::{
    assembly::assemble,
    boot::{Boot, Command, Facility, Phase},
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
fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn start(facility: Facility) -> Command {
    Command::Start {
        facility,
        reverse_request: true,
    }
}
struct Case {
    dir: PathBuf,
    store: Store,
    rules: String,
    input_sha: String,
    checkpoints: Vec<Value>,
}
impl Case {
    fn new(dir: &Path, v: &Value, rules: &str, capacity: u32, space: bool) -> Result<Self, String> {
        let input = serde_json::to_vec(v).map_err(err)?;
        let resources = vec![
            Resource::new(101, capacity).map_err(err)?,
            Resource::new(102, capacity).map_err(err)?,
        ];
        let store = if space {
            Store::create_spatial(
                dir,
                &input,
                resources,
                rules,
                include_bytes!("../../../fixtures/m0-spatial.json"),
                include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
            )
        } else {
            Store::create(dir, &input, resources, rules)
        }
        .map_err(err)?;
        Ok(Self {
            dir: dir.into(),
            store,
            rules: rules.into(),
            input_sha: sha256(&input),
            checkpoints: vec![],
        })
    }
    fn state(&self) -> Result<Boot, String> {
        self.store
            .engine()
            .map_err(err)?
            .world()
            .view()
            .boot
            .ok_or("boot_missing".into())
    }
    fn command(&self, c: Command) -> Result<Envelope, String> {
        let e = self.store.engine().map_err(err)?;
        Ok(Envelope {
            world: 1,
            branch: 1,
            operation_id: e.journal().len() as u64 + 1,
            expected_revision: e.world().view().revision,
            actions: vec![Action::Boot { command: c }],
        })
    }
    fn execute(&mut self, c: Command) -> Result<Envelope, String> {
        let env = self.command(c)?;
        let result = self
            .store
            .execute(Caller::System, env.clone())
            .map_err(err)?;
        result.model.map_err(err)?;
        require(result.has_persistent_receipt, "missing_persistent_receipt")?;
        Ok(env)
    }
    fn reject(&mut self, c: Command) -> Result<(Envelope, TxError), String> {
        let env = self.command(c)?;
        let hash = self.store.state_hash().map_err(err)?;
        let r = self
            .store
            .execute(Caller::System, env.clone())
            .map_err(err)?;
        let failure = r.model.err().ok_or("expected_rejection")?;
        require(
            r.has_persistent_receipt && hash == self.store.state_hash().map_err(err)?,
            "rejection_partially_applied",
        )?;
        Ok((env, failure))
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
fn recover(mut c: Case, label: &str, env: Envelope, success: bool) -> Result<Case, String> {
    let hash = c.store.state_hash().map_err(err)?;
    let seq = c.store.committed_sequence();
    let expected = c
        .store
        .engine()
        .map_err(err)?
        .journal()
        .iter()
        .find(|r| r.operation_id == env.operation_id)
        .ok_or("missing_original_receipt")?
        .outcome;
    drop(c.store);
    let mut store = Store::open(&c.dir, &c.rules).map_err(err)?;
    let r = store.execute(Caller::System, env).map_err(err)?;
    require(
        r.has_persistent_receipt
            && r.model == expected
            && r.model.is_ok() == success
            && hash == store.state_hash().map_err(err)?
            && seq == store.committed_sequence(),
        "boot_recovery_retry_mismatch",
    )?;
    c.store = store;
    c.record(label)?;
    Ok(c)
}
fn set_stock(v: &mut Value, id: u32, quantity: i64, basis: i64) -> Result<(), String> {
    let s = v["stocks"]
        .as_array_mut()
        .ok_or("stocks")?
        .iter_mut()
        .find(|s| s["id"] == id)
        .ok_or("stock")?;
    let source = s["source"].as_u64().ok_or("source")?;
    s["quantity"] = json!(quantity);
    s["basis"] = json!(basis);
    v["sources"]
        .as_array_mut()
        .ok_or("sources")?
        .iter_mut()
        .find(|s| s["id"] == source)
        .ok_or("source")?["amount"] = json!(basis);
    Ok(())
}
pub fn run(bytes: &[u8], root: &Path, rules: &str) -> Result<Value, String> {
    let w = assemble(bytes)?;
    let view = w.view();
    let b = view.boot.ok_or("boot_profile_required")?;
    require(
        view.scope.world() == 1
            && view.scope.branch() == 1
            && b.operator == 3
            && b.contractor == 4
            && b.stocks == [1, 2, 3, 4, 5, 6]
            && b.resources == [101, 102]
            && b.initial.map(|q| q.value()) == [5000, 100000, 0, 6000000, 0, 0],
        "canonical_boot_fixture_required",
    )?;
    let base: Value = serde_json::from_slice(bytes).map_err(err)?;
    std::fs::create_dir(root).map_err(err)?;
    let mut normal = Case::new(&root.join("normal"), &base, rules, 2, true)?;
    normal.reject(start(Facility::Generator))?;
    let last = normal.execute(start(Facility::Pump))?;
    normal = recover(normal, "pump_admitted_no_future_water", last, true)?;
    let last = normal.execute(Command::Advance { seconds: 30 })?;
    require(normal.quantity(3)? == 0, "premature_water")?;
    normal = recover(normal, "pump_halfway_no_output", last, true)?;
    let last = normal.execute(Command::Advance { seconds: 30 })?;
    normal = recover(normal, "pump_complete", last, true)?;
    let last = normal.execute(start(Facility::Generator))?;
    normal = recover(normal, "generator_wip", last, true)?;
    let last = normal.execute(Command::Advance { seconds: 60 })?;
    normal = recover(normal, "generator_complete", last, true)?;
    normal.execute(start(Facility::Pump))?;
    normal.execute(start(Facility::Generator))?;
    let last = normal.execute(Command::Advance { seconds: 60 })?;
    normal = recover(normal, "parallel_complete", last, true)?;
    for (id, quantity) in [
        (1, 3000),
        (2, 700000),
        (3, 1080000),
        (4, 4800000),
        (5, 120000),
        (6, 1400000),
    ] {
        require(normal.quantity(id)? == quantity, "normal_physical_oracle")?;
    }
    require(
        normal.state()?.fees == Money::new(1000).map_err(err)?,
        "normal_fee_oracle",
    )?;
    let mut no_seed = base.clone();
    set_stock(&mut no_seed, 2, 0, 0)?;
    let mut no_seed = Case::new(&root.join("no-seed"), &no_seed, rules, 2, false)?;
    no_seed.reject(start(Facility::Pump))?;
    let (last, _) = no_seed.reject(start(Facility::Generator))?;
    no_seed = recover(no_seed, "both_dependency_failures_persistent", last, false)?;
    no_seed.execute(Command::Advance { seconds: 60 })?;
    require(
        no_seed.quantity(2)? == 0
            && no_seed.quantity(3)? == 0
            && no_seed.state()?.fees == Money::ZERO,
        "free_circular_start",
    )?;
    let mut water = base.clone();
    set_stock(&mut water, 2, 0, 0)?;
    set_stock(&mut water, 3, 60000, 60)?;
    let mut water = Case::new(&root.join("water-seed"), &water, rules, 2, false)?;
    water.execute(start(Facility::Generator))?;
    water.execute(Command::Advance { seconds: 60 })?;
    water.execute(start(Facility::Pump))?;
    let last = water.execute(Command::Advance { seconds: 60 })?;
    water = recover(water, "alternative_real_water_tank_seed", last, true)?;
    let mut competition = base.clone();
    set_stock(&mut competition, 3, 60000, 60)?;
    let mut competition = Case::new(&root.join("competition"), &competition, rules, 1, false)?;
    competition.execute(start(Facility::Pump))?;
    let (failed, reason) = competition.reject(start(Facility::Generator))?;
    require(
        reason == TxError::Capacity,
        "expected_shared_capacity_conflict",
    )?;
    competition.execute(Command::Advance { seconds: 60 })?;
    competition = recover(
        competition,
        "old_failed_operation_stays_failed_after_release",
        failed,
        false,
    )?;
    let last = competition.execute(start(Facility::Generator))?;
    competition = recover(
        competition,
        "new_operation_can_retry_after_capacity_release",
        last,
        true,
    )?;
    competition.execute(Command::Advance { seconds: 60 })?;
    let mut faults = Vec::new();
    for facility in [Facility::Pump, Facility::Generator] {
        let mut c = Case::new(
            &root.join(format!("fault-{facility:?}")),
            &base,
            rules,
            2,
            false,
        )?;
        if facility == Facility::Generator {
            c.execute(start(Facility::Pump))?;
            c.execute(Command::Advance { seconds: 60 })?;
        }
        c.execute(start(facility))?;
        c.execute(Command::Advance { seconds: 30 })?;
        let last = c.execute(Command::Fault { facility })?;
        c = recover(c, "failure_keeps_cost_and_actual_receivers", last, true)?;
        let i = if facility == Facility::Pump { 0 } else { 1 };
        require(
            c.state()?.jobs[i].phase == Phase::Failed && c.state()?.failed_expense > Money::ZERO,
            "failure_loss_missing",
        )?;
        c.reject(start(facility))?;
        let before = c.quantity(5)?;
        let last = c.execute(Command::ClearFailure { facility })?;
        c = recover(
            c,
            "cleanup_releases_work_leases_retains_receiver_contents",
            last,
            true,
        )?;
        require(
            c.quantity(5)? == before
                && c.store
                    .engine()
                    .map_err(err)?
                    .resources()
                    .iter()
                    .all(|r| r.occupied == 0),
            "cleanup_deleted_waste_or_leaked_capacity",
        )?;
        faults.push(c.report()?);
    }
    let mut exhausted = Case::new(&root.join("exhausted"), &base, rules, 2, false)?;
    exhausted.execute(start(Facility::Pump))?;
    exhausted.execute(Command::Advance { seconds: 60 })?;
    for _ in 0..5 {
        exhausted.execute(start(Facility::Generator))?;
        exhausted.execute(Command::Advance { seconds: 60 })?;
    }
    let (last, _) = exhausted.reject(start(Facility::Generator))?;
    exhausted = recover(exhausted, "finite_fuel_is_not_recreated", last, false)?;
    require(exhausted.quantity(1)? == 0, "fuel_not_exhausted")?;
    Ok(
        json!({"schema":"mycity-m0-boot-evidence-v1","result":"P_BOOT_finite_startup_interlock_executed_and_verified","profile":mycity_core::boot::PROFILE,"normal":normal.report()?,"no_seed":no_seed.report()?,"water_seed":water.report()?,"competition":competition.report()?,"faults":faults,"exhausted":exhausted.report()?,"boot_inline_bytes":std::mem::size_of::<Boot>(),"scope":"finite_micro_profile_not_LC38_or_city_utility_network","resource_units":{"electric_slot_envelope_watts":24000,"water_slot_envelope_liters_per_second":10,"duration_seconds":60,"resource_limits":"same_generic_lease_pool_plus_separate_finite_physical_buffers"},"memory_16gb_gate":"not_run","long_run_balance":"not_tested","ui":"not_implemented"}),
    )
}
