use serde::Deserialize;

mod arena {
    pub use mycity_core::arena::*;
}

mod transaction {
    pub use mycity_core::transaction::*;
}

#[path = "../src/venue_capacity.rs"]
mod venue_capacity;

use arena::Scope;
use transaction::{CapacityToken, Lease, Resource};
use venue_capacity::*;

#[derive(Deserialize)]
struct OpeningFixture {
    scope: Scope,
    venues: Vec<VenueDefinition>,
    resources: Vec<Resource>,
    non_venue_resource_ids: Vec<u32>,
    offers: Vec<UseOffer>,
}

fn fixture() -> OpeningFixture {
    serde_json::from_str(include_str!("../../../tests/fixtures/issue-118-venue/opening.json"))
        .expect("checked-in venue opening fixture is valid JSON")
}

fn policies(opening: &OpeningFixture) -> Vec<VenuePolicy> {
    opening
        .venues
        .iter()
        .map(|venue| VenuePolicy {
            venue_id: venue.venue_id,
            owner: venue.owner,
            operator: venue.operator,
            permit_version: venue.permit_version,
            open: true,
            revoked: false,
        })
        .collect()
}

fn opening_view(opening: &OpeningFixture) -> OpeningView<'_> {
    OpeningView {
        scope: opening.scope,
        registry: VenueRegistry {
            venues: &opening.venues,
        },
        offers: &opening.offers,
        resources: &opening.resources,
        non_venue_resource_ids: &opening.non_venue_resource_ids,
    }
}

fn snapshot<'a>(
    opening: &'a OpeningFixture,
    policies: &'a [VenuePolicy],
    states: &'a [UseState],
    resources: &'a [Resource],
    leases: &'a [Option<Lease>],
    revision: u64,
    tick: Option<u64>,
) -> Snapshot<'a> {
    let completed_clock = tick.map(|tick| {
        VerifiedCompletedClock::after_root_completed_timeline(opening.scope, revision, tick)
    });
    Snapshot {
        scope: opening.scope,
        revision,
        completed_clock,
        registry: VenueRegistry {
            venues: &opening.venues,
        },
        offers: &opening.offers,
        policies,
        use_states: states,
        resources,
        leases,
        non_venue_resource_ids: &opening.non_venue_resource_ids,
    }
}

fn event(
    snapshot: Snapshot<'_>,
    request: Request,
    actor: RootActor,
    permission: EffectivePermission,
    token: Option<CapacityToken>,
    event_id: u64,
    occurred_at: u64,
) -> VerifiedRootEvent {
    let offer = *snapshot
        .offers
        .iter()
        .find(|offer| offer.use_id == request.use_id)
        .expect("fixture contains requested offer");
    let venue = *snapshot
        .registry
        .venues
        .iter()
        .find(|venue| venue.venue_id == offer.venue_id)
        .expect("fixture contains offer venue");
    VerifiedRootEvent::after_root_event_validation(
        request,
        snapshot,
        actor,
        permission,
        venue,
        offer,
        SourceRef {
            kind: SourceKind::GameEvent,
            scope: snapshot.scope,
            revision: snapshot.revision,
            operation_id: request.operation_id,
            event_id,
            source_id: 0,
        },
        occurred_at,
        token,
    )
}

fn request(use_id: u64, operation_id: u64, kind: RequestKind, occurred_at: Option<u64>) -> Request {
    Request {
        use_id,
        operation_id,
        kind,
        occurred_at,
    }
}

fn game_event(scope: Scope, revision: u64, operation_id: u64, event_id: u64) -> SourceRef {
    SourceRef {
        kind: SourceKind::GameEvent,
        scope,
        revision,
        operation_id,
        event_id,
        source_id: 0,
    }
}

