use mycity_core::{canonical::Codec, transaction::Engine};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{env, fs::File, io::Read, path::Path, process, time::Instant};

const INPUT_LIMIT: usize = Codec::LIMIT;
const ROUNDS: usize = 7;

#[derive(Serialize)]
struct Round {
    index: usize,
    elapsed_ns: u64,
    sha256: String,
}

fn read_input(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|error| format!("input_open:{error}"))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("input_metadata:{error}"))?;
    if !metadata.is_file() {
        return Err("input_must_be_regular_file".into());
    }
    let expected_len = metadata.len();
    if expected_len > INPUT_LIMIT as u64 {
        return Err("input_exceeds_codec_limit".into());
    }
    let mut input = Vec::with_capacity(expected_len as usize);
    (&mut file)
        .take(INPUT_LIMIT as u64 + 1)
        .read_to_end(&mut input)
        .map_err(|error| format!("input_read:{error}"))?;
    let final_len = file
        .metadata()
        .map_err(|error| format!("input_metadata_after_read:{error}"))?
        .len();
    if input.len() != expected_len as usize || final_len != expected_len {
        return Err("input_changed_during_read".into());
    }
    Ok(input)
}

fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn run(path: &Path) -> Result<(), String> {
    let input = read_input(path)?;
    let input_sha256 = format!("{:x}", Sha256::digest(&input));
    let engine = Engine::restore_checkpoint_reader(
        File::open(path).map_err(|error| format!("restore_open:{error}"))?,
    )
    .map_err(|error| format!("engine_restore:{error}"))?;
    let mut codec = Codec::new().map_err(|error| format!("codec_init:{error}"))?;

    // One untimed warmup exercises the exact production Engine + Codec::hash path.
    let warmup_sha256 = codec.hash(&engine)?;
    let mut rounds = Vec::with_capacity(ROUNDS);
    for index in 0..ROUNDS {
        let started = Instant::now();
        let sha256 = codec.hash(&engine)?;
        let duration_ns = elapsed_ns(started);
        if sha256 != warmup_sha256 {
            return Err(format!("hash_changed_round_{}", index + 1));
        }
        rounds.push(Round {
            index: index + 1,
            elapsed_ns: duration_ns,
            sha256,
        });
    }

    let report = serde_json::json!({
        "schema": "mycity-canonical-integer-probe-v1",
        "input_path": path.to_string_lossy(),
        "input_sha256": input_sha256,
        "input_bytes": input.len(),
        "input_limit_bytes": INPUT_LIMIT,
        "engine_restore": "Engine::restore_checkpoint_reader(File)",
        "hash_api": "unchanged Codec::hash(&Engine)",
        "codec_owned_capacity_bytes": codec.owned_capacity_bytes(),
        "warmup": {
            "measured": false,
            "sha256": warmup_sha256,
        },
        "rounds": rounds,
        "limits": [
            "The benchmark restores one real Engine checkpoint and times only whole Codec::hash calls.",
            "No per-number clocks, synthetic model, or typed diagnostic profile API is used.",
            "This measures repeated hash calls for one restored Engine, not a full simulation workload."
        ]
    });
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: canonical_integer_probe CPU_FINAL_ENGINE_JSON");
        process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: canonical_integer_probe CPU_FINAL_ENGINE_JSON");
        process::exit(2);
    }
    if let Err(error) = run(Path::new(&path)) {
        eprintln!("canonical_integer_probe: {error}");
        process::exit(2);
    }
}
