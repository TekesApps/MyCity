//! Bounded conservative quantiles plus raw durations for independent exact audit.
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};
const LIMIT_US: usize = 40000;
pub const CLASSES: [&str; 10] = [
    "L0_pe0_model_transaction",
    "L0_pe1_model_transaction",
    "L2_claims_batch",
    "L2_payroll_batch",
    "L2_region_exchange_boot_batch",
    "L2_background_capture_admit_cpu",
    "air_source_page",
    "air_cell_page",
    "L1_native_record_index_page",
    "L1_stub_record_index_page",
];
struct Histogram {
    bins: Vec<u32>,
    count: u64,
    overflow: u64,
    max_ns: u64,
}
impl Histogram {
    fn new() -> Self {
        Self {
            bins: Vec::new(),
            count: 0,
            overflow: 0,
            max_ns: 0,
        }
    }
    fn add(&mut self, ns: u64) -> bool {
        if self.bins.is_empty() {
            self.bins.resize(LIMIT_US + 1, 0);
        }
        let us = ns.div_ceil(1000);
        // Four billion observations in ONE microsecond bucket is outside the
        // frozen run, but still refuse explicitly rather than wrapping/saturating.
        if us <= LIMIT_US as u64 {
            let Some(next) = self.bins[us as usize].checked_add(1) else {
                return false;
            };
            self.bins[us as usize] = next;
        } else {
            self.overflow += 1;
        }
        self.count += 1;
        self.max_ns = self.max_ns.max(ns);
        true
    }
    fn quantile(&self, percent: u64) -> Value {
        if self.count == 0 {
            return Value::Null;
        }
        let rank = (self.count * percent).div_ceil(100);
        let mut n = 0;
        for (us, count) in self.bins.iter().enumerate() {
            n += u64::from(*count);
            if n >= rank {
                return json!({"nearest_rank":rank,"lower_exclusive_ns":(us as u64).saturating_sub(1)*1000,"upper_inclusive_ns":us as u64*1000});
            }
        }
        json!({"nearest_rank":rank,"lower_exclusive_ns":LIMIT_US*1000,"upper_inclusive_ns":self.max_ns})
    }
    fn report(&self) -> Value {
        let p95 = self.quantile(95);
        let p99 = self.quantile(99);
        let pass = self.count >= 200
            && p95["upper_inclusive_ns"]
                .as_u64()
                .is_some_and(|n| n <= 20_000_000)
            && p99["upper_inclusive_ns"]
                .as_u64()
                .is_some_and(|n| n <= 40_000_000);
        json!({"samples":self.count,"overflow_40ms_samples":self.overflow,"maximum_ns":self.max_ns,"p95":p95,"p99":p99,"gate":if self.count<200{"not_passed_insufficient_samples"}else if pass{"passed_conservative_1us_bounds"}else{"failed"}})
    }
}
pub struct Metrics {
    classes: Vec<Histogram>,
    file: BufWriter<File>,
    bytes: u64,
    error: Option<String>,
}
impl Metrics {
    pub fn create(path: &Path) -> Result<Self, String> {
        Ok(Self {
            classes: (0..10).map(|_| Histogram::new()).collect(),
            file: BufWriter::with_capacity(
                65536,
                File::create_new(path).map_err(|e| e.to_string())?,
            ),
            bytes: 0,
            error: None,
        })
    }
    pub fn owned_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.classes.capacity() * std::mem::size_of::<Histogram>()
            + self
                .classes
                .iter()
                .map(|h| h.bins.capacity() * std::mem::size_of::<u32>())
                .sum::<usize>()
            + self.file.capacity()
    }
    pub fn observe(&mut self, class: &str, ns: u64) {
        if self.error.is_some() {
            return;
        }
        let Some(index) = CLASSES.iter().position(|c| *c == class) else {
            self.error = Some("metrics_unknown_class".into());
            return;
        };
        if !self.classes[index].add(ns) {
            self.error = Some("metrics_bucket_count_overflow".into());
            return;
        }
        if self.bytes + 9 > 128 * 1024 * 1024 {
            self.error = Some("metrics_raw_cap".into());
            return;
        }
        let mut raw = [0; 9];
        raw[0] = index as u8;
        raw[1..].copy_from_slice(&ns.to_le_bytes());
        if let Err(e) = self.file.write_all(&raw) {
            self.error = Some(e.to_string());
        } else {
            self.bytes += 9;
        }
    }
    pub fn check(&self) -> Result<(), String> {
        self.error.as_ref().map_or(Ok(()), |e| Err(e.clone()))
    }
    pub fn report(&mut self) -> Result<Value, String> {
        self.check()?;
        self.file.flush().map_err(|e| e.to_string())?;
        self.file.get_ref().sync_all().map_err(|e| e.to_string())?;
        let values: serde_json::Map<_, _> = CLASSES
            .iter()
            .zip(&self.classes)
            .map(|(name, h)| (name.to_string(), h.report()))
            .collect();
        Ok(
            json!({"schema":"mycity-compute-latencies-v1","raw_encoding":"9 bytes: class u8, duration_ns u64 little endian","raw_bytes":self.bytes,"diagnostic_owned_bytes":self.owned_bytes(),"quantiles":"nearest rank with conservative 1us bounds; raw file supports exact audit","classes":values}),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_bucket_refuses_overflow_without_corrupting_quantile_or_count() {
        let mut h = Histogram::new();
        assert!(h.add(1000));
        h.bins[1] = u32::MAX;
        h.count = u64::from(u32::MAX);
        assert!(!h.add(1000));
        assert_eq!(h.count, u64::from(u32::MAX));
        assert_eq!(h.bins[1], u32::MAX);
        assert_eq!(h.quantile(99)["upper_inclusive_ns"], 1000);
        assert!(h.add(40_000_001));
        assert_eq!(h.count, u64::from(u32::MAX) + 1);
    }
    #[test]
    fn inactive_histograms_allocate_no_bins() {
        let mut h = Histogram::new();
        assert_eq!(h.bins.capacity(), 0);
        assert_eq!(h.report()["samples"], 0);
        h.add(1234);
        assert_eq!(h.bins.len(), LIMIT_US + 1);
        assert_eq!(h.quantile(99)["upper_inclusive_ns"], 2000);
    }
    #[test]
    fn quantile_does_not_hide_tail_or_sparse_class() {
        let mut h = Histogram::new();
        for _ in 0..199 {
            h.add(1000);
        }
        assert_eq!(h.report()["gate"], "not_passed_insufficient_samples");
        h.add(1000);
        assert_eq!(h.report()["gate"], "passed_conservative_1us_bounds");
        for _ in 0..3 {
            h.add(40_000_001);
        }
        assert_eq!(h.report()["gate"], "failed");
        assert_eq!(h.quantile(99)["upper_inclusive_ns"], 40_000_001);
    }
}
