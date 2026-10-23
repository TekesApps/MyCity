//! Fixed upper-bound probe for the bounded group traffic assignment.
//!
//! Runs the exact MAX_* input sizes through normal, partially closed, congested, and
//! fully unreachable networks. This reports assignment time and owned Vec capacities;
//! allocator overhead and transient peak allocation are intentionally not inferred.

use mycity_core::transport::{
    Assignment, Entrance, Node, OdDemand, Opening, RoadEdge, UnservedReason, MAX_DEMANDS,
    MAX_EDGES, MAX_ENTRANCES, MAX_NODES,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{env, mem::size_of, process, time::Instant};

const MODE: u8 = 1;
const SOURCE_BASE: u64 = 10_000;
const LINK_LANES: u32 = 4;
const FREE_FLOW_SECONDS: u32 = 1_000;

#[derive(Clone, Copy, Debug)]
enum Case {
    Normal,
    ClosedPreferredLane,
    Congested,
    NoOpenRoad,
}

impl Case {
    const ALL: [Self; 4] = [
        Self::Normal,
        Self::ClosedPreferredLane,
        Self::Congested,
        Self::NoOpenRoad,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::ClosedPreferredLane => "closed_preferred_lane",
            Self::Congested => "congested",
            Self::NoOpenRoad => "no_open_road",
        }
    }

    const fn capacity(self) -> u32 {
        match self {
            Self::Congested => MAX_DEMANDS as u32 / LINK_LANES,
            _ => 4_096,
        }
    }

    const fn open(self, edge_id: u32) -> bool {
        match self {
            Self::NoOpenRoad => false,
            // Close the lowest-ID parallel edge on each link. The remaining authorized
            // parallel edges preserve a route and exercise deterministic fallback.
            Self::ClosedPreferredLane => (edge_id - 1) % LINK_LANES != 0,
            Self::Normal | Self::Congested => true,
        }
    }
}

#[derive(Debug, Serialize)]
struct CaseReport {
    case: &'static str,
    assignment_elapsed_ns: u128,
    deterministic_repeat_elapsed_ns: u128,
    assignment_sha256: String,
    input_fingerprint: String,
    requested_people: u64,
    served_people: u64,
    unserved_people: u64,
    unserved_no_open_route: u64,
    unserved_capacity_exhausted: u64,
    unserved_assignment_budget_exhausted: u64,
    routing_steps: u16,
    edge_flow_records: usize,
    edges_with_flow: usize,
    total_edge_flow_people: u64,
    max_edge_flow_people: u64,
    path_receipts: usize,
    path_edge_indices: usize,
    maximum_path_length: usize,
    mean_travel_seconds_min: Option<u64>,
    mean_travel_seconds_max: Option<u64>,
    output_vec_owned_capacity_bytes: usize,
    output_inline_bytes: usize,
    deterministic_repeat_equal: bool,
}

