//! Version 2, source-attributed CPU worker observations.
//!
//! This schema records what the existing worker path can observe. It is not a
//! grant, wait receipt, Store acceptance, scheduler, or authorization token.
use serde::{Deserialize, Serialize};

// JSON numbers are not a lossless transport for all u128 values through
// serde_json::Value. Keep process/pool instances as canonical decimal strings.
mod decimal_u128 {
    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        parse(&value).map_err(de::Error::custom)
    }

    pub fn serialize_option<S>(value: &Option<u128>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => serializer.serialize_some(&value.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize_option<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|value| parse(&value).map_err(de::Error::custom))
            .transpose()
    }

    fn parse(value: &str) -> Result<u128, &'static str> {
        if value.is_empty()
            || value.len() > 39
            || value.starts_with('0')
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("invalid_u128_decimal");
        }
        let parsed = value.parse::<u128>().map_err(|_| "invalid_u128_decimal")?;
        if parsed == 0 {
            return Err("invalid_u128_decimal");
        }
        Ok(parsed)
    }
}

pub const OBSERVATION_VERSION: u16 = 2;
pub const MAX_WIRE_LABEL_BYTES: usize = 64;
pub const MAX_CPU_CANDIDATE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AirTicketIdentity {
    pub world: u64,
    pub branch: u64,
    pub job: u64,
    pub incarnation: u64,
    pub generation: u32,
    pub attempt: u8,
    pub base_revision: u64,
    pub field_version: u64,
    pub start: u32,
    pub end: u32,
    pub rules_sha256: [u8; 32],
    pub source_sha256: [u8; 32],
    pub input_sha256: [u8; 32],
    pub checkpoint_sha256: [u8; 32],
}

impl AirTicketIdentity {
    pub fn validate(&self) -> Result<(), ObservationError> {
        if self.world == 0
            || self.branch == 0
            || self.job == 0
            || self.incarnation == 0
            || self.generation == 0
            || !(1..=3).contains(&self.attempt)
            || self.start >= self.end
            || self.rules_sha256 == [0; 32]
            || self.source_sha256 == [0; 32]
            || self.input_sha256 == [0; 32]
            || self.checkpoint_sha256 == [0; 32]
        {
            return Err(ObservationError::InvalidRequest);
        }
        // base_revision=0 and field_version=0 are valid original wire values.
        Ok(())
    }
}

/// Exact bounded request identity. Hashes use fixed-width binary form; strategy
/// and wire labels remain exact strings and are length-checked before storage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestIdentity {
    pub wire_protocol: String,
    pub strategy: String,
    pub ticket: AirTicketIdentity,
}

impl RequestIdentity {
    pub fn validate(&self) -> Result<(), ObservationError> {
        let valid_label = |s: &str| {
            !s.is_empty()
                && s.len() <= MAX_WIRE_LABEL_BYTES
                && s.bytes().all(|b| b.is_ascii_graphic())
        };
        if !valid_label(&self.wire_protocol) || !valid_label(&self.strategy) {
            return Err(ObservationError::InvalidRequest);
        }
        self.ticket.validate()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownAllocationReason {
    PendingRegistration,
    UnconfirmedPoolOwner,
    MissingPoolIdentity,
    MissingOwnedProcess,
}

/// Direct Grant and broker Lease identities are different namespaces. Optional
/// broker fields preserve known lease evidence while keeping incomplete
/// generation/boot identity explicitly incomplete.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CpuAllocationIdentity {
    Unpooled,
    Unknown { reason: UnknownAllocationReason },
    DirectGrant {
        #[serde(with = "decimal_u128")]
        pool_boot: u128,
        grant_id: u64,
    },
    BrokerLease {
        #[serde(
            serialize_with = "decimal_u128::serialize_option",
            deserialize_with = "decimal_u128::deserialize_option"
        )]
        pool_boot: Option<u128>,
        generation: Option<u64>,
        lease_id: Option<u64>,
    },
}

