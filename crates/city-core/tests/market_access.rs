use mycity_core::{
    market_access::{
        plan_accessible_day, resolve_delivery_quotes, resolve_delivery_quotes_from_snapshot,
        AccessStatus, Error, FeePolicyInput, TransportSnapshotRef, TripPartition,
    },
    town_economy::{
        AccountSnapshot, DeliveryQuote, HouseholdGroup, InventorySnapshot, MarketInput,
        ShopSnapshot,
    },
    transport::{Assignment, Entrance, Node, OdDemand, Opening, RoadEdge},
    units::{Money, Quantity},
};

fn market() -> MarketInput {
    MarketInput {
        schema: "mycity-town-market-v0.1".into(),
        world: 1,
        branch: 1,
        revision: 4,
        day: 28,
        operation_id: 20,
        households: vec![HouseholdGroup {
            id: 1,
            account: 101,
            households: 1,
            need_good: 0,
            need_per_household: Quantity::new(100).unwrap(),
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
            quantity: Quantity::new(100).unwrap(),
            spendable_basis: Money::ZERO,
            origin_quantity: Quantity::new(100).unwrap(),
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
            reachable: true,
            route_source: 88,
            freight_account: 303,
            utility_account: 303,
            available_capacity: Quantity::new(1000).unwrap(),
            freight_per_q: Money::ZERO,
            utility_surcharge_per_q: Money::ZERO,
        }],
    }
}

fn opening() -> Opening {
    Opening {
        revision: 9,
        nodes: vec![Node { id: 1, city_id: 1 }, Node { id: 2, city_id: 1 }],
        edges: vec![
            RoadEdge {
                id: 10,
                from: 1,
                to: 2,
                city_from: 1,
                city_to: 1,
                free_flow_seconds: 10,
                capacity_people: 10,
                mode_mask: 1,
                open: true,
                boundary_source: None,
            },
            RoadEdge {
                id: 11,
                from: 2,
                to: 1,
                city_from: 1,
                city_to: 1,
                free_flow_seconds: 10,
                capacity_people: 10,
                mode_mask: 1,
                open: true,
                boundary_source: None,
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
                people: 5,
            },
            OdDemand {
                source_id: 5002,
                origin_entrance: 200,
                destination_entrance: 100,
                mode: 1,
                people: 5,
            },
        ],
    }
}

fn partition(trips: u32) -> TripPartition {
    TripPartition {
        quote_id: 7001,
        outbound_source: 5001,
        return_source: 5002,
        trip_count: trips,
        goods_per_trip: Quantity::new(20).unwrap(),
    }
}

fn tariff() -> FeePolicyInput {
    FeePolicyInput {
        provider_account: 303,
        cents_per_travel_second: 1,
    }
}

#[test]
fn round_trip_access_caps_goods_and_uses_external_tariff_for_pure_plan() {
    let input = market();
    let network = opening();
    let assignment = network.assign().unwrap();
    let plan =
        plan_accessible_day(&input, &network, &assignment, &[partition(5)], tariff()).unwrap();
    assert_eq!(plan.delivery_access[0].status, AccessStatus::Reachable);
    assert_eq!(
        plan.delivery_access[0].accessible_quantity,
        Quantity::new(100).unwrap()
    );
    assert_eq!(plan.batch.planned_freight, Money::new(100).unwrap());
    assert_eq!(
        plan.batch
            .lines
            .iter()
            .filter_map(|l| match l {
                mycity_core::town_economy::PlanLine::Sale { quantity, .. } =>
                    Some(quantity.value()),
                _ => None,
            })
            .sum::<i64>(),
        100
    );
    assert_eq!(plan.transport_fingerprint, assignment.input_fingerprint);
}

