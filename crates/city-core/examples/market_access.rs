//! End-to-end transport access -> pure town market plan demonstration.
use mycity_core::{
    market_access::{plan_accessible_day, FeePolicyInput, TripPartition},
    town_economy::{
        AccountSnapshot, DeliveryQuote, HouseholdGroup, InventorySnapshot, MarketInput, PlanLine,
        ShopSnapshot,
    },
    transport::{Entrance, Node, OdDemand, Opening, RoadEdge},
    units::{Money, Quantity},
};

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scenario = std::env::args().nth(1).unwrap_or_else(|| "normal".into());
    if ![
        "normal",
        "road_closed",
        "bottleneck",
        "capacity_expanded",
        "cross_city",
        "unrelated_cross_city",
    ]
    .contains(&scenario.as_str())
    {
        return Err("usage: market_access [normal|road_closed|bottleneck|capacity_expanded|cross_city|unrelated_cross_city]".into());
    }
    let capacity = match scenario.as_str() {
        "bottleneck" => 2,
        "capacity_expanded" => 20,
        _ => 10,
    };
    let trips = 10;
    let destination_city = if scenario == "cross_city" { 2 } else { 1 };
    let mut opening = Opening {
        revision: 1,
        nodes: vec![
            Node { id: 1, city_id: 1 },
            Node {
                id: 2,
                city_id: destination_city,
            },
        ],
        edges: vec![
            RoadEdge {
                id: 10,
                from: 1,
                to: 2,
                city_from: 1,
                city_to: destination_city,
                free_flow_seconds: 30,
                capacity_people: capacity,
                mode_mask: 1,
                open: true,
                boundary_source: (destination_city != 1).then_some(9100),
            },
            RoadEdge {
                id: 11,
                from: 2,
                to: 1,
                city_from: destination_city,
                city_to: 1,
                free_flow_seconds: 30,
                capacity_people: capacity,
                mode_mask: 1,
                open: true,
                boundary_source: (destination_city != 1).then_some(9200),
            },
        ],
        entrances: vec![
            Entrance {
                id: 100,
                owner: 101,
                node: 1,
                mode_mask: 1,
                authorized: true,
            },
            Entrance {
                id: 200,
                owner: 202,
                node: 2,
                mode_mask: 1,
                authorized: true,
            },
        ],
        demands: vec![
            OdDemand {
                source_id: 5001,
                origin_entrance: 100,
                destination_entrance: 200,
                mode: 1,
                people: 10,
            },
            OdDemand {
                source_id: 5002,
                origin_entrance: 200,
                destination_entrance: 100,
                mode: 1,
                people: 10,
            },
        ],
    };
    if scenario == "unrelated_cross_city" {
        opening
            .nodes
            .extend([Node { id: 3, city_id: 3 }, Node { id: 4, city_id: 4 }]);
        opening.edges.extend([
            RoadEdge {
                id: 12,
                from: 3,
                to: 4,
                city_from: 3,
                city_to: 4,
                free_flow_seconds: 40,
                capacity_people: 4,
                mode_mask: 1,
                open: true,
                boundary_source: Some(9300),
            },
            RoadEdge {
                id: 13,
                from: 4,
                to: 3,
                city_from: 4,
                city_to: 3,
                free_flow_seconds: 40,
                capacity_people: 4,
                mode_mask: 1,
                open: true,
                boundary_source: Some(9400),
            },
        ]);
        opening.entrances.extend([
            Entrance {
                id: 300,
                owner: 301,
                node: 3,
                mode_mask: 1,
                authorized: true,
            },
            Entrance {
                id: 400,
                owner: 302,
                node: 4,
                mode_mask: 1,
                authorized: true,
            },
        ]);
        opening.demands.extend([
            OdDemand {
                source_id: 6001,
                origin_entrance: 300,
                destination_entrance: 400,
                mode: 1,
                people: 4,
            },
            OdDemand {
                source_id: 6002,
                origin_entrance: 400,
                destination_entrance: 300,
                mode: 1,
                people: 4,
            },
        ]);
    }
    if scenario == "road_closed" {
        opening.edges[1].open = false;
    }
    let assignment = opening
        .assign()
        .map_err(|e| format!("transport assignment failed: {e:?}"))?;
    let input = MarketInput {
        schema: "mycity-town-market-v0.1".into(),
        world: 1,
        branch: 1,
        revision: 1,
        day: 28,
        operation_id: 10,
        households: vec![HouseholdGroup {
            id: 1,
            account: 101,
            households: 1,
            need_good: 0,
            need_per_household: Quantity::new(200).unwrap(),
            reference_price_per_q: Money::new(100).unwrap(),
        }],
        employers: vec![],
        accounts: vec![
            AccountSnapshot {
                account: 101,
                cash: Money::new(10_000).unwrap(),
                protected: Money::ZERO,
            },
            AccountSnapshot {
                account: 202,
                cash: Money::new(10_000).unwrap(),
                protected: Money::ZERO,
            },
            AccountSnapshot {
                account: 303,
                cash: Money::new(10_000).unwrap(),
                protected: Money::ZERO,
            },
        ],
        inventories: vec![InventorySnapshot {
            stock_id: 900,
            owner: 202,
            good: 0,
            quantity: Quantity::new(200).unwrap(),
            spendable_basis: Money::ZERO,
            origin_quantity: Quantity::new(200).unwrap(),
            origin_basis: Money::ZERO,
            offset: Quantity::ZERO,
        }],
        shops: vec![ShopSnapshot {
            shop_id: 2,
            account: 202,
            stock_id: 900,
            good: 0,
            shelf_price_per_q: Money::new(1).unwrap(),
        }],
        recipes: vec![],
        delivery: vec![DeliveryQuote {
            quote_id: 7001,
            shop: 2,
            household_group: 1,
            good: 0,
            reachable: false,
            route_source: 7001,
            freight_account: 303,
            utility_account: 303,
            available_capacity: Quantity::ZERO,
            freight_per_q: Money::ZERO,
            utility_surcharge_per_q: Money::ZERO,
        }],
    };
    let access = plan_accessible_day(
        &input,
        &opening,
        &assignment,
        &[TripPartition {
            quote_id: 7001,
            outbound_source: 5001,
            return_source: 5002,
            trip_count: trips,
            goods_per_trip: Quantity::new(20).unwrap(),
        }],
        FeePolicyInput {
            provider_account: 303,
            cents_per_travel_second: 1,
        },
    )
    .map_err(|e| format!("market access plan failed: {e:?}"))?;
    let sold: i64 = access
        .batch
        .lines
        .iter()
        .filter_map(|line| match line {
            PlanLine::Sale { quantity, .. } => Some(quantity.value()),
            _ => None,
        })
        .sum();
    let delivery = &access.delivery_access[0];
    let outbound_served = assignment
        .demand_outcomes
        .iter()
        .find(|x| x.source_id == 5001)
        .map_or(0, |x| x.served_people);
    let return_served = assignment
        .demand_outcomes
        .iter()
        .find(|x| x.source_id == 5002)
        .map_or(0, |x| x.served_people);
    println!("{{\"scenario\":\"{scenario}\",\"status\":\"pureplan_not_settled\",\"transport_revision\":{},\"source_digest\":\"{}\",\"outbound_served_people\":{},\"return_served_people\":{},\"outbound_boundary_sources\":{},\"return_boundary_sources\":{},\"reachable\":{},\"accessible_quantity\":{},\"reference_need\":{},\"market_demand\":{},\"sold_quantity\":{},\"unmet_market_demand\":{},\"unmet_reference_need\":{},\"fee_policy_status\":\"external_input_unintegrated\",\"fee_policy_input\":{},\"engine_commit_required\":true}}",
        access.transport_revision, hex_digest(&access.source_digest),
        outbound_served, return_served,
        serde_json::to_string(&delivery.outbound_boundary_sources)?,
        serde_json::to_string(&delivery.return_boundary_sources)?,
        delivery.accessible_quantity.value() > 0,
        delivery.accessible_quantity.value(), access.batch.reference_need.value(),
        access.batch.market_demand.value(), sold, access.batch.unmet_market_demand.value(),
        access.batch.unmet_need.value(),
        serde_json::to_string(&access.fee_policy_input)?);
    Ok(())
}
