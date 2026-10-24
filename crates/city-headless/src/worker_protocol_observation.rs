//! Private projection of the existing CPU worker wire into v2 observations.
//! Include only as a child module of compute_worker. This does not change worker
//! acceptance, process ownership, grants, scheduling, or Store behavior.
use super::{Event, Request, Stage, Worker, PROTOCOL};
use mycity_core::worker_protocol_observation as schema;

/// One latest-only observation state. The complete Request is retained from
/// submit time; Event.ticket is never used as a substitute for Request identity.
pub(super) struct Attempt {
    request: Request,
    state: schema::AttemptObservation,
}

impl Attempt {
    /// Call only after a real owned worker exists. Missing child birth identity
    /// remains Unknown; owner identity comes from the actual Worker at creation.
    pub(super) fn begin(request: &Request, worker: &Worker) -> Result<Self, &'static str> {
        let identity = request_identity(request)?;
        let execution = execution_identity(worker)?;
        let state = schema::AttemptObservation::new(identity, execution)
            .map_err(|_| "worker_observation_identity")?;
        Ok(Self { request: request.clone(), state })
    }

    /// Call from Worker::next only after its existing event identity and
    /// candidate asset/hash/size checks. This is host read/hash evidence, never
    /// domain or Store acceptance.
    pub(super) fn observe_after_next(
        &mut self,
        worker: &Worker,
        event: &Event,
    ) -> Result<&schema::CpuObservation, &'static str> {
        if event.protocol != self.request.protocol || event.ticket != self.request.ticket {
            return Err("worker_observation_wire_binding");
        }
        let execution = execution_identity(worker)?;
        if event.pid != execution.pid {
            return Err("worker_observation_process_binding");
        }
        let candidate = candidate_evidence(event)?;
        let observation = schema::CpuObservation {
            version: schema::OBSERVATION_VERSION,
            request: request_identity(&self.request)?,
            execution,
            stage: observed_stage(event.stage),
            candidate,
            // Existing CPU wire supplies no CPU time, elapsed interval, work
            // counter, or per-attempt scratch peak. Do not infer these from
            // cursor, capacity, or output bytes.
            measurements: schema::CpuMeasurements::unavailable(),
        };
        self.state.record(observation)
            .map_err(|_| "worker_observation_stale_execution")?;
        self.state.latest().ok_or("worker_observation_missing")
    }

    pub(super) fn latest(&self) -> Option<&schema::CpuObservation> {
        self.state.latest()
    }
}

fn request_identity(request: &Request) -> Result<schema::RequestIdentity, &'static str> {
    if request.protocol != PROTOCOL
        || request.protocol.len() > schema::MAX_WIRE_LABEL_BYTES
        || request.strategy.len() > schema::MAX_WIRE_LABEL_BYTES
        || !request.strategy.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err("worker_observation_protocol");
    }
    let ticket = &request.ticket;
    let identity = schema::RequestIdentity {
        wire_protocol: request.protocol.clone(),
        strategy: request.strategy.clone(),
        ticket: schema::AirTicketIdentity {
            world: ticket.world,
            branch: ticket.branch,
            job: ticket.job,
            incarnation: ticket.incarnation,
            generation: ticket.generation,
            attempt: ticket.attempt,
            base_revision: ticket.base_revision,
            field_version: ticket.field_version,
            start: ticket.start,
            end: ticket.end,
            rules_sha256: digest(&ticket.rules_sha256)?,
            source_sha256: digest(&ticket.source_sha256)?,
            input_sha256: digest(&ticket.input_sha256)?,
            checkpoint_sha256: digest(&ticket.checkpoint_sha256)?,
        },
    };
    identity.validate().map_err(|_| "worker_observation_request")?;
    Ok(identity)
}

fn execution_identity(worker: &Worker) -> Result<schema::CpuExecutionIdentity, &'static str> {
    let pid = worker.pid_opt().ok_or("worker_observation_pid_unknown")?;
    let allocation = match (
        worker.pool_grant.as_ref(),
        worker.broker.as_ref(),
        worker.pending_grant.is_some(),
        worker.broker_start.as_ref().is_some_and(|start| start.unconfirmed()),
        worker.child.is_some(),
    ) {
        (Some(grant), None, false, false, true) => {
            let (pool_boot, grant_id) = grant.protocol_identity();
            schema::CpuAllocationIdentity::DirectGrant { pool_boot, grant_id }
        }
        (None, Some(lease), false, false, false) => {
            let (pool_boot, generation, lease_id) = lease.observation_identity();
            schema::CpuAllocationIdentity::BrokerLease {
                pool_boot: Some(pool_boot),
                generation: Some(generation),
                lease_id: Some(lease_id),
            }
        }
        (None, None, true, _, _) => schema::CpuAllocationIdentity::Unknown {
            reason: schema::UnknownAllocationReason::PendingRegistration,
        },
        (None, None, _, true, _) => schema::CpuAllocationIdentity::Unknown {
            reason: schema::UnknownAllocationReason::UnconfirmedPoolOwner,
        },
        (None, None, false, false, true) => schema::CpuAllocationIdentity::Unpooled,
        (None, None, false, false, false) => schema::CpuAllocationIdentity::Unknown {
            reason: schema::UnknownAllocationReason::MissingOwnedProcess,
        },
        _ => schema::CpuAllocationIdentity::Unknown {
            reason: schema::UnknownAllocationReason::MissingPoolIdentity,
        },
    };
    let process_instance = None; // Current Event has PID but no child birth-instance field.
    let supervisor_owner_instance = (worker.owner_process_instance != 0)
        .then_some(worker.owner_process_instance);
    let execution = schema::CpuExecutionIdentity {
        pid,
        process_instance,
        supervisor_owner_instance,
        allocation,
    };
    execution.validate().map_err(|_| "worker_observation_execution")?;
    Ok(execution)
}

fn candidate_evidence(event: &Event) -> Result<schema::CandidateEvidence, &'static str> {
    match event.stage {
        Stage::Sources | Stage::Cells => event.cursor
            .filter(|cursor| *cursor > 0)
            .map(|cursor_claim| schema::CandidateEvidence::None { cursor_claim })
            .ok_or("worker_observation_cursor"),
        Stage::Published | Stage::Complete => {
            let hash = event.candidate_sha256.as_deref().ok_or("worker_observation_candidate_hash")?;
            Ok(schema::CandidateEvidence::HostReadHashValidated {
                sha256: digest(hash)?,
                bytes: u64::try_from(event.candidate_bytes)
                    .map_err(|_| "worker_observation_candidate_size")?,
            })
        }
    }
}

fn observed_stage(stage: Stage) -> schema::ObservedStage {
    match stage {
        Stage::Sources => schema::ObservedStage::Sources,
        Stage::Cells => schema::ObservedStage::Cells,
        Stage::Published => schema::ObservedStage::Published,
        Stage::Complete => schema::ObservedStage::Complete,
    }
}

fn digest(value: &str) -> Result<[u8; 32], &'static str> {
    if value.len() != 64 {
        return Err("worker_observation_digest");
    }
    let mut output = [0u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let nibble = |byte| match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err("worker_observation_digest"),
        };
        output[index] = nibble(pair[0])? * 16 + nibble(pair[1])?;
    }
    if output == [0; 32] {
        return Err("worker_observation_digest");
    }
    Ok(output)
}

#[cfg(test)]
#[path = "worker_protocol_observation_tests.rs"]
mod tests;