fn reserved_state(opening: &OpeningFixture, accepted_at: u64) -> UseState {
    let offer = opening.offers[0];
    let venue = opening.venues[0];
    let accepted_event = SourceRef {
        kind: SourceKind::GameEvent,
        scope: opening.scope,
        revision: 0,
        operation_id: 1,
        event_id: 1001,
        source_id: 0,
    };
    UseState {
        use_id: offer.use_id,
        token: CapacityToken {
            world: opening.scope.world(),
            branch: opening.scope.branch(),
            number: 1,
        },
        actor: offer.operator,
        resource_id: venue.resource_id,
        units: offer.simultaneous_people().unwrap(),
        accepted_scope: opening.scope,
        accepted_revision: 0,
        accepted_operation_id: 1,
        accepted_owner: venue.owner,
        accepted_operator: offer.operator,
        accepted_version: offer.accepted_version,
        accepted_event,
        last_event: accepted_event,
        accepted_at,
        entered_at: None,
        released_at: None,
        status: UseStatus::Reserved,
    }
}

#[test]
fn fixture_maps_the_independent_literal_contract_into_the_real_api() {
    let opening = fixture();
    assert_eq!(opening.scope, Scope::new(77, 4).unwrap());
    assert_eq!(opening.venues.len(), 2);
    assert_eq!(opening.offers.len(), 2);
    assert_eq!(opening.offers[0].use_id, 501);
    assert_eq!(opening.offers[0].venue_id, 101);
    assert_eq!(opening.offers[0].group_id, 601);
    assert_eq!(opening.offers[0].simultaneous_people().unwrap(), 3);
    assert_eq!(opening.offers[0].partition.member_range_start, 1000);
    assert_eq!(opening.offers[0].partition.member_range_end, 1003);
    assert_eq!(opening.offers[1].simultaneous_people().unwrap(), 2);
    assert_eq!(opening.resources[0].capacity, 8);
    assert_eq!(opening.non_venue_resource_ids, vec![99]);
    assert_eq!(validate_opening(opening_view(&opening)), Ok(()));
}

#[test]
fn opening_rejects_alias_resource_domain_population_and_partition_conflicts() {
    let mut opening = fixture();

    opening.venues[0].aliases = [Some(opening.venues[1].venue_id), None, None, None];
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::DuplicateAlias)
    );

    opening = fixture();
    opening.venues[0].resource_id = opening.non_venue_resource_ids[0];
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::ResourceDomainConflict)
    );

    opening = fixture();
    opening.offers[0].population.learners = u32::MAX;
    opening.offers[0].population.teachers = 1;
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::Overflow)
    );

    opening = fixture();
    opening.offers[1].partition.member_range_start = 1002;
    opening.offers[1].partition.member_range_end = 1004;
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::InvalidOffer)
    );

    opening = fixture();
    opening.offers[0].group_id = 901;
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::InvalidOffer)
    );
}

#[test]
fn opening_rejects_zero_people_bad_enrollment_bad_window_and_unidentified_source() {
    let mut opening = fixture();
    opening.offers[0].population = Population {
        learners: 0,
        teachers: 0,
        staff: 0,
        visitors: 0,
        enrollment: 10,
    };
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::InvalidPopulation)
    );

    opening = fixture();
    opening.offers[0].population.learners = 3;
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::EnrollmentExceeded)
    );

    opening = fixture();
    opening.offers[0].window.end = opening.offers[0].window.start;
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::InvalidOffer)
    );

    opening = fixture();
    opening.offers[0].source_refs[0] = Some(SourceRef {
        kind: SourceKind::Genesis,
        scope: opening.scope,
        revision: 0,
        operation_id: 0,
        event_id: 0,
        source_id: 0,
    });
    assert_eq!(
        validate_opening(opening_view(&opening)),
        Err(ValidationError::InvalidOffer)
    );
}

