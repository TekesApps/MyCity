use mycity_core::{canonical::Codec, transaction::Engine};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    process,
};

const INPUT_LIMIT: usize = Codec::LIMIT;

#[derive(Default, Serialize)]
struct NodeCounts {
    objects: u64,
    arrays: u64,
    strings: u64,
    numbers: u64,
    booleans: u64,
    nulls: u64,
    total: u64,
}

#[derive(Default, Serialize)]
struct Shape {
    nodes: NodeCounts,
    object_key_count_distribution: BTreeMap<String, u64>,
    object_count: u64,
    object_keys_total: u64,
    object_key_utf8_bytes: u64,
    string_value_count: u64,
    string_value_utf8_bytes: u64,
    array_count: u64,
    array_elements_total: u64,
    array_scalar_elements: u64,
    array_non_scalar_elements: u64,
    array_scalar_elements_after_first: u64,
    primitive_non_string_elements_after_first: u64,
    max_depth: u64,
}

impl Shape {
    fn visit(&mut self, value: &Value, depth: u64) -> Result<(), String> {
        self.nodes.total = checked_add(self.nodes.total, 1, "node_count_overflow")?;
        self.max_depth = self.max_depth.max(depth);
        match value {
            Value::Object(object) => {
                self.nodes.objects = checked_add(self.nodes.objects, 1, "object_count_overflow")?;
                self.object_count = checked_add(self.object_count, 1, "object_count_overflow")?;
                let key_count = u64::try_from(object.len())
                    .map_err(|_| "object_key_count_overflow".to_owned())?;
                let histogram_key = key_count.to_string();
                let bucket = self
                    .object_key_count_distribution
                    .entry(histogram_key)
                    .or_default();
                *bucket = checked_add(*bucket, 1, "object_histogram_overflow")?;

                for (key, child) in object {
                    self.object_keys_total =
                        checked_add(self.object_keys_total, 1, "object_keys_overflow")?;
                    self.object_key_utf8_bytes = checked_add(
                        self.object_key_utf8_bytes,
                        key.len() as u64,
                        "object_key_bytes_overflow",
                    )?;
                    self.visit(child, depth.saturating_add(1))?;
                }
            }
            Value::Array(array) => {
                self.nodes.arrays = checked_add(self.nodes.arrays, 1, "array_count_overflow")?;
                self.array_count = checked_add(self.array_count, 1, "array_count_overflow")?;
                self.array_elements_total = checked_add(
                    self.array_elements_total,
                    u64::try_from(array.len()).map_err(|_| "array_length_overflow".to_owned())?,
                    "array_elements_overflow",
                )?;

                for (index, child) in array.iter().enumerate() {
                    let scalar = !child.is_array() && !child.is_object();
                    if scalar {
                        self.array_scalar_elements =
                            checked_add(self.array_scalar_elements, 1, "array_scalar_overflow")?;
                        if index > 0 {
                            self.array_scalar_elements_after_first = checked_add(
                                self.array_scalar_elements_after_first,
                                1,
                                "array_scalar_separator_overflow",
                            )?;
                            if !child.is_string() {
                                self.primitive_non_string_elements_after_first = checked_add(
                                    self.primitive_non_string_elements_after_first,
                                    1,
                                    "array_primitive_separator_overflow",
                                )?;
                            }
                        }
                    } else {
                        self.array_non_scalar_elements = checked_add(
                            self.array_non_scalar_elements,
                            1,
                            "array_non_scalar_overflow",
                        )?;
                    }
                    self.visit(child, depth.saturating_add(1))?;
                }
            }
            Value::String(text) => {
                self.nodes.strings = checked_add(self.nodes.strings, 1, "string_count_overflow")?;
                self.string_value_count =
                    checked_add(self.string_value_count, 1, "string_value_count_overflow")?;
                self.string_value_utf8_bytes = checked_add(
                    self.string_value_utf8_bytes,
                    text.len() as u64,
                    "string_value_bytes_overflow",
                )?;
            }
            Value::Number(_) => {
                self.nodes.numbers = checked_add(self.nodes.numbers, 1, "number_count_overflow")?;
            }
            Value::Bool(_) => {
                self.nodes.booleans =
                    checked_add(self.nodes.booleans, 1, "boolean_count_overflow")?;
            }
            Value::Null => {
                self.nodes.nulls = checked_add(self.nodes.nulls, 1, "null_count_overflow")?;
            }
        }
        Ok(())
    }
}

fn checked_add(value: u64, increment: u64, error: &str) -> Result<u64, String> {
    value.checked_add(increment).ok_or_else(|| error.to_owned())
}

