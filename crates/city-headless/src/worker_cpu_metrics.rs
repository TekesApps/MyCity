//! Bounded, best-effort process CPU accounting for one air-worker Request.
//!
//! Included by the explicit air-v2 worker wire. It never grants resources,
//! authorizes Store acceptance, or marks completion.
use serde::{Deserialize, Serialize};

pub const METRICS_VERSION: u16 = 1;
pub const MAX_PROCESS_INSTANCE_DECIMAL_BYTES: usize = 39;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricSource {
    ChildProcessAccounting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    MissingProcessInstance,
    UnsupportedPlatform,
    ClockReadFailed,
    InvalidTimespec,
    NanosecondOverflow,
    ClockWentBackwards,
    NoStartSample,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownCpuTime {
    /// Process CPU clock at the start of this Request, including all its threads.
    pub start_ns: u64,
    /// Cumulative process CPU clock at this event boundary.
    pub end_ns: u64,
    /// Checked cumulative difference from the Request start, never a sum of samples.
    pub delta_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum CpuTimeValue {
    Known { cumulative: KnownCpuTime },
    Unknown {
        reason: UnknownReason,
        /// Last successful cumulative reading is a lower bound only, not a final value.
        last_known_lower_bound: Option<KnownCpuTime>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuTelemetry {
    pub metrics_version: u16,
    /// Canonical positive decimal string; avoids serde_json::Value u128 truncation.
    pub process_instance: Option<String>,
    pub source: MetricSource,
    pub process_cpu_time: CpuTimeValue,
}

impl CpuTelemetry {
    pub fn validate(&self) -> Result<(), MetricError> {
        if self.metrics_version != METRICS_VERSION
            || self
                .process_instance
                .as_deref()
                .is_some_and(|value| !valid_instance_decimal(value))
        {
            return Err(MetricError::InvalidIdentityOrVersion);
        }
        match (&self.process_instance, self.process_cpu_time) {
            (
                None,
                CpuTimeValue::Unknown {
                    reason: UnknownReason::MissingProcessInstance,
                    last_known_lower_bound: None,
                },
            ) => {}
            (None, _) => return Err(MetricError::InvalidIdentityOrVersion),
            (
                Some(_),
                CpuTimeValue::Unknown {
                    reason: UnknownReason::MissingProcessInstance,
                    ..
                },
            ) => return Err(MetricError::InvalidIdentityOrVersion),
            _ => {}
        }
        match self.process_cpu_time {
            CpuTimeValue::Known { cumulative } => validate_cumulative(cumulative),
            CpuTimeValue::Unknown { last_known_lower_bound, .. } => {
                if let Some(cumulative) = last_known_lower_bound {
                    validate_cumulative(cumulative)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricError {
    InvalidIdentityOrVersion,
    InvalidCumulativeValue,
}

fn validate_cumulative(value: KnownCpuTime) -> Result<(), MetricError> {
    if value.end_ns < value.start_ns
        || value.end_ns.checked_sub(value.start_ns) != Some(value.delta_ns)
    {
        return Err(MetricError::InvalidCumulativeValue);
    }
    Ok(())
}

fn valid_instance_decimal(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROCESS_INSTANCE_DECIMAL_BYTES
        && !value.starts_with('0')
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u128>().map_or(false, |parsed| parsed > 0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimespecParts {
    pub seconds: i64,
    pub nanoseconds: i64,
}

pub fn timespec_to_ns(seconds: i64, nanoseconds: i64) -> Result<u64, UnknownReason> {
    if seconds < 0 || !(0..1_000_000_000).contains(&nanoseconds) {
        return Err(UnknownReason::InvalidTimespec);
    }
    let seconds = u64::try_from(seconds).map_err(|_| UnknownReason::InvalidTimespec)?;
    let nanoseconds = u64::try_from(nanoseconds).map_err(|_| UnknownReason::InvalidTimespec)?;
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|base| base.checked_add(nanoseconds))
        .ok_or(UnknownReason::NanosecondOverflow)
}

/// One sampler belongs to one fully decoded Request. Construct a new sampler
/// for every Request; its lifetime ends with that attempt and never proves finality.
#[derive(Clone, Debug)]
pub struct CpuAttemptSampler {
    process_instance: Option<String>,
    start_ns: Option<u64>,
    last_known: Option<KnownCpuTime>,
    initial_failure: Option<UnknownReason>,
    latched_failure: Option<UnknownReason>,
}

impl CpuAttemptSampler {
    /// Call only after the child has decoded and identity-checked a complete Request.
    pub fn begin(process_instance: u128) -> Self {
        let initial = read_process_cpu_ns();
        Self::begin_with_initial(process_instance, initial)
    }

    fn begin_with_initial(
        process_instance: u128,
        initial: Result<u64, UnknownReason>,
    ) -> Self {
        let process_instance = (process_instance != 0).then(|| process_instance.to_string());
        let (start_ns, initial_failure) = if process_instance.is_none() {
            (None, Some(UnknownReason::MissingProcessInstance))
        } else {
            match initial {
                Ok(start_ns) => (Some(start_ns), None),
                Err(reason) => (None, Some(reason)),
            }
        };
        Self {
            process_instance,
            start_ns,
            last_known: None,
            initial_failure,
            latched_failure: None,
        }
    }

    /// Test-only pure injection seam; unavailable to production callers.
    #[cfg(test)]
    pub fn begin_with_sample(
        process_instance: u128,
        initial: Result<u64, UnknownReason>,
    ) -> Self {
        Self::begin_with_initial(process_instance, initial)
    }

    /// Read immediately before emitting each Event. Values stay cumulative from
    /// this Request's same start sample; they are never added across Events.
    pub fn sample_before_event(&mut self) -> CpuTelemetry {
        self.sample_with_result(read_process_cpu_ns())
    }

    fn sample_with_result(&mut self, reading: Result<u64, UnknownReason>) -> CpuTelemetry {
        let value = if let Some(reason) = self.initial_failure {
            CpuTimeValue::Unknown {
                reason,
                last_known_lower_bound: self.last_known,
            }
        } else if let Some(reason) = self.latched_failure {
            CpuTimeValue::Unknown {
                reason,
                last_known_lower_bound: None,
            }
        } else if let Some(start_ns) = self.start_ns {
            match reading {
                Ok(end_ns) if end_ns >= start_ns => {
                    let Some(delta_ns) = end_ns.checked_sub(start_ns) else {
                        self.latch(UnknownReason::NanosecondOverflow);
                        return self.telemetry(CpuTimeValue::Unknown {
                            reason: UnknownReason::NanosecondOverflow,
                            last_known_lower_bound: None,
                        });
                    };
                    let known = KnownCpuTime { start_ns, end_ns, delta_ns };
                    if self.last_known.is_some_and(|last| end_ns < last.end_ns) {
                        self.latch(UnknownReason::ClockWentBackwards);
                        CpuTimeValue::Unknown {
                            reason: UnknownReason::ClockWentBackwards,
                            last_known_lower_bound: None,
                        }
                    } else {
                        self.last_known = Some(known);
                        CpuTimeValue::Known { cumulative: known }
                    }
                }
                Ok(_) => {
                    self.latch(UnknownReason::ClockWentBackwards);
                    CpuTimeValue::Unknown {
                        reason: UnknownReason::ClockWentBackwards,
                        last_known_lower_bound: None,
                    }
                }
                Err(reason @ (UnknownReason::NanosecondOverflow | UnknownReason::InvalidTimespec)) => {
                    self.latch(reason);
                    CpuTimeValue::Unknown {
                        reason,
                        last_known_lower_bound: None,
                    }
                }
                Err(reason) => CpuTimeValue::Unknown {
                    reason,
                    last_known_lower_bound: self.last_known,
                },
            }
        } else {
            CpuTimeValue::Unknown {
                reason: UnknownReason::NoStartSample,
                last_known_lower_bound: None,
            }
        };
        self.telemetry(value)
    }

    /// Test-only pure injection seam; unavailable to production callers.
    #[cfg(test)]
    pub fn sample_with_reading(
        &mut self,
        reading: Result<u64, UnknownReason>,
    ) -> CpuTelemetry {
        self.sample_with_result(reading)
    }

    fn telemetry(&self, process_cpu_time: CpuTimeValue) -> CpuTelemetry {
        CpuTelemetry {
            metrics_version: METRICS_VERSION,
            process_instance: self.process_instance.clone(),
            source: MetricSource::ChildProcessAccounting,
            process_cpu_time,
        }
    }

    fn latch(&mut self, reason: UnknownReason) {
        self.latched_failure = Some(reason);
        // A broken/regressed clock invalidates any claimed continuous lower bound.
        self.last_known = None;
    }
}

#[cfg(target_os = "macos")]
fn read_process_cpu_ns() -> Result<u64, UnknownReason> {
    let timespec = rustix::time::clock_gettime_dynamic(
        rustix::time::DynamicClockId::Known(rustix::time::ClockId::ProcessCPUTime),
    )
    .map_err(|_| UnknownReason::ClockReadFailed)?;
    let nanoseconds = i64::try_from(timespec.tv_nsec).map_err(|_| UnknownReason::InvalidTimespec)?;
    timespec_to_ns(timespec.tv_sec, nanoseconds)
}

#[cfg(not(target_os = "macos"))]
fn read_process_cpu_ns() -> Result<u64, UnknownReason> {
    Err(UnknownReason::UnsupportedPlatform)
}

#[cfg(test)]
#[path = "worker_cpu_metrics_tests.rs"]
mod tests;