#[test]
fn resolver_returns_dynamic_quotes_and_audit_without_mutating_input_or_planning() {
    let input = market();
    let network = opening();
    let assignment = network.assign().unwrap();
    let resolved =
        resolve_delivery_quotes(&input, &network, &assignment, &[partition(5)], tariff()).unwrap();

    assert_eq!(input.delivery[0].route_source, 88);
    assert_eq!(
        input.delivery[0].available_capacity,
        Quantity::new(1000).unwrap()
    );
    assert!(resolved.delivery_quotes[0].reachable);
    assert_eq!(resolved.delivery_quotes[0].route_source, 7001);
    assert_eq!(
        resolved.delivery_quotes[0].available_capacity,
        Quantity::new(100).unwrap()
    );
    assert_eq!(resolved.delivery_quotes[0].freight_account, 303);
    assert_eq!(
        resolved.delivery_quotes[0].freight_per_q,
        Money::new(1).unwrap()
    );
    assert_eq!(
        resolved.delivery_quotes[0].utility_account,
        input.delivery[0].utility_account
    );
    assert_eq!(
        resolved.delivery_quotes[0].utility_surcharge_per_q,
        input.delivery[0].utility_surcharge_per_q
    );
    assert_eq!(resolved.delivery_access[0].outbound_source, 5001);
    assert_eq!(resolved.delivery_access[0].return_source, 5002);
    assert_eq!(resolved.delivery_access[0].allocated_trips, 5);
    assert_eq!(resolved.transport_revision, network.revision);
    assert_eq!(resolved.transport_fingerprint, assignment.input_fingerprint);
    assert_ne!(resolved.source_digest, [0; 32]);
}

#[test]
fn snapshot_resolver_binds_compact_generation_ref_and_profile_without_expanding_command_data() {
    let input = market();
    let network = opening();
    let assignment = network.assign().unwrap();
    let source_ref = TransportSnapshotRef { generation: 12 };
    assert!(serde_json::to_vec(&(7_u32, &source_ref)).unwrap().len() < 128);
    let resolved = resolve_delivery_quotes_from_snapshot(
        7,
        source_ref.clone(),
        &input,
        &network,
        &assignment,
        &[partition(5)],
        tariff(),
    )
    .unwrap();
    assert_eq!(resolved.profile_id, Some(7));
    assert_eq!(resolved.source_snapshot, Some(source_ref));

    let another_generation = resolve_delivery_quotes_from_snapshot(
        7,
        TransportSnapshotRef { generation: 13 },
        &input,
        &network,
        &assignment,
        &[partition(5)],
        tariff(),
    )
    .unwrap();
    assert_ne!(resolved.source_digest, another_generation.source_digest);

    assert_eq!(
        resolve_delivery_quotes_from_snapshot(
            0,
            TransportSnapshotRef { generation: 12 },
            &input,
            &network,
            &assignment,
            &[partition(5)],
            tariff()
        ),
        Err(Error::InvalidSnapshotRef)
    );
    assert_eq!(
        resolve_delivery_quotes_from_snapshot(
            7,
            TransportSnapshotRef { generation: 0 },
            &input,
            &network,
            &assignment,
            &[partition(5)],
            tariff()
        ),
        Err(Error::InvalidSnapshotRef)
    );
}

#[test]
fn resolver_rejects_stale_and_forged_assignments_and_wrong_entrance_owner() {
    let input = market();
    let mut network = opening();
    let assignment = network.assign().unwrap();
    network.edges[0].capacity_people += 1;
    assert_eq!(
        resolve_delivery_quotes(&input, &network, &assignment, &[partition(5)], tariff()),
        Err(Error::StaleAssignment)
    );

    network = opening();
    let mut forged = network.assign().unwrap();
    forged.path_receipts[0].people += 1;
    assert_eq!(
        resolve_delivery_quotes(&input, &network, &forged, &[partition(5)], tariff()),
        Err(Error::InvalidAssignment)
    );

    network.entrances[0].owner = 999;
    let owner_mismatch = network.assign().unwrap();
    assert_eq!(
        resolve_delivery_quotes(&input, &network, &owner_mismatch, &[partition(5)], tariff()),
        Err(Error::InvalidPartition)
    );

    network = opening();
    let authorized_assignment = network.assign().unwrap();
    network.entrances[0].authorized = false;
    assert_eq!(
        resolve_delivery_quotes(
            &input,
            &network,
            &authorized_assignment,
            &[partition(5)],
            tariff()
        ),
        Err(Error::StaleAssignment)
    );
}