#[test]
fn only_authorized_exact_reservation_candidate_binds_a_new_original_lease() {
    let opening = fixture();
    let policies = policies(&opening);
    let states = [];
    let leases = [None, None];
    let before_resources = opening.resources.clone();
    let before = snapshot(&opening, &policies, &states, &before_resources, &leases, 0, Some(10));
    let accept_request = request(501, 1, RequestKind::Accept, None);
    let evidence = event(
        before,
        accept_request,
        RootActor::Account(8),
        EffectivePermission::ReserveOffer,
        None,
        1001,
        10,
    );
    let candidate = plan(before, accept_request, &evidence).unwrap();
    assert_eq!(
        candidate.pool,
        PoolDelta::Reserve {
            resource_id: 11,
            actor: 8,
            units: 3,
        }
    );
    assert_eq!(candidate.metadata, UseStateDelta::None);

    let after_resources = [
        Resource {
            id: 11,
            capacity: 8,
            reserved: 3,
            occupied: 0,
        },
        Resource {
            id: 12,
            capacity: 8,
            reserved: 0,
            occupied: 0,
        },
        Resource {
            id: 99,
            capacity: 20,
            reserved: 0,
            occupied: 0,
        },
    ];
    let after_leases = [
        Some(Lease {
            token: 1,
            actor: 8,
            resource: 11,
            units: 3,
            occupied: false,
        }),
        None,
    ];
    let after = snapshot(
        &opening,
        &policies,
        &states,
        &after_resources,
        &after_leases,
        0,
        Some(10),
    );
    let bound = bind_reserved_offer(before, after, accept_request, &evidence).unwrap();
    let UseStateDelta::Insert(state) = bound.metadata else {
        panic!("binding should return one metadata row")
    };
    assert_eq!(state.token.number, 1);
    assert_eq!(state.units, 3);
    assert_eq!(state.status, UseStatus::Reserved);
    assert_eq!(state.accepted_event.event_id, 1001);
    assert_eq!(state.accepted_revision, 0);
    assert_eq!(
        validate_recovery(snapshot(
            &opening,
            &policies,
            &[state],
            &after_resources,
            &after_leases,
            0,
            Some(10),
        ))
        .unwrap()
        .checked_active_uses,
        1
    );

    let wrong_actor = event(
        before,
        accept_request,
        RootActor::Account(77),
        EffectivePermission::ReserveOffer,
        None,
        1002,
        10,
    );
    assert_eq!(plan(before, accept_request, &wrong_actor), Err(PlanError::Unauthorized));

    let wrong_source = VerifiedRootEvent::after_root_event_validation(
        accept_request,
        before,
        RootActor::Account(8),
        EffectivePermission::ReserveOffer,
        opening.venues[0],
        opening.offers[0],
        SourceRef {
            kind: SourceKind::GameEvent,
            scope: Scope::new(88, 4).unwrap(),
            revision: 0,
            operation_id: 1,
            event_id: 1003,
            source_id: 0,
        },
        10,
        None,
    );
    assert_eq!(plan(before, accept_request, &wrong_source), Err(PlanError::Stale));

    let tombstone = UseState { status: UseStatus::Cancelled, last_event: SourceRef {
        kind: SourceKind::GameEvent,
        scope: opening.scope,
        revision: 1,
        operation_id: 9,
        event_id: 9009,
        source_id: 0,
    }, ..state };
    let tombstone_states = [tombstone];
    let tombstone_before = snapshot(
        &opening,
        &policies,
        &tombstone_states,
        &opening.resources,
        &leases,
        1,
        Some(10),
    );
    let different_request = request(502, 10, RequestKind::Accept, None);
    let different_event = event(
        tombstone_before,
        different_request,
        RootActor::Account(9),
        EffectivePermission::ReserveOffer,
        None,
        9010,
        10,
    );
    let token_reuse_after_resources = [
        opening.resources[0],
        Resource {
            id: 12,
            capacity: 8,
            reserved: 2,
            occupied: 0,
        },
        opening.resources[2],
    ];
    let token_reuse_after_leases = [
        Some(Lease {
            token: 1,
            actor: 9,
            resource: 12,
            units: 2,
            occupied: false,
        }),
        None,
    ];
    let token_reuse_after = snapshot(
        &opening,
        &policies,
        &tombstone_states,
        &token_reuse_after_resources,
        &token_reuse_after_leases,
        1,
        Some(10),
    );
    assert_eq!(
        bind_reserved_offer(
            tombstone_before,
            token_reuse_after,
            different_request,
            &different_event,
        ),
        Err(PlanError::Conflict)
    );
}

