use mycity_core::{
    canonical::{CanonicalHashProfile, Codec},
    transaction::Engine,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process,
    time::Instant,
};

const INPUT_LIMIT: usize = Codec::LIMIT;

struct CountedHash {
    hasher: Sha256,
    write_calls: u64,
    bytes: u64,
}

impl CountedHash {
    fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            write_calls: 0,
            bytes: 0,
        }
    }
}

impl Write for CountedHash {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_calls = self
            .write_calls
            .checked_add(1)
            .ok_or_else(|| io::Error::other("counted_hash_write_call_overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("counted_hash_byte_overflow"))?;
        self.hasher.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Serialize)]
struct Round {
    round: u8,
    order: &'static str,
    typed_hash_profile: CanonicalHashProfile,
    full_null_pass_ns: u64,
    profile_total_ns: u64,
    hash_minus_null_estimate_ns_signed: String,
}

#[derive(Serialize)]
struct CountedVerification {
    sha256: String,
    writer_calls: u64,
    canonical_bytes: u64,
    production_codec_hash_sha256: String,
}

fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn read_checkpoint(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = File::open(path).map_err(|e| format!("input_open:{e}"))?;
    let metadata = file.metadata().map_err(|e| format!("input_metadata:{e}"))?;
    if !metadata.is_file() {
        return Err("input_must_be_regular_file".into());
    }
    let expected_len = metadata.len();
    if expected_len > INPUT_LIMIT as u64 {
        return Err("input_exceeds_checkpoint_limit".into());
    }
    let mut bytes = Vec::with_capacity(expected_len as usize);
    (&mut file)
        .take(INPUT_LIMIT as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("input_read:{e}"))?;
    let final_len = file
        .metadata()
        .map_err(|e| format!("input_metadata_after_read:{e}"))?
        .len();
    if bytes.len() != expected_len as usize || final_len != expected_len {
        return Err("input_changed_during_read".into());
    }
    // Engine::write_checkpoint uses compact serde_json::to_writer and emits no newline.
    if bytes.last().is_some_and(u8::is_ascii_whitespace) {
        return Err("checkpoint_trailing_whitespace_not_expected".into());
    }
    Ok(bytes)
}

fn null_pass(codec: &mut Codec, engine: &Engine) -> Result<u64, String> {
    let started = Instant::now();
    codec.write(engine, io::sink())?;
    Ok(elapsed_ns(started))
}

fn counted_verification(codec: &mut Codec, engine: &Engine) -> Result<CountedVerification, String> {
    let mut writer = CountedHash::new();
    codec.write(engine, &mut writer)?;
    let sha256 = format!("{:x}", writer.hasher.finalize());
    let production_codec_hash_sha256 = codec.hash(engine)?;
    Ok(CountedVerification {
        sha256,
        writer_calls: writer.write_calls,
        canonical_bytes: writer.bytes,
        production_codec_hash_sha256,
    })
}

fn profile_total(profile: &CanonicalHashProfile) -> u64 {
    profile
        .typed_serialize_ns
        .saturating_add(profile.json_validation_ns)
        .saturating_add(profile.canonical_output_with_sha_ns)
        .saturating_add(profile.finalization_ns)
}

fn run(path: &Path) -> Result<(), String> {
    let input = read_checkpoint(path)?;
    let input_sha256 = format!("{:x}", Sha256::digest(&input));
    let oracle_value: Value =
        serde_json::from_slice(&input).map_err(|e| format!("oracle_parse:{e}"))?;
    let oracle_canonical =
        serde_json::to_vec(&oracle_value).map_err(|e| format!("oracle_encode:{e}"))?;
    if oracle_canonical.len() > INPUT_LIMIT {
        return Err("oracle_canonical_exceeds_codec_limit".into());
    }
    let oracle_sha256 = format!("{:x}", Sha256::digest(&oracle_canonical));

    // Restore through the real bounded Engine reader. This checkpoint format writes an
    // empty journal by design; it does not represent a hot Store's complete history.
    let engine = Engine::restore_checkpoint_reader(
        File::open(path).map_err(|e| format!("restore_open:{e}"))?,
    )
    .map_err(|e| format!("engine_restore:{e}"))?;
    let mut codec = Codec::new().map_err(|e| format!("codec_init:{e}"))?;
    let mut rounds = Vec::with_capacity(3);

    for round in 0..3 {
        let (profile, null_ns, order) = if round % 2 == 0 {
            let profile = codec.diagnostic_hash_profile(&engine)?;
            let null_ns = null_pass(&mut codec, &engine)?;
            (profile, null_ns, "typed_hash_then_full_null")
        } else {
            let null_ns = null_pass(&mut codec, &engine)?;
            let profile = codec.diagnostic_hash_profile(&engine)?;
            (profile, null_ns, "full_null_then_typed_hash")
        };
        if profile.sha256 != oracle_sha256 {
            return Err(format!("typed_digest_oracle_mismatch_round_{}", round + 1));
        }
        if profile.canonical_bytes != oracle_canonical.len() {
            return Err(format!("typed_byte_oracle_mismatch_round_{}", round + 1));
        }
        let total_ns = profile_total(&profile);
        let delta = i128::from(total_ns) - i128::from(null_ns);
        rounds.push(Round {
            round: round + 1,
            order,
            typed_hash_profile: profile,
            full_null_pass_ns: null_ns,
            profile_total_ns: total_ns,
            hash_minus_null_estimate_ns_signed: delta.to_string(),
        });
    }

    // Independent, untimed reference pass: direct canonical SHA plus counts, followed by
    // the untouched production Codec::hash path. Neither is included in the timed rounds.
    let verification = counted_verification(&mut codec, &engine)?;
    if verification.sha256 != oracle_sha256
        || verification.production_codec_hash_sha256 != oracle_sha256
        || verification.canonical_bytes != oracle_canonical.len() as u64
    {
        return Err("untimed_counted_hash_verification_mismatch".into());
    }

    let report = serde_json::json!({
        "schema": "mycity-canonical-typed-profile-v1",
        "input_path": path.to_string_lossy(),
        "input_sha256": input_sha256,
        "input_bytes": input.len(),
        "input_limit_bytes": INPUT_LIMIT,
        "engine_restore": "Engine::restore_checkpoint_reader(File)",
        "checkpoint_journal_scope": "checkpoint writer serializes journal as empty; not full hot Store history",
        "serde_value_oracle": {
            "canonical_bytes": oracle_canonical.len(),
            "sha256": oracle_sha256,
            "memory_scope": "diagnostic_unmetered"
        },
        "rounds": rounds,
        "untimed_counted_verification": verification,
        "difference_interpretation": "profile total minus full null pass is only a rough estimate; it is not exact SHA cost",
        "memory_and_scope": {
            "diagnostic_value_dom_and_canonical_vec_unmetered": true,
            "restored_engine_and_codec_are_diagnostic_process_allocations": true,
            "codec_backing_bytes": codec.owned_capacity_bytes(),
            "additional_persistent_codec_fields": 0,
            "per_write_timing_or_global_counters": false,
            "physical_stack_or_heap_acceptance": "not_established",
            "full_m0_acceptance": "not_established"
        },
        "limits": [
            "Typed profiling reuses the actual Engine and Codec but runs only this checkpoint, not the full M0 workload.",
            "The JSON Value DOM and independent canonical bytes are unmetered diagnostic allocations.",
            "The Engine checkpoint journal is empty by format and does not claim complete hot Store history.",
            "The io::sink pass performs full typed serialization, validation, and canonical traversal without SHA output."
        ]
    });
    println!(
        "{}",
        serde_json::to_string(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn main() {
    let mut args = env::args_os().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: canonical_typed_probe CPU_FINAL_ENGINE_JSON");
        process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: canonical_typed_probe CPU_FINAL_ENGINE_JSON");
        process::exit(2);
    }
    if let Err(error) = run(Path::new(&path)) {
        eprintln!("canonical_typed_probe: {error}");
        process::exit(2);
    }
}
