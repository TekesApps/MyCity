//! Read an unmodified v0.5.33 world using the current compiled storage library.
#![forbid(unsafe_code)]
use mycity_storage::Store;
use serde_json::{json,Value};
fn main() {
    let args:Vec<_>=std::env::args().collect();assert_eq!(args.len(),3);
    let expected:Value=serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let mut store=Store::open(std::path::Path::new(&args[1]),"t13a-storage-growth-diagnostic-v1").unwrap();
    let actual:Value=serde_json::from_slice(&store.engine().unwrap().export_checkpoint().unwrap()).unwrap();
    assert_eq!(actual,expected["engine"]);
    for old in expected["receipts"].as_array().unwrap() {
        let id=old["operation_id"].as_u64().unwrap();
        assert_eq!(serde_json::to_value(store.receipt(id).unwrap().unwrap()).unwrap(),*old);
    }
    assert!(store.receipt(999999).unwrap().is_none());assert_eq!(store.committed_sequence(),88);
    println!("{}",json!({"old_version":"v0.5.33","world_fields_equal":true,"all_88_original_receipts_equal":true,"sequence":store.committed_sequence(),"implicit_migration":false}));
}
