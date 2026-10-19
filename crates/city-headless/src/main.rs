use mycity_core::{parse_bootstrap, read_bounded, sha256, INPUT_LIMIT, NUMERIC_STRATEGY, SCHEMA};
use serde_json::json;
use std::{
    env,
    fs::File,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn system(command: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(command).args(args).output().ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 || !["baseline", "layout"].contains(&args[1].as_str()) {
        return Err("usage: mycity-headless <baseline|layout> <bootstrap.json>".into());
    }
    let file = File::open(&args[2]).map_err(|e| e.to_string())?;
    let bytes = read_bounded(file, INPUT_LIMIT).map_err(|e| e.to_string())?;
    let input = parse_bootstrap(&bytes)?;
    let rules = json!({"schema": SCHEMA, "numeric_strategy": NUMERIC_STRATEGY, "input_limit_bytes": INPUT_LIMIT, "mode": "headless_laboratory"});
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let mut report = json!({
        "report_schema": "mycity-m0-evidence-v1",
        "task": "M0-T01", "run_id": format!("{}-{now}", std::process::id()),
        "result": "bootstrap_validated_only",
        "source_sha256": env!("MYCITY_SOURCE_SHA256"), "lock_sha256": env!("MYCITY_LOCK_SHA256"),
        "input_sha256": sha256(&bytes), "rules_sha256": sha256(&serde_json::to_vec(&rules).map_err(|e| e.to_string())?),
        "rules": rules, "input": input,
        "build": {"rustc": env!("MYCITY_RUSTC"), "target": env!("MYCITY_TARGET"), "profile": env!("MYCITY_PROFILE")},
        "host": {"os": env::consts::OS, "arch": env::consts::ARCH,
            "os_version": system("/usr/bin/sw_vers", &["-productVersion"]),
            "memory_bytes": system("/usr/sbin/sysctl", &["-n", "hw.memsize"]),
            "chip": system("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"])},
        "ui": "not_implemented_by_scope", "gpu": "not_implemented",
        "economic_prototypes": "not_run", "memory_16gb_gate": "not_run"
    });
    if args[1] == "layout" {
        use mycity_core::arena::{AccountDomain, Arena, Scope};
        let mut table = Arena::<AccountDomain, u64>::new(
            Scope::new(input.world, input.branch).map_err(|e| format!("{e:?}"))?,
            32768,
            2 * 1024 * 1024,
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut checksum = 0u64;
        for n in 0..32768u64 {
            let h = table.insert(n).map_err(|e| format!("{e:?}"))?;
            checksum += *table.get(h).map_err(|e| format!("{e:?}"))?;
        }
        report["task"] = json!("M0-T02");
        report["result"] = json!("fixed_record_layout_executed");
        report["layout"] = json!({"type": "u64_probe_not_full_financial_record", "table": table.layout(), "readback_sum": checksum, "allocator_metadata_in_table_bytes": false});
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
