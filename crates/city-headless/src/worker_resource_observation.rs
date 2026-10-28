//! Original Air owner facts. These observations cannot allocate or return credit.
use super::Worker;
use mycity_core::worker_protocol_observation::{CpuAllocationIdentity as Identity,UnknownAllocationReason};
use serde::Serialize;

#[derive(Serialize)]
#[serde(tag="state",rename_all="snake_case")]
pub(crate) enum Capacity {
    Known { bytes:usize, source:&'static str },
    Unknown { reason:&'static str },
}
pub(super) fn capacity(identity:Identity)->Capacity {
    match identity {
        Identity::DirectGrant{..}|Identity::BrokerLease{pool_boot:Some(_),generation:Some(_),lease_id:Some(_)}=>Capacity::Known {
            bytes:crate::normal_pool::GRANT_BYTES,
            source:"acknowledged_original_normal_pool_v1_same_source_fixed_layout_contract_not_measured_usage",
        },
        Identity::Unpooled=>Capacity::Unknown{reason:"no_pool_authorization"},
        _=>Capacity::Unknown{reason:"original_allocation_identity_unconfirmed"},
    }
}
#[derive(Serialize)]
#[serde(rename_all="snake_case")]
enum Phase { NoCurrentOwner, PendingUnconfirmed, OriginalOwnerAcknowledged,
    RetirementUnconfirmed, OriginalRetirementAcknowledged, UnpooledOwner, OwnerContextUnavailable }
#[derive(Serialize)]
pub(crate) struct CurrentOwner {
    schema:&'static str,
    phase:Phase,
    pid:Option<u32>,
    identity:Identity,
    original_contract_capacity:Capacity,
    has_active_execution:bool,
    ready_to_submit:bool,
    scope:&'static str,
    authority:&'static str,
}
pub(super) fn current(w:&Worker)->CurrentOwner {
    let unknown=Identity::Unknown{reason:UnknownAllocationReason::MissingPoolIdentity};
    let (phase,identity)=if w.is_empty() {
        (Phase::NoCurrentOwner,unknown)
    }else if w.pending_grant.is_some()||w.broker_start.as_ref().is_some_and(|s|s.unconfirmed()) {
        (Phase::PendingUnconfirmed,unknown)
    }else if let Some(grant)=w.pool_grant.as_ref() {
        let(boot,id)=grant.protocol_identity();
        (if grant.return_acknowledged(){Phase::OriginalRetirementAcknowledged}
            else if w.reaped.is_some(){Phase::RetirementUnconfirmed}else{Phase::OriginalOwnerAcknowledged},
            Identity::DirectGrant{pool_boot:boot,grant_id:id})
    }else if let Some(lease)=w.broker.as_ref() {
        let(boot,generation,id)=lease.observation_identity();let(attempted,acknowledged)=lease.stop_observation();
        (if acknowledged{Phase::OriginalRetirementAcknowledged}else if attempted{Phase::RetirementUnconfirmed}
            else{Phase::OriginalOwnerAcknowledged},
            Identity::BrokerLease{pool_boot:Some(boot),generation:Some(generation),lease_id:Some(id)})
    }else if w.child.is_some(){(Phase::UnpooledOwner,Identity::Unpooled)}
    else{(Phase::OwnerContextUnavailable,unknown)};
    CurrentOwner{schema:"mycity-air-current-owner-resources-v1",original_contract_capacity:capacity(identity),
        phase,pid:w.pid_opt(),identity,has_active_execution:w.has_active_execution(),ready_to_submit:w.ready_to_submit,
        scope:"original_input_candidate_slots_plus_typed_air_workspace; count_once_not_extra_to_pool_or_process_usage; historical_capacity_not_a_live_charge_when_unconfirmed_or_retired",
        authority:"observation_only_original_owner_wait_and_pool_ack_remain_authoritative"}
}