#[test]
fn reserved_use_enters_once_and_safe_exit_releases_actual_after_due_or_revocation() {
    let opening = fixture();
    let mut current_policies = policies(&opening);
    current_policies[0].open = false;
    current_policies[0].revoked = true;
    current_policies[0].owner = 99;
    current_policies[0].operator = 100;
    current_policies[0].permit_version = 99;
    let mut state = reserved_state(&opening, 10);
    let mut resources = opening.resources.clone();
    resources[0].reserved = 3;
    let mut leases = [
        Some(Lease { token: 1, actor: 8, resource: 11, units: 3, occupied: false }),
        None,
    ];
    let states = [state];
    let open_policies = policies(&opening);
    let pre_enter = snapshot(&opening, &open_policies, &states, &resources, &leases, 1, Some(19));
    let enter_request = request(501, 2, RequestKind::Enter, Some(19));
    let late_event = event(
        pre_enter,
        enter_request,
        RootActor::System,
        EffectivePermission::EnterVenue,
        Some(state.token),
        1002,
        20,
    );
    assert_eq!(plan(pre_enter, enter_request, &late_event), Err(PlanError::Stale));

    let enter_evidence = event(
        pre_enter,
        enter_request,
        RootActor::System,
        EffectivePermission::EnterVenue,
        Some(state.token),
        1002,
        19,
    );
    let enter = plan(pre_enter, enter_request, &enter_evidence).unwrap();
    assert_eq!(enter.pool, PoolDelta::Occupy { token: state.token });
    let UseStateDelta::Transition { from, to, event: transition_source, occurred_at, .. } = enter.metadata else {
        panic!("enter must produce atomic lifecycle metadata")
    };
    assert_eq!((from, to, transition_source.event_id, occurred_at), (UseStatus::Reserved, UseStatus::Actual, 1002, 19));

    state.status = UseStatus::Actual;
    state.last_event = game_event(opening.scope, 1, 2, 1002);
    state.entered_at = Some(19);
    resources[0].reserved = 0;
    resources[0].occupied = 3;
    leases[0] = Some(Lease { token: 1, actor: 8, resource: 11, units: 3, occupied: true });
    let actual_states = [state];
    let actual_snapshot = snapshot(&opening, &current_policies, &actual_states, &resources, &leases, 2, Some(40));
    assert_eq!(validate_recovery(actual_snapshot).unwrap().checked_active_uses, 1);
    let exit_request = request(501, 3, RequestKind::SafeExit, Some(40));
    let exit_evidence = event(
        actual_snapshot,
        exit_request,
        RootActor::System,
        EffectivePermission::SafeExitVenue,
        Some(state.token),
        1003,
        40,
    );
    let exit = plan(actual_snapshot, exit_request, &exit_evidence).unwrap();
    assert_eq!(exit.pool, PoolDelta::Release { token: state.token });
    assert!(matches!(exit.metadata, UseStateDelta::Transition { from: UseStatus::Actual, to: UseStatus::Released, .. }));
    let released = UseState {
        status: UseStatus::Released,
        last_event: game_event(opening.scope, 2, 3, 1003),
        released_at: Some(40),
        ..state
    };
    let released_resources = [
        Resource { id: 11, capacity: 8, reserved: 0, occupied: 0 },
        opening.resources[1],
        opening.resources[2],
    ];
    let released_leases = [None, None];
    let released_states = [released];
    let after_exit = snapshot(&opening, &current_policies, &released_states, &released_resources, &released_leases, 3, Some(41));
    let repeated_exit_request = request(501, 4, RequestKind::SafeExit, Some(41));
    let repeated_exit = event(after_exit, repeated_exit_request, RootActor::System, EffectivePermission::SafeExitVenue, Some(state.token), 1004, 41);
    assert_eq!(plan(after_exit, repeated_exit_request, &repeated_exit), Err(PlanError::Conflict));
}

