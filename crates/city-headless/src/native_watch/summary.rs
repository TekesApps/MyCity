//! Stream the terminal report without retaining a second JSON DOM.
use super::{system, tracker, Stats};
use serde::{ser::SerializeMap, Serialize};
pub(super) struct Report<'a> {
    pub host: &'a system::Host,
    pub success: bool,
    pub reason: Option<&'a str>,
    pub cleanup: Option<&'a str>,
    pub command: &'a [String],
    pub binary_hash: &'a str,
    pub unchanged: bool,
    pub exit: Option<i32>,
    pub duration: f64,
    pub stats: &'a Stats,
    pub tracker: &'a tracker::Tracker,
    pub registrations: u64,
    pub other: Option<u64>,
    pub initial_swap: u64,
    pub guard: &'a system::Guard,
}
impl Serialize for Report<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        macro_rules! field {
            ($k:expr,$v:expr) => {
                m.serialize_entry($k, &($v))?
            };
        }
        field!("schema", "mycity-bounded-native-group-observer-v1");
        field!("source_sha256", env!("MYCITY_SOURCE_SHA256"));
        field!("host", self.host);
        field!(
            "status",
            if self.success {
                "passed_measurement_wrapper"
            } else {
                "failed"
            }
        );
        field!("reason", self.reason);
        field!("cleanup_error", self.cleanup);
        field!("command", self.command);
        field!("binary_sha256", self.binary_hash);
        field!("binary_unchanged", self.unchanged);
        field!("model_exit_code", self.exit);
        field!("actual_wall_seconds", self.duration);
        let q = self.stats;
        field!("samples", q.samples);
        field!("sample_log_bytes", q.log_bytes);
        field!("maximum_sample_gap_seconds", q.max_gap as f64 / 1e9);
        field!("maximum_capture_window_seconds", q.max_capture as f64 / 1e9);
        field!("peak_simultaneous_rss_bytes", q.rss);
        field!("peak_simultaneous_footprint_bytes", q.foot);
        field!("peak_rss_raw_frame_index", q.rss_frame);
        field!("peak_footprint_raw_frame_index", q.foot_frame);
        field!("missing_footprint_process_samples", q.missing);
        field!("missing_rss_process_samples", q.missing);
        field!("peak_world_logical_bytes", q.disk);
        let t = self.tracker;
        field!("identities_recorded", t.identities);
        field!("identities_retired", t.retired);
        field!("maximum_live_tracked_identities", t.max_live);
        field!("unpolled_registered_processes", t.unpolled_registered);
        field!("registration_records", self.registrations);
        field!(
            "final_live_identity_count",
            t.live_other_than(std::process::id()) + 1
        );
        field!("identity_completeness","enumerated descendants plus authenticated headless lifecycle journal and seen orphans; unpolled registrations flagged; unregistered external short helpers not proven");
        field!("discovery_pid_capacity","8192 post-return admission only, not a preallocation bound; fixed 2MiB arena OOM fails observation");
        field!(
            "observer_hot_bound_bytes",
            crate::diagnostic_contract::OBSERVER
        );
        field!("other_diagnostic_owned_bytes", self.other);
        field!(
            "combined_diagnostic_hot_bound_bytes",
            self.other
                .map(|n| n + crate::diagnostic_contract::OBSERVER as u64)
        );
        field!("arena", crate::heap_meter::arena_snapshot());
        field!("swap_initial_bytes", self.initial_swap);
        field!(
            "maximum_observed_swap_net_growth_bytes",
            self.guard.maximum_swap_growth
        );
        field!(
            "maximum_sampled_non_normal_pressure_seconds",
            self.guard.maximum_non_normal_ns as f64 / 1e9
        );
        field!("pressure_semantics","dispatch flags 1 normal/2 warning/4 critical; pressure/swap each frame; periodic named VM counters every60s");
        field!(
            "cpu_units",
            "raw proc_pid_rusage counters; no unverified ns conversion"
        );
        field!("memory_credit_bytes", 0);
        field!("memory_16gb_gate", "not_run");
        field!("eight_hour_gate", "not_run");
        field!("full_M0", "not_accepted");
        m.end()
    }
}