#[derive(Default)]
struct CountedHash {
    hasher: Sha256,
    write_calls: u64,
    bytes: u64,
}

impl Write for CountedHash {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_calls = self
            .write_calls
            .checked_add(1)
            .ok_or_else(|| io::Error::other("writer_call_count_overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("writer_byte_count_overflow"))?;
        self.hasher.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
        return Err("input_exceeds_4mib_limit".into());
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

fn run(path: &Path) -> Result<(), String> {
    let input = read_input(path)?;
    let input_sha256 = format!("{:x}", Sha256::digest(&input));
    let value: Value =
        serde_json::from_slice(&input).map_err(|error| format!("json_parse:{error}"))?;
    let mut shape = Shape::default();
    shape.visit(&value, 1)?;

    // Diagnostic-only independent canonical representation. The Value DOM and this buffer
    // are explicitly unmetered and bounded by the 4 MiB input contract.
    let oracle = serde_json::to_vec(&value).map_err(|error| format!("oracle_encode:{error}"))?;
    if oracle.len() > INPUT_LIMIT {
        return Err("oracle_canonical_exceeds_4mib_limit".into());
    }
    let oracle_sha256 = format!("{:x}", Sha256::digest(&oracle));

    // Count calls from the unchanged production Codec path on the real restored Engine.
    // These counts are exact for this probe run, but do not attribute individual calls to
    // nodes or delimiters; the Value-based separator figure below is only a structural bound.
    let engine = Engine::restore_checkpoint_reader(
        File::open(path).map_err(|error| format!("restore_open:{error}"))?,
    )
    .map_err(|error| format!("engine_restore:{error}"))?;
    let mut codec = Codec::new().map_err(|error| format!("codec_init:{error}"))?;
    let mut writer = CountedHash::default();
    codec
        .write(&engine, &mut writer)
        .map_err(|error| format!("codec_write:{error}"))?;
    let codec_sha256 = format!("{:x}", writer.hasher.finalize());
    if codec_sha256 != oracle_sha256 || writer.bytes != oracle.len() as u64 {
        return Err("codec_value_oracle_mismatch".into());
    }

    let total_nodes = shape.nodes.total;
    let all_scalar_commas = shape.array_scalar_elements_after_first;
    let candidate_commas = shape.primitive_non_string_elements_after_first;
    let observed_write_calls = writer.write_calls;
    let report = serde_json::json!({
        "schema": "mycity-canonical-shape-probe-v1",
        "input_path": path.to_string_lossy(),
        "input_sha256": input_sha256,
        "input_bytes": input.len(),
        "input_limit_bytes": INPUT_LIMIT,
        "diagnostic_value": {
            "memory_scope": "unmetered_dom_and_oracle_buffer",
            "canonical_bytes": oracle.len(),
            "canonical_sha256": oracle_sha256,
            "shape": shape,
        },
        "unchanged_codec_observation": {
            "canonical_bytes": writer.bytes,
            "sha256": codec_sha256,
            "write_calls": writer.write_calls,
            "measurement_scope": "single diagnostic Codec::write on restored Engine; exact total calls for this run only"
        },
        "array_separator_estimate": {
            "all_scalar_array_elements_after_first_upper_bound_calls": all_scalar_commas,
            "primitive_non_string_array_elements_after_first_candidate_bound_calls": candidate_commas,
            "candidate_calls_as_fraction_of_observed_calls_upper_bound": if observed_write_calls == 0 {
                0.0
            } else {
                candidate_commas as f64 / observed_write_calls as f64
            },
            "candidate_calls_as_fraction_of_total_nodes_upper_bound": if total_nodes == 0 {
                0.0
            } else {
                candidate_commas as f64 / total_nodes as f64
            },
            "interpretation": "Structural upper bound only: eligible comma writes cannot save more calls than this count; no per-node writer attribution is measured."
        },
        "limits": [
            "The serde_json::Value DOM and oracle Vec are diagnostic unmetered allocations.",
            "The probe reads one checkpoint and performs one Codec::write; it does not advance simulation time.",
            "Writer call totals are exact for this Codec::write, but the shape estimate does not identify which call belongs to a specific node.",
            "This is not a performance acceptance result or a production Codec change."
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
        eprintln!("usage: canonical_shape_probe CPU_FINAL_ENGINE_JSON");
        process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("usage: canonical_shape_probe CPU_FINAL_ENGINE_JSON");
        process::exit(2);
    }
    if let Err(error) = run(Path::new(&path)) {
        eprintln!("canonical_shape_probe: {error}");
        process::exit(2);
    }
}
