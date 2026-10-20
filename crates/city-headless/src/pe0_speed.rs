//! Actual session IPC P-E0 trajectory. No scaled waits or reduced business input.
use crate::joint_driver::{self, Job};
use mycity_core::{
    assembly::{assemble, WorldView},
    read_bounded, sha256,
    transaction::{Action, Caller, Engine, Envelope},
};
use mycity_storage::Store;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::Path,
    process::{ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};
const SOURCE: &str = env!("MYCITY_SOURCE_SHA256");
const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-economy-opening.json");
const DAY: u64 = 86400;
const LINE: usize = 32768;
const LOG_LIMIT: u64 = 8 * 1024 * 1024;
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    Step,
    Sixty,
}
impl Rate {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "step" => Ok(Self::Step),
            "60" => Ok(Self::Sixty),
            _ => Err("pe0_rate_requires_step_or_60".into()),
        }
    }
    fn name(self) -> &'static str {
        if self == Self::Step {
            "step"
        } else {
            "60"
        }
    }
}
/// This new slicing is frozen separately from the old half-hour probe sequence.
fn targets() -> BTreeSet<u64> {
    (1..=720).map(|h| h * 3600).chain([1800, 34200]).collect()
}
fn envelope(operation: u64, seconds: u32) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: operation,
        expected_revision: operation - 1,
        actions: vec![Action::Economy {
            command: mycity_core::economy::Command::Advance { seconds },
        }],
    }
}
fn save(path: &Path, v: &impl serde::Serialize) -> Result<(), String> {
    let mut f = File::create_new(path).map_err(err)?;
    serde_json::to_writer_pretty(&mut f, v).map_err(err)?;
    f.write_all(b"\n").map_err(err)?;
    f.sync_all().map_err(err)
}
fn facts(view: &WorldView) -> Result<Value, String> {
    let e = view.economy.ok_or("pe0_economy_missing")?;
    let cash: Vec<_> = [1, 2, 3, 4]
        .into_iter()
        .map(|id| {
            view.accounts
                .iter()
                .find(|a| a.id == id)
                .map(|a| a.cash.value())
                .ok_or("pe0_account")
        })
        .collect::<Result<_, _>>()?;
    let supplier = view
        .stocks
        .iter()
        .find(|s| s.id == 1)
        .ok_or("pe0_supplier")?
        .quantity
        .value();
    if cash != [808000, 56000, 816000, 1320000]
        || e.closed_days != 30
        || e.raw_used.value() != 300000
        || e.produced.value() != 300000
        || e.consumed_f.value() != 180000
        || e.consumed_g.value() != 120000
        || e.tax_accrued.value() != 24000
        || e.claims[2].is_none_or(|c| c.outstanding().value() != 0)
        || e.raw_transit.is_some()
        || e.sales_transit.iter().any(Option::is_some)
        || supplier != 300000
        || view
            .stocks
            .iter()
            .any(|s| s.id != 1 && s.quantity.value() != 0)
    {
        return Err("pe0_original_endpoints".into());
    }
    Ok(
        json!({"cash_cents":cash,"closed_days":e.closed_days,"raw_used":e.raw_used,"produced":e.produced,
        "consumed_f":e.consumed_f,"consumed_g":e.consumed_g,"tax_accrued":e.tax_accrued,"supplier_remaining":supplier,
        "other_stocks_empty":true,"in_transit_empty":true,"claim2_outstanding":0}),
    )
}
fn reference(dir: &Path) -> Result<(), String> {
    let mut engine = Engine::new(assemble(OPENING)?, vec![]).map_err(err)?;
    let mut f = BufWriter::new(File::create_new(dir.join("reference-targets.jsonl")).map_err(err)?);
    let mut at = 0;
    for (i, target) in targets().into_iter().enumerate() {
        engine
            .execute(Caller::System, envelope(i as u64 + 1, (target - at) as u32))
            .map_err(err)?;
        let e = engine.world().view().economy.ok_or("reference_economy")?;
        if (target == 1800 && e.raw_transit.is_none())
            || (target == 34200
                && (!e.sales_transit.iter().any(Option::is_some) || e.consumed_f.value() != 0))
        {
            return Err("pe0_original_transit_check".into());
        }
        if target == 30 * DAY - 3600 && (e.closed_days != 29 || e.claims[2].is_some()) {
            return Err("pe0_before_day30_tax".into());
        }
        if target == 30 * DAY {
            facts(&engine.world().view())?;
        }
        serde_json::to_writer(
            &mut f,
            &json!({"target":target,"operation":i+1,"state_sha256":engine.state_hash(),
                "raw_in_transit":e.raw_transit.is_some(),"sales_in_transit":e.sales_transit.iter().any(Option::is_some),
                "consumed_f":e.consumed_f,"closed_days":e.closed_days,"tax_claim_exists":e.claims[2].is_some()}),
        )
        .map_err(err)?;
        f.write_all(b"\n").map_err(err)?;
        at = target;
    }
    f.flush().map_err(err)?;
    f.get_ref().sync_all().map_err(err)?;
    save(
        &dir.join("reference-facts.json"),
        &facts(&engine.world().view())?,
    )?;
    let mut f = File::create_new(dir.join("reference-engine.json")).map_err(err)?;
    engine.write_checkpoint(&mut f)?;
    f.sync_all().map_err(err)
}
struct Ipc {
    job: Job,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    log: BufWriter<File>,
    id: u64,
    bytes: u64,
    began: Instant,
}
impl Ipc {
    fn start(dir: &Path, began: Instant) -> Result<Self, String> {
        let mut job = Job(Command::new(std::env::current_exe().map_err(err)?)
            .args([
                "session",
                dir.join("world").to_str().ok_or("world_path")?,
                SOURCE,
                "operator",
                "system",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(
                File::create_new(dir.join("session.stderr")).map_err(err)?,
            ))
            .env("MYCITY_HEAP_LOG", dir.join("heaps.jsonl"))
            .spawn()
            .map_err(err)?);
        let input = job.0.stdin.take().ok_or("stdin")?;
        let output = BufReader::with_capacity(LINE, job.0.stdout.take().ok_or("stdout")?);
        Ok(Self {
            job,
            input: Some(input),
            output,
            log: BufWriter::with_capacity(
                LINE,
                File::create_new(dir.join("transcript.jsonl")).map_err(err)?,
            ),
            id: 0,
            bytes: 0,
            began,
        })
    }
    fn call(&mut self, command: Value) -> Result<Value, String> {
        self.id += 1;
        let request = json!({"id":self.id,"request":command});
        let bytes = serde_json::to_vec(&request).map_err(err)?;
        if bytes.len() + 1 > LINE {
            return Err("pe0_request_bound".into());
        }
        let submitted = self.began.elapsed().as_secs_f64();
        let input = self.input.as_mut().ok_or("closed_input")?;
        input.write_all(&bytes).map_err(err)?;
        input.write_all(b"\n").map_err(err)?;
        input.flush().map_err(err)?;
        let mut bytes = Vec::with_capacity(1024);
        let count = (&mut self.output)
            .take((LINE + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(err)?;
        if count == 0 || count > LINE || !bytes.ends_with(b"\n") {
            return Err("pe0_response_bound".into());
        }
        let response: Value = serde_json::from_slice(&bytes).map_err(err)?;
        let row = json!({"submitted_seconds":submitted,"completed_seconds":self.began.elapsed().as_secs_f64(),"request":request,"response":response});
        let row = serde_json::to_vec(&row).map_err(err)?;
        self.bytes += row.len() as u64 + 1;
        if self.bytes > LOG_LIMIT {
            return Err("pe0_transcript_bound".into());
        }
        self.log.write_all(&row).map_err(err)?;
        self.log.write_all(b"\n").map_err(err)?;
        if response["id"] != self.id || response.get("error").is_some() {
            self.sync()?;
            return Err(format!("pe0_session_response:{response}"));
        }
        Ok(response["result"].clone())
    }
    fn sync(&mut self) -> Result<(), String> {
        self.log.flush().map_err(err)?;
        self.log.get_ref().sync_all().map_err(err)
    }
    fn stop(&mut self) -> Result<(), String> {
        self.input.take();
        if !self.job.0.wait().map_err(err)?.success() {
            return Err("pe0_session_failed".into());
        }
        self.sync()
    }
}
pub fn run(dir: &Path, rate: Rate) -> Result<(), String> {
    fs::create_dir(dir).map_err(err)?;
    let began = Instant::now();
    let args = [
        std::ffi::OsStr::new("pe0-speed-driver"),
        dir.as_os_str(),
        std::ffi::OsStr::new(rate.name()),
    ];
    let mut job = joint_driver::start(dir, "driver", &args)?;
    loop {
        if joint_driver::done(&mut job)? {
            break;
        }
        if began.elapsed() > Duration::from_secs(3600) {
            return Err("pe0_trial_timeout".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    save(
        &dir.join("supervisor-report.json"),
        &json!({"source_sha256":SOURCE,"experiment_completed":true,"whole_wall_seconds":began.elapsed().as_secs_f64(),"rate":rate.name(),"eight_hour":"not_run","real_16GB":"not_run","full_M0":"not_accepted"}),
    )
}
pub fn driver(dir: &Path, rate: Rate) -> Result<(), String> {
    let whole = Instant::now();
    save(
        &dir.join("manifest.json"),
        &json!({"schema":"pe0-session-speed-v1","source_sha256":SOURCE,"rate":rate.name(),
        "opening_sha256":sha256(OPENING),"spatial_sha256":sha256(include_bytes!("../../../fixtures/m0-spatial.json")),
        "terrain_sha256":sha256(include_bytes!("../../../fixtures/terrain/m0-flat16.png")),"targets":targets(),
        "trajectory":"722 new slicing targets; same P-E0 business, not old command sequence","t0":"before actual Store::create_spatial",
        "operating_end":"after final necessary Checkpoint response and transcript fsync","recovery":"end-only cold reopen; no mid-run pacing reset"}),
    )?;
    reference(dir)?; // Engine is dropped before actual Store creation; still in process lifetime peak.
    let reference_seconds = whole.elapsed().as_secs_f64();
    let mut refs =
        BufReader::new(File::open(dir.join("reference-targets.jsonl")).map_err(err)?).lines();
    let began = Instant::now();
    drop(
        Store::create_spatial(
            &dir.join("world"),
            OPENING,
            vec![],
            SOURCE,
            include_bytes!("../../../fixtures/m0-spatial.json"),
            include_bytes!("../../../fixtures/terrain/m0-flat16.png"),
        )
        .map_err(err)?,
    );
    let mut ipc = Ipc::start(dir, began)?;
    if rate == Rate::Sixty {
        ipc.call(json!({"command":"resume"}))?;
    }
    let mut at = 0;
    let mut days = Vec::with_capacity(30);
    let mut previous_day = 0.0;
    let mut last = None;
    for (i, target) in targets().into_iter().enumerate() {
        let operation = i as u64 + 1;
        let env = envelope(operation, (target - at) as u32);
        let r =
            ipc.call(json!({"command":if rate==Rate::Step {"step"} else {"run"},"envelope":env}))?;
        let expected: Value =
            serde_json::from_str(&refs.next().ok_or("reference_short")?.map_err(err)?)
                .map_err(err)?;
        if expected["target"] != target
            || r["status"]["engine_state_sha256"] != expected["state_sha256"]
            || r["status"]["committed_game_seconds"] != target
            || r["outcome"]["has_persistent_receipt"] != true
            || r["outcome"]["model"].get("Ok").is_none()
            || r["already_recorded"] != false
        {
            return Err(format!("pe0_target_mismatch:{target}"));
        }
        if target % DAY == 0 {
            ipc.sync()?;
            let completed = began.elapsed().as_secs_f64();
            let day = target / DAY;
            let fixed = (day - 1) as f64 * 60.0;
            let measured = if rate == Rate::Step {
                completed - previous_day
            } else {
                completed - fixed
            };
            days.push(json!({"day":day,"completed_seconds":completed,"adjacent_seconds":completed-previous_day,
                "fixed_start_seconds":if rate==Rate::Sixty{Some(fixed)}else{None},"budget_seconds":measured,"budget_met":measured<=60.0}));
            previous_day = completed;
        }
        last = Some(env);
        at = target;
    }
    if refs.next().is_some() || days.len() != 30 {
        return Err("pe0_incomplete".into());
    }
    let final_checkpoint_request = ipc.id + 1;
    ipc.call(json!({"command":"checkpoint"}))?;
    ipc.sync()?;
    let operating_end = began.elapsed().as_secs_f64();
    let last_start = if rate == Rate::Step {
        days[28]["completed_seconds"].as_f64().ok_or("day29")?
    } else {
        29.0 * 60.0
    };
    let daily = days.iter().all(|d| d["budget_met"] == true) && operating_end - last_start <= 60.0;
    let total = operating_end <= 1800.0;
    ipc.stop()?;
    let proof = Instant::now();
    let mut store = Store::open(&dir.join("world"), SOURCE).map_err(err)?;
    let expected = read_bounded(
        File::open(dir.join("reference-engine.json")).map_err(err)?,
        4 * 1024 * 1024,
    )
    .map_err(err)?;
    let before = store.engine().map_err(err)?.export_checkpoint()?;
    if before != expected {
        return Err("pe0_whole_engine_mismatch".into());
    }
    let final_facts = facts(&store.engine().map_err(err)?.world().view())?;
    let mut previous = 0;
    for (i, t) in targets().into_iter().enumerate() {
        let op = i as u64 + 1;
        let record = store
            .receipt(op)
            .map_err(err)?
            .ok_or("pe0_original_receipt_missing")?;
        let hash: [u8; 32] =
            Sha256::digest(&serde_json::to_vec(&envelope(op, (t - previous) as u32)).map_err(err)?)
                .into();
        if record.actor != 0
            || record.operation_id != op
            || record.expected_revision != op - 1
            || record.payload_sha256 != hash
            || record.outcome.is_err()
        {
            return Err("pe0_original_receipt_mismatch".into());
        }
        previous = t;
    }
    let oldest = store
        .execute(Caller::System, envelope(1, 1800))
        .map_err(err)?;
    let latest = store
        .execute(Caller::System, last.ok_or("last")?)
        .map_err(err)?;
    if !oldest.has_persistent_receipt
        || oldest.model.is_err()
        || !latest.has_persistent_receipt
        || latest.model.is_err()
        || store.committed_sequence() != 722
        || store.engine().map_err(err)?.export_checkpoint()? != before
    {
        return Err("pe0_recovery_retry".into());
    }
    save(&dir.join("facts.json"), &final_facts)?;
    save(
        &dir.join("report.json"),
        &json!({"schema":"pe0-session-speed-result-v1","source_sha256":SOURCE,"rate":rate.name(),
        "completed_game_seconds":at,"completed_targets":722,"days":days,"operating_seconds":operating_end,
        "final_checkpoint_request_id":final_checkpoint_request,"final_day_with_checkpoint_seconds":operating_end-last_start,
        "all_daily_budgets_met":daily,"whole_30day_budget_met":total,"speed_target_met":daily&&total,
        "original_receipts_verified":722,"original_endpoints":"passed","complete_engine_including_metadata":"equal_direct_engine_same_trajectory",
        "final_engine_sha256":sha256(&before),"cold_reopen_oldest_and_latest_retry":"passed",
        "reference_seconds":reference_seconds,"cold_proof_seconds":proof.elapsed().as_secs_f64(),"whole_wall_seconds":whole.elapsed().as_secs_f64(),
        "L1":"absent","common_8MiB":"not_accepted","normal_64MiB":"not_accepted","full_L2_four_rates":"not_run","eight_hour":"not_run","real_16GB":"not_run","full_M0":"not_accepted"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_targets_preserve_every_day_and_transit_observations() {
        let t = targets();
        assert_eq!(t.len(), 722);
        assert!(t.contains(&1800));
        assert!(t.contains(&34200));
        let mut previous = 0;
        for target in t {
            assert!(target > previous && target - previous <= 3600);
            previous = target;
        }
        assert_eq!(previous, 30 * DAY);
        assert!(Rate::parse("1").is_err());
        assert!(Rate::parse("step").is_ok());
        assert!(Rate::parse("60").is_ok());
    }
}