#[test]
fn no_return_service_means_no_accessible_sales_or_fake_completion() {
    let input = market();
    let mut network = opening();
    network.edges[1].open = false;
    let assignment = network.assign().unwrap();
    let plan =
        plan_accessible_day(&input, &network, &assignment, &[partition(0)], tariff()).unwrap();
    assert_eq!(
        plan.delivery_access[0].status,
        AccessStatus::NoReturnService
    );
    assert_eq!(plan.delivery_access[0].accessible_quantity, Quantity::ZERO);
    assert_eq!(plan.batch.planned_goods_payments, Money::ZERO);
    assert_eq!(plan.batch.reference_need, Quantity::new(100).unwrap());
    assert_eq!(plan.batch.market_demand, Quantity::ZERO);
    assert_eq!(plan.batch.unmet_market_demand, Quantity::ZERO);
    // No executable offer means no market demand is quoted, but baseline need remains unmet.
    assert_eq!(plan.batch.unmet_need, Quantity::new(100).unwrap());
}

#[test]
fn same_revision_graph_edit_invalidates_assignment() {
    let input = market();
    let mut network = opening();
    let assignment = network.assign().unwrap();
    network.edges[0].capacity_people += 1;
    assert_eq!(
        plan_accessible_day(&input, &network, &assignment, &[partition(5)], tariff()),
        Err(Error::StaleAssignment)
    );
}

#[test]
fn explicit_entrance_owner_binding_rejects_wrong_account() {
    let input = market();
    let mut network = opening();
    network.entrances[0].owner = 999;
    let assignment = network.assign().unwrap();
    assert_eq!(
        plan_accessible_day(&input, &network, &assignment, &[partition(5)], tariff()),
        Err(Error::InvalidPartition)
    );
}

#[test]
fn unauthorized_entrance_is_rejected_even_when_geometrically_at_the_road_node() {
    let mut network = opening();
    network.entrances[0].authorized = false;
    assert!(network.assign().is_err());
}

#[test]
fn cross_city_market_roundtrip_reports_only_its_actual_boundary_sources() {
    let input = market();
    let mut network = opening();
    network.nodes[1].city_id = 2;
    network.nodes.push(Node { id: 3, city_id: 2 });
    network.edges[0].city_to = 2;
    network.edges[0].boundary_source = Some(60);
    network.edges[0].capacity_people = 2;
    network.edges[1].city_from = 2;
    network.edges[1].boundary_source = Some(61);
    network.edges.push(RoadEdge {
        id: 12,
        from: 1,
        to: 3,
        city_from: 1,
        city_to: 2,
        free_flow_seconds: 20,
        capacity_people: 10,
        mode_mask: 1,
        open: true,
        boundary_source: Some(62),
    });
    network.edges.push(RoadEdge {
        id: 13,
        from: 3,
        to: 2,
        city_from: 2,
        city_to: 2,
        free_flow_seconds: 20,
        capacity_people: 10,
        mode_mask: 1,
        open: true,
        boundary_source: None,
    });
    let assignment = network.assign().unwrap();
    let plan =
        plan_accessible_day(&input, &network, &assignment, &[partition(5)], tariff()).unwrap();
    assert_eq!(
        plan.delivery_access[0].outbound_boundary_sources,
        vec![60, 62]
    );
    assert_eq!(plan.delivery_access[0].return_boundary_sources, vec![61]);
    assert_eq!(
        plan.delivery_access[0].accessible_quantity,
        Quantity::new(100).unwrap()
    );
}

