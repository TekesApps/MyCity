//! Frozen full table counts through real model rules, not an endurance/performance claim.
use crate::joint_reference::{self, Reference};
use mycity_core::{
    assembly::assemble,
    background::Command as Background,
    boot::{Command as Boot, Facility},
    exchange::{Command as Exchange, Status},
    inbox::Command as Inbox,
    timeline::{Command as Timeline, PayrollSpec},
    transaction::{Action, Engine, Resource},
};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::time::Instant;
pub const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-t13a-full-opening-candidate.json");
pub const POLICY: &[u8] = include_bytes!("../../../fixtures/m0-t13a-real-policy-candidate.json");
pub const ORACLE: &[u8] =
    include_bytes!("../../../fixtures/m0-t13a-expected-oracle-candidate.json");
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Small,
    Medium,
    Full,
}
impl Tier {
    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "small" => Ok(Self::Small),
            "medium" => Ok(Self::Medium),
            "full" => Ok(Self::Full),
            _ => Err("scale_tier_name".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Full => "full",
        }
    }
    pub fn counts(self) -> (usize, usize, usize, usize) {
        match self {
            Self::Small => (16, 32, 8, 2000),
            Self::Medium => (32, 64, 32, 10000),
            Self::Full => (64, 128, 64, 20000),
        }
    }
    pub fn opening(self) -> &'static [u8] {
        match self {
            Self::Full => OPENING,
            Self::Small => include_bytes!("../../../fixtures/m0-t13a-small-opening.json"),
            Self::Medium => include_bytes!("../../../fixtures/m0-t13a-medium-opening.json"),
        }
    }
    pub fn policy(self) -> &'static [u8] {
        match self {
            Self::Full => POLICY,
            Self::Small => include_bytes!("../../../fixtures/m0-t13a-small-policy.json"),
            Self::Medium => include_bytes!("../../../fixtures/m0-t13a-medium-policy.json"),
        }
    }
    pub fn oracle(self) -> &'static [u8] {
        match self {
            Self::Full => ORACLE,
            Self::Small => include_bytes!("../../../fixtures/m0-t13a-small-oracle.json"),
            Self::Medium => include_bytes!("../../../fixtures/m0-t13a-medium-oracle.json"),
        }
    }
}
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
pub fn resources() -> Result<Vec<Resource>, String> {
    [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
        (203, 1000),
        (204, 1000),
    ]
    .into_iter()
    .map(|(id, cap)| Resource::new(id, cap).map_err(err))
    .collect()
}
/// Exact startup shared by direct reference and storage diagnostics; no time advance.
pub fn startup_actions(policy: &Value) -> Result<Vec<Action>, String> {
    let mut actions = vec![
        Action::Inbox {
            command: Inbox::Enable,
        },
        Action::Boot {
            command: Boot::Start {
                facility: Facility::Pump,
                reverse_request: false,
            },
        },
        Action::Timeline {
            command: Timeline::Enable,
        },
    ];
    for value in policy["payroll"].as_array().ok_or("scale_payroll_policy")? {
        actions.push(Action::Timeline {
            command: Timeline::AddPayroll {
                spec: serde_json::from_value::<PayrollSpec>(value.clone()).map_err(err)?,
            },
        });
    }
    for value in policy["claims"].as_array().ok_or("scale_claim_policy")? {
        let mut command = value.clone();
        command["kind"] = json!("claims");
        actions.push(Action::Timeline {
            command: serde_json::from_value(command).map_err(err)?,
        });
    }
    actions.push(Action::Background {
        command: Background::Enable,
    });
    Ok(actions)
}
#[derive(Clone, Debug)]
pub enum Step {
    Action(Action),
    Drive(u64),
    Initialized,
    PeakShipments,
    HourComplete,
}
/// Frozen input order shared by independent CPU and process executions.
pub fn walk(step: impl FnMut(Step) -> Result<(), String>) -> Result<(), String> {
    walk_tier(Tier::Full, step)
}
pub fn walk_tier(
    tier: Tier,
    mut step: impl FnMut(Step) -> Result<(), String>,
) -> Result<(), String> {
    let policy: Value = serde_json::from_slice(tier.policy()).map_err(err)?;
    if policy["boot_start_seconds"] != json!({"pump":0,"generator":60})
        || policy["first_group_settle_seconds"] != 180
        || policy["freight_dispatch"]
            != json!([{"at":60,"quotes":[1,2,3,4]},{"at":1296000,"quotes":[5,6,7,8]}])
        || policy["inactive_city"]
            != json!({"city":1,"switch_to":2,"closed_at":604800,"opened_at":691200})
        || policy["game_seconds"] != 2592000
    {
        return Err("scale_frozen_schedule".into());
    }

    for action in startup_actions(&policy)? {
        step(Step::Action(action))?;
    }
    step(Step::Initialized)?;
    step(Step::Drive(60))?;
    step(Step::Action(Action::Boot {
        command: Boot::Start {
            facility: Facility::Generator,
            reverse_request: false,
        },
    }))?;
    for quote in 1..=4 {
        step(Step::Action(Action::Exchange {
            command: Exchange::Dispatch { quote },
        }))?;
    }
    step(Step::PeakShipments)?;
    step(Step::Drive(180))?;
    for sequence in 1..=4 {
        step(Step::Action(Action::Inbox {
            command: Inbox::Settle { sequence },
        }))?;
        step(Step::Action(Action::Inbox {
            command: Inbox::Archive { sequence },
        }))?;
    }
    for hour in 1..=720 {
        step(Step::Drive(hour * 3600))?;
        step(Step::HourComplete)?;
        if hour == 168 {
            step(Step::Action(Action::RegionActivate { city: 2 }))?;
        }
        if hour == 192 {
            step(Step::Action(Action::RegionActivate { city: 1 }))?;
        }
        if hour == 360 {
            for quote in 5..=8 {
                step(Step::Action(Action::Exchange {
                    command: Exchange::Dispatch { quote },
                }))?;
            }
            step(Step::Drive(hour * 3600 + 120))?;
            for sequence in 5..=8 {
                step(Step::Action(Action::Inbox {
                    command: Inbox::Settle { sequence },
                }))?;
                step(Step::Action(Action::Inbox {
                    command: Inbox::Archive { sequence },
                }))?;
            }
        }
    }
    Ok(())
}
pub fn run() -> Result<(Reference, Value), String> {
    run_observed(|_, _| Ok(()))
}
/// Observer receives actual CPU results; it cannot alter the frozen inputs.
pub fn run_observed(
    observe: impl FnMut(&Reference, &Step) -> Result<(), String>,
) -> Result<(Reference, Value), String> {
    run_tier_observed(Tier::Full, observe)
}
// Exact old Value report number semantics without allocating a report subtree.
struct LegacyField<'a>(&'a mycity_core::air::Report);
impl Serialize for LegacyField<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let r = self.0;
        let mut m = serializer.serialize_map(Some(19))?;
        macro_rules! fields {($($key:literal:$value:expr),+ $(,)?)=>{$(m.serialize_entry($key,&$value)?;)+};}
        fields! {"background":r.background,"boundary_exchange":r.boundary_exchange,"complete":r.complete,"decay":r.decay,
        "emitted_micro":r.emitted_micro,"field_sha256":r.field_sha256,"input_sha256":r.input_sha256,"integral_sha256":r.integral_sha256,
        "maximum":f64::from(r.maximum),"minimum":f64::from(r.minimum),"owned_bytes":r.owned_bytes,"progress":r.progress,
        "rounding_residual":r.rounding_residual,"side":r.side,"steps":r.steps,"strategy":r.strategy,"sum":r.sum,"target":r.target,"time":r.time}
        m.end()
    }
}
#[derive(Serialize)]
struct ReferenceCounts {
    accounts: usize,
    cities: usize,
    claims: usize,
    inflight_peak: usize,
    payroll: usize,
    quotes: usize,
    stocks: usize,
}
// Two report destinations share the same model work. The compatibility API
// builds its original Value directly; the helper streams without a full tree.
trait ReportSink {
    fn field(&mut self, key: &str, value: &(impl Serialize + ?Sized)) -> Result<(), String>;
}
struct ValueReport(Map<String, Value>);
impl ReportSink for ValueReport {
    fn field(&mut self, key: &str, value: &(impl Serialize + ?Sized)) -> Result<(), String> {
        self.0
            .insert(key.into(), serde_json::to_value(value).map_err(err)?);
        Ok(())
    }
}
struct StreamReport<'a, W> {
    writer: &'a mut W,
    first: bool,
}
impl<W: std::io::Write> ReportSink for StreamReport<'_, W> {
    fn field(&mut self, key: &str, value: &(impl Serialize + ?Sized)) -> Result<(), String> {
        if !self.first {
            self.writer.write_all(b",").map_err(err)?;
        }
        self.first = false;
        serde_json::to_writer(&mut *self.writer, key).map_err(err)?;
        self.writer.write_all(b":").map_err(err)?;
        serde_json::to_writer(&mut *self.writer, value).map_err(err)
    }
}
pub fn run_tier_observed(
    tier: Tier,
    observe: impl FnMut(&Reference, &Step) -> Result<(), String>,
) -> Result<(Reference, Value), String> {
    let mut report = ValueReport(Map::new());
    let reference = run_tier_report(tier, observe, &mut report)?;
    Ok((reference, Value::Object(report.0)))
}
/// Same oracle and model work, with borrowed report fields serialized directly.
/// The caller owns and bounds the output; no full diagnostic Value is built here.
pub fn run_tier_streamed(
    tier: Tier,
    observe: impl FnMut(&Reference, &Step) -> Result<(), String>,
    report: &mut impl std::io::Write,
) -> Result<Reference, String> {
    report.write_all(b"{").map_err(err)?;
    let reference = run_tier_report(
        tier,
        observe,
        &mut StreamReport {
            writer: report,
            first: true,
        },
    )?;
    report.write_all(b"}").map_err(err)?;
    Ok(reference)
}
fn run_tier_report(
    tier: Tier,
    mut observe: impl FnMut(&Reference, &Step) -> Result<(), String>,
    report: &mut impl ReportSink,
) -> Result<Reference, String> {
    let started = Instant::now();
    let oracle: Value = serde_json::from_slice(tier.oracle()).map_err(err)?;
    let (account_count, stock_count, payroll_count, claim_count) = tier.counts();
    let mut r = Reference::from_unstarted_engine(
        Engine::new(assemble(tier.opening())?, resources()?).map_err(err)?,
    )?;
    let initial = r.engine.world().view();
    let initial_extra = serde_json::to_value(&initial.stocks[14..]).map_err(err)?;
    if (initial.accounts.len(), initial.stocks.len()) != (account_count, stock_count)
        || initial
            .region
            .as_ref()
            .ok_or("scale_region")?
            .cities
            .iter()
            .flatten()
            .count()
            != 8
        || initial
            .exchange
            .as_ref()
            .ok_or("scale_exchange")?
            .maps
            .len()
            != 8
    {
        return Err("scale_full_counts".into());
    }
    let mut initialized_owned = 0;
    let mut peak_shipments = 0;
    let mut completed_hours = 0;
    walk_tier(tier, |step| {
        match &step {
            Step::Action(action) => {
                r.execute(vec![*action])?;
            }
            Step::Drive(target) => r.drive(*target)?,
            Step::Initialized => initialized_owned = r.engine.owned_capacity_bytes(),
            Step::PeakShipments => {
                peak_shipments = r
                    .engine
                    .world()
                    .view()
                    .exchange
                    .ok_or("scale_exchange")?
                    .shipments
                    .iter()
                    .flatten()
                    .count();
                if peak_shipments != 4 {
                    return Err("scale_inflight_not_full".into());
                }
            }
            Step::HourComplete => completed_hours += 1,
        }
        observe(&r, &step)
    })?;
    let view = r.engine.world().view();
    let t = r.engine.timeline().ok_or("scale_timeline")?;
    let x = view.exchange.as_ref().ok_or("scale_exchange")?;
    if t.claims.len() != claim_count
        || t.payroll.len() != payroll_count
        || t.claims
            .iter()
            .any(|c| !c.processed || c.paid != c.principal)
        || t.payroll
            .iter()
            .any(|p| p.paid != p.accrued || p.work_seconds != 29 * 8 * 3600)
        || x.quotes
            .iter()
            .any(|q| q.status != Status::Delivered || q.delivered.value() != 1000)
        || x.inbox.accrued.value() != 40
        || x.inbox.paid.value() != 40
        || x.inbox.closed != 8
        || x.inbox
            .facts
            .iter()
            .chain(x.inbox.retained.iter())
            .any(Option::is_some)
        || r.engine
            .resources()
            .iter()
            .any(|q| q.reserved != 0 || q.occupied != 0)
        || r.engine.leases().iter().any(Option::is_some)
    {
        return Err("scale_actual_business_incomplete".into());
    }
    for (index, quote) in x.quotes.iter().enumerate() {
        let fragment = quote
            .source_fragment
            .ok_or("scale_missing_source_fragment")?;
        if quote.invoice.value() != 2000
            || quote.freight.value() != 20
            || quote.source_cost.value() != 400
            || fragment.quantity.value() != 1000
            || fragment.basis.value() != 400
            || fragment.origin_quantity.value() != 20000
            || fragment.origin_basis.value() != 8000
            || fragment.offset.value() != index as i64 * 1000
        {
            return Err(format!(
                "scale_delivery_cost_or_provenance:{}",
                quote.spec.id
            ));
        }
    }
    let actual_accounts: Vec<Value> = view
        .accounts
        .iter()
        .map(|a| json!({"id":a.id,"cash":a.cash.value()}))
        .collect();
    let actual_paid: Vec<i64> = t.payroll.iter().map(|p| p.paid.value()).collect();
    if actual_accounts
        != oracle["expected_cash_accounts"]
            .as_array()
            .ok_or("scale_oracle_accounts")?
            .clone()
        || serde_json::to_value(actual_paid).map_err(err)? != oracle["expected_payroll_paid"]
        || view.stocks[6].quantity.value() != 12000
        || view.stocks[6].basis.value() != 4800
        || view.stocks[14].quantity.value() != 1000
        || view.stocks[14].basis.value() != 300
    {
        return Err(format!(
            "scale_oracle_mismatch:accounts={actual_accounts:?}"
        ));
    }
    for id in 8..=11 {
        let stock = &view.stocks[id - 1];
        if stock.id as usize != id
            || stock.quantity.value() != 2000
            || stock.basis.value() != 4040
            || stock.origin_quantity.value() != 2000
            || stock.origin_basis.value() != 4040
            || stock.offset.value() != 0
            || stock.lot_id != 0xc0000000 + (id as u32 - 3)
        {
            return Err(format!("scale_destination_pool:{id}"));
        }
    }
    if serde_json::to_value(&view.stocks[14..]).map_err(err)? != initial_extra {
        return Err("scale_extra_full_record_changed".into());
    }
    let obligations: Vec<_> = view
        .region
        .as_ref()
        .ok_or("scale_region")?
        .obligations
        .iter()
        .flatten()
        .collect();
    if obligations.len() != 16
        || obligations
            .iter()
            .any(|o| o.arrears.value() != 0 || o.next_due.is_some())
    {
        return Err("scale_obligation_final".into());
    }
    let field = r
        .engine
        .background()
        .and_then(|b| b.field())
        .ok_or("scale_field")?
        .report();
    if field.emitted_micro != 960000 {
        return Err("scale_actual_emissions".into());
    }
    let restored = Engine::restore_checkpoint(&r.engine.export_checkpoint()?)?;
    if restored.world().view() != view
        || restored.business_comparison_checkpoint()?
            != r.engine.business_comparison_checkpoint()?
    {
        return Err("scale_final_checkpoint_mismatch".into());
    }
    // Hashing the full business state is part of the independent model proof,
    // not a report buffer. Compute it before serializing the bounded report.
    let business_sha256 = joint_reference::business_digest(&r.engine)?;
    // Preserve the old json! report's f32 -> JSON Number(f64) conversion.
    // The fixed 19-field adapter borrows scalar data and widens extrema;
    // no diagnostic Value subtree or complete report tree is constructed.
    let field = LegacyField(&field);
    macro_rules! report_fields {
        ($($key:literal: $value:expr),+ $(,)?) => {
            $(report.field($key, &$value)?;)+
        };
    }
    report_fields! {
        "schema": "mycity-scale-reference-v1", "source_sha256": env!("MYCITY_SOURCE_SHA256"),
        "opening_sha256": mycity_core::sha256(tier.opening()),
        "policy_sha256": mycity_core::sha256(tier.policy()), "oracle_sha256": mycity_core::sha256(tier.oracle()),
        "tier": tier.name(), "scope": "named_tier_direct_cpu_model_only", "work_wall_seconds": started.elapsed().as_secs_f64(),
        "counts": ReferenceCounts{accounts:account_count,stocks:stock_count,cities:8,payroll:payroll_count,claims:claim_count,quotes:8,inflight_peak:peak_shipments},
        "game_seconds": 30*86400, "completed_hour_targets": completed_hours, "air_windows": r.windows, "field": field,
        "initial_owned_capacity_bytes": initialized_owned, "final_owned_capacity_bytes": r.engine.owned_capacity_bytes(),
        "business_sha256": business_sha256, "cash_accounts": actual_accounts,
        "wages_paid_total": t.payroll.iter().map(|p|p.paid.value()).sum::<i64>(),
        "claims_paid_total": t.claims.iter().map(|c|c.paid.value()).sum::<i64>(),
        "inbox": x.inbox, "destination_pools": view.stocks[7..11], "periodic_obligations": obligations,
        "delivery_contracts": x.quotes, "managed_stock_records":14, "extra_stock_records": stock_count-14,"extra_stock_nonzero_records":1,
        "reference_arithmetic_and_checkpoint":"passed", "performance_acceptance":"not_run",
        "eight_hour_gate":"not_run", "memory_16gb_gate":"not_run", "full_M0":"not_accepted"
    }
    Ok(r)
}

