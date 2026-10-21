//! Exact canonical file comparison, not a Store restore or a business-hash recomputation.
use super::*;
use crate::worker_slots::{Bytes as Slot, Owner, SLOT_BYTES};
use std::os::unix::fs::MetadataExt;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Proof {
    method: Text<64>,
    reference_mode: Mode,
    engine: Artifact,
    report: Artifact,
}
struct Image {
    file: File,
    point: ImagePoint,
    root: Bytes,
    stamp: RootStamp,
    identity: [u64; 7],
}
fn identity(file: &File) -> Result<[u64; 7], String> {
    let m = file.metadata().map_err(err)?;
    if !m.is_file() {
        return Err("model_compare_regular_file".into());
    }
    Ok([
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime() as u64,
        m.mtime_nsec() as u64,
        m.ctime() as u64,
        m.ctime_nsec() as u64,
    ])
}
fn image(world: &Path) -> Result<Image, String> {
    let c: Current = control_data::parse(&joined(world, "CURRENT.json")?)?;
    if c.slot > 1 {
        return Err("scale_sim_root_slot".into());
    }
    let root_bytes = Bytes::read(&joined(world, &format!("root-{}.json", c.slot))?)?;
    if sha256(root_bytes.as_slice()) != c.root_sha256.as_str() {
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
    if index.current.bytes > FILE_LIMIT {
        return Err("model_compare_image_bound".into());
    }
    let file = File::open(&*joined(
        world,
        &format!("world-{}.json", index.current.generation),
    )?)
    .map_err(err)?;
    let id = identity(&file)?;
    if id[2] != index.current.bytes {
        return Err("scale_sim_world_hash".into());
    }
    let stamp = RootStamp {
        sequence: root.sequence,
        bytes: root_bytes.as_slice().len() as u64,
        sha256: Text::new(&sha256(root_bytes.as_slice()))?,
    };
    Ok(Image {
        file,
        point: index.current,
        root: root_bytes,
        stamp,
        identity: id,
    })
}
fn digest(file: &mut File, slot: &mut Slot) -> Result<(u64, String), String> {
    let mut hash = Sha256::new();
    let mut total = 0u64;
    loop {
        let n = slot.read_from((&mut *file).take(SLOT_BYTES as u64))?;
        total += n as u64;
        if total > FILE_LIMIT {
            return Err("model_compare_image_bound".into());
        }
        hash.update(slot.as_slice());
        if n < SLOT_BYTES {
            break;
        }
    }
    Ok((total, format!("{:x}", hash.finalize())))
}
fn equal(
    mut a: impl Read,
    mut b: impl Read,
    left: &mut Slot,
    right: &mut Slot,
) -> Result<(u64, String), String> {
    let mut hash = Sha256::new();
    let mut total = 0u64;
    loop {
        let n = left.read_from((&mut a).take(SLOT_BYTES as u64))?;
        right.read_from((&mut b).take(SLOT_BYTES as u64))?;
        if left.as_slice() != right.as_slice() {
            return Err("endurance_entire_engine_including_metadata".into());
        }
        total += n as u64;
        if total > FILE_LIMIT {
            return Err("model_compare_image_bound".into());
        }
        hash.update(left.as_slice());
        if n < SLOT_BYTES {
            break;
        }
    }
    Ok((total, format!("{:x}", hash.finalize())))
}
#[derive(Deserialize)]
struct BusinessReport {
    source_sha256: Text<64>,
    tier: Text<8>,
    opening_sha256: Text<64>,
    policy_sha256: Text<64>,
    oracle_sha256: Text<64>,
    business_sha256: Text<64>,
}
fn decode_report(reader: impl Read) -> Result<BusinessReport, String> {
    serde_json::from_reader(mycity_core::fixed_restore::JsonReader::new(reader)).map_err(err)
}
fn business(dir: &Path, tier: Tier) -> Result<Text<64>, String> {
    // Bound read, ignored fields stream through serde IgnoredAny, no Value DOM.
    let f = File::open(&*joined(dir, "cpu-report.json")?).map_err(err)?;
    if f.metadata().map_err(err)?.len() > 65536 {
        return Err("model_compare_report_bound".into());
    }
    let report = decode_report(f.take(65537))?;
    if report.source_sha256.as_str() != SOURCE
        || report.tier.as_str() != tier.name()
        || report.opening_sha256.as_str() != sha256(tier.opening())
        || report.policy_sha256.as_str() != sha256(tier.policy())
        || report.oracle_sha256.as_str() != sha256(tier.oracle())
        || report.business_sha256.as_str().len() != 64
        || !report
            .business_sha256
            .as_str()
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err("model_compare_report_identity".into());
    }
    Ok(report.business_sha256)
}
pub(super) fn execute(
    dir: &Path,
    request: &Request,
    tier: Tier,
    out: &mut ResultRecord,
) -> Result<(), String> {
    heap_meter::fixed_helper();
    let epoch = crate::normal_pool::broker::codec_epoch_from_env()?.unwrap_or(1);
    let mut owner = Owner::new(epoch, 1, 1, 1)?;
    crate::normal_pool::broker::acknowledge_role(epoch, &owner.report()?)?;
    // Explicit fault-injection gate only; never used by measured controllers.
    if std::env::var_os("MYCITY_HELPER_COMPARE_PROBE").is_some() {
        publish(dir, "filled-probe.json", 65536, |f| {
            serde_json::to_writer(f, &owner.report()?).map_err(err)
        })?;
        let began = Instant::now();
        while !joined(dir, "release-probe")?.exists() {
            if began.elapsed() > Duration::from_secs(10) {
                return Err("model_compare_probe_timeout".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    owner.with_pair(|left, right| {
        let decoded = control_data::root_decode(request.root.as_str())?;
        let root = Path::new(decoded.as_str());
        let world = joined(root, "world")?;
        let mut actual = image(&world)?;
        let found = if request.mode == Mode::BeforeReopen {
            digest(&mut actual.file, left)?
        } else {
            let before = validate(&joined(root, Mode::BeforeReopen.name())?)?;
            if before.image_sha256.as_ref() != Some(&actual.point.sha256) {
                return Err("scale_sim_saved_image_changed".into());
            }
            let reference_mode = if request.mode == Mode::CompareEndurance {
                Mode::FaultReference
            } else {
                Mode::Reference
            };
            let reference_dir = joined(root, reference_mode.name())?;
            let reference = validate(&reference_dir)?;
            let engine = reference.artifacts[2].ok_or("model_compare_reference_engine")?;
            let report = reference.artifacts[1].ok_or("model_compare_reference_report")?;
            let mut expected =
                File::open(&*joined(&reference_dir, engine.name.as_str())?).map_err(err)?;
            let expected_id = identity(&expected)?;
            let found = equal(&mut actual.file, &mut expected, left, right)?;
            if found.0 != engine.bytes
                || found.1 != engine.sha256.as_str()
                || identity(&expected)? != expected_id
            {
                return Err("model_compare_reference_changed".into());
            }
            // This is an explicit reference to verified proof work, not a local recomputation.
            out.business_sha256 = Some(business(&reference_dir, tier)?);
            out.comparison_proof = Some(Proof {
                method: Text::new("full-bytes-equal-reference-business-digest")?,
                reference_mode,
                engine,
                report,
            });
            // Recheck every reference artifact after reading the report.
            validate(&reference_dir)?;
            found
        };
        if found.0 != actual.point.bytes
            || found.1 != actual.point.sha256.as_str()
            || identity(&actual.file)? != actual.identity
        {
            return Err("scale_sim_world_hash".into());
        }
        let current = image(&world)?;
        if current.identity != actual.identity
            || current.root.as_slice() != actual.root.as_slice()
            || current.point.sha256 != actual.point.sha256
        {
            return Err("model_compare_saved_identity_changed".into());
        }
        out.image_sha256 = Some(actual.point.sha256);
        out.root = Some(actual.stamp);
        publish(dir, "root.json", 8192, |f| {
            f.write_all(actual.root.as_slice()).map_err(err)
        })?;
        out.artifacts[0] = Some(artifact(dir, "root.json")?);
        Ok(())
    })?;
    owner.check_balance()?;
    publish(dir, "workspace.json", 65536, |f| {
        serde_json::to_writer(f, &owner.report()?).map_err(err)
    })?;
    out.artifacts[1] = Some(artifact(dir, "workspace.json")?);
    Ok(())
}
pub(super) fn validate_proof(
    _dir: &Path,
    request: &Request,
    tier: Tier,
    result: &ResultRecord,
) -> Result<(), String> {
    match request.mode {
        Mode::CompareSpeed | Mode::CompareEndurance => {
            let p = result
                .comparison_proof
                .as_ref()
                .ok_or("model_compare_proof_missing")?;
            let mode = if request.mode == Mode::CompareEndurance {
                Mode::FaultReference
            } else {
                Mode::Reference
            };
            if p.method.as_str() != "full-bytes-equal-reference-business-digest"
                || p.reference_mode != mode
            {
                return Err("model_compare_proof_identity".into());
            }
            let root = control_data::root_decode(request.root.as_str())?;
            let reference_dir = joined(Path::new(root.as_str()), mode.name())?;
            let reference = validate(&reference_dir)?;
            for (proof, slot) in [(&p.engine, 2), (&p.report, 1)] {
                let actual = reference.artifacts[slot]
                    .as_ref()
                    .ok_or("model_compare_proof_asset")?;
                if proof.name != actual.name
                    || proof.bytes != actual.bytes
                    || proof.sha256 != actual.sha256
                {
                    return Err("model_compare_proof_asset".into());
                }
            }
            if result.image_sha256 != Some(p.engine.sha256)
                || result.business_sha256 != Some(business(&reference_dir, tier)?)
            {
                return Err("model_compare_proof_hash".into());
            }
        }
        _ if result.comparison_proof.is_some() => {
            return Err("model_compare_unexpected_proof".into())
        }
        _ => (),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Error};
    #[test]
    fn report_rejects_long_keys_values_deep_unknown_fields_and_trailing_bytes() {
        for bad in [
            format!("{{\"{}\":0}}", "x".repeat(60000)),
            format!("{{\"source_sha256\":\"{}\"}}", "x".repeat(60000)),
            format!("{{\"ignored\":{}{}}}", "[".repeat(130), "]".repeat(130)),
        ] {
            assert!(decode_report(bad.as_bytes()).is_err());
        }
        let h = "a".repeat(64);
        let text=format!("{{\"source_sha256\":\"{h}\",\"tier\":\"full\",\"opening_sha256\":\"{h}\",\"policy_sha256\":\"{h}\",\"oracle_sha256\":\"{h}\",\"business_sha256\":\"{h}\"}}");
        assert!(decode_report(text.as_bytes()).is_ok());
        assert!(decode_report(format!("{text} {{}}").as_bytes()).is_err());
    }
    #[test]
    fn exact_fixed_comparison_covers_chunks_lengths_and_io_failure() {
        for n in [
            0,
            1,
            SLOT_BYTES - 1,
            SLOT_BYTES,
            SLOT_BYTES + 1,
            FILE_LIMIT as usize,
        ] {
            let data: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
            let mut owner = Owner::new(1, 1, 1, 1).unwrap();
            let found = owner
                .with_pair(|a, b| equal(Cursor::new(&data), Cursor::new(&data), a, b))
                .unwrap();
            assert_eq!(found, (n as u64, sha256(&data)));
            owner.report().unwrap();
            if n > 0 {
                for offset in [0, n / 2, n - 1] {
                    let mut other = data.clone();
                    other[offset] ^= 1;
                    let mut owner = Owner::new(1, 1, 1, 1).unwrap();
                    assert!(owner
                        .with_pair(|a, b| equal(Cursor::new(&data), Cursor::new(&other), a, b))
                        .is_err());
                    owner.report().unwrap();
                }
            }
            let mut more = data.clone();
            more.push(0);
            let mut owner = Owner::new(1, 1, 1, 1).unwrap();
            assert!(owner
                .with_pair(|a, b| equal(Cursor::new(&data), Cursor::new(&more), a, b))
                .is_err());
            owner.report().unwrap();
        }
        struct Failed;
        impl Read for Failed {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(Error::other("read_fault"))
            }
        }
        let mut owner = Owner::new(1, 1, 1, 1).unwrap();
        assert!(owner
            .with_pair(|a, b| equal(Failed, Cursor::new([]), a, b))
            .unwrap_err()
            .contains("read_fault"));
        owner.report().unwrap();
        let mut owner = Owner::new(1, 1, 1, 1).unwrap();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.with_pair::<()>(|_, _| panic!("unwind_fault"))
        }));
        owner.check_balance().unwrap();
    }
}