#[test]
fn unrelated_cross_city_od_does_not_block_local_market_access() {
    let input = market();
    let mut network = opening();
    network
        .nodes
        .extend([Node { id: 3, city_id: 1 }, Node { id: 4, city_id: 2 }]);
    network.edges.extend([
        RoadEdge {
            id: 12,
            from: 3,
            to: 4,
            city_from: 1,
            city_to: 2,
            free_flow_seconds: 7,
            capacity_people: 4,
            mode_mask: 1,
            open: true,
            boundary_source: Some(70),
        },
        RoadEdge {
            id: 13,
            from: 4,
            to: 3,
            city_from: 2,
            city_to: 1,
            free_flow_seconds: 7,
            capacity_people: 4,
            mode_mask: 1,
            open: true,
            boundary_source: Some(71),
        },
    ]);
    network.entrances.extend([
        Entrance {
            id: 300,
            owner: 303,
            node: 3,
            mode_mask: 1,
            authorized: true,
        },
        Entrance {
            id: 400,
            owner: 303,
            node: 4,
            mode_mask: 1,
            authorized: true,
        },
    ]);
    network.demands.extend([
        OdDemand {
            source_id: 6001,
            origin_entrance: 300,
            destination_entrance: 400,
            mode: 1,
            people: 2,
        },
        OdDemand {
            source_id: 6002,
            origin_entrance: 400,
            destination_entrance: 300,
            mode: 1,
            people: 2,
        },
    ]);
    let assignment = network.assign().unwrap();
    assert_eq!(assignment.boundary_flows.len(), 2);
    let plan =
        plan_accessible_day(&input, &network, &assignment, &[partition(5)], tariff()).unwrap();
    assert_eq!(
        plan.delivery_access[0].accessible_quantity,
        Quantity::new(100).unwrap()
    );
    assert!(plan.delivery_access[0].outbound_boundary_sources.is_empty());
}

#[test]
fn missing_or_forged_path_receipt_cannot_supply_access_capacity() {
    let input = market();
    let network = opening();
    let assignment = network.assign().unwrap();
    let mut missing = assignment.clone();
    missing.path_receipts.pop();
    assert_eq!(
        plan_accessible_day(&input, &network, &missing, &[partition(5)], tariff()),
        Err(Error::InvalidAssignment)
    );
    let mut forged = assignment;
    forged.path_receipts[0].people += 1;
    assert_eq!(
        plan_accessible_day(&input, &network, &forged, &[partition(5)], tariff()),
        Err(Error::InvalidAssignment)
    );
    let mut mismatched_edge_flow = network.assign().unwrap();
    mismatched_edge_flow.edge_flows[0].people += 1;
    assert_eq!(
        plan_accessible_day(
            &input,
            &network,
            &mismatched_edge_flow,
            &[partition(5)],
            tariff()
        ),
        Err(Error::InvalidAssignment)
    );
}

#[test]
fn same_od_is_shared_across_goods_without_reissuing_served_trips() {
    let mut input = market();
    input.inventories.push(InventorySnapshot {
        stock_id: 901,
        owner: 202,
        good: 1,
        quantity: Quantity::new(100).unwrap(),
        spendable_basis: Money::ZERO,
        origin_quantity: Quantity::new(100).unwrap(),
        origin_basis: Money::ZERO,
        offset: Quantity::ZERO,
    });
    input.shops.push(ShopSnapshot {
        shop_id: 2,
        account: 202,
        stock_id: 901,
        good: 1,
        shelf_price_per_q: Money::new(1).unwrap(),
    });
    input.delivery.push(DeliveryQuote {
        quote_id: 7002,
        shop: 2,
        household_group: 1,
        good: 1,
        reachable: false,
        route_source: 89,
        freight_account: 303,
        utility_account: 303,
        available_capacity: Quantity::ZERO,
        freight_per_q: Money::ZERO,
        utility_surcharge_per_q: Money::ZERO,
    });
    let network = opening();
    let assignment: Assignment = network.assign().unwrap();
    let p2 = TripPartition {
        quote_id: 7002,
        outbound_source: 5001,
        return_source: 5002,
        trip_count: 3,
        goods_per_trip: Quantity::new(1).unwrap(),
    };
    let plan =
        plan_accessible_day(&input, &network, &assignment, &[partition(3), p2], tariff()).unwrap();
    let allocated: u32 = plan.delivery_access.iter().map(|x| x.allocated_trips).sum();
    assert_eq!(allocated, 5);
    assert_eq!(
        plan.delivery_access
            .iter()
            .find(|x| x.quote_id == 7001)
            .unwrap()
            .allocated_trips,
        3
    );
    assert_eq!(
        plan.delivery_access
            .iter()
            .find(|x| x.quote_id == 7002)
            .unwrap()
            .allocated_trips,
        2
    );
}