#[test]
fn recovery_preserves_overcommitted_actual_but_rejects_bad_bijections_and_counter_values() {
    let opening = fixture();
    let policies = policies(&opening);
    let mut state = reserved_state(&opening, 10);
    state.status = UseStatus::Actual;
    state.last_event = SourceRef {
        kind: SourceKind::GameEvent,
        scope: opening.scope,
        revision: 1,
        operation_id: 2,
        event_id: 1002,
        source_id: 0,
    };
    state.entered_at = Some(19);
    let resources = [
        Resource { id: 11, capacity: 2, reserved: 0, occupied: 3 },
        opening.resources[1],
        opening.resources[2],
    ];
    let leases = [
        Some(Lease { token: 1, actor: 8, resource: 11, units: 3, occupied: true }),
        None,
    ];
    let states = [state];
    let actual_snapshot = snapshot(&opening, &policies, &states, &resources, &leases, 2, None);
    assert_eq!(validate_recovery(actual_snapshot).unwrap().overcommitted_resources, 1);

    let bad_counter = [
        Resource { id: 11, capacity: 2, reserved: 0, occupied: 2 },
        opening.resources[1],
        opening.resources[2],
    ];
    let bad = snapshot(&opening, &policies, &states, &bad_counter, &leases, 2, None);
    assert_eq!(validate_recovery(bad), Err(PlanError::Integrity));

    let missing_lease = [None, None];
    let orphan = snapshot(&opening, &policies, &states, &resources, &missing_lease, 2, None);
    assert_eq!(validate_recovery(orphan), Err(PlanError::Integrity));

    let future_state = UseState {
        accepted_revision: 3,
        accepted_event: game_event(opening.scope, 3, 3, 3003),
        last_event: game_event(opening.scope, 3, 3, 3003),
        ..state
    };
    let future_states = [future_state];
    let future = snapshot(&opening, &policies, &future_states, &resources, &leases, 2, None);
    assert_eq!(validate_recovery(future), Err(PlanError::Integrity));
}

#[test]
fn recovery_rejects_actual_and_released_entry_times_outside_the_accepted_window() {
    let opening = fixture();
    let policies = policies(&opening);
    let base = reserved_state(&opening, 0);
    let actual = UseState {
        status: UseStatus::Actual,
        last_event: game_event(opening.scope, 1, 2, 1002),
        entered_at: Some(1),
        ..base
    };
    let actual_states = [actual];
    let actual_resources = [
        Resource {
            id: 11,
            capacity: 8,
            reserved: 0,
            occupied: 3,
        },
        opening.resources[1],
        opening.resources[2],
    ];
    let actual_leases = [
        Some(Lease {
            token: 1,
            actor: 8,
            resource: 11,
            units: 3,
            occupied: true,
        }),
        None,
    ];
    let bad_actual = snapshot(
        &opening,
        &policies,
        &actual_states,
        &actual_resources,
        &actual_leases,
        2,
        None,
    );
    assert_eq!(validate_recovery(bad_actual), Err(PlanError::Integrity));

    let released = UseState {
        status: UseStatus::Released,
        last_event: game_event(opening.scope, 2, 3, 1003),
        entered_at: Some(99),
        released_at: Some(100),
        ..base
    };
    let released_states = [released];
    let released_resources = [
        Resource {
            id: 11,
            capacity: 8,
            reserved: 0,
            occupied: 0,
        },
        opening.resources[1],
        opening.resources[2],
    ];
    let released_leases = [None, None];
    let bad_released = snapshot(
        &opening,
        &policies,
        &released_states,
        &released_resources,
        &released_leases,
        3,
        None,
    );
    assert_eq!(validate_recovery(bad_released), Err(PlanError::Integrity));
}