/// Export the actual direct-CPU reference for later independent process comparison.
/// NEW_DIR is never reused; this is not the persisted Sim or an eight-hour harness.
pub fn export(dir: &std::path::Path) -> Result<Value, String> {
    use std::{
        fs::{self, File},
        io::Write,
    };
    fs::create_dir(dir).map_err(err)?;
    let (reference, mut report) = run()?;
    let checkpoint = reference.engine.export_checkpoint()?;
    let mut file = File::create(dir.join("final-engine.json")).map_err(err)?;
    file.write_all(&checkpoint).map_err(err)?;
    file.sync_all().map_err(err)?;
    let exe = std::env::current_exe().map_err(err)?;
    let binary =
        mycity_core::read_bounded(File::open(exe).map_err(err)?, 64 * 1024 * 1024).map_err(err)?;
    report["binary_sha256"] = json!(mycity_core::sha256(&binary));
    report["final_checkpoint_sha256"] = json!(mycity_core::sha256(&checkpoint));
    report["final_checkpoint_bytes"] = json!(checkpoint.len());
    report["host_physical_memory_bytes"] = std::process::Command::new("sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map_or(Value::Null, Value::from);
    report["created_unix_ms"] = json!(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(err)?
        .as_millis());
    let mut file = File::create(dir.join("report.json")).map_err(err)?;
    serde_json::to_writer_pretty(&mut file, &report).map_err(err)?;
    file.write_all(b"\n").map_err(err)?;
    file.sync_all().map_err(err)?;
    File::open(dir).map_err(err)?.sync_all().map_err(err)?;
    Ok(report)
}
