//! Bounded external diagnostics, not business state. Log failure never rolls back money.
use crate::scale_metrics::Metrics;
use mycity_core::{
    diagnostics::{AirPage, Class, Role, Summary, Trace},
    transaction::{Caller, Envelope},
};
use mycity_storage::{Outcome, Store};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
// Never reuse or remove an earlier instance, including a partially initialized
// directory left by a crashed process. PID remains the second path component for
// old tools, but new pairing uses the explicit process-instance identity.
fn instance_dir(
    base: &Path,
    kind: &str,
    pid: u32,
    counter: &AtomicU64,
) -> Result<(PathBuf, u64), String> {
    for _ in 0..4096 {
        let instance = counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| "cpu_instance_counter_exhausted")?;
        let root = base.join(format!("{kind}-{pid}-{instance}"));
        match fs::create_dir(&root) {
            Ok(()) => return Ok((root, instance)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(err(e)),
        }
    }
    Err("cpu_instance_collision_limit".into())
}
pub struct CpuLog {
    external_diagnostics_owned_bytes: usize,
    instance: u64,
    root: PathBuf,
    budget: PathBuf,
    raw_bytes: [u64; 3],
    reserved_raw: u64,
    reserved_log: u64,
    producer: &'static str,
    metrics: [Metrics; 3],
    log: BufWriter<File>,
    bytes: usize,
    page_counts: [u64; 3],
    empty_pages: [u64; 3],
    page_limits: [[[u32; 2]; 2]; 3],
    page_items: [[u64; 2]; 3],
    last_error: Option<String>,
    l0: Option<&'static str>,
}
impl CpuLog {
    pub fn create(base: &Path, kind: &'static str) -> Result<Self, String> {
        fs::create_dir_all(base).map_err(err)?;
        static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(0);
        let (root, instance) = instance_dir(base, kind, std::process::id(), &NEXT_INSTANCE)?;
        let mut ms = Vec::new();
        for role in ["execution", "quote", "replay"] {
            let d = root.join(role);
            fs::create_dir(&d).map_err(err)?;
            ms.push(Metrics::create(&d.join("compute-durations.bin"))?);
        }
        let mut log = Self {
            external_diagnostics_owned_bytes: 0,
            instance,
            producer: kind,
            log: BufWriter::with_capacity(
                65536,
                File::create_new(root.join("transactions.jsonl")).map_err(err)?,
            ),
            budget: base.join("run-budget.bin"),
            raw_bytes: [0; 3],
            reserved_raw: 0,
            reserved_log: 0,
            root,
            metrics: ms.try_into().map_err(|_| "cpu_metrics_roles")?,
            bytes: 0,
            page_counts: [0; 3],
            empty_pages: [0; 3],
            page_limits: [[[0; 2]; 2]; 3],
            page_items: [[0; 2]; 3],
            last_error: None,
            l0: None,
        };
        // First record survives a killed producer without adding one file per
        // process. Publication must complete before any diagnosed computation.
        let identity = json!({"phase":"producer_identity","identity":{
            "schema":"mycity-cpu-producer-identity-v1",
            "producer":kind,"process_pid":std::process::id(),
            "process_instance_unix_ns":crate::heap_meter::process_instance(),
            "producer_instance":instance,"source_sha256":env!("MYCITY_SOURCE_SHA256")
        }});
        if serde_json::to_vec(&identity).map_err(err)?.len() + 1 > 1024 {
            return Err("cpu_identity_cap".into());
        }
        log.write(identity)?;
        log.log.flush().map_err(err)?;
        log.log.get_ref().sync_all().map_err(err)?;
        File::open(&log.root)
            .map_err(err)?
            .sync_all()
            .map_err(err)?;
        Ok(log)
    }
    pub fn set_external_diagnostics_owned_bytes(&mut self, bytes: usize) -> Result<(), String> {
        self.external_diagnostics_owned_bytes = bytes;
        self.check()
    }
    pub fn worker_controlled_slots(
        &mut self,
        report: &crate::worker_slots::Report<'_>,
    ) -> Result<(), String> {
        self.write(json!({"phase":"worker_controlled_slots","owner":report}))
    }
    pub fn worker_air_workspace(
        &mut self,
        loaded: &mycity_core::air::WorkspaceReport,
        reset: &mycity_core::air::WorkspaceReport,
    ) -> Result<(), String> {
        self.write(json!({"phase":"worker_air_workspace","loaded":loaded,"reset":reset}))
    }
    pub fn sim_controlled_slots(
        &mut self,
        report: &crate::worker_slots::Report<'_>,
    ) -> Result<(), String> {
        self.write(json!({"phase":"sim_controlled_slots","owner":report}))
    }
    pub fn set_l0_class(&mut self, class: &'static str) -> Result<(), String> {
        if !matches!(
            class,
            "L0_pe0_model_transaction" | "L0_pe1_model_transaction"
        ) {
            return Err("cpu_l0_class".into());
        }
        self.l0 = Some(class);
        Ok(())
    }
    pub fn page(&mut self, page: AirPage) {
        let index = page.role.index();
        if page.ok && page.items > 0 {
            let class = match page.class {
                Class::AirSources => "air_source_page",
                Class::AirCells => "air_cell_page",
                _ => {
                    self.last_error = Some("cpu_air_class".into());
                    return;
                }
            };
            let kind = usize::from(page.class == Class::AirCells);
            let limit = if kind == 0 {
                page.source_limit
            } else {
                page.cell_limit
            };
            let bounds = &mut self.page_limits[index][kind];
            bounds[0] = if bounds[0] == 0 {
                limit
            } else {
                bounds[0].min(limit)
            };
            bounds[1] = bounds[1].max(limit);
            self.page_items[index][kind] += u64::from(page.items);
            if let Err(e) = self.reserve(0, 9) {
                self.last_error = Some(e);
                return;
            }
            self.raw_bytes[index] += 9;
            self.metrics[index].observe(class, page.nanos);
            self.page_counts[index] += 1;
        } else {
            self.empty_pages[index] += 1;
        }
    }
    pub fn record(
        &mut self,
        operation: u64,
        kind: &str,
        summary: &Summary,
        durable: bool,
    ) -> Result<(), String> {
        // A successful model batch is ONE sample per transaction, not one per payment.
        for role in [Role::Execution, Role::Quote, Role::Replay] {
            let i = role.index();
            let c = &summary.roles[i];
            if role == Role::Execution
                && (!durable || !summary.applied || !summary.hook_succeeded || summary.replayed)
            {
                continue;
            }
            if role == Role::Replay {
                continue;
            }
            if role == Role::Execution {
                if let Some(class) = self.l0 {
                    self.reserve(0, 9)?;
                    self.raw_bytes[i] += 9;
                    self.metrics[i].observe(
                        class,
                        c[Class::Prepare as usize]
                            .nanos
                            .saturating_add(c[Class::Apply as usize].nanos),
                    );
                }
            }
            if self.l0.is_some() {
                continue;
            }
            for (class, name) in [
                (Class::Claims, "L2_claims_batch"),
                (Class::Payroll, "L2_payroll_batch"),
                (Class::Region, "L2_region_exchange_boot_batch"),
            ] {
                let row = c[class as usize];
                if row.work > 0 {
                    self.reserve(0, 9)?;
                    self.raw_bytes[i] += 9;
                    self.metrics[i].observe(name, row.nanos);
                }
            }
            let capture = c[Class::Capture as usize];
            let admit = c[Class::Admit as usize];
            if capture.work + admit.work > 0 {
                self.reserve(0, 9)?;
                self.raw_bytes[i] += 9;
                self.metrics[i].observe(
                    "L2_background_capture_admit_cpu",
                    capture.nanos.saturating_add(admit.nanos),
                );
            }
        }
        self.write(
            json!({"operation":operation,"kind":kind,"durable_receipt":durable,"commit_status":if kind=="candidate_import_quote"{"not_applicable"}else if durable{"confirmed"}else{"unconfirmed"},"summary":summary,"raw_offsets_end":self.raw_bytes}),
        )?;
        self.check()
    }
    pub fn begin(&mut self, identity: Value) -> Result<(), String> {
        self.write(json!({"phase":"begin", "identity":identity,"raw_offsets":self.raw_bytes}))
    }
    pub fn worker_complete(&mut self, identity: Value) -> Result<(), String> {
        self.write(
            json!({"phase":"worker_computation_complete","identity":identity,
            "raw_offsets":self.raw_bytes,"sim_admission":"not_implied"}),
        )?;
        self.finish()?;
        Ok(())
    }
    // Shared across producers. Reserve bounded chunks before writes; unused reservations
    // are intentionally retained after process death, so crashes cannot free quota twice.
    fn reserve(&mut self, kind: usize, extra: u64) -> Result<(), String> {
        let used = if kind == 0 {
            self.raw_bytes.iter().sum::<u64>()
        } else {
            self.bytes as u64
        };
        let reserved = if kind == 0 {
            self.reserved_raw
        } else {
            self.reserved_log
        };
        if used + extra <= reserved {
            return Ok(());
        }
        let more = (used + extra - reserved).div_ceil(65536) * 65536;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&self.budget)
            .map_err(err)?;
        fs2::FileExt::lock_exclusive(&file).map_err(err)?;
        let mut b = [0u8; 16];
        if file.metadata().map_err(err)?.len() != 0 {
            file.read_exact(&mut b).map_err(err)?;
        }
        let offset = kind * 8;
        let total = u64::from_le_bytes(b[offset..offset + 8].try_into().map_err(err)?) + more;
        let cap = if kind == 0 {
            128 * 1024 * 1024
        } else {
            256 * 1024 * 1024
        };
        if total > cap {
            return Err("cpu_run_shared_disk_quota".into());
        }
        b[offset..offset + 8].copy_from_slice(&total.to_le_bytes());
        file.seek(SeekFrom::Start(0)).map_err(err)?;
        file.write_all(&b).map_err(err)?;
        file.sync_all().map_err(err)?;
        if kind == 0 {
            self.reserved_raw += more;
        } else {
            self.reserved_log += more;
        }
        Ok(())
    }
    fn write(&mut self, mut value: Value) -> Result<(), String> {
        value.as_object_mut().ok_or("cpu_log_object")?.insert(
            "process_clock".into(),
            serde_json::to_value(crate::heap_meter::clock_reading()).map_err(err)?,
        );
        let b = serde_json::to_vec(&value).map_err(err)?;
        if b.len() > 32768 || self.bytes + b.len() + 1 > 128 * 1024 * 1024 {
            return Err("cpu_transaction_log_cap".into());
        }
        self.reserve(1, b.len() as u64 + 1)?;
        self.log.write_all(&b).map_err(err)?;
        self.log.write_all(b"\n").map_err(err)?;
        self.bytes += b.len() + 1;
        Ok(())
    }
    pub fn check(&self) -> Result<(), String> {
        if let Some(e) = &self.last_error {
            return Err(e.clone());
        }
        let cap = if self.producer == "worker" {
            crate::diagnostic_contract::WORKER
        } else {
            crate::diagnostic_contract::SIM
        };
        if self.owned_bytes().saturating_add(256 * 1024) > cap {
            return Err("cpu_producer_hot_quota".into());
        }
        for m in &self.metrics {
            m.check()?;
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<Value, String> {
        self.check()?;
        self.log.flush().map_err(err)?;
        self.log.get_ref().sync_all().map_err(err)?;
        let mut reports = Vec::new();
        for m in &mut self.metrics {
            reports.push(m.report()?);
        }
        let report = json!({"schema":"mycity-cpu-regions-v1","producer":self.producer,"process_pid":std::process::id(),"process_instance_unix_ns":crate::heap_meter::process_instance(),"producer_instance":self.instance,"producer_directory":self.root.file_name().and_then(|s|s.to_str()),"source_sha256":env!("MYCITY_SOURCE_SHA256"),"roles":["execution","quote","replay"],"classes":mycity_core::diagnostics::CLASSES,"payroll_subtypes":["accrual","billing"],"region_subtypes":["clock","expiry","obligation","boot_finish","freight_arrival","population_arrival","economy_phase","direct_command"],
            "role_compute":reports,"nonempty_air_page_counts":self.page_counts,"empty_or_failed_pages":self.empty_pages,
            "air_page_limit_bounds_role_source_cell_min_max":self.page_limits,"air_processed_items_role_source_cell":self.page_items,
            "transaction_log_bytes":self.bytes,"owned_diagnostics_bytes":self.owned_bytes(),"external_diagnostics_owned_bytes":self.external_diagnostics_owned_bytes,"transient_reservation_bytes":256*1024,"charged_diagnostics_bound_bytes":self.owned_bytes()+256*1024,
            "whole_process_rust_heap":crate::heap_meter::snapshot(),"clock":"elapsed compute-region Instant; excludes page sink callback; includes preemption, not thread CPU clock",
            "overlap":"prepare includes child regions; region P0 includes capture; admit includes admission air pages; do not sum overlapping counters",
            "air_samples":"pure computation attempts; raw ranges bind to transaction outcome or worker ticket; not committed/admitted sample gates",
            "replay":"not instrumented by cold Store open; empty replay strata are unmeasured, not zero cost",
            "shared_disk_reservations_bytes":{"raw":self.reserved_raw,"log":self.reserved_log},
            "quote_samples":"separate diagnostic strata; never satisfy execution sample minimum","formal_gate":"not_accepted_here"});
        let mut f = File::create(self.root.join("report.json")).map_err(err)?;
        serde_json::to_writer_pretty(&mut f, &report).map_err(err)?;
        f.sync_all().map_err(err)?;
        Ok(report)
    }
    pub fn owned_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.external_diagnostics_owned_bytes
            + self.root.capacity()
            + self.budget.capacity()
            + self.log.capacity()
            + self.metrics.iter().map(Metrics::owned_bytes).sum::<usize>()
    }
}
pub fn execute(
    store: &mut Store,
    caller: Caller,
    envelope: Envelope,
    diag: &mut Option<CpuLog>,
) -> Result<Outcome, String> {
    let Some(log) = diag.as_mut() else {
        return store.execute(caller, envelope).map_err(err);
    };
    let operation = envelope.operation_id;
    log.begin(json!({"operation":operation,"kind":"transaction"}))?;
    let mut sink = |p| log.page(p);
    let mut trace = Trace::with_pages(&mut sink);
    let started = std::time::Instant::now();
    let mut points = [None; 14];
    let mut transport_proof_points = [None; 2];
    let mut observer = |p| {
        record_write_point(
            p,
            started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
            &mut points,
            &mut transport_proof_points,
        );
        Ok(())
    };
    let result = store.execute_observed_diagnosed(caller, envelope, &mut observer, &mut trace);
    let summary = trace.summary.clone();
    let durable = result.as_ref().is_ok_and(|o| o.has_persistent_receipt);
    log.write(json!({"phase":"persistence_boundaries","operation":operation,
        "clock":"elapsed ns from Store call; fixed observer callback overhead included",
        "point_names":["prepared","wal_encoded_hashed","wal_admitted","wal_header_half","wal_payload_half","wal_checksum_half","wal_written","wal_synced","root_encoded","root_published_synced","current_encoded_hashed","current_renamed","current_directory_synced","applied"],
        "transport_proof_point_names":["transport_proof_encoded","transport_proof_published"],
        "transport_proof_points_ns":transport_proof_points,
        "points_ns":points,"durable_receipt":durable,"commit_status":if durable{"confirmed"}else{"unconfirmed"},"store_result":if result.is_ok(){"returned_outcome"}else{"error_requires_receipt_recovery"}}))?;
    log.record(operation, "transaction", &summary, durable)
        .map_err(|e| {
            format!("measurement_failed_after_transaction:{e};durable_receipt={durable}")
        })?;
    result.map_err(err)
}

