//! Bounded deterministic group traffic assignment on an explicitly authorized directed graph.
//! This module is pure CPU calculation; callers persist inputs/results through the shared host.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_NODES: usize = 128;
pub const MAX_EDGES: usize = 512;
pub const MAX_ENTRANCES: usize = 128;
pub const MAX_DEMANDS: usize = 512;
pub const MAX_ASSIGNMENT_STEPS: usize = 512;
pub const MAX_PATH_RECEIPTS: usize = MAX_ASSIGNMENT_STEPS;
pub const MAX_PATH_EDGES: usize = MAX_NODES - 1;
const COST_SCALE: u64 = 10_000;
const BPR_ALPHA: u128 = 1_500; // 0.15 * COST_SCALE
const BPR_SCALE: u128 = 10_000;
const BPR_POWER_DENOMINATOR: u128 = 10_000_000_000_000_000; // SCALE^4

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub id: u32,
    pub city_id: u32,
}

/// `city_from != city_to` marks a city boundary. `boundary_source` must be unique.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoadEdge {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub city_from: u32,
    pub city_to: u32,
    pub free_flow_seconds: u32,
    pub capacity_people: u32,
    pub mode_mask: u8,
    pub open: bool,
    pub boundary_source: Option<u64>,
}

/// Entry is a granted topological connection, never inferred from geometric proximity.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entrance {
    pub id: u32,
    pub owner: u32,
    pub node: u32,
    pub mode_mask: u8,
    pub authorized: bool,
}

