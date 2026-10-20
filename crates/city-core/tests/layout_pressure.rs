use mycity_core::layout_pressure::NativePressure;
#[test]
fn native_jprod_storage_counts_real_widths_and_all_scanned_records() {
    let records = NativePressure::jprod().unwrap();
    let manifest = records.manifest();
    assert_eq!(manifest.account_arena.live_records, 40000);
    assert_eq!(manifest.inventory_arena.live_records, 110000);
    assert_eq!(manifest.vectors[2].1, 180000);
    assert_eq!(manifest.vectors[3].1, 31200);
    assert!(manifest.owned_capacity_bytes > 20 * 1024 * 1024);
    assert!(manifest.owned_capacity_bytes < 64 * 1024 * 1024);
    let mut scanned = 0;
    let a = records.scan(|n, _| scanned += n).unwrap();
    assert_eq!(scanned, 40000 + 110000 + 180000 + 31200 + 8 + 64);
    assert_eq!(a, records.scan(|_, _| {}).unwrap());
    let mut writer = CountingWriter(0);
    records.write_checkpoint(&mut writer).unwrap();
    assert!(writer.0 > 10 * 1024 * 1024);
    println!(
        "T13A_NATIVE {} checkpoint_bytes={}",
        serde_json::to_string(&manifest).unwrap(),
        writer.0
    );
}
#[test]
fn native_full_snapshot_roundtrip_and_truncation_use_real_files() {
    use std::{
        fs::File,
        io::{BufReader, BufWriter, Write},
        time::{Instant, SystemTime, UNIX_EPOCH},
    };
    let dir = std::env::temp_dir().join(format!(
        "mycity-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("native.json");
    let original = NativePressure::jprod().unwrap();
    let started = Instant::now();
    let mut writer = BufWriter::new(File::create(&path).unwrap());
    original.write_checkpoint(&mut writer).unwrap();
    writer.flush().unwrap();
    writer.get_ref().sync_all().unwrap();
    drop(writer);
    let restored =
        NativePressure::restore_checkpoint(BufReader::new(File::open(&path).unwrap())).unwrap();
    assert_eq!(
        original.scan(|_, _| {}).unwrap(),
        restored.scan(|_, _| {}).unwrap()
    );
    assert_eq!(
        original.manifest().owned_capacity_bytes,
        restored.manifest().owned_capacity_bytes
    );
    let mut a = HashWriter::default();
    let mut b = HashWriter::default();
    original.write_checkpoint(&mut a).unwrap();
    restored.write_checkpoint(&mut b).unwrap();
    use sha2::Digest;
    assert_eq!(a.0.finalize(), b.0.finalize());
    use std::io::Read;
    assert!(NativePressure::restore_checkpoint(File::open(&path).unwrap().take(1000)).is_err());
    println!(
        "T13A_NATIVE_REAL_IO seconds={} bytes={}",
        started.elapsed().as_secs_f64(),
        std::fs::metadata(&path).unwrap().len()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

struct CountingWriter(u64);
impl std::io::Write for CountingWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0 += b.len() as u64;
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct HashWriter(sha2::Sha256);
impl std::io::Write for HashWriter {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        use sha2::Digest;
        self.0.update(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