#[test]
fn transitions_require_clock_system_event_and_reservation_authority() {
    let opening = fixture();
    let policies = policies(&opening);
    let state = reserved_state(&opening, 10);
    let resources = [
        Resource { id: 11, capacity: 8, reserved: 3, occupied: 0 },
        opening.resources[1],
        opening.resources[2],
    ];
    let leases = [
        Some(Lease { token: 1, actor: 8, resource: 11, units: 3, occupied: false }),
        None,
    ];
    let states = [state];
    let no_clock = snapshot(&opening, &policies, &states, &resources, &leases, 1, None);
    let enter_request = request(501, 2, RequestKind::Enter, Some(11));
    let enter_evidence = event(
        no_clock,
        enter_request,
        RootActor::System,
        EffectivePermission::EnterVenue,
        Some(state.token),
        1002,
        11,
    );
    assert_eq!(plan(no_clock, enter_request, &enter_evidence), Err(PlanError::Unknown));

    let timed = snapshot(&opening, &policies, &states, &resources, &leases, 1, Some(11));
    let account_enter = event(
        timed,
        enter_request,
        RootActor::Account(8),
        EffectivePermission::EnterVenue,
        Some(state.token),
        1003,
        11,
    );
    assert_eq!(plan(timed, enter_request, &account_enter), Err(PlanError::Unauthorized));

    let cancel_request = request(501, 4, RequestKind::CancelReserved, None);
    let absent_right = event(
        timed,
        cancel_request,
        RootActor::Account(8),
        EffectivePermission::ReserveOffer,
        Some(state.token),
        1004,
        11,
    );
    assert_eq!(plan(timed, cancel_request, &absent_right), Err(PlanError::Unauthorized));

    let cancel = event(
        timed,
        cancel_request,
        RootActor::Account(8),
        EffectivePermission::CancelReservation,
        Some(state.token),
        1005,
        11,
    );
    assert_eq!(plan(timed, cancel_request, &cancel).unwrap().pool, PoolDelta::Release { token: state.token });
    let cancelled = UseState {
        status: UseStatus::Cancelled,
        last_event: game_event(opening.scope, 1, 4, 1005),
        ..state
    };
    let cancelled_resources = [
        Resource { id: 11, capacity: 8, reserved: 0, occupied: 0 },
        opening.resources[1],
        opening.resources[2],
    ];
    let cancelled_states = [cancelled];
    let cancelled_leases = [None, None];
    let cancelled_snapshot = snapshot(&opening, &policies, &cancelled_states, &cancelled_resources, &cancelled_leases, 2, Some(12));
    let repeated_cancel_request = request(501, 5, RequestKind::CancelReserved, None);
    let repeated_cancel = event(cancelled_snapshot, repeated_cancel_request, RootActor::Account(8), EffectivePermission::CancelReservation, Some(state.token), 1006, 12);
    assert_eq!(plan(cancelled_snapshot, repeated_cancel_request, &repeated_cancel), Err(PlanError::Conflict));
}

#[test]
fn shared_group_cannot_enter_twice_and_conservative_capacity_gate_is_not_physical_shortage() {
    let mut opening = fixture();
    let mut repeated = opening.offers[0];
    repeated.use_id = 503;
    repeated.window.start = 10;
    repeated.window.end = 22;
    opening.offers.push(repeated);

    let opening_policies = policies(&opening);
    let first = reserved_state(&opening, 10);
    let second = UseState {
        use_id: 503,
        token: CapacityToken { world: 77, branch: 4, number: 2 },
        actor: 8,
        resource_id: 11,
        units: 3,
        accepted_scope: opening.scope,
        accepted_revision: 0,
        accepted_operation_id: 4,
        accepted_owner: 7,
        accepted_operator: 8,
        accepted_version: 5,
        accepted_event: SourceRef { kind: SourceKind::GameEvent, scope: opening.scope, revision: 0, operation_id: 4, event_id: 1004, source_id: 0 },
        last_event: SourceRef { kind: SourceKind::GameEvent, scope: opening.scope, revision: 0, operation_id: 4, event_id: 1004, source_id: 0 },
        accepted_at: 10,
        entered_at: None,
        released_at: None,
        status: UseStatus::Reserved,
    };
    let states = [first, second];
    let resources = [
        Resource { id: 11, capacity: 8, reserved: 6, occupied: 0 },
        opening.resources[1],
        opening.resources[2],
    ];
    let leases = [
        Some(Lease { token: 1, actor: 8, resource: 11, units: 3, occupied: false }),
        Some(Lease { token: 2, actor: 8, resource: 11, units: 3, occupied: false }),
    ];
    let ready = snapshot(&opening, &opening_policies, &states, &resources, &leases, 1, Some(10));
    let enter_request = request(501, 5, RequestKind::Enter, Some(10));
    let enter_event = event(ready, enter_request, RootActor::System, EffectivePermission::EnterVenue, Some(first.token), 1005, 10);
    assert!(plan(ready, enter_request, &enter_event).is_ok());

    let actual = UseState { status: UseStatus::Actual, last_event: game_event(opening.scope, 1, 5, 1005), entered_at: Some(10), ..first };
    let actual_resources = [
        Resource { id: 11, capacity: 8, reserved: 3, occupied: 3 },
        opening.resources[1],
        opening.resources[2],
    ];
    let actual_leases = [
        Some(Lease { token: 1, actor: 8, resource: 11, units: 3, occupied: true }),
        Some(Lease { token: 2, actor: 8, resource: 11, units: 3, occupied: false }),
    ];
    let actual_states = [actual, second];
    let actual_snapshot = snapshot(&opening, &opening_policies, &actual_states, &actual_resources, &actual_leases, 2, Some(10));
    let second_enter_request = request(503, 6, RequestKind::Enter, Some(10));
    let second_enter = event(actual_snapshot, second_enter_request, RootActor::System, EffectivePermission::EnterVenue, Some(second.token), 1006, 10);
    assert_eq!(plan(actual_snapshot, second_enter_request, &second_enter), Err(PlanError::Conflict));

    let mut constrained = fixture();
    constrained.resources[0].capacity = 2;
    let constrained_policies = policies(&constrained);
    let empty_states = [];
    let empty_leases = [None, None];
    let admission = snapshot(&constrained, &constrained_policies, &empty_states, &constrained.resources, &empty_leases, 0, Some(10));
    let accept_request = request(501, 7, RequestKind::Accept, None);
    let accept_event = event(admission, accept_request, RootActor::Account(8), EffectivePermission::ReserveOffer, None, 1007, 10);
    assert_eq!(plan(admission, accept_request, &accept_event), Err(PlanError::CapacityGateRejected));
}