// Keep the original fourteen diagnostic slots stable for historical consumers.
// Proof publication belongs to separate slots and is absent on ordinary writes.
fn record_write_point(
    point: mycity_storage::WritePoint,
    nanos: u64,
    points: &mut [Option<u64>; 14],
    transport: &mut [Option<u64>; 2],
) {
    use mycity_storage::WritePoint::*;
    let slot = match point {
        TransportProofEncoded => &mut transport[0],
        TransportProofPublished => &mut transport[1],
        Prepared => &mut points[0],
        WalEncoded => &mut points[1],
        WalAdmitted => &mut points[2],
        WalHeaderHalf => &mut points[3],
        WalPayloadHalf => &mut points[4],
        WalChecksumHalf => &mut points[5],
        WalWritten => &mut points[6],
        WalSynced => &mut points[7],
        RootEncoded => &mut points[8],
        RootPublished => &mut points[9],
        CurrentEncoded => &mut points[10],
        CurrentRenamed => &mut points[11],
        CurrentSynced => &mut points[12],
        Applied => &mut points[13],
    };
    *slot = Some(nanos);
}

pub fn import(
    store: &mut Store,
    job: u64,
    generation: u32,
    bytes: &[u8],
    observer: impl FnMut(mycity_storage::BackgroundPoint) -> std::io::Result<()>,
    diag: &mut Option<CpuLog>,
) -> Result<[u8; 32], mycity_storage::Error> {
    let Some(log) = diag.as_mut() else {
        return store.import_background_candidate_with_observer(job, generation, bytes, observer);
    };
    log.begin(json!({"job":job,"generation":generation,"kind":"candidate_import_quote"}))
        .map_err(|e| mycity_storage::Error::Io(std::io::Error::other(e)))?;
    let mut sink = |p| log.page(p);
    let mut trace = Trace::with_pages(&mut sink);
    let result = store.import_candidate_diagnosed(job, generation, bytes, observer, &mut trace);
    let summary = trace.summary.clone();
    log.record(
        mycity_core::background::accept_operation_id(job).map_err(mycity_storage::Error::Model)?,
        "candidate_import_quote",
        &summary,
        false,
    )
    .map_err(|e| {
        mycity_storage::Error::Io(std::io::Error::other(format!(
            "measurement_failed_after_import:{e}"
        )))
    })?;
    result
}
pub fn enabled() -> bool {
    std::env::var_os("MYCITY_DIAGNOSTICS").is_some_and(|v| v == "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transport_proof_boundaries_do_not_shift_or_overwrite_legacy_slots() {
        use mycity_storage::WritePoint::*;
        let mut points = [None; 14];
        let mut transport = [None; 2];
        for (index, point) in [
            Prepared,
            WalEncoded,
            WalAdmitted,
            WalHeaderHalf,
            WalPayloadHalf,
            WalChecksumHalf,
            WalWritten,
            WalSynced,
            RootEncoded,
            RootPublished,
            CurrentEncoded,
            CurrentRenamed,
            CurrentSynced,
            Applied,
        ]
        .into_iter()
        .enumerate()
        {
            record_write_point(point, index as u64, &mut points, &mut transport);
        }
        let legacy = points;
        assert!(transport.iter().all(Option::is_none));
        record_write_point(TransportProofEncoded, 100, &mut points, &mut transport);
        record_write_point(TransportProofPublished, 101, &mut points, &mut transport);
        assert_eq!(points, legacy);
        assert_eq!(points, std::array::from_fn(|index| Some(index as u64)));
        assert_eq!(transport, [Some(100), Some(101)]);
    }
    #[test]
    fn same_pid_logs_keep_original_files_and_shared_quota_across_instances() {
        let base =
            std::env::temp_dir().join(format!("mycity-cpu-instances-{}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let mut first = CpuLog::create(&base, "sim").unwrap();
        first.begin(json!({"operation": 17})).unwrap();
        first.page(AirPage {
            role: Role::Execution,
            class: Class::AirSources,
            nanos: 123,
            items: 1,
            source_limit: 256,
            cell_limit: 256,
            ok: true,
        });
        let a = first.finish().unwrap();
        let original = fs::read(first.root.join("report.json")).unwrap();
        let quota = fs::read(base.join("run-budget.bin")).unwrap();
        assert_eq!(u64::from_le_bytes(quota[..8].try_into().unwrap()), 65536);
        assert_eq!(u64::from_le_bytes(quota[8..].try_into().unwrap()), 65536);
        let mut second = CpuLog::create(&base, "sim").unwrap();
        assert_ne!(first.root, second.root);
        let new_quota = fs::read(base.join("run-budget.bin")).unwrap();
        assert_eq!(
            u64::from_le_bytes(new_quota[..8].try_into().unwrap()),
            65536
        );
        assert_eq!(
            u64::from_le_bytes(new_quota[8..].try_into().unwrap()),
            131072
        );
        // Identity is durable before any samples or transaction work.
        let raw = fs::read(second.root.join("transactions.jsonl")).unwrap();
        assert_eq!(raw.len(), second.bytes);
        assert!(raw.len() <= 1024 && raw.ends_with(b"\n"));
        let row: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(row["phase"], "producer_identity");
        let identity = &row["identity"];
        assert!(!second.root.join("identity.json").exists());
        assert_eq!(identity["producer_instance"], second.instance);
        assert_eq!(
            identity["process_instance_unix_ns"].to_string(),
            crate::heap_meter::process_instance().to_string()
        );
        assert!(!second.root.join("report.json").exists());
        second.begin(json!({"operation": 18})).unwrap();
        let b = second.finish().unwrap();
        assert_eq!(a["process_pid"], b["process_pid"]);
        assert_eq!(a["process_instance_unix_ns"], b["process_instance_unix_ns"]);
        assert_eq!(
            a["process_instance_unix_ns"].to_string(),
            crate::heap_meter::process_instance().to_string()
        );
        assert_ne!(a["producer_instance"], b["producer_instance"]);
        assert_eq!(fs::read(first.root.join("report.json")).unwrap(), original);
        let quota = fs::read(base.join("run-budget.bin")).unwrap();
        assert_eq!(u64::from_le_bytes(quota[..8].try_into().unwrap()), 65536);
        assert_eq!(u64::from_le_bytes(quota[8..].try_into().unwrap()), 131072);
        drop(first);
        drop(second);
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn reused_pid_and_partial_initialization_skip_existing_directories_atomically() {
        let base =
            std::env::temp_dir().join(format!("mycity-cpu-collision-{}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let partial = base.join("worker-42-0");
        fs::create_dir(&partial).unwrap();
        fs::write(partial.join("sentinel"), b"original partial initialization").unwrap();
        let (a, b) = std::thread::scope(|scope| {
            let first =
                scope.spawn(|| instance_dir(&base, "worker", 42, &AtomicU64::new(0)).unwrap());
            let second =
                scope.spawn(|| instance_dir(&base, "worker", 42, &AtomicU64::new(0)).unwrap());
            (first.join().unwrap(), second.join().unwrap())
        });
        assert_ne!(a.0, b.0);
        assert!(a.1 > 0 && b.1 > 0);
        assert_eq!(
            fs::read(partial.join("sentinel")).unwrap(),
            b"original partial initialization"
        );
        // A third simulated restart with a reset counter must keep all old evidence.
        let (c, n) = instance_dir(&base, "worker", 42, &AtomicU64::new(0)).unwrap();
        assert!(n > 0 && c != a.0 && c != b.0);
        let (real, _) = instance_dir(&base, "worker", 42, &AtomicU64::new(4)).unwrap();
        assert!(real.is_dir());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn instance_creation_does_not_retry_io_errors_or_wrap_counter() {
        let base = std::env::temp_dir().join(format!("mycity-cpu-notdir-{}", std::process::id()));
        fs::write(&base, b"not a directory").unwrap();
        let counter = AtomicU64::new(0);
        assert!(instance_dir(&base, "sim", 42, &counter).is_err());
        assert_eq!(counter.load(Ordering::Relaxed), 1);
        assert_eq!(
            instance_dir(&base, "sim", 42, &AtomicU64::new(u64::MAX)).unwrap_err(),
            "cpu_instance_counter_exhausted"
        );
        fs::remove_file(base).unwrap();
    }
    #[test]
    fn failed_transaction_batches_never_fill_success_histogram_and_raw_pages_remain_attempts() {
        let root = std::env::temp_dir().join(format!("mycity-cpu-log-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let mut log = CpuLog::create(&root, "test").unwrap();
        log.begin(json!({"operation":1})).unwrap();
        let mut summary = Summary::default();
        summary.roles[0][Class::Claims as usize].work = 20000;
        summary.roles[0][Class::Claims as usize].nanos = 100;
        log.record(1, "failed_persistence", &summary, false)
            .unwrap();
        summary.hook_succeeded = true;
        summary.applied = true;
        log.record(2, "committed", &summary, true).unwrap();
        log.page(AirPage {
            role: Role::Execution,
            class: Class::AirSources,
            nanos: 100,
            items: 1,
            source_limit: 256,
            cell_limit: 256,
            ok: true,
        });
        let report = log.finish().unwrap();
        assert_eq!(
            report["role_compute"][0]["classes"]["L2_claims_batch"]["samples"],
            1
        );
        assert_eq!(
            report["role_compute"][0]["classes"]["L2_claims_batch"]["gate"],
            "not_passed_insufficient_samples"
        );
        assert_eq!(
            report["air_page_limit_bounds_role_source_cell_min_max"][0][0],
            json!([256, 256])
        );
        assert!(report["air_samples"]
            .as_str()
            .unwrap()
            .contains("not committed"));
        assert!(report["owned_diagnostics_bytes"].as_u64().unwrap() < 3 * 1024 * 1024);
        drop(log);
        fs::remove_dir_all(root).unwrap();
    }
}