impl CpuAllocationIdentity {
    pub fn validate(self) -> Result<(), ObservationError> {
        match self {
            Self::Unpooled | Self::Unknown { .. } => Ok(()),
            Self::DirectGrant { pool_boot, grant_id } => {
                if pool_boot == 0 || grant_id == 0 {
                    Err(ObservationError::InvalidExecution)
                } else {
                    Ok(())
                }
            }
            Self::BrokerLease { pool_boot, generation, lease_id } => {
                if pool_boot == Some(0) || generation == Some(0) || lease_id == Some(0) {
                    Err(ObservationError::InvalidExecution)
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuExecutionIdentity {
    /// PID comes from the retained real Child or broker Lease, never Event.pid alone.
    pub pid: u32,
    /// Child birth instance is absent from the current worker wire and remains Unknown.
    #[serde(
        serialize_with = "decimal_u128::serialize_option",
        deserialize_with = "decimal_u128::deserialize_option"
    )]
    pub process_instance: Option<u128>,
    /// Captured by the actual Worker owner at construction.
    #[serde(
        serialize_with = "decimal_u128::serialize_option",
        deserialize_with = "decimal_u128::deserialize_option"
    )]
    pub supervisor_owner_instance: Option<u128>,
    pub allocation: CpuAllocationIdentity,
}

impl CpuExecutionIdentity {
    pub fn validate(&self) -> Result<(), ObservationError> {
        if self.pid == 0
            || self.process_instance == Some(0)
            || self.supervisor_owner_instance == Some(0)
        {
            return Err(ObservationError::InvalidExecution);
        }
        self.allocation.validate()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementMeaning {
    ProcessCpuTime,
    HostElapsedFromSubmitToReceive,
    CompletedWork,
    AttemptScratchPeak,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementUnit {
    Nanoseconds,
    WorkUnits,
    Bytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementSource {
    HostMonotonicClock,
    HostProcessAccounting,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownMeasurementReason {
    NotInstrumented,
    ChildProcessInstanceUnavailable,
    PoolIdentityUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonotonicInterval {
    pub start_ns: u64,
    pub end_ns: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum MeasurementValue {
    Known { value: u64 },
    Unknown { reason: UnknownMeasurementReason },
}

/// A numeric zero is a measured zero only when source and interval are present.
/// Unknown carries no numeric value, no fabricated interval, and an explicit
/// unavailable provenance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub meaning: MeasurementMeaning,
    pub unit: MeasurementUnit,
    pub value: MeasurementValue,
    pub source: MeasurementSource,
    pub interval: Option<MonotonicInterval>,
}

impl Measurement {
    pub const fn unknown(
        meaning: MeasurementMeaning,
        unit: MeasurementUnit,
        reason: UnknownMeasurementReason,
    ) -> Self {
        Self {
            meaning,
            unit,
            value: MeasurementValue::Unknown { reason },
            source: MeasurementSource::Unavailable,
            interval: None,
        }
    }

    pub fn validate(&self) -> Result<(), ObservationError> {
        let unit_matches = matches!(
            (self.meaning, self.unit),
            (MeasurementMeaning::ProcessCpuTime, MeasurementUnit::Nanoseconds)
                | (MeasurementMeaning::HostElapsedFromSubmitToReceive, MeasurementUnit::Nanoseconds)
                | (MeasurementMeaning::CompletedWork, MeasurementUnit::WorkUnits)
                | (MeasurementMeaning::AttemptScratchPeak, MeasurementUnit::Bytes)
        );
        if !unit_matches {
            return Err(ObservationError::InvalidMeasurement);
        }
        match self.value {
            MeasurementValue::Known { value } => {
                // No current collector establishes completed work or per-attempt
                // scratch peak. A wall-clock interval or capacity value cannot
                // be substituted for either metric.
                if matches!(
                    self.meaning,
                    MeasurementMeaning::CompletedWork | MeasurementMeaning::AttemptScratchPeak
                ) {
                    return Err(ObservationError::InvalidMeasurement);
                }
                let Some(interval) = self.interval else {
                    return Err(ObservationError::InvalidMeasurement);
                };
                if interval.start_ns > interval.end_ns
                    || self.source == MeasurementSource::Unavailable
                {
                    return Err(ObservationError::InvalidMeasurement);
                }
                match self.meaning {
                    MeasurementMeaning::ProcessCpuTime
                        if self.source != MeasurementSource::HostProcessAccounting =>
                    {
                        return Err(ObservationError::InvalidMeasurement);
                    }
                    MeasurementMeaning::HostElapsedFromSubmitToReceive
                        if self.source != MeasurementSource::HostMonotonicClock
                            || value != interval.end_ns.saturating_sub(interval.start_ns) =>
                    {
                        return Err(ObservationError::InvalidMeasurement);
                    }
                    _ => {}
                }
            }
            MeasurementValue::Unknown { .. } => {
                if self.source != MeasurementSource::Unavailable || self.interval.is_some() {
                    return Err(ObservationError::InvalidMeasurement);
                }
            }
        }
        Ok(())
    }

    fn validate_as(
        &self,
        expected_meaning: MeasurementMeaning,
        expected_unit: MeasurementUnit,
    ) -> Result<(), ObservationError> {
        if self.meaning != expected_meaning || self.unit != expected_unit {
            return Err(ObservationError::InvalidMeasurement);
        }
        self.validate()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedStage {
    Sources,
    Cells,
    Published,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateEvidence {
    None { cursor_claim: u32 },
    WorkerDeclared { sha256: [u8; 32], bytes: u64 },
    HostReadHashValidated { sha256: [u8; 32], bytes: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuMeasurements {
    pub cpu_time: Measurement,
    pub elapsed_time: Measurement,
    pub work_units: Measurement,
    pub scratch_peak: Measurement,
}

impl CpuMeasurements {
    pub const fn unavailable() -> Self {
        Self {
            cpu_time: Measurement::unknown(
                MeasurementMeaning::ProcessCpuTime,
                MeasurementUnit::Nanoseconds,
                UnknownMeasurementReason::NotInstrumented,
            ),
            elapsed_time: Measurement::unknown(
                MeasurementMeaning::HostElapsedFromSubmitToReceive,
                MeasurementUnit::Nanoseconds,
                UnknownMeasurementReason::NotInstrumented,
            ),
            work_units: Measurement::unknown(
                MeasurementMeaning::CompletedWork,
                MeasurementUnit::WorkUnits,
                UnknownMeasurementReason::NotInstrumented,
            ),
            scratch_peak: Measurement::unknown(
                MeasurementMeaning::AttemptScratchPeak,
                MeasurementUnit::Bytes,
                UnknownMeasurementReason::NotInstrumented,
            ),
        }
    }

    pub fn validate(&self) -> Result<(), ObservationError> {
        self.cpu_time
            .validate_as(MeasurementMeaning::ProcessCpuTime, MeasurementUnit::Nanoseconds)?;
        self.elapsed_time.validate_as(
            MeasurementMeaning::HostElapsedFromSubmitToReceive,
            MeasurementUnit::Nanoseconds,
        )?;
        self.work_units
            .validate_as(MeasurementMeaning::CompletedWork, MeasurementUnit::WorkUnits)?;
        self.scratch_peak
            .validate_as(MeasurementMeaning::AttemptScratchPeak, MeasurementUnit::Bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuObservation {
    pub version: u16,
    pub request: RequestIdentity,
    pub execution: CpuExecutionIdentity,
    pub stage: ObservedStage,
    pub candidate: CandidateEvidence,
    pub measurements: CpuMeasurements,
}

impl CpuObservation {
    pub fn validate(&self) -> Result<(), ObservationError> {
        if self.version != OBSERVATION_VERSION {
            return Err(ObservationError::WrongVersion);
        }
        self.request.validate()?;
        self.execution.validate()?;
        self.measurements.validate()?;
        match (self.stage, self.candidate) {
            (ObservedStage::Sources | ObservedStage::Cells,
                CandidateEvidence::None { cursor_claim }) if cursor_claim > 0 => Ok(()),
            (ObservedStage::Published | ObservedStage::Complete,
                CandidateEvidence::WorkerDeclared { sha256, bytes }
                | CandidateEvidence::HostReadHashValidated { sha256, bytes })
                if sha256 != [0; 32] && bytes > 0 && bytes <= MAX_CPU_CANDIDATE_BYTES => Ok(()),
            _ => Err(ObservationError::InvalidCandidate),
        }
    }

    pub fn same_attempt_and_execution(&self, other: &Self) -> bool {
        self.request == other.request && self.execution == other.execution
    }
}

/// Bounded latest-observation state for one exact Request and one execution.
/// It is observational only; accepting Complete never grants Store authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptObservation {
    request: RequestIdentity,
    execution: CpuExecutionIdentity,
    latest: Option<CpuObservation>,
}

impl AttemptObservation {
    pub fn new(
        request: RequestIdentity,
        execution: CpuExecutionIdentity,
    ) -> Result<Self, ObservationError> {
        request.validate()?;
        execution.validate()?;
        Ok(Self { request, execution, latest: None })
    }

    pub fn record(&mut self, observation: CpuObservation) -> Result<(), ObservationError> {
        observation.validate()?;
        if observation.request != self.request || observation.execution != self.execution {
            return Err(ObservationError::StaleAttemptOrProcess);
        }
        self.latest = Some(observation);
        Ok(())
    }

    pub fn latest(&self) -> Option<&CpuObservation> {
        self.latest.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationError {
    WrongVersion,
    InvalidRequest,
    InvalidExecution,
    InvalidMeasurement,
    InvalidCandidate,
    StaleAttemptOrProcess,
}

#[cfg(test)]
#[path = "worker_protocol_observation_tests.rs"]
mod tests;
