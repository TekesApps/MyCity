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

const INPUT_LIMIT: usize = mycity_core::canonical::Codec::LIMIT;
const HASH_BUFFER_BYTES: usize = 1024;

struct DirectHash {
    hasher: Sha256,
}

impl DirectHash {
    fn new() -> Self {
        Self {
            hasher: Sha256::new(),
        }
    }
}

impl Write for DirectHash {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.hasher.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

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
struct DirectResult {
    sha256: String,
    elapsed_ns: u64,
}

#[derive(Serialize)]
struct CountedReferenceResult {
    sha256: String,
    writer_calls: u64,
    written_bytes: u64,
}

#[derive(Serialize)]
struct BufferedResult {
    sha256: String,
    elapsed_ns: u64,
}

#[derive(Serialize)]
struct Round {
    round: u8,
    order: &'static str,
    direct: DirectResult,
    buffered: BufferedResult,
}

fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

fn direct_hash(
    codec: &mut mycity_core::canonical::Codec,
    value: &Value,
) -> Result<DirectResult, String> {
    let started = Instant::now();
    let mut writer = DirectHash::new();
    codec.write(value, &mut writer)?;
    let result = DirectResult {
        sha256: format!("{:x}", writer.hasher.finalize()),
        elapsed_ns: elapsed_ns(started),
    };
    Ok(result)
}

fn counted_reference(
    codec: &mut mycity_core::canonical::Codec,
    value: &Value,
) -> Result<CountedReferenceResult, String> {
    let mut writer = CountedHash::new();
    codec.write(value, &mut writer)?;
    Ok(CountedReferenceResult {
        sha256: format!("{:x}", writer.hasher.finalize()),
        writer_calls: writer.write_calls,
        written_bytes: writer.bytes,
    })
}

fn buffered_hash(
    codec: &mut mycity_core::canonical::Codec,
    value: &Value,
) -> Result<BufferedResult, String> {
    let started = Instant::now();
    let sha256 = codec.hash(value)?;
    Ok(BufferedResult {
        sha256,
        elapsed_ns: elapsed_ns(started),
    })
}

fn run(path: &Path) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| format!("input_open:{e}"))?;
    let metadata = file.metadata().map_err(|e| format!("input_metadata:{e}"))?;
    if !metadata.is_file() {
        return Err("input_must_be_regular_file".into());
    }
    let initial_len = metadata.len();
    if initial_len > INPUT_LIMIT as u64 {
        return Err("input_exceeds_codec_limit".into());
    }
    let mut input = Vec::with_capacity(initial_len as usize);
    (&mut file)
        .take(INPUT_LIMIT as u64)
        .read_to_end(&mut input)
        .map_err(|e| format!("input_read:{e}"))?;
    let final_len = file
        .metadata()
        .map_err(|e| format!("input_metadata_after_read:{e}"))?
        .len();
    if input.len() != initial_len as usize || final_len != initial_len {
        return Err("input_changed_during_read".into());
    }
    // CPU-final-engine is written with Engine::write_checkpoint/serde_json::to_writer,
    // which is compact and emits no final newline; its published 4 MiB bound matches Codec.
    if input.last().is_some_and(u8::is_ascii_whitespace) {
        return Err("checkpoint_trailing_whitespace_not_expected".into());
    }
    let input_sha256 = format!("{:x}", Sha256::digest(&input));
    let value: Value = serde_json::from_slice(&input).map_err(|e| format!("input_json:{e}"))?;

    // This oracle intentionally allocates outside model accounting. It is used only to
    // prove that every measured digest matches canonical serde_json bytes independently.
    let canonical = serde_json::to_vec(&value).map_err(|e| format!("oracle_encode:{e}"))?;
    if canonical.len() > INPUT_LIMIT {
        return Err("canonical_oracle_exceeds_codec_limit".into());
    }
    let expected_sha256 = format!("{:x}", Sha256::digest(&canonical));
    let mut codec = mycity_core::canonical::Codec::new().map_err(|e| format!("codec_init:{e}"))?;
    let mut rounds = Vec::with_capacity(3);

    for round in 0..3 {
        let (direct, buffered, order) = if round % 2 == 0 {
            let direct = direct_hash(&mut codec, &value)?;
            let buffered = buffered_hash(&mut codec, &value)?;
            (direct, buffered, "direct_then_buffered")
        } else {
            let buffered = buffered_hash(&mut codec, &value)?;
            let direct = direct_hash(&mut codec, &value)?;
            (direct, buffered, "buffered_then_direct")
        };
        if direct.sha256 != expected_sha256 || buffered.sha256 != expected_sha256 {
            return Err(format!("digest_mismatch_round_{}", round + 1));
        }
        rounds.push(Round {
            round: round + 1,
            order,
            direct,
            buffered,
        });
    }
    let counted_reference = counted_reference(&mut codec, &value)?;
    if counted_reference.sha256 != expected_sha256 {
        return Err("counted_reference_digest_mismatch".into());
    }
    if counted_reference.written_bytes != canonical.len() as u64 {
        return Err("counted_reference_byte_count_mismatch".into());
    }

    let report = serde_json::json!({
        "schema": "mycity-canonical-hash-probe-v1",
        "input_path": path.to_string_lossy(),
        "input_sha256": input_sha256,
        "input_bytes": input.len(),
        "input_format": "compact CPU Engine checkpoint; no trailing newline; maximum Codec::LIMIT",
        "independent_canonical_bytes": canonical.len(),
        "independent_canonical_sha256": expected_sha256,
        "rounds": rounds,
        "counted_reference_pass": {
            "timed": false,
            "scope": "Codec::write with the direct SHA sink plus write/byte counters",
            "result": counted_reference
        },
        "buffered_writer": {
            "fixed_buffer_bytes": HASH_BUFFER_BYTES,
            "location": "temporary_stack",
            "persistent_heap_bytes": 0,
            "write_call_count_observed": false
        },
        "diagnostic_scope": {
            "value_dom_and_oracle_bytes_unmetered": true,
            "model_memory_acceptance": "not_established",
            "new_writer_call_count": "not_observed"
        },
        "limits": [
            "This measures canonical hashing of parsed serde_json::Value, not typed Engine hashing.",
            "Codec and oracle allocations are diagnostic and excluded from model memory accounting.",
            "Timing is a local probe and does not establish full-run performance acceptance."
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
        eprintln!("usage: canonical_hash_probe CPU_FINAL_ENGINE_JSON");
        process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: canonical_hash_probe CPU_FINAL_ENGINE_JSON");
        process::exit(2);
    }
    if let Err(error) = run(Path::new(&path)) {
        eprintln!("canonical_hash_probe: {error}");
        process::exit(2);
    }
}