fn opening(case: Case) -> Opening {
    let nodes = (1..=MAX_NODES as u32)
        .map(|id| Node { id, city_id: 1 })
        .collect();

    let mut edges = Vec::with_capacity(MAX_EDGES);
    let mut edge_id = 1u32;
    for from in 1..MAX_NODES as u32 {
        for _ in 0..LINK_LANES {
            edges.push(RoadEdge {
                id: edge_id,
                from,
                to: from + 1,
                city_from: 1,
                city_to: 1,
                free_flow_seconds: FREE_FLOW_SECONDS,
                capacity_people: case.capacity(),
                mode_mask: MODE,
                open: case.open(edge_id),
                boundary_source: None,
            });
            edge_id += 1;
        }
    }
    // 127 links × 4 lanes = 508. Four slower parallel lanes fill the exact edge limit;
    // their higher cost makes them a valid but unused fallback in reachable cases.
    for _ in 0..(MAX_EDGES - edges.len()) {
        edges.push(RoadEdge {
            id: edge_id,
            from: 1,
            to: 2,
            city_from: 1,
            city_to: 1,
            free_flow_seconds: FREE_FLOW_SECONDS * 2,
            capacity_people: case.capacity(),
            mode_mask: MODE,
            open: case.open(edge_id),
            boundary_source: None,
        });
        edge_id += 1;
    }

    let entrances = (1..=MAX_ENTRANCES as u32)
        .map(|id| Entrance {
            id,
            owner: 1_000 + id,
            node: id,
            mode_mask: MODE,
            authorized: true,
        })
        .collect();

    let demands = (0..MAX_DEMANDS)
        .map(|index| OdDemand {
            source_id: SOURCE_BASE + index as u64,
            origin_entrance: 1,
            destination_entrance: MAX_NODES as u32,
            mode: MODE,
            people: 1,
        })
        .collect();

    Opening {
        revision: 1,
        nodes,
        edges,
        entrances,
        demands,
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex_bytes(&Sha256::digest(bytes))
}

fn hex_bytes(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn vec_capacity_bytes<T>(capacity: usize) -> Result<usize, String> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or_else(|| "capacity_byte_count_overflow".to_owned())
}

fn opening_vec_capacity_bytes(value: &Opening) -> Result<usize, String> {
    [
        vec_capacity_bytes::<Node>(value.nodes.capacity())?,
        vec_capacity_bytes::<RoadEdge>(value.edges.capacity())?,
        vec_capacity_bytes::<Entrance>(value.entrances.capacity())?,
        vec_capacity_bytes::<OdDemand>(value.demands.capacity())?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, item| {
        sum.checked_add(item)
            .ok_or_else(|| "capacity_byte_count_overflow".to_owned())
    })
}

fn assignment_vec_capacity_bytes(value: &Assignment) -> Result<usize, String> {
    let outer = [
        vec_capacity_bytes::<mycity_core::transport::EdgeFlow>(value.edge_flows.capacity())?,
        vec_capacity_bytes::<mycity_core::transport::Unserved>(value.unserved.capacity())?,
        vec_capacity_bytes::<mycity_core::transport::BoundaryFlow>(
            value.boundary_flows.capacity(),
        )?,
        vec_capacity_bytes::<mycity_core::transport::DemandOutcome>(
            value.demand_outcomes.capacity(),
        )?,
        vec_capacity_bytes::<mycity_core::transport::PathReceipt>(value.path_receipts.capacity())?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, item| {
        sum.checked_add(item)
            .ok_or_else(|| "capacity_byte_count_overflow".to_owned())
    })?;
    value.path_receipts.iter().try_fold(outer, |sum, path| {
        sum.checked_add(vec_capacity_bytes::<u32>(path.edge_ids.capacity())?)
            .ok_or_else(|| "capacity_byte_count_overflow".to_owned())
    })
}

fn count_reason(assignment: &Assignment, expected: UnservedReason) -> u64 {
    assignment
        .unserved
        .iter()
        .filter(|entry| entry.reason == expected)
        .map(|entry| u64::from(entry.people))
        .sum()
}

fn run_case(case: Case) -> Result<(CaseReport, u128, usize), String> {
    let input = opening(case);
    input
        .validate()
        .map_err(|error| format!("{}:{error:?}", case.name()))?;
    if input.nodes.len() != MAX_NODES
        || input.edges.len() != MAX_EDGES
        || input.entrances.len() != MAX_ENTRANCES
        || input.demands.len() != MAX_DEMANDS
    {
        return Err(format!("{}:not_exact_maximum_input", case.name()));
    }

    let opening_bytes = opening_vec_capacity_bytes(&input)?;
    let started = Instant::now();
    let result = input
        .assign()
        .map_err(|error| format!("{}:assign:{error:?}", case.name()))?;
    let assignment_elapsed_ns = started.elapsed().as_nanos();

    let repeat_started = Instant::now();
    let repeat = input
        .assign()
        .map_err(|error| format!("{}:repeat:{error:?}", case.name()))?;
    let deterministic_repeat_elapsed_ns = repeat_started.elapsed().as_nanos();
    let deterministic_repeat_equal = result == repeat;
    if !deterministic_repeat_equal {
        return Err(format!("{}:deterministic_repeat_mismatch", case.name()));
    }

    if result.requested_people != MAX_DEMANDS as u64
        || result.requested_people != result.served_people + result.unserved_people
        || result.demand_outcomes.len() != MAX_DEMANDS
        || result.demand_outcomes.iter().any(|outcome| {
            u64::from(outcome.requested_people)
                != u64::from(outcome.served_people) + u64::from(outcome.unserved_people)
        })
    {
        return Err(format!("{}:demand_conservation_failed", case.name()));
    }

    let (expected_served, expected_unserved, expected_steps, expected_path_cost) = match case {
        Case::Normal | Case::ClosedPreferredLane => (512, 0, 512, Some(127_000)),
        Case::Congested => (512, 0, 512, Some(146_050)),
        Case::NoOpenRoad => (0, 512, 0, None),
    };
    if (
        result.served_people,
        result.unserved_people,
        result.routing_steps,
    ) != (expected_served, expected_unserved, expected_steps)
    {
        return Err(format!("{}:unexpected_service_totals", case.name()));
    }
    if matches!(case, Case::NoOpenRoad) && count_reason(&result, UnservedReason::NoOpenRoute) != 512
    {
        return Err("no_open_road:unexpected_unserved_reason".into());
    }
    if matches!(case, Case::Congested)
        && result
            .edge_flows
            .iter()
            .filter(|edge| edge.people > 0)
            .any(|edge| edge.cost_seconds <= u64::from(FREE_FLOW_SECONDS))
    {
        return Err("congested:travel_cost_not_above_free_flow".into());
    }
    if matches!(case, Case::ClosedPreferredLane)
        && result.path_receipts.iter().any(|path| {
            path.edge_ids
                .iter()
                .any(|edge_id| (edge_id - 1) % LINK_LANES == 0)
        })
    {
        return Err("closed_preferred_lane:closed_edge_used".into());
    }
    if result.path_receipts.len() != expected_served as usize
        || result
            .path_receipts
            .iter()
            .any(|path| path.edge_ids.len() != MAX_NODES - 1)
    {
        return Err(format!("{}:path_receipt_shape_failed", case.name()));
    }
    if result
        .demand_outcomes
        .iter()
        .any(|outcome| outcome.mean_travel_seconds != expected_path_cost)
    {
        return Err(format!("{}:travel_cost_expectation_failed", case.name()));
    }

    let serialized = serde_json::to_vec(&result).map_err(|error| error.to_string())?;
    let flow_sum = result
        .edge_flows
        .iter()
        .try_fold(0u64, |sum, edge| sum.checked_add(edge.people))
        .ok_or_else(|| "edge_flow_sum_overflow".to_owned())?;
    let expected_flow_sum = result
        .served_people
        .checked_mul((MAX_NODES - 1) as u64)
        .ok_or_else(|| "expected_edge_flow_sum_overflow".to_owned())?;
    if flow_sum != expected_flow_sum {
        return Err(format!("{}:edge_flow_conservation_failed", case.name()));
    }
    let max_travel = result
        .demand_outcomes
        .iter()
        .filter_map(|outcome| outcome.mean_travel_seconds)
        .max();
    let min_travel = result
        .demand_outcomes
        .iter()
        .filter_map(|outcome| outcome.mean_travel_seconds)
        .min();
    let max_path_length = result
        .path_receipts
        .iter()
        .map(|path| path.edge_ids.len())
        .max()
        .unwrap_or(0);

    let report = CaseReport {
        case: case.name(),
        assignment_elapsed_ns,
        deterministic_repeat_elapsed_ns,
        assignment_sha256: sha256(&serialized),
        input_fingerprint: hex_bytes(&result.input_fingerprint),
        requested_people: result.requested_people,
        served_people: result.served_people,
        unserved_people: result.unserved_people,
        unserved_no_open_route: count_reason(&result, UnservedReason::NoOpenRoute),
        unserved_capacity_exhausted: count_reason(&result, UnservedReason::CapacityExhausted),
        unserved_assignment_budget_exhausted: count_reason(
            &result,
            UnservedReason::AssignmentBudgetExhausted,
        ),
        routing_steps: result.routing_steps,
        edge_flow_records: result.edge_flows.len(),
        edges_with_flow: result
            .edge_flows
            .iter()
            .filter(|edge| edge.people > 0)
            .count(),
        total_edge_flow_people: flow_sum,
        max_edge_flow_people: result
            .edge_flows
            .iter()
            .map(|edge| edge.people)
            .max()
            .unwrap_or(0),
        path_receipts: result.path_receipts.len(),
        path_edge_indices: result
            .path_receipts
            .iter()
            .map(|path| path.edge_ids.len())
            .sum(),
        maximum_path_length: max_path_length,
        mean_travel_seconds_min: min_travel,
        mean_travel_seconds_max: max_travel,
        output_vec_owned_capacity_bytes: assignment_vec_capacity_bytes(&result)?,
        output_inline_bytes: size_of::<Assignment>(),
        deterministic_repeat_equal,
    };
    Ok((
        report,
        assignment_elapsed_ns + deterministic_repeat_elapsed_ns,
        opening_bytes,
    ))
}

fn main() {
    if env::args().len() != 1 {
        eprintln!("usage: cargo run --release -p mycity-core --example transport_capacity");
        process::exit(2);
    }

    let started = Instant::now();
    let mut reports = Vec::with_capacity(Case::ALL.len());
    let mut assignment_elapsed_ns = 0u128;
    let mut deterministic_repeat_elapsed_ns = 0u128;
    let mut input_vec_owned_capacity_bytes = 0usize;
    for case in Case::ALL {
        match run_case(case) {
            Ok((report, _, input_bytes)) => {
                assignment_elapsed_ns += report.assignment_elapsed_ns;
                deterministic_repeat_elapsed_ns += report.deterministic_repeat_elapsed_ns;
                input_vec_owned_capacity_bytes = input_bytes;
                reports.push(report);
            }
            Err(error) => {
                eprintln!("transport_capacity_probe=failed error={error}");
                process::exit(1);
            }
        }
    }
    let suite_elapsed_ns = started.elapsed().as_nanos();
    let report = serde_json::json!({
        "status": "passed",
        "scope": "fixed_transport_upper_bound_functional_and_timing_probe",
        "limits": {
            "nodes": MAX_NODES,
            "directed_edges": MAX_EDGES,
            "entrances": MAX_ENTRANCES,
            "group_od_demands": MAX_DEMANDS,
        },
        "topology": {
            "nodes": MAX_NODES,
            "edges": MAX_EDGES,
            "entrances": MAX_ENTRANCES,
            "demands": MAX_DEMANDS,
            "source_id_range": [SOURCE_BASE, SOURCE_BASE + MAX_DEMANDS as u64 - 1],
            "all_demands_unique_and_authorized": true,
            "cohort_people_per_demand": 1,
            "route_hops_when_reachable": MAX_NODES - 1,
            "maximum_path_edge_indices": MAX_DEMANDS * (MAX_NODES - 1),
        },
        "timing": {
            "first_assignment_total_ns": assignment_elapsed_ns,
            "determinism_repeat_total_ns": deterministic_repeat_elapsed_ns,
            "suite_wall_elapsed_ns": suite_elapsed_ns,
            "includes_input_generation_and_serialization": true,
        },
        "memory": {
            "opening_vec_owned_capacity_bytes": input_vec_owned_capacity_bytes,
            "opening_inline_bytes": size_of::<Opening>(),
            "measurement_basis": "Vec.capacity * size_of(element); excludes allocator metadata and temporary scratch",
            "assignment_output_owned_capacity_bytes_by_case": reports.iter().map(|case| {
                (case.case, case.output_vec_owned_capacity_bytes)
            }).collect::<Vec<_>>(),
            "assignment_inline_bytes_by_case": reports.iter().map(|case| {
                (case.case, case.output_inline_bytes)
            }).collect::<Vec<_>>(),
            "peak_or_transient_allocation_bytes": "not_measured",
            "process_rss_or_footprint": "not_measured",
        },
        "cases": reports,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report serializes")
    );
}