/// One aggregate OD cohort in a single assignment period.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OdDemand {
    pub source_id: u64,
    pub origin_entrance: u32,
    pub destination_entrance: u32,
    pub mode: u8,
    pub people: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Opening {
    pub revision: u64,
    pub nodes: Vec<Node>,
    pub edges: Vec<RoadEdge>,
    pub entrances: Vec<Entrance>,
    pub demands: Vec<OdDemand>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Error {
    Limit,
    DuplicateId,
    MissingNode,
    InvalidEdge,
    DuplicateBoundarySource,
    InvalidEntrance,
    UnauthorizedEntrance,
    InvalidDemand,
    DuplicateDemandSource,
    Overflow,
    Fingerprint,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EdgeFlow {
    pub edge_id: u32,
    pub people: u64,
    pub capacity_people: u32,
    pub cost_seconds: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Unserved {
    pub source_id: u64,
    pub people: u32,
    pub reason: UnservedReason,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum UnservedReason {
    NoOpenRoute,
    CapacityExhausted,
    AssignmentBudgetExhausted,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoundaryFlow {
    pub source_id: u64,
    pub people: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DemandOutcome {
    pub source_id: u64,
    pub requested_people: u32,
    pub served_people: u32,
    pub unserved_people: u32,
    /// Integer mean path cost for served people; `None` means nobody could travel.
    pub mean_travel_seconds: Option<u64>,
}
/// One bounded aggregate path slice for an OD demand source. This is group flow, never
/// an individual vehicle/person record. `edge_ids` are stable opening identifiers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathReceipt {
    pub source_id: u64,
    pub people: u32,
    pub edge_ids: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assignment {
    pub network_revision: u64,
    /// SHA-256 of canonicalized topology, permissions, capacities, demands, and revision.
    pub input_fingerprint: [u8; 32],
    pub edge_flows: Vec<EdgeFlow>,
    pub unserved: Vec<Unserved>,
    pub boundary_flows: Vec<BoundaryFlow>,
    pub demand_outcomes: Vec<DemandOutcome>,
    /// At most `MAX_PATH_RECEIPTS`; each simple path has at most `MAX_PATH_EDGES`.
    pub path_receipts: Vec<PathReceipt>,
    pub requested_people: u64,
    pub served_people: u64,
    pub unserved_people: u64,
    pub routing_steps: u16,
}

impl Assignment {
    /// A result is reusable only for the exact canonicalized input and revision it names.
    pub fn matches(&self, opening: &Opening) -> bool {
        self.network_revision == opening.revision
            && opening.input_fingerprint().ok() == Some(self.input_fingerprint)
    }

    /// Verify the complete deterministic result once against its opening. A matching input
    /// fingerprint alone does not attest to output fields or path receipts.
    pub fn verify(&self, opening: &Opening) -> Result<(), Error> {
        if !self.matches(opening) {
            return Err(Error::Fingerprint);
        }
        if opening.assign()?.ne(self) {
            return Err(Error::Fingerprint);
        }
        Ok(())
    }
}

impl Opening {
    pub fn validate(&self) -> Result<(), Error> {
        if self.nodes.is_empty()
            || self.nodes.len() > MAX_NODES
            || self.edges.len() > MAX_EDGES
            || self.entrances.len() > MAX_ENTRANCES
            || self.demands.len() > MAX_DEMANDS
        {
            return Err(Error::Limit);
        }
        let mut nodes = BTreeMap::new();
        for n in &self.nodes {
            if n.city_id == 0 {
                return Err(Error::InvalidEdge);
            }
            if nodes.insert(n.id, n.city_id).is_some() {
                return Err(Error::DuplicateId);
            }
        }
        let mut edges = BTreeSet::new();
        let mut boundary = BTreeSet::new();
        for e in &self.edges {
            if !edges.insert(e.id) {
                return Err(Error::DuplicateId);
            }
            let from_city = *nodes.get(&e.from).ok_or(Error::MissingNode)?;
            let to_city = *nodes.get(&e.to).ok_or(Error::MissingNode)?;
            if e.city_from != from_city || e.city_to != to_city {
                return Err(Error::InvalidEdge);
            }
            if e.from == e.to
                || e.free_flow_seconds == 0
                || e.capacity_people == 0
                || e.mode_mask == 0
            {
                return Err(Error::InvalidEdge);
            }
            if e.city_from != e.city_to {
                let Some(key) = e.boundary_source else {
                    return Err(Error::InvalidEdge);
                };
                if !boundary.insert(key) {
                    return Err(Error::DuplicateBoundarySource);
                }
            } else if e.boundary_source.is_some() {
                return Err(Error::InvalidEdge);
            }
        }
        let mut entrances = BTreeSet::new();
        for x in &self.entrances {
            if !entrances.insert(x.id) {
                return Err(Error::DuplicateId);
            }
            if !nodes.contains_key(&x.node) || x.mode_mask == 0 {
                return Err(Error::InvalidEntrance);
            }
        }
        let emap: Vec<_> = self.entrances.iter().collect();
        let mut sources = BTreeSet::new();
        for d in &self.demands {
            if d.people == 0 || d.mode == 0 || d.mode.count_ones() != 1 {
                return Err(Error::InvalidDemand);
            }
            if !sources.insert(d.source_id) {
                return Err(Error::DuplicateDemandSource);
            }
            let a = emap
                .iter()
                .find(|x| x.id == d.origin_entrance)
                .ok_or(Error::InvalidDemand)?;
            let b = emap
                .iter()
                .find(|x| x.id == d.destination_entrance)
                .ok_or(Error::InvalidDemand)?;
            if !a.authorized
                || !b.authorized
                || a.mode_mask & d.mode == 0
                || b.mode_mask & d.mode == 0
            {
                return Err(Error::UnauthorizedEntrance);
            }
        }
        Ok(())
    }

    /// Canonical fingerprint for stale-result detection. Input ordering does not affect it.
    pub fn input_fingerprint(&self) -> Result<[u8; 32], Error> {
        self.validate()?;
        let mut nodes = self.nodes.clone();
        let mut edges = self.edges.clone();
        let mut entrances = self.entrances.clone();
        let mut demands = self.demands.clone();
        nodes.sort_by_key(|x| x.id);
        edges.sort_by_key(|x| x.id);
        entrances.sort_by_key(|x| x.id);
        demands.sort_by_key(|x| x.source_id);
        let bytes = serde_json::to_vec(&(self.revision, nodes, edges, entrances, demands))
            .map_err(|_| Error::Fingerprint)?;
        Ok(Sha256::digest(bytes).into())
    }

    /// Deterministic incremental loading. The result fingerprint must match before reuse.
    pub fn assign(&self) -> Result<Assignment, Error> {
        self.validate()?;
        let input_fingerprint = self.input_fingerprint()?;
        let mut edges = self.edges.clone();
        edges.sort_by_key(|e| e.id);
        let mut nodes = self.nodes.clone();
        nodes.sort_by_key(|n| n.id);
        let mut entrances = self.entrances.clone();
        entrances.sort_by_key(|e| e.id);
        let mut demands = self.demands.clone();
        demands.sort_by_key(|d| d.source_id);
        let mut flow = vec![0u64; edges.len()];
        let mut served = 0u64;
        let mut requested = 0u64;
        let mut unserved = Vec::new();
        let mut demand_outcomes = Vec::with_capacity(demands.len());
        let mut path_receipts = Vec::with_capacity(MAX_PATH_RECEIPTS);
        let mut steps = 0u16;
        for d in demands {
            requested = requested
                .checked_add(u64::from(d.people))
                .ok_or(Error::Overflow)?;
            let from = entrances
                .binary_search_by_key(&d.origin_entrance, |x| x.id)
                .map_err(|_| Error::InvalidDemand)?;
            let to = entrances
                .binary_search_by_key(&d.destination_entrance, |x| x.id)
                .map_err(|_| Error::InvalidDemand)?;
            let (origin, destination) = (entrances[from].node, entrances[to].node);
            let mut remaining = u64::from(d.people);
            let mut demand_served = 0u64;
            let mut unserved_reason = None;
            if origin == destination {
                if usize::from(steps) < MAX_ASSIGNMENT_STEPS {
                    demand_served = remaining;
                    remaining = 0;
                    served = served.checked_add(demand_served).ok_or(Error::Overflow)?;
                    path_receipts.push(PathReceipt {
                        source_id: d.source_id,
                        people: d.people,
                        edge_ids: Vec::new(),
                    });
                    steps = steps.checked_add(1).ok_or(Error::Overflow)?;
                } else {
                    unserved_reason = Some(UnservedReason::AssignmentBudgetExhausted);
                }
            }
            while remaining > 0 && usize::from(steps) < MAX_ASSIGNMENT_STEPS {
                let path = shortest_path(&nodes, &edges, &flow, origin, destination, d.mode)?;
                let Some(path) = path else {
                    unserved_reason = Some(
                        if has_open_route(&nodes, &edges, origin, destination, d.mode)? {
                            UnservedReason::CapacityExhausted
                        } else {
                            UnservedReason::NoOpenRoute
                        },
                    );
                    break;
                };
                let room = path
                    .iter()
                    .map(|i| u64::from(edges[*i].capacity_people).saturating_sub(flow[*i]))
                    .min()
                    .unwrap_or(0);
                if room == 0 {
                    unserved_reason = Some(UnservedReason::CapacityExhausted);
                    break;
                }
                let amount = remaining.min(room);
                if path.len() > MAX_PATH_EDGES || path_receipts.len() >= MAX_PATH_RECEIPTS {
                    return Err(Error::Limit);
                }
                let edge_ids = path.iter().map(|i| edges[*i].id).collect();
                for i in path {
                    flow[i] = flow[i].checked_add(amount).ok_or(Error::Overflow)?;
                }
                path_receipts.push(PathReceipt {
                    source_id: d.source_id,
                    people: u32::try_from(amount).map_err(|_| Error::Overflow)?,
                    edge_ids,
                });
                remaining -= amount;
                demand_served = demand_served.checked_add(amount).ok_or(Error::Overflow)?;
                served = served.checked_add(amount).ok_or(Error::Overflow)?;
                steps = steps.checked_add(1).ok_or(Error::Overflow)?;
            }
            if remaining > 0 && unserved_reason.is_none() {
                unserved_reason = Some(UnservedReason::AssignmentBudgetExhausted);
            }
            if remaining > 0 {
                unserved.push(Unserved {
                    source_id: d.source_id,
                    people: u32::try_from(remaining).map_err(|_| Error::Overflow)?,
                    reason: unserved_reason.ok_or(Error::Overflow)?,
                });
            }
            demand_outcomes.push(DemandOutcome {
                source_id: d.source_id,
                requested_people: d.people,
                served_people: u32::try_from(demand_served).map_err(|_| Error::Overflow)?,
                unserved_people: u32::try_from(remaining).map_err(|_| Error::Overflow)?,
                mean_travel_seconds: None,
            });
        }
        let mut edge_flows = Vec::with_capacity(edges.len());
        for (i, e) in edges.iter().enumerate() {
            if !e.open {
                continue;
            }
            edge_flows.push(EdgeFlow {
                edge_id: e.id,
                people: flow[i],
                capacity_people: e.capacity_people,
                cost_seconds: congested_cost(e.free_flow_seconds, e.capacity_people, flow[i])?,
            });
        }
        let mut weighted_seconds = vec![0u128; demand_outcomes.len()];
        let outcome_indices: BTreeMap<_, _> = demand_outcomes
            .iter()
            .enumerate()
            .map(|(index, outcome)| (outcome.source_id, index))
            .collect();
        for receipt in &path_receipts {
            if receipt.edge_ids.len() > MAX_PATH_EDGES {
                return Err(Error::Limit);
            }
            let path_cost = receipt.edge_ids.iter().try_fold(0u64, |sum, edge_id| {
                let index = edges
                    .binary_search_by_key(edge_id, |edge| edge.id)
                    .map_err(|_| Error::InvalidEdge)?;
                let edge = &edges[index];
                sum.checked_add(congested_cost(
                    edge.free_flow_seconds,
                    edge.capacity_people,
                    flow[index],
                )?)
                .ok_or(Error::Overflow)
            })?;
            let weighted = u128::from(receipt.people)
                .checked_mul(u128::from(path_cost))
                .ok_or(Error::Overflow)?;
            let outcome_index = *outcome_indices
                .get(&receipt.source_id)
                .ok_or(Error::InvalidDemand)?;
            let total = weighted_seconds
                .get_mut(outcome_index)
                .ok_or(Error::InvalidDemand)?;
            *total = total.checked_add(weighted).ok_or(Error::Overflow)?;
        }
        for (index, outcome) in demand_outcomes.iter_mut().enumerate() {
            if outcome.served_people > 0 {
                outcome.mean_travel_seconds = Some(
                    u64::try_from(weighted_seconds[index] / u128::from(outcome.served_people))
                        .map_err(|_| Error::Overflow)?,
                );
            }
        }
        let mut boundary_flows = Vec::new();
        for (i, e) in edges.iter().enumerate() {
            if let Some(source_id) = e.boundary_source {
                if flow[i] > 0 {
                    boundary_flows.push(BoundaryFlow {
                        source_id,
                        people: flow[i],
                    });
                }
            }
        }
        boundary_flows.sort_by_key(|x| x.source_id);
        let unserved_people = unserved.iter().try_fold(0u64, |a, x| {
            a.checked_add(u64::from(x.people)).ok_or(Error::Overflow)
        })?;
        Ok(Assignment {
            network_revision: self.revision,
            input_fingerprint,
            edge_flows,
            unserved,
            boundary_flows,
            demand_outcomes,
            path_receipts,
            requested_people: requested,
            served_people: served,
            unserved_people,
            routing_steps: steps,
        })
    }
}

fn congested_cost(free: u32, cap: u32, flow: u64) -> Result<u64, Error> {
    let ratio = u128::from(flow)
        .checked_mul(u128::from(COST_SCALE))
        .ok_or(Error::Overflow)?
        / u128::from(cap);
    let fourth = ratio
        .checked_mul(ratio)
        .and_then(|x| x.checked_mul(x))
        .ok_or(Error::Overflow)?;
    let increment = BPR_ALPHA.checked_mul(fourth).ok_or(Error::Overflow)? / BPR_POWER_DENOMINATOR;
    let scaled = BPR_SCALE.checked_add(increment).ok_or(Error::Overflow)?;
    u64::try_from(
        u128::from(free)
            .checked_mul(scaled)
            .ok_or(Error::Overflow)?
            / BPR_SCALE,
    )
    .map_err(|_| Error::Overflow)
}

fn shortest_path(
    nodes: &[Node],
    edges: &[RoadEdge],
    flow: &[u64],
    origin: u32,
    destination: u32,
    mode: u8,
) -> Result<Option<Vec<usize>>, Error> {
    let n = nodes.len();
    let mut dist = vec![u64::MAX; n];
    let mut prev = vec![None; n];
    let mut used = vec![false; n];
    let oi = nodes
        .binary_search_by_key(&origin, |x| x.id)
        .map_err(|_| Error::MissingNode)?;
    let di = nodes
        .binary_search_by_key(&destination, |x| x.id)
        .map_err(|_| Error::MissingNode)?;
    dist[oi] = 0;
    for _ in 0..n {
        let Some(u) = (0..n)
            .filter(|i| !used[*i])
            .min_by_key(|i| (dist[*i], nodes[*i].id))
        else {
            break;
        };
        if dist[u] == u64::MAX {
            break;
        }
        if u == di {
            break;
        }
        used[u] = true;
        for (ei, e) in edges.iter().enumerate() {
            if !e.open
                || e.from != nodes[u].id
                || e.mode_mask & mode == 0
                || flow[ei] >= u64::from(e.capacity_people)
            {
                continue;
            }
            let v = nodes
                .binary_search_by_key(&e.to, |x| x.id)
                .map_err(|_| Error::MissingNode)?;
            if used[v] {
                continue;
            }
            let cost = congested_cost(e.free_flow_seconds, e.capacity_people, flow[ei])?;
            let alt = dist[u].checked_add(cost).ok_or(Error::Overflow)?;
            if alt < dist[v]
                || (alt == dist[v]
                    && prev[v]
                        .map(|old: usize| edges[old].id > e.id)
                        .unwrap_or(true))
            {
                dist[v] = alt;
                prev[v] = Some(ei);
            }
        }
    }
    if dist[di] == u64::MAX {
        return Ok(None);
    }
    let mut path = Vec::new();
    let mut at = di;
    while at != oi {
        let Some(ei) = prev[at] else { return Ok(None) };
        path.push(ei);
        at = nodes
            .binary_search_by_key(&edges[ei].from, |x| x.id)
            .map_err(|_| Error::MissingNode)?;
        if path.len() > n {
            return Err(Error::InvalidEdge);
        }
    }
    path.reverse();
    Ok(Some(path))
}

/// Checks topological reachability while ignoring residual capacity so that a saturated
/// cut is reported separately from a physically closed/mode-incompatible network.
fn has_open_route(
    nodes: &[Node],
    edges: &[RoadEdge],
    origin: u32,
    destination: u32,
    mode: u8,
) -> Result<bool, Error> {
    let mut reached = vec![false; nodes.len()];
    let start = nodes
        .binary_search_by_key(&origin, |n| n.id)
        .map_err(|_| Error::MissingNode)?;
    let target = nodes
        .binary_search_by_key(&destination, |n| n.id)
        .map_err(|_| Error::MissingNode)?;
    let mut queue = Vec::with_capacity(nodes.len());
    reached[start] = true;
    queue.push(start);
    let mut cursor = 0;
    while cursor < queue.len() {
        let u = queue[cursor];
        cursor += 1;
        if u == target {
            return Ok(true);
        }
        for edge in edges {
            if !edge.open || edge.from != nodes[u].id || edge.mode_mask & mode == 0 {
                continue;
            }
            let v = nodes
                .binary_search_by_key(&edge.to, |n| n.id)
                .map_err(|_| Error::MissingNode)?;
            if !reached[v] {
                reached[v] = true;
                queue.push(v);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base(capacity: u32, people: u32) -> Opening {
        Opening {
            revision: 1,
            nodes: vec![
                Node { id: 1, city_id: 1 },
                Node { id: 2, city_id: 1 },
                Node { id: 3, city_id: 1 },
            ],
            edges: vec![
                RoadEdge {
                    id: 10,
                    from: 1,
                    to: 2,
                    city_from: 1,
                    city_to: 1,
                    free_flow_seconds: 10,
                    capacity_people: capacity,
                    mode_mask: 1,
                    open: true,
                    boundary_source: None,
                },
                RoadEdge {
                    id: 11,
                    from: 1,
                    to: 3,
                    city_from: 1,
                    city_to: 1,
                    free_flow_seconds: 20,
                    capacity_people: 20,
                    mode_mask: 1,
                    open: true,
                    boundary_source: None,
                },
                RoadEdge {
                    id: 12,
                    from: 3,
                    to: 2,
                    city_from: 1,
                    city_to: 1,
                    free_flow_seconds: 20,
                    capacity_people: 20,
                    mode_mask: 1,
                    open: true,
                    boundary_source: None,
                },
            ],
            entrances: vec![
                Entrance {
                    id: 1,
                    owner: 1,
                    node: 1,
                    mode_mask: 1,
                    authorized: true,
                },
                Entrance {
                    id: 2,
                    owner: 2,
                    node: 2,
                    mode_mask: 1,
                    authorized: true,
                },
            ],
            demands: vec![OdDemand {
                source_id: 7,
                origin_entrance: 1,
                destination_entrance: 2,
                mode: 1,
                people,
            }],
        }
    }
    #[test]
    fn nearby_geometry_without_granted_entrance_does_not_create_access() {
        let mut x = base(20, 5);
        x.entrances[0].authorized = false;
        assert_eq!(x.assign(), Err(Error::UnauthorizedEntrance));
    }
    #[test]
    fn entrance_must_allow_the_demand_mode() {
        let mut x = base(20, 5);
        x.entrances[0].mode_mask = 2;
        assert_eq!(x.assign(), Err(Error::UnauthorizedEntrance));
    }
    #[test]
    fn closed_directed_road_changes_reachable_assignment() {
        let mut x = base(20, 15);
        x.edges[0].open = false;
        let a = x.assign().unwrap();
        assert_eq!(a.served_people, 15);
        assert!(!a.edge_flows.iter().any(|e| e.edge_id == 10));
    }
    #[test]
    fn capacity_limits_served_people_and_expansion_changes_cost() {
        let mut x = base(5, 10);
        let a = x.assign().unwrap();
        assert_eq!(a.served_people, 10);
        assert_eq!(a.unserved_people, 0);
        assert!(
            a.edge_flows
                .iter()
                .find(|e| e.edge_id == 10)
                .unwrap()
                .people
                <= 5
        );
        let before = a
            .edge_flows
            .iter()
            .find(|e| e.edge_id == 10)
            .unwrap()
            .cost_seconds;
        x.revision += 1;
        x.edges[0].capacity_people = 20;
        let b = x.assign().unwrap();
        assert!(
            b.edge_flows
                .iter()
                .find(|e| e.edge_id == 10)
                .unwrap()
                .cost_seconds
                < before
        );
        assert_ne!(a.network_revision, b.network_revision);
    }
    #[test]
    fn no_path_is_reported_as_unserved() {
        let mut x = base(20, 4);
        x.edges.iter_mut().for_each(|e| e.open = false);
        let a = x.assign().unwrap();
        assert_eq!(a.unserved_people, 4);
        assert_eq!(a.served_people, 0);
    }
    #[test]
    fn duplicate_demand_and_boundary_sources_are_rejected() {
        let mut x = base(20, 2);
        x.demands.push(x.demands[0]);
        assert_eq!(x.validate(), Err(Error::DuplicateDemandSource));
        let mut y = base(20, 2);
        y.nodes[1].city_id = 2;
        y.nodes[2].city_id = 2;
        y.edges[0].city_to = 2;
        y.edges[0].boundary_source = Some(9);
        y.edges[1].city_to = 2;
        y.edges[1].boundary_source = Some(9);
        y.edges[2].city_from = 2;
        y.edges[2].city_to = 2;
        assert_eq!(y.validate(), Err(Error::DuplicateBoundarySource));
    }
    #[test]
    fn boundary_transit_is_counted_once_under_its_source_key() {
        let mut x = base(20, 7);
        x.nodes[1].city_id = 2;
        x.edges[0].city_to = 2;
        x.edges[0].boundary_source = Some(900);
        x.edges[2].city_to = 2;
        x.edges[2].boundary_source = Some(901);
        let result = x.assign().unwrap();
        assert_eq!(
            result.boundary_flows,
            vec![BoundaryFlow {
                source_id: 900,
                people: 7
            }]
        );
    }
    #[test]
    fn final_route_cost_matches_final_edge_cost_after_loading() {
        let mut x = base(20, 20);
        x.demands[0].people = 20;
        let result = x.assign().unwrap();
        let edge_cost = result
            .edge_flows
            .iter()
            .find(|e| e.edge_id == 10)
            .unwrap()
            .cost_seconds;
        assert_eq!(edge_cost, 11);
        assert_eq!(
            result.demand_outcomes[0].mean_travel_seconds,
            Some(edge_cost)
        );
    }
    #[test]
    fn same_node_entrances_are_zero_cost_reachable() {
        let mut x = base(20, 9);
        x.entrances[1].node = x.entrances[0].node;
        let result = x.assign().unwrap();
        assert_eq!(result.served_people, 9);
        assert_eq!(result.unserved_people, 0);
        assert_eq!(result.demand_outcomes[0].mean_travel_seconds, Some(0));
        assert!(result.edge_flows.iter().all(|e| e.people == 0));
        assert_eq!(result.path_receipts.len(), 1);
        assert!(result.path_receipts[0].edge_ids.is_empty());
        assert_eq!(result.path_receipts[0].people, 9);
    }
    #[test]
    fn edge_city_labels_must_match_the_cities_of_both_nodes() {
        let mut x = base(20, 3);
        x.nodes[2].city_id = 2;
        x.edges[1].city_to = 1;
        assert_eq!(x.validate(), Err(Error::InvalidEdge));
    }
    #[test]
    fn assignment_reports_capacity_closed_route_and_budget_failures_separately() {
        let mut full = base(1, 4);
        full.edges.truncate(1);
        let capacity = full.assign().unwrap();
        assert_eq!(
            capacity.unserved[0].reason,
            UnservedReason::CapacityExhausted
        );
        let mut closed = base(1, 4);
        closed.edges.iter_mut().for_each(|e| e.open = false);
        let no_route = closed.assign().unwrap();
        assert_eq!(no_route.unserved[0].reason, UnservedReason::NoOpenRoute);

        let mut budget = base(1, 2);
        budget.edges[1].capacity_people = 1_000;
        budget.edges[2].capacity_people = 1_000;
        let demand_template = budget.demands[0];
        budget.demands = (0..MAX_DEMANDS)
            .map(|index| OdDemand {
                source_id: index as u64 + 1,
                people: if index == 0 { 2 } else { 1 },
                ..demand_template
            })
            .collect();
        // First cohort splits over the saturated direct road and alternate path (2 steps).
        // The remaining cohorts each use one path allocation; 512 total demand groups would
        // need 513 steps, so the final group must expose the computation-budget shortfall.
        let result = budget.assign().unwrap();
        assert_eq!(result.routing_steps as usize, MAX_ASSIGNMENT_STEPS);
        assert_eq!(
            result.unserved.last().unwrap().reason,
            UnservedReason::AssignmentBudgetExhausted
        );
    }
    #[test]
    fn input_fingerprint_detects_same_revision_changes_and_ignores_record_order() {
        let x = base(20, 8);
        let result = x.assign().unwrap();
        assert!(result.matches(&x));
        let mut reordered = x.clone();
        reordered.edges.reverse();
        reordered.nodes.reverse();
        reordered.entrances.reverse();
        assert!(result.matches(&reordered));
        let mut changed = x;
        changed.edges[0].capacity_people += 1;
        assert!(!result.matches(&changed));
        changed = reordered;
        changed.entrances[0].authorized = !changed.entrances[0].authorized;
        assert!(!result.matches(&changed));
    }
    #[test]
    fn identical_input_produces_identical_aggregates() {
        let x = base(20, 18);
        assert_eq!(x.assign().unwrap(), x.assign().unwrap());
    }

    #[test]
    fn path_receipts_are_stable_edge_ids_and_verify_all_output_fields() {
        let x = base(20, 8);
        let result = x.assign().unwrap();
        assert!(result.verify(&x).is_ok());
        for outcome in &result.demand_outcomes {
            let receipt_total: u64 = result
                .path_receipts
                .iter()
                .filter(|path| path.source_id == outcome.source_id)
                .map(|path| u64::from(path.people))
                .sum();
            assert_eq!(receipt_total, u64::from(outcome.served_people));
        }
        for edge_flow in &result.edge_flows {
            let receipt_total: u64 = result
                .path_receipts
                .iter()
                .filter(|path| path.edge_ids.contains(&edge_flow.edge_id))
                .map(|path| u64::from(path.people))
                .sum();
            assert_eq!(receipt_total, edge_flow.people);
        }
        for boundary in &result.boundary_flows {
            let receipt_total: u64 = result
                .path_receipts
                .iter()
                .filter(|path| {
                    path.edge_ids.iter().any(|edge_id| {
                        x.edges.iter().any(|edge| {
                            edge.id == *edge_id && edge.boundary_source == Some(boundary.source_id)
                        })
                    })
                })
                .map(|path| u64::from(path.people))
                .sum();
            assert_eq!(receipt_total, boundary.people);
        }
        let mut reordered = x.clone();
        reordered.edges.reverse();
        assert_eq!(
            result.path_receipts,
            reordered.assign().unwrap().path_receipts
        );

        let mut missing_path = result.clone();
        missing_path.path_receipts.clear();
        assert_eq!(missing_path.verify(&x), Err(Error::Fingerprint));

        let mut forged_flow = result;
        forged_flow.edge_flows[0].people += 1;
        assert_eq!(forged_flow.verify(&x), Err(Error::Fingerprint));
    }
}