#[test]
fn resolver_partitions_shared_od_service_once_across_multiple_goods() {
    let mut input = market();
    input.inventories.push(InventorySnapshot {
        stock_id: 901,
        owner: 202,
        good: 1,
        quantity: Quantity::new(100).unwrap(),
        spendable_basis: Money::ZERO,
        origin_quantity: Quantity::new(100).unwrap(),
        origin_basis: Money::ZERO,
        offset: Quantity::ZERO,
    });
    input.shops.push(ShopSnapshot {
        shop_id: 2,
        account: 202,
        stock_id: 901,
        good: 1,
        shelf_price_per_q: Money::new(1).unwrap(),
    });
    input.delivery.push(DeliveryQuote {
        quote_id: 7002,
        shop: 2,
        household_group: 1,
        good: 1,
        reachable: false,
        route_source: 89,
        freight_account: 303,
        utility_account: 303,
        available_capacity: Quantity::ZERO,
        freight_per_q: Money::ZERO,
        utility_surcharge_per_q: Money::ZERO,
    });
    let network = opening();
    let assignment = network.assign().unwrap();
    let second = TripPartition {
        quote_id: 7002,
        outbound_source: 5001,
        return_source: 5002,
        trip_count: 3,
        goods_per_trip: Quantity::new(1).unwrap(),
    };
    let resolved = resolve_delivery_quotes(
        &input,
        &network,
        &assignment,
        &[partition(3), second],
        tariff(),
    )
    .unwrap();
    assert_eq!(
        resolved
            .delivery_access
            .iter()
            .map(|row| row.allocated_trips)
            .sum::<u32>(),
        5
    );
    assert_eq!(
        resolved
            .delivery_quotes
            .iter()
            .map(|quote| quote.available_capacity.value())
            .sum::<i64>(),
        62
    );
}

#[test]
fn duplicate_quote_partition_is_rejected() {
    let mut input = market();
    input.delivery.push(input.delivery[0]);
    let network = opening();
    let assignment = network.assign().unwrap();
    assert_eq!(
        plan_accessible_day(
            &input,
            &network,
            &assignment,
            &[partition(1), partition(1)],
            tariff()
        ),
        Err(Error::DuplicatePartition)
    );
}

#[test]
fn capacity_shortfall_yields_only_served_roundtrips_and_unmet_market_demand() {
    let mut input = market();
    input.households[0].need_per_household = Quantity::new(200).unwrap();
    input.inventories[0].quantity = Quantity::new(200).unwrap();
    input.inventories[0].origin_quantity = Quantity::new(200).unwrap();
    let mut network = opening();
    network.demands.iter_mut().for_each(|d| d.people = 10);
    network.edges.iter_mut().for_each(|e| e.capacity_people = 5);
    let assignment = network.assign().unwrap();
    let plan =
        plan_accessible_day(&input, &network, &assignment, &[partition(10)], tariff()).unwrap();
    assert_eq!(
        plan.delivery_access[0].status,
        AccessStatus::PartiallyServed
    );
    assert_eq!(plan.delivery_access[0].allocated_trips, 5);
    assert_eq!(
        plan.delivery_access[0].accessible_quantity,
        Quantity::new(100).unwrap()
    );
    assert_eq!(plan.batch.unmet_need, Quantity::new(100).unwrap());
}

#[test]
fn expanded_capacity_allows_more_sales_from_same_od_demand() {
    let mut input = market();
    input.households[0].need_per_household = Quantity::new(200).unwrap();
    input.inventories[0].quantity = Quantity::new(200).unwrap();
    input.inventories[0].origin_quantity = Quantity::new(200).unwrap();
    let mut network = opening();
    network.demands.iter_mut().for_each(|d| d.people = 10);
    network
        .edges
        .iter_mut()
        .for_each(|e| e.capacity_people = 10);
    let assignment = network.assign().unwrap();
    let plan =
        plan_accessible_day(&input, &network, &assignment, &[partition(10)], tariff()).unwrap();
    assert_eq!(plan.delivery_access[0].allocated_trips, 10);
    assert_eq!(
        plan.delivery_access[0].accessible_quantity,
        Quantity::new(200).unwrap()
    );
    assert_eq!(plan.batch.unmet_need, Quantity::ZERO);
}
