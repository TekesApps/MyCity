//! Integrated resource directory boundary, independently from byte/lease budgets.
use mycity_core::{
    assembly::assemble,
    transaction::{
        Action, Caller, Engine, EngineWorkspace, Envelope, Resource, TxError, ENGINE_BYTE_BUDGET,
        MAX_RESOURCES,
    },
};

fn resources() -> Vec<Resource> {
    [
        80, 101, 102, 103, 104, 201, 202, 900, 901, 902, 903, 904, 905, 906, 907, 908,
    ]
    .into_iter()
    .map(|id| Resource::new(id, 100_000).unwrap())
    .collect()
}
fn engine(rows: Vec<Resource>) -> Result<Engine, TxError> {
    Engine::new(
        assemble(include_bytes!(
            "../../../fixtures/m0-background-joint-opening.json"
        ))
        .unwrap(),
        rows,
    )
}
#[test]
fn sixteen_resources_roundtrip_and_seventeenth_or_duplicate_cannot_enter_model() {
    assert_eq!(MAX_RESOURCES, 16);
    let mut e = engine(resources()).unwrap();
    let before = e.export_checkpoint().unwrap();
    let restored = Engine::restore_checkpoint(&before).unwrap();
    assert_eq!(restored.export_checkpoint().unwrap(), before);
    assert_eq!(
        serde_json::to_value(restored.resources()).unwrap(),
        serde_json::to_value(e.resources()).unwrap()
    );
    let mut too_many = resources();
    too_many.push(Resource::new(909, 1).unwrap());
    assert!(matches!(engine(too_many), Err(TxError::Resource)));
    let mut duplicate = resources();
    duplicate[15] = duplicate[0];
    assert!(matches!(engine(duplicate), Err(TxError::Resource)));
    let mut invalid: serde_json::Value = serde_json::from_slice(&before).unwrap();
    invalid["resources"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::to_value(Resource::new(909, 1).unwrap()).unwrap());
    assert!(Engine::restore_checkpoint(&serde_json::to_vec(&invalid).unwrap()).is_err());
    let rows = serde_json::to_value(e.resources()).unwrap();
    let envelope = |e: &Engine, id, resource, units| Envelope {
        world: e.world().view().scope.world(),
        branch: e.world().view().scope.branch(),
        operation_id: id,
        expected_revision: e.world().view().revision,
        actions: vec![Action::ReserveCapacity { resource, units }],
    };
    assert_eq!(
        e.execute(Caller::Account(4), envelope(&e, 90001, 999, 1)),
        Err(TxError::Resource)
    );
    assert_eq!(serde_json::to_value(e.resources()).unwrap(), rows);
    assert_eq!(
        e.execute(Caller::Account(4), envelope(&e, 90002, 80, 100_001)),
        Err(TxError::Capacity)
    );
    assert_eq!(serde_json::to_value(e.resources()).unwrap(), rows);
}
#[test]
fn actual_workspace_and_resource_capacity_are_reported_inside_existing_budgets() {
    let e = engine(resources()).unwrap();
    let workspace = EngineWorkspace::new().unwrap();
    let identity = workspace.identity().unwrap();
    assert!(e.owned_capacity_bytes() <= ENGINE_BYTE_BUDGET);
    eprintln!(
        "RESOURCE_CAPACITY {}",
        serde_json::json!({
            "max_resources":MAX_RESOURCES,"resource_record_bytes":std::mem::size_of::<Resource>(),
            "per_resource_backing_bytes":MAX_RESOURCES*std::mem::size_of::<Resource>(),
            "per_backing_delta_from_original_eight":(MAX_RESOURCES-8)*std::mem::size_of::<Resource>(),
            "engine_owned_bytes":e.owned_capacity_bytes(),"engine_byte_budget":ENGINE_BYTE_BUDGET,
            "workspace":identity,"scope":"actual allocated capacity inventory, not native footprint or allocator peak"
        })
    );
}
