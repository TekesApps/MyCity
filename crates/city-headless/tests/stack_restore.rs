//! Regression for the actual ce9efd v2 source-anchor failure. Default test stack.
//! The old rules are deliberate compatibility input; production Sim still fences its source.
use std::{fs, path::PathBuf, process::Command};
#[test]
fn original_full_v2_anchor_opens_on_default_stack_and_preserves_old_engine_bytes() {
    let root = std::env::temp_dir().join(format!("mycity-stack-restore-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/regressions/t13a-legacy-stack-world.zip");
    let status = Command::new("unzip")
        .arg("-q")
        .arg(fixture)
        .arg("-d")
        .arg(&root)
        .status()
        .unwrap();
    assert!(status.success());
    let origin: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("fixture-origin.json")).unwrap()).unwrap();
    let expected = fs::read(root.join("expected-engine.json")).unwrap();
    assert_eq!(
        mycity_core::sha256(&expected),
        origin["expected_engine_sha256"]
    );
    let store = mycity_storage::Store::open(&root, origin["rules"].as_str().unwrap()).unwrap();
    assert_eq!(store.committed_sequence(), 97);
    let engine = store.engine().unwrap();
    assert_eq!(engine.export_checkpoint().unwrap(), expected);
    // Heap accounting includes the boxed World exactly once, plus Engine's pointer.
    let world_owned = engine.world().layout()["owned_capacity_bytes"]
        .as_u64()
        .unwrap() as usize;
    assert!(engine.owned_capacity_bytes() >= world_owned + std::mem::size_of_val(engine));
    drop(store);
    fs::remove_dir_all(root).unwrap();
}