#[test]
fn source_and_layout_bounds_are_explicit_and_max_json_rows_fit_the_formula() {
    println!("VenueDefinition={} VenuePolicy={} UseOffer={} GroupPartition={} UseState={} Request={} VerifiedRootEvent={} Candidate={} opening_rows={} policy_rows={} runtime_rows={} metadata_rows={} original_pool_rows={}",
        VENUE_DEFINITION_BYTES,
        VENUE_POLICY_BYTES,
        USE_OFFER_BYTES,
        GROUP_PARTITION_BYTES,
        USE_STATE_BYTES,
        REQUEST_BYTES,
        VERIFIED_ROOT_EVENT_BYTES,
        CANDIDATE_BYTES,
        OPENING_ROW_LAYOUT_BYTES,
        POLICY_ROW_LAYOUT_BYTES,
        RUNTIME_ROW_LAYOUT_BYTES,
        MAX_METADATA_ROW_BYTES,
        BORROWED_ORIGINAL_POOL_LAYOUT_BYTES,
    );
    assert_eq!(MAX_USE_STATES, 64);
    assert_eq!(MAX_USE_OFFERS, 64);
    assert_eq!(MAX_VENUES, 16);
    assert_eq!(SOURCE_REF_JSON_MAX, 223);
    assert_eq!(VENUE_DEFINITION_JSON_MAX, 249);
    assert_eq!(GROUP_PARTITION_JSON_MAX, 199);
    assert_eq!(USE_OFFER_JSON_MAX, 1488);
    assert_eq!(USE_STATE_JSON_MAX, 1059);
    assert_eq!(MAX_VENUES_JSON_BYTES, 4001);
    assert_eq!(MAX_USE_OFFERS_JSON_BYTES, 95297);
    assert_eq!(MAX_USE_STATES_JSON_BYTES, 67841);
    assert_eq!(MAX_OPENING_JSON_BYTES, 99298);
    assert_eq!(MAX_RUNTIME_JSON_BYTES, 67841);
    assert_eq!(MAX_OPENING_JSON_BYTES + MAX_RUNTIME_JSON_BYTES, 167139);
    assert_eq!(METADATA_LAYOUT_CEILING_BYTES, 59_392);
    assert!(VENUE_DEFINITION_BYTES <= VENUE_DEFINITION_LAYOUT_CEILING);
    assert!(USE_OFFER_BYTES <= USE_OFFER_LAYOUT_CEILING);
    assert!(USE_STATE_BYTES <= USE_STATE_LAYOUT_CEILING);

    let scope = Scope::new(u64::MAX, u64::MAX).unwrap();
    let source = SourceRef {
        kind: SourceKind::GameEvent,
        scope,
        revision: u64::MAX,
        operation_id: u64::MAX,
        event_id: u64::MAX,
        source_id: 0,
    };
    let venue = VenueDefinition {
        venue_id: u64::MAX,
        resource_id: u32::MAX,
        aliases: [Some(u64::MAX); MAX_ALIASES_PER_VENUE],
        owner: u32::MAX,
        operator: u32::MAX,
        purpose: PurposeId(u16::MAX),
        permit_version: u64::MAX,
    };
    let partition = GroupPartition {
        group_id: u64::MAX,
        partition_source_id: u64::MAX,
        partition_version: u64::MAX,
        member_range_start: u64::MAX,
        member_range_end: u64::MAX,
    };
    let population = Population {
        learners: u32::MAX,
        teachers: u32::MAX,
        staff: u32::MAX,
        visitors: u32::MAX,
        enrollment: u32::MAX,
    };
    let offer = UseOffer {
        use_id: u64::MAX,
        venue_id: u64::MAX,
        group_id: u64::MAX,
        partition,
        population,
        purpose: PurposeId(u16::MAX),
        operator: u32::MAX,
        window: UseWindow { start: u64::MAX, end: u64::MAX },
        source_refs: [Some(source); MAX_SOURCE_REFS_PER_OFFER],
        accepted_version: u64::MAX,
    };
    let state = UseState {
        use_id: u64::MAX,
        token: CapacityToken { world: u64::MAX, branch: u64::MAX, number: u64::MAX },
        actor: u32::MAX,
        resource_id: u32::MAX,
        units: u32::MAX,
        accepted_scope: scope,
        accepted_revision: u64::MAX,
        accepted_operation_id: u64::MAX,
        accepted_owner: u32::MAX,
        accepted_operator: u32::MAX,
        accepted_version: u64::MAX,
        accepted_event: source,
        last_event: source,
        accepted_at: u64::MAX,
        entered_at: Some(u64::MAX),
        released_at: Some(u64::MAX),
        status: UseStatus::Cancelled,
    };
    let request = Request {
        use_id: u64::MAX,
        operation_id: u64::MAX,
        kind: RequestKind::CancelReserved,
        occurred_at: Some(u64::MAX),
    };
    let resource = Resource {
        id: u32::MAX,
        capacity: u32::MAX,
        reserved: u32::MAX,
        occupied: u32::MAX,
    };
    let lease = Lease {
        token: u64::MAX,
        actor: u32::MAX,
        resource: u32::MAX,
        units: u32::MAX,
        occupied: true,
    };
    assert!(serde_json::to_vec(&scope).unwrap().len() <= SCOPE_JSON_MAX);
    assert!(serde_json::to_vec(&source).unwrap().len() <= SOURCE_REF_JSON_MAX);
    assert!(serde_json::to_vec(&venue).unwrap().len() <= VENUE_DEFINITION_JSON_MAX);
    assert!(serde_json::to_vec(&partition).unwrap().len() <= GROUP_PARTITION_JSON_MAX);
    assert!(serde_json::to_vec(&population).unwrap().len() <= POPULATION_JSON_MAX);
    assert!(serde_json::to_vec(&offer).unwrap().len() <= USE_OFFER_JSON_MAX);
    assert!(serde_json::to_vec(&state).unwrap().len() <= USE_STATE_JSON_MAX);
    assert!(serde_json::to_vec(&request).unwrap().len() <= REQUEST_JSON_MAX);
    assert!(serde_json::to_vec(&resource).unwrap().len() <= RESOURCE_JSON_MAX);
    assert!(serde_json::to_vec(&lease).unwrap().len() <= LEASE_JSON_MAX);
    let venue_rows = vec![venue; MAX_VENUES];
    let offer_rows = vec![offer; MAX_USE_OFFERS];
    let state_rows = vec![state; MAX_USE_STATES];
    assert!(serde_json::to_vec(&venue_rows).unwrap().len() <= MAX_VENUES_JSON_BYTES);
    assert!(serde_json::to_vec(&offer_rows).unwrap().len() <= MAX_USE_OFFERS_JSON_BYTES);
    assert!(serde_json::to_vec(&state_rows).unwrap().len() <= MAX_USE_STATES_JSON_BYTES);
    assert!(serde_json::to_vec(&venue_rows).unwrap().len()
        + serde_json::to_vec(&offer_rows).unwrap().len()
        <= MAX_OPENING_JSON_BYTES);
    assert!(serde_json::to_vec(&state_rows).unwrap().len() <= MAX_RUNTIME_JSON_BYTES);
}
