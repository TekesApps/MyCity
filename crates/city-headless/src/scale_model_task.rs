//! Independent CPU proof work. These children remain observed model workload;
//! extracting them never removes their workspaces or diagnostics from the budget.
use crate::{
    heap_meter,
    joint_reference::business_digest,
    scale_reference::{self, Step, Tier},
    scale_sim::image,
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
const SOURCE: &str = env!("MYCITY_SOURCE_SHA256");
const FILE_LIMIT: u64 = 4 * 1024 * 1024;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Reference,
    BeforeReopen,
    Compare,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::BeforeReopen => "before-reopen",
            Self::Compare => "compare",
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema: String,
    source_sha256: String,
    tier: String,
    opening_sha256: String,
    policy_sha256: String,
    oracle_sha256: String,
    mode: Mode,
    root: PathBuf,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultRecord {
    schema: String,
    source_sha256: String,
    request_sha256: String,
    mode: Mode,
    tier: String,
    pub pid: u32,
    pub instance_unix_ns: u128,
    pub artifacts: Vec<Artifact>,
    pub image_sha256: Option<String>,
    pub business_sha256: Option<String>,
    pub root: Option<serde_json::Value>,
    diagnostic_budget_acceptance: String,
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    read_bounded(File::open(path).map_err(err)?, limit).map_err(err)
}
/// Fixed stack readback; limits are enforced before accepting the artifact.
pub fn fingerprint(path: &Path, limit: u64) -> Result<(u64, String), String> {
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
    let tmp = dir.join(format!("{name}.tmp"));
    let mut output = BoundedFile {
        file: File::create_new(&tmp).map_err(err)?,
        bytes: 0,
        limit,
    };
    write(&mut output)?;
    output.flush().map_err(err)?;
    output.file.sync_all().map_err(err)?;
    fs::hard_link(&tmp, dir.join(name)).map_err(err)?;
    fs::remove_file(tmp).map_err(err)?;
    File::open(dir).map_err(err)?.sync_all().map_err(err)
}
fn artifact(dir: &Path, name: &str) -> Result<Artifact, String> {
    let (bytes, sha256) = fingerprint(
        &dir.join(name),
        if name == "cpu-report.json" {
            65536
        } else {
            FILE_LIMIT
        },
    )?;
    Ok(Artifact {
        name: name.into(),
        bytes,
        sha256,
    })
}
fn request(dir: &Path) -> Result<(Request, String, Tier), String> {
    let bytes = read(&dir.join("request.json"), 8192)?;
    let r: Request = serde_json::from_slice(&bytes).map_err(err)?;
    let tier = Tier::parse(&r.tier)?;
    if r.schema != "mycity-model-task-request-v1"
        || r.source_sha256 != SOURCE
        || r.opening_sha256 != sha256(tier.opening())
        || r.policy_sha256 != sha256(tier.policy())
        || r.oracle_sha256 != sha256(tier.oracle())
        || !r.root.is_absolute()
        || r.root.canonicalize().map_err(err)? != r.root
        || dir.parent() != Some(r.root.as_path())
        || dir.file_name().and_then(|s| s.to_str()) != Some(r.mode.name())
    {
        return Err("model_task_request_identity".into());
    }
    Ok((r, sha256(&bytes), tier))
}
/// CLI-internal entry. No result is published until all original proof work passes.
pub fn execute(dir: &Path) -> Result<(), String> {
    let dir = dir.canonicalize().map_err(err)?;
    let (r, request_sha256, tier) = request(&dir)?;
    if dir.join("result.json").exists() {
        return Err("model_task_result_exists".into());
    }
    let mut result = ResultRecord {
        schema: "mycity-model-task-result-v1".into(),
        source_sha256: SOURCE.into(),
        request_sha256,
        mode: r.mode,
        tier: r.tier.clone(),
        pid: std::process::id(),
        instance_unix_ns: heap_meter::process_instance(),
        artifacts: Vec::new(),
        image_sha256: None,
        business_sha256: None,
        root: None,
        diagnostic_budget_acceptance: "not_accepted_separate_owner_closure_required".into(),
    };
    match r.mode {
        Mode::Reference => {
            let mut reference = None;
            publish(&dir, "cpu-targets.jsonl", FILE_LIMIT, |golden_file| {
                let mut golden = BufWriter::with_capacity(65536, golden_file);
                publish(&dir, "cpu-report.json", 65536, |report| {
                    reference = Some(scale_reference::run_tier_streamed(
                        tier,
                        |state, step| {
                            if let Step::Drive(target) = step {
                                let air = state
                                    .engine
                                    .background()
                                    .and_then(|b| b.field())
                                    .ok_or("scale_sim_cpu_field")?;
                                let row = serde_json::json!({"target":target,"business_sha256":business_digest(&state.engine)?,"air_sha256":sha256(&air.checkpoint()?),"windows":state.windows});
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
            for name in [
                "cpu-targets.jsonl",
                "cpu-report.json",
                "cpu-final-engine.json",
            ] {
                result.artifacts.push(artifact(&dir, name)?);
            }
        }
        Mode::BeforeReopen | Mode::Compare => {
            let (bytes, root) = image(&r.root.join("world"))?;
            result.image_sha256 = Some(sha256(&bytes));
            result.root = Some(root);
            if r.mode == Mode::Compare {
                let before = validate(&r.root.join(Mode::BeforeReopen.name()))?;
                if before.image_sha256 != result.image_sha256 {
                    return Err("scale_sim_saved_image_changed".into());
                }
                validate(&r.root.join(Mode::Reference.name()))?;
                let actual = Engine::restore_checkpoint(&bytes)?;
                drop(bytes);
                let expected = Engine::restore_checkpoint(&read(
                    &r.root.join("reference/cpu-final-engine.json"),
                    FILE_LIMIT,
                )?)?;
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
                result.business_sha256 = Some(business_digest(&actual)?);
            }
        }
    }
    publish(&dir, "result.json", 65536, |file| {
        serde_json::to_writer(file, &result).map_err(err)
    })
}
/// Check source, request, exact artifact list, sizes and hashes after a successful exit.
pub fn validate(dir: &Path) -> Result<ResultRecord, String> {
    let (r, digest, _) = request(dir)?;
    let result: ResultRecord =
        serde_json::from_slice(&read(&dir.join("result.json"), 65536)?).map_err(err)?;
    if result.schema != "mycity-model-task-result-v1"
        || result.source_sha256 != SOURCE
        || result.request_sha256 != digest
        || result.mode != r.mode
        || result.tier != r.tier
        || result.pid == 0
        || result.instance_unix_ns == 0
        || result.diagnostic_budget_acceptance != "not_accepted_separate_owner_closure_required"
    {
        return Err("model_task_result_identity".into());
    }
    let names: &[&str] = match r.mode {
        Mode::Reference => &[
            "cpu-targets.jsonl",
            "cpu-report.json",
            "cpu-final-engine.json",
        ],
        _ => &[],
    };
    if result.artifacts.len() != names.len() {
        return Err("model_task_artifact_list".into());
    }
    for (a, name) in result.artifacts.iter().zip(names) {
        if a.name != *name {
            return Err("model_task_artifact_name".into());
        }
        let expected = artifact(dir, name)?;
        if a.bytes != expected.bytes || a.sha256 != expected.sha256 {
            return Err("model_task_artifact_integrity".into());
        }
    }
    match r.mode {
        Mode::Reference
            if result.image_sha256.is_none()
                && result.business_sha256.is_none()
                && result.root.is_none() => {}
        Mode::BeforeReopen | Mode::Compare
            if result.image_sha256.as_ref().is_some_and(|s| s.len() == 64)
                && result.root.is_some()
                && match r.mode {
                    Mode::Compare => result
                        .business_sha256
                        .as_ref()
                        .is_some_and(|s| s.len() == 64),
                    _ => result.business_sha256.is_none(),
                } => {}
        _ => return Err("model_task_result_shape".into()),
    }
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
    let root = root.canonicalize().map_err(err)?;
    let dir = root.join(mode.name());
    fs::create_dir(&dir).map_err(err)?;
    let r = Request {
        schema: "mycity-model-task-request-v1".into(),
        source_sha256: SOURCE.into(),
        tier: tier.name().into(),
        opening_sha256: sha256(tier.opening()),
        policy_sha256: sha256(tier.policy()),
        oracle_sha256: sha256(tier.oracle()),
        mode,
        root,
    };
    publish(&dir, "request.json", 8192, |f| {
        serde_json::to_writer(f, &r).map_err(err)
    })?;
    Ok(dir)
}
pub fn run(root: &Path, tier: Tier, mode: Mode) -> Result<ResultRecord, String> {
    let dir = prepare(root, tier, mode)?;
    let mut job = Job(Command::new(std::env::current_exe().map_err(err)?)
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
    // Publish identity before releasing the child to model work. Keep stdin
    // alive until child exit; abrupt parent death closes it at the kernel.
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
    if !status.success() {
        return Err(format!("model_task_child_failed:{}:{status}", job.0.id()));
    }
    let result = validate(&dir)?;
    if result.pid != job.0.id() {
        return Err("model_task_child_pid".into());
    }
    // Preserve the original public artifact paths; publication never overwrites.
    if mode == Mode::Reference {
        for a in &result.artifacts {
            fs::hard_link(dir.join(&a.name), root.join(&a.name)).map_err(err)?;
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
