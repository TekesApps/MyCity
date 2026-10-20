//! Disk-backed pure task experiments, separate from the city Store.
use mycity_core::{
    air::{Input, Source, State, CHECKPOINT_BYTES, INPUT_BYTES},
    read_bounded,
};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
    process::Command,
    time::Instant,
};
const MAGIC: &[u8] = b"MYCITY-AIR-TASK-1\n";
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn read_input(p: &Path) -> Result<Input, String> {
    Input::parse(&read_bounded(File::open(p).map_err(error)?, INPUT_BYTES as u64).map_err(error)?)
}
fn save(path: &Path, s: &State) -> Result<(), String> {
    let payload = s.checkpoint()?;
    let pending = path.with_extension("pending");
    let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&pending)
        .map_err(error)?;
    f.write_all(MAGIC).map_err(error)?;
    f.write_all(env!("MYCITY_SOURCE_SHA256").as_bytes())
        .map_err(error)?;
    f.write_all(s.input().identity().as_bytes())
        .map_err(error)?;
    f.write_all(&payload).map_err(error)?;
    f.sync_all().map_err(error)?;
    drop(f);
    // Publish a complete immutable artifact without replacing an existing target.
    let link = fs::hard_link(&pending, path);
    fs::remove_file(&pending).map_err(error)?;
    link.map_err(error)?;
    File::open(path.parent().ok_or("artifact_parent")?)
        .map_err(error)?
        .sync_all()
        .map_err(error)?;
    Ok(())
}
fn load(path: &Path, input: &Input) -> Result<State, String> {
    let b = read_bounded(
        File::open(path).map_err(error)?,
        CHECKPOINT_BYTES as u64 + MAGIC.len() as u64 + 128,
    )
    .map_err(error)?;
    let off = MAGIC.len();
    if b.len() < off + 128
        || &b[..off] != MAGIC
        || &b[off..off + 64] != env!("MYCITY_SOURCE_SHA256").as_bytes()
        || &b[off + 64..off + 128] != input.identity().as_bytes()
    {
        return Err("air_artifact_source_or_input".into());
    }
    State::restore_for(&b[off + 128..], input)
}
fn finish(s: &mut State, source: u32, cells: u32) -> Result<u64, String> {
    let mut pages = 0;
    while !s.complete() {
        s.page(source, cells)?;
        pages += 1;
    }
    Ok(pages)
}
pub fn resume(input: &Path, checkpoint: &Path, output: &Path) -> Result<Value, String> {
    let i = read_input(input)?;
    let mut state = load(checkpoint, &i)?;
    let pages = finish(&mut state, 611, 127)?;
    save(output, &state)?;
    Ok(
        json!({"schema":"mycity-air-resume-v1","report":state.report(),"pages":pages,"source_sha256":env!("MYCITY_SOURCE_SHA256"),"world_effects":"none"}),
    )
}
// Independent unpaged f64 stencil over a padded field and a sorted event set.
fn compare(input: &Input, actual: &State) -> Result<Value, String> {
    let side = usize::from(input.side);
    let pitch = side + 2;
    let mut field = vec![input.initial_micro as f64 / 1e6; pitch * pitch];
    for x in 1..=side {
        field[x] = input.boundary_micro[0].ok_or("boundary")? as f64 / 1e6;
        field[(side + 1) * pitch + x] = input.boundary_micro[1].ok_or("boundary")? as f64 / 1e6;
    }
    for y in 1..=side {
        field[y * pitch] = input.boundary_micro[3].ok_or("boundary")? as f64 / 1e6;
        field[y * pitch + side + 1] = input.boundary_micro[2].ok_or("boundary")? as f64 / 1e6;
    }
    let mut anchors = std::collections::BTreeSet::from([0, input.target]);
    for t in (60..input.target).step_by(60) {
        anchors.insert(t);
    }
    for s in &input.sources {
        anchors.insert(s.start);
        anchors.insert(s.end);
    }
    let mut at = 0;
    for end in anchors.into_iter().filter(|t| *t > 0) {
        let h = f64::from(end - at) / 60.0;
        let r = 0.02 * h;
        let d = 0.01 * h;
        let mut next = field.clone();
        for y in 1..=side {
            for x in 1..=side {
                let p = y * pitch + x;
                next[p] = (1.0 - d)
                    * ((1.0 - 4.0 * r) * field[p]
                        + r * (field[p - pitch] + field[p + pitch] + field[p + 1] + field[p - 1]))
                    + 2.0 * d;
            }
        }
        for s in &input.sources {
            let secs = end.min(s.end).saturating_sub(at.max(s.start));
            let c = s.cell as usize;
            next[(c / side + 1) * pitch + c % side + 1] +=
                f64::from(s.rate_micro) * f64::from(secs) / 1e6;
        }
        field = next;
        at = end;
    }
    let mut worst_abs = 0.0f64;
    let mut worst_scaled = 0.0f64;
    for (i, b) in actual.field_bits().iter().enumerate() {
        let expected = field[(i / side + 1) * pitch + i % side + 1];
        let e = (f64::from(f32::from_bits(*b)) - expected).abs();
        worst_abs = worst_abs.max(e);
        worst_scaled = worst_scaled.max(e / (0.001 + 0.0001 * expected.abs()));
    }
    if worst_scaled > 1.0 {
        return Err("air_f64_engineering_gate".into());
    }
    Ok(
        json!({"max_absolute_error":worst_abs,"max_normalized_error":worst_scaled,"criterion":"abs_error <= 0.001 + 0.0001 * abs(reference)","passed":true,"physical_accuracy":"not_verified","gpu_error_gate":"not_established"}),
    )
}
fn case(root: &Path, name: &str, input: Input) -> Result<Value, String> {
    let started = Instant::now();
    let dir = root.join(name);
    fs::create_dir(&dir).map_err(error)?;
    let input_path = dir.join("input.json");
    fs::write(
        &input_path,
        serde_json::to_vec_pretty(&input).map_err(error)?,
    )
    .map_err(error)?;
    let mut direct = State::new(input.clone())?;
    let direct_pages = finish(&mut direct, 8192, 1024)?;
    let mut partial = State::new(input.clone())?;
    partial.page(1, 17)?;
    let source_file = dir.join("source.air");
    save(&source_file, &partial)?;
    drop(partial);
    let mut partial = load(&source_file, &input)?;
    while partial
        .progress()
        .is_some_and(|p| p.phase == mycity_core::air::Phase::Sources)
    {
        partial.page(8192, 17)?;
    }
    partial.page(8192, 17)?;
    let tile_file = dir.join("tile.air");
    save(&tile_file, &partial)?;
    drop(partial);
    let final_file = dir.join("complete.air");
    let child = Command::new(std::env::current_exe().map_err(error)?)
        .arg("air-resume")
        .arg(&input_path)
        .arg(&tile_file)
        .arg(&final_file)
        .output()
        .map_err(error)?;
    if !child.status.success() {
        return Err(format!(
            "air_child: {}",
            String::from_utf8_lossy(&child.stderr)
        ));
    }
    let child_json: Value = serde_json::from_slice(&child.stdout).map_err(error)?;
    let restored = load(&final_file, &input)?;
    if direct != restored {
        return Err("air_resume_not_bit_exact".into());
    }
    let emitted = input
        .sources
        .iter()
        .map(|s| u64::from(s.end - s.start) * u64::from(s.rate_micro))
        .sum::<u64>();
    if emitted != direct.report().emitted_micro {
        return Err("air_source_integral".into());
    }
    let comparison = compare(&input, &direct)?;
    Ok(
        json!({"name":name,"input_sha256":input.identity(),"source_count":input.sources.len(),"source_struct_bytes":std::mem::size_of::<Source>(),"direct_pages":direct_pages,"same_state_after_source_and_tile_file_restore":true,"new_process_resume":child_json,"report":direct.report(),"comparison":comparison,"wall_seconds":started.elapsed().as_secs_f64(),"checkpoint_file_bytes":fs::metadata(&final_file).map_err(error)?.len()}),
    )
}
pub fn run(input_path: &Path, root: &Path) -> Result<Value, String> {
    let input = read_input(input_path)?;
    fs::create_dir(root).map_err(error)?;
    let started = Instant::now();
    let mut cases = vec![case(root, "configured", input.clone())?];
    for side in [64, 128].into_iter().filter(|side| *side >= input.side) {
        let mut i = input.clone();
        i.side = side;
        cases.push(case(root, &format!("side-{side}"), i)?);
    }
    let mut month = input.clone();
    month.target = 30 * 86400;
    month.boundary_until = month.target;
    if let Some(s) = month.sources.last_mut() {
        s.end = month.target;
    }
    cases.push(case(root, "fixed-30-days", month)?);
    let mut dense = input;
    dense.side = 128;
    dense.target = 120;
    dense.boundary_until = 120;
    dense.sources = (1..=20000)
        .map(|id| Source {
            id,
            cell: id % 16384,
            start: if id % 2 == 0 { 37 } else { 0 },
            end: 83,
            rate_micro: id % 37 + 1,
        })
        .collect();
    cases.push(case(root, "dense-20000", dense)?);
    Ok(
        json!({"schema":"mycity-air-cpu-evidence-v1","source_sha256":env!("MYCITY_SOURCE_SHA256"),"lock_sha256":env!("MYCITY_LOCK_SHA256"),"wall_seconds":started.elapsed().as_secs_f64(),"cases":cases,"world_store_integration":"not_exercised_in_this_probe","physical_pollution_accuracy":"not_verified","worker_pool":"not_implemented","gpu":"not_run","memory_16gb_gate":"not_run","process_memory_scope":"parent /usr/bin/time; child peak and combined footprint not measured"}),
    )
}
