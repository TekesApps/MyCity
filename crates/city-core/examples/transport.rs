use mycity_core::transport::{Assignment, Entrance, Node, OdDemand, Opening, RoadEdge};
use std::env;

fn scenario(name: &str) -> Opening {
    let mut roads = vec![
        RoadEdge {
            id: 1,
            from: 1,
            to: 2,
            city_from: 1,
            city_to: 1,
            free_flow_seconds: 60,
            capacity_people: 20,
            mode_mask: 1,
            open: true,
            boundary_source: None,
        },
        RoadEdge {
            id: 2,
            from: 1,
            to: 3,
            city_from: 1,
            city_to: 1,
            free_flow_seconds: 90,
            capacity_people: 20,
            mode_mask: 1,
            open: true,
            boundary_source: None,
        },
        RoadEdge {
            id: 3,
            from: 3,
            to: 2,
            city_from: 1,
            city_to: 1,
            free_flow_seconds: 90,
            capacity_people: 20,
            mode_mask: 1,
            open: true,
            boundary_source: None,
        },
    ];
    let people = match name {
        "normal" => 8,
        "closed" => 25,
        "congested" => 20,
        "unreachable" => 5,
        _ => panic!("scenario must be normal|closed|congested|unreachable"),
    };
    if name == "closed" {
        roads[0].open = false
    }
    if name == "unreachable" {
        roads.iter_mut().for_each(|e| e.open = false)
    }
    Opening {
        revision: match name {
            "closed" => 2,
            "congested" => 3,
            "unreachable" => 4,
            _ => 1,
        },
        nodes: vec![
            Node { id: 1, city_id: 1 },
            Node { id: 2, city_id: 1 },
            Node { id: 3, city_id: 1 },
        ],
        edges: roads,
        entrances: vec![
            Entrance {
                id: 1,
                owner: 10,
                node: 1,
                mode_mask: 1,
                authorized: true,
            },
            Entrance {
                id: 2,
                owner: 20,
                node: 2,
                mode_mask: 1,
                authorized: true,
            },
        ],
        demands: vec![OdDemand {
            source_id: 100,
            origin_entrance: 1,
            destination_entrance: 2,
            mode: 1,
            people,
        }],
    }
}
fn main() {
    let args: Vec<_> = env::args().collect();
    let name = args.get(1).map(String::as_str).unwrap_or("normal");
    let result: Assignment = scenario(name)
        .assign()
        .expect("valid bounded traffic scenario");
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}
