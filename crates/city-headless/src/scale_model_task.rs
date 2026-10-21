//! Independent CPU proof work. These children remain observed model workload;
//! extracting them never removes their workspaces or diagnostics from the budget.
use crate::{
    control_data::{self, joined, Bytes, Capacity, Scratch, Text},
    heap_meter,
    joint_reference::business_digest,
    scale_reference::{self, Step, Tier},
};
use mycity_core::{read_bounded, sha256, transaction::Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
mod stream;
const SOURCE: &str = env!("MYCITY_SOURCE_SHA256");
const FILE_LIMIT: u64 = 4 * 1024 * 1024;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Reference,
    FaultReference,
    CompareEndurance,
    CompareSpeed,
    BeforeReopen,
    Compare,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::FaultReference => "fault-reference",
            Self::CompareEndurance => "compare-endurance",
            Self::CompareSpeed => "compare-speed",
            Self::BeforeReopen => "before-reopen",
            Self::Compare => "compare",
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: Text<64>,
    source_sha256: Text<64>,
    tier: Text<8>,
    opening_sha256: Text<64>,
    policy_sha256: Text<64>,
    oracle_sha256: Text<64>,
    mode: Mode,
    retry_job: Option<u64>,
    root: Text<2048>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub name: Text<32>,
    pub bytes: u64,
    pub sha256: Text<64>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootStamp {
    pub sequence: u64,
    pub bytes: u64,
    pub sha256: Text<64>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultRecord {
    schema: Text<64>,
    source_sha256: Text<64>,
    request_sha256: Text<64>,
    mode: Mode,
    tier: Text<8>,
    pub pid: u32,
    pub instance_unix_ns: u128,
    pub artifacts: [Option<Artifact>; 3],
    #[serde(default)]
    comparison_proof: Option<stream::Proof>,
    pub image_sha256: Option<Text<64>>,
    pub business_sha256: Option<Text<64>>,
    pub root: Option<RootStamp>,
    diagnostic_budget_acceptance: Text<64>,
    pub diagnostic_capacity: Capacity,
}
#[derive(Deserialize)]
struct Current {
    slot: u8,
    root_sha256: Text<64>,
}
#[derive(Deserialize)]
struct Point {
    generation: u64,
    sha256: Text<64>,
}
#[derive(Deserialize)]
struct Root {
    sequence: u64,
    checkpoint_layout: u8,
    checkpoint_v2: Point,
}
#[derive(Deserialize)]
struct ImagePoint {
    generation: u64,
    sequence: u64,
    bytes: u64,
    sha256: Text<64>,
}
#[derive(Deserialize)]
struct Index {
    current: ImagePoint,
}
fn checked_image(world: &Path) -> Result<(Vec<u8>, Bytes, RootStamp), String> {
    let current: Current = control_data::parse(&joined(world, "CURRENT.json")?)?;
    if current.slot > 1 {
        return Err("scale_sim_root_slot".into());
    }
    let root_bytes = Bytes::read(&joined(world, &format!("root-{}.json", current.slot))?)?;
    if sha256(root_bytes.as_slice()) != current.root_sha256.as_str() {
        return Err("scale_sim_root_hash".into());
    }
    let root: Root = control_data::decode(root_bytes.as_slice())?;
    if root.checkpoint_layout != 2 {
        return Err("scale_sim_layout".into());
    }
    let index_bytes = Bytes::read(&joined(
        world,
        &format!("world-index-{}.json", root.checkpoint_v2.generation),
    )?)?;
    if sha256(index_bytes.as_slice()) != root.checkpoint_v2.sha256.as_str() {
        return Err("scale_sim_manifest_hash".into());
    }
    let index: Index = control_data::decode(index_bytes.as_slice())?;
    if index.current.generation != root.checkpoint_v2.generation
        || index.current.sequence != root.sequence
    {
        return Err("scale_sim_not_at_saved_boundary".into());
    }
    drop(index_bytes);
    let bytes = read(
        &joined(world, &format!("world-{}.json", index.current.generation))?,
        FILE_LIMIT,
    )?;
    if bytes.len() as u64 != index.current.bytes || sha256(&bytes) != index.current.sha256.as_str()
    {
        return Err("scale_sim_world_hash".into());
    }
    let stamp = RootStamp {
        sequence: root.sequence,
        bytes: root_bytes.as_slice().len() as u64,
        sha256: Text::new(&sha256(root_bytes.as_slice()))?,
    };
    Ok((bytes, root_bytes, stamp))
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    read_bounded(File::open(path).map_err(err)?, limit).map_err(err)
}
/// Fixed stack readback; limits are enforced before accepting the artifact.
pub fn fingerprint(path: &Path, limit: u64) -> Result<(u64, String), String> {
    let _scratch = Scratch::hash()?;
    let mut f = File::open(path).map_err(err)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut bytes = 0u64;
    loop {
        let n = f.read(&mut buffer).map_err(err)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or("model_task_size_overflow")?;
        if bytes > limit {
            return Err("model_task_artifact_bound".into());
        }
        hash.update(&buffer[..n]);
    }
    Ok((bytes, format!("{:x}", hash.finalize())))
}
struct BoundedFile {
    file: File,
    bytes: u64,
    limit: u64,
}
impl Write for BoundedFile {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.saturating_add(bytes.len() as u64) > self.limit {
            return Err(std::io::Error::other("model_task_output_bound"));
        }
        self.file.write_all(bytes)?;
        self.bytes += bytes.len() as u64;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
/// A partial .tmp has no authority. Link publication refuses an existing result.
fn publish(
    dir: &Path,
    name: &str,
    limit: u64,
    write: impl FnOnce(&mut BoundedFile) -> Result<(), String>,
) -> Result<(), String> {
    let tmp = joined(dir, &format!("{name}.tmp"))?;
    let mut output = BoundedFile {
        file: File::create_new(&*tmp).map_err(err)?,
        bytes: 0,
        limit,
    };
    write(&mut output)?;
    output.flush().map_err(err)?;
    output.file.sync_all().map_err(err)?;
    fs::hard_link(&*tmp, &*joined(dir, name)?).map_err(err)?;
    fs::remove_file(&*tmp).map_err(err)?;
    File::open(dir).map_err(err)?.sync_all().map_err(err)
}
fn artifact(dir: &Path, name: &str) -> Result<Artifact, String> {
    let (bytes, sha256) = fingerprint(
        &joined(dir, name)?,
        if name == "root.json" {
            8192
        } else if name == "cpu-report.json" {
            65536
        } else {
            FILE_LIMIT
        },
    )?;
    Ok(Artifact {
        name: Text::new(name)?,
        bytes,
        sha256: Text::new(&sha256)?,
    })
}
fn request(dir: &Path) -> Result<(Request, String, Tier), String> {
    let bytes = Bytes::read(&joined(dir, "request.json")?)?;
    let r: Request = control_data::decode(bytes.as_slice())?;
    let tier = Tier::parse(r.tier.as_str())?;
    let decoded = control_data::root_decode(r.root.as_str())?;
    let root = Path::new(decoded.as_str());
    if r.schema.as_str() != "mycity-model-task-request-v2"
        || r.source_sha256.as_str() != SOURCE
        || r.opening_sha256.as_str() != sha256(tier.opening())
        || r.policy_sha256.as_str() != sha256(tier.policy())
        || r.oracle_sha256.as_str() != sha256(tier.oracle())
        || !root.is_absolute()
        || root.canonicalize().map_err(err)? != root
        || dir.parent() != Some(root)
        || dir.file_name().and_then(|s| s.to_str()) != Some(r.mode.name())
    {
        return Err("model_task_request_identity".into());
    }
    Ok((r, sha256(bytes.as_slice()), tier))
}
/// CLI-internal entry. No result is published until all original proof work passes.
pub fn execute(dir: &Path) -> Result<(), String> {
    let dir = dir.canonicalize().map_err(err)?;
    let (r, request_sha256, tier) = request(&dir)?;
    if joined(&dir, "result.json")?.exists() {
        return Err("model_task_result_exists".into());
    }
    let mut result = ResultRecord {
        schema: Text::new("mycity-model-task-result-v2")?,
        source_sha256: Text::new(SOURCE)?,
        request_sha256: Text::new(&request_sha256)?,
        mode: r.mode,
        tier: r.tier,
        pid: std::process::id(),
        instance_unix_ns: heap_meter::process_instance(),
        artifacts: [None; 3],
        comparison_proof: None,
        image_sha256: None,
        business_sha256: None,
        root: None,
        diagnostic_budget_acceptance: Text::new("fixed-control-v2-not-global-acceptance")?,
        diagnostic_capacity: control_data::capacity(),
    };
    match r.mode {
        Mode::Reference | Mode::FaultReference => {
            if (r.mode == Mode::FaultReference) != r.retry_job.is_some() {
                return Err("model_task_retry_mode".into());
            }
            let mut reference = None;
            publish(&dir, "cpu-targets.jsonl", FILE_LIMIT, |golden_file| {
                let _scratch = Scratch::golden()?;
                let mut golden = BufWriter::with_capacity(65536, golden_file);
                if golden.capacity() != 65536 {
                    return Err("model_golden_capacity".into());
                }
                publish(&dir, "cpu-report.json", 65536, |report| {
                    reference = Some(scale_reference::run_tier_streamed_retry(
                        tier,
                        r.retry_job,
                        |state, step| {
                            if let Step::Drive(target) = step {
                                let air = state
                                    .engine
                                    .background()
                                    .and_then(|b| b.field())
                                    .ok_or("scale_sim_cpu_field")?;
                                #[derive(Serialize)]
                                struct Target {
                                    air_sha256: String,
                                    business_sha256: String,
                                    target: u64,
                                    windows: u64,
                                }
                                let row = Target {
                                    air_sha256: sha256(&air.checkpoint()?),
                                    business_sha256: business_digest(&state.engine)?,
                                    target: *target,
                                    windows: state.windows,
                                };
                                serde_json::to_writer(&mut golden, &row).map_err(err)?;
                                golden.write_all(b"\n").map_err(err)?;
                            }
                            Ok(())
                        },
                        report,
                    )?);
                    Ok(())
                })?;
                golden.flush().map_err(err)
            })?;
            let reference = reference.ok_or("model_task_reference_missing")?;
            publish(&dir, "cpu-final-engine.json", FILE_LIMIT, |file| {
                file.write_all(&reference.engine.export_checkpoint()?)
                    .map_err(err)
            })?;
            for (slot, name) in result.artifacts.iter_mut().zip([
                "cpu-targets.jsonl",
                "cpu-report.json",
                "cpu-final-engine.json",
            ]) {
                *slot = Some(artifact(&dir, name)?);
            }
        }
        Mode::BeforeReopen | Mode::CompareEndurance | Mode::CompareSpeed => {
            stream::execute(&dir, &r, tier, &mut result)?;
        }
        Mode::Compare => {
            let decoded = control_data::root_decode(r.root.as_str())?;
            let root = Path::new(decoded.as_str());
            let (bytes, root_bytes, stamp) = checked_image(&joined(root, "world")?)?;
            result.image_sha256 = Some(Text::new(&sha256(&bytes))?);
            publish(&dir, "root.json", 8192, |file| {
                file.write_all(root_bytes.as_slice()).map_err(err)
            })?;
            drop(root_bytes);
            result.root = Some(stamp);
            result.artifacts[0] = Some(artifact(&dir, "root.json")?);
            if matches!(
                r.mode,
                Mode::Compare | Mode::CompareEndurance | Mode::CompareSpeed
            ) {
                let before = validate(&joined(root, Mode::BeforeReopen.name())?)?;
                if before.image_sha256 != result.image_sha256 {
                    return Err("scale_sim_saved_image_changed".into());
                }
                let reference_mode = if r.mode == Mode::CompareEndurance {
                    Mode::FaultReference
                } else {
                    Mode::Reference
                };
                validate(&joined(root, reference_mode.name())?)?;
                let actual = Engine::restore_checkpoint(&bytes)?;
                drop(bytes);
                let expected = Engine::restore_checkpoint(&read(
                    &joined(
                        &joined(root, reference_mode.name())?,
                        "cpu-final-engine.json",
                    )?,
                    FILE_LIMIT,
                )?)?;
                if matches!(r.mode, Mode::CompareEndurance | Mode::CompareSpeed)
                    && actual.export_checkpoint()? != expected.export_checkpoint()?
                {
                    return Err("endurance_entire_engine_including_metadata".into());
                }
                if actual.business_comparison_checkpoint()?
                    != expected.business_comparison_checkpoint()?
                    || actual
                        .background()
                        .and_then(|b| b.field())
                        .ok_or("scale_sim_actual_field")?
                        .checkpoint()?
                        != expected
                            .background()
                            .and_then(|b| b.field())
                            .ok_or("scale_sim_expected_field")?
                            .checkpoint()?
                {
                    return Err("scale_sim_full_final_comparison".into());
                }
                result.business_sha256 = Some(Text::new(&business_digest(&actual)?)?);
            }
        }
    }
    if !control_data::idle() {
        return Err("model_task_control_owner_not_returned".into());
    }
    result.diagnostic_capacity = control_data::capacity();
    publish(&dir, "result.json", 8192, |file| {
        serde_json::to_writer(file, &result).map_err(err)
    })
}
/// Check source, request, exact artifact list, sizes and hashes after a successful exit.
pub fn validate(dir: &Path) -> Result<ResultRecord, String> {
    let (r, digest, tier) = request(dir)?;
    let result: ResultRecord = control_data::parse(&joined(dir, "result.json")?)?;
    if result.schema.as_str() != "mycity-model-task-result-v2"
        || result.source_sha256.as_str() != SOURCE
        || result.request_sha256.as_str() != digest
        || result.mode != r.mode
        || result.tier != r.tier
        || result.pid == 0
        || result.instance_unix_ns == 0
        || result.diagnostic_budget_acceptance.as_str() != "fixed-control-v2-not-global-acceptance"
    {
        return Err("model_task_result_identity".into());
    }
    let names: &[&str] = match r.mode {
        Mode::Reference | Mode::FaultReference => &[
            "cpu-targets.jsonl",
            "cpu-report.json",
            "cpu-final-engine.json",
        ],
        Mode::BeforeReopen | Mode::CompareEndurance | Mode::CompareSpeed => {
            &["root.json", "workspace.json"]
        }
        _ => &["root.json"],
    };
    for (index, slot) in result.artifacts.iter().enumerate() {
        if index >= names.len() {
            if slot.is_some() {
                return Err("model_task_artifact_list".into());
            }
            continue;
        }
        let a = slot.as_ref().ok_or("model_task_artifact_list")?;
        let name = names[index];
        if a.name.as_str() != name {
            return Err("model_task_artifact_name".into());
        }
        let expected = artifact(dir, name)?;
        if a.bytes != expected.bytes || a.sha256 != expected.sha256 {
            return Err("model_task_artifact_integrity".into());
        }
    }
    let bill = &result.diagnostic_capacity;
    let expected = control_data::capacity();
    if bill.limit_bytes != expected.limit_bytes
        || bill.control_slots_bytes != expected.control_slots_bytes
        || bill.parser_error_scratch_bytes != expected.parser_error_scratch_bytes
        || bill.paths_and_metadata_bytes != expected.paths_and_metadata_bytes
        || bill.golden_or_hash_bytes != expected.golden_or_hash_bytes
        || bill.exit_tail_bytes != expected.exit_tail_bytes
        || bill.usage.control_peak_owners > 3
        || bill.usage.path_peak_owners > 3
        || bill.usage.golden_peak_owners > 1
        || bill.usage.hash_peak_owners > 1
    {
        return Err("model_task_capacity_contract".into());
    }
    if let Some(stamp) = result.root {
        let root_bytes = Bytes::read(&joined(dir, "root.json")?)?;
        if stamp.bytes != root_bytes.as_slice().len() as u64
            || stamp.sha256.as_str() != sha256(root_bytes.as_slice())
        {
            return Err("model_task_root_asset".into());
        }
        let root: Root = control_data::decode(root_bytes.as_slice())?;
        if root.sequence != stamp.sequence {
            return Err("model_task_root_sequence".into());
        }
    }
    match r.mode {
        Mode::Reference | Mode::FaultReference
            if result.image_sha256.is_none()
                && result.business_sha256.is_none()
                && result.root.is_none() => {}
        Mode::BeforeReopen | Mode::Compare | Mode::CompareEndurance | Mode::CompareSpeed
            if result
                .image_sha256
                .as_ref()
                .is_some_and(|s| s.as_str().len() == 64)
                && result.root.is_some()
                && match r.mode {
                    Mode::Compare | Mode::CompareEndurance | Mode::CompareSpeed => result
                        .business_sha256
                        .as_ref()
                        .is_some_and(|s| s.as_str().len() == 64),
                    _ => result.business_sha256.is_none(),
                } => {}
        _ => return Err("model_task_result_shape".into()),
    }
    stream::validate_proof(dir, &r, tier, &result)?;
    Ok(result)
}
struct Job(Child);
impl Drop for Job {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
pub fn prepare(root: &Path, tier: Tier, mode: Mode) -> Result<PathBuf, String> {
    prepare_retry(root, tier, mode, None)
}
fn prepare_retry(
    root: &Path,
    tier: Tier,
    mode: Mode,
    retry_job: Option<u64>,
) -> Result<PathBuf, String> {
    if (mode == Mode::FaultReference) != retry_job.is_some() {
        return Err("model_task_retry_mode".into());
    }
    let root = root.canonicalize().map_err(err)?;
    let dir = root.join(mode.name());
    fs::create_dir(&dir).map_err(err)?;
    let r = Request {
        schema: Text::new("mycity-model-task-request-v2")?,
        source_sha256: Text::new(SOURCE)?,
        tier: Text::new(tier.name())?,
        opening_sha256: Text::new(&sha256(tier.opening()))?,
        policy_sha256: Text::new(&sha256(tier.policy()))?,
        oracle_sha256: Text::new(&sha256(tier.oracle()))?,
        mode,
        retry_job,
        root: control_data::root_encode(&root)?,
    };
    publish(&dir, "request.json", 8192, |f| {
        serde_json::to_writer(f, &r).map_err(err)
    })?;
    Ok(dir)
}
pub fn run(root: &Path, tier: Tier, mode: Mode) -> Result<ResultRecord, String> {
    run_retry(root, tier, mode, None)
}
pub(crate) fn run_retry(
    root: &Path,
    tier: Tier,
    mode: Mode,
    retry_job: Option<u64>,
) -> Result<ResultRecord, String> {
    let dir = prepare_retry(root, tier, mode, retry_job)?;
    // Register releases the exact pages before child allocation. Supervisor is
    // declared before Job; Job must kill+wait before any explicit credit return.
    let fixed = matches!(
        mode,
        Mode::BeforeReopen | Mode::CompareEndurance | Mode::CompareSpeed
    );
    let mut supervisor = if fixed {
        std::env::var_os("MYCITY_NORMAL_POOL")
            .map(|p| crate::normal_pool::broker::Supervisor::register_helper(Path::new(&p)))
            .transpose()?
    } else {
        None
    };
    let mut command = Command::new(std::env::current_exe().map_err(err)?);
    if let Some(s) = supervisor.as_ref() {
        s.configure(&mut command)?;
    }
    // Non-fixed reference work remains EXTRA; it must not inherit a Sim lease.
    else {
        command.env_remove("MYCITY_BROKER_SESSION");
    }
    let mut job = Job(command
        .arg("scale-model-task")
        .arg(&dir)
        .arg("parent-pipe")
        .stdin(Stdio::piped())
        .stdout(Stdio::from(
            File::create_new(dir.join("stdout")).map_err(err)?,
        ))
        .stderr(Stdio::from(
            File::create_new(dir.join("stderr")).map_err(err)?,
        ))
        .spawn()
        .map_err(err)?);
    if let Some(s) = supervisor.as_mut() {
        if let Err(error) = s.bind(job.0.id()) {
            let _ = job.0.kill();
            let status = job.0.wait().map_err(err)?;
            s.after_sim_wait(job.0.id(), status)?;
            return Err(error);
        }
    }
    // Publish identity before releasing the child to model work. Keep stdin
    // alive until child exit; abrupt parent death closes it at the kernel.
    let outcome = (|| {
        publish(&dir, "child.json", 4096, |file| {
            serde_json::to_writer(
                file,
                &serde_json::json!({"pid":job.0.id(),"parent_pid":std::process::id()}),
            )
            .map_err(err)
        })?;
        job.0
            .stdin
            .as_mut()
            .ok_or("model_task_parent_pipe")?
            .write_all(b"1")
            .map_err(err)?;
        let began = Instant::now();
        let status = loop {
            if let Some(status) = job.0.try_wait().map_err(err)? {
                break status;
            }
            if began.elapsed() > Duration::from_secs(600) {
                return Err("model_task_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        Ok::<_, String>(status)
    })();
    let status = match outcome {
        Ok(status) => status,
        Err(error) => {
            let _ = job.0.kill();
            let status = job.0.wait().map_err(err)?;
            if let Some(s) = supervisor.as_mut() {
                s.after_sim_wait(job.0.id(), status)?;
            }
            return Err(error);
        }
    };
    if let Some(s) = supervisor.as_mut() {
        s.after_sim_wait(job.0.id(), status)?;
    }
    if !status.success() {
        return Err(format!("model_task_child_failed:{}:{status}", job.0.id()));
    }
    let result = validate(&dir)?;
    if result.pid != job.0.id() {
        return Err("model_task_child_pid".into());
    }
    // Preserve the original public artifact paths; publication never overwrites.
    if mode == Mode::Reference {
        for a in result.artifacts.iter().flatten() {
            fs::hard_link(dir.join(a.name.as_str()), root.join(a.name.as_str())).map_err(err)?;
        }
        File::open(root).map_err(err)?.sync_all().map_err(err)?;
    }
    Ok(result)
}

/// Only the supervised path calls this. The single parent owns the write end;
/// EOF terminates pure work even if the parent was killed without Rust Drop.
/// No credit is returned here: the surviving supervisor must confirm exit.
pub fn watch_parent_pipe() -> Result<(), String> {
    let mut token = [0u8; 1];
    std::io::stdin().read_exact(&mut token).map_err(err)?;
    if token != *b"1" {
        return Err("model_task_parent_handshake".into());
    }
    std::thread::Builder::new()
        .name("model-parent-pipe".into())
        .stack_size(128 * 1024)
        .spawn(|| {
            let mut token = [0u8; 1];
            let _ = std::io::stdin().read(&mut token);
            std::process::exit(2);
        })
        .map_err(err)?;
    Ok(())
}
/// Isolated reference entry for bounded functional checks. Not a performance gate.
pub fn reference_probe(dir: &Path, tier: Tier) -> Result<ResultRecord, String> {
    fs::create_dir(dir).map_err(err)?;
    run(dir, tier, Mode::Reference)
}

// Diagnostic control frames and path conversion reserve are distinct. Fixed
// metadata must remain within the 16KiB part of the 64KiB metadata/path bill.
const _: () = assert!(
    std::mem::size_of::<Request>() * 2
        + std::mem::size_of::<ResultRecord>() * 3
        + std::mem::size_of::<Root>() * 3
        + std::mem::size_of::<Index>() * 2
        + 4096
        <= 16 * 1024
);
