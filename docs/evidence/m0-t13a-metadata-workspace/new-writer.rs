//! Actual new v1 writer output for an independently frozen old reader.
#![forbid(unsafe_code)]
use std::{env,fs,path::Path};
use mycity_core::transaction::{Action,Caller,Envelope};
fn main(){
 let args:Vec<_>=env::args().collect();assert_eq!(args.len(),3);
 let mut store=mycity_storage::Store::create_spatial_in_prepared_workspace(Path::new(&args[1]),
 include_bytes!("../../../fixtures/m0-e0-opening.json"),vec![],"metadata-compat-proof-v1",
 (include_bytes!("../../../fixtures/m0-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")),false,
 mycity_storage::StoreWorkspace::new().unwrap()).unwrap();
 for id in 1..=2{
 let w=store.engine().unwrap().world();
 let env=Envelope{world:1,branch:1,operation_id:id,expected_revision:w.view().revision,
 actions:vec![Action::Transfer{from:w.account_ref(3).unwrap(),to:w.account_ref(2).unwrap(),amount:mycity_core::units::Money::new(1).unwrap()}]};
 store.execute(Caller::System,env).unwrap().model.unwrap();
 }
 store.checkpoint().unwrap();
 let bytes=store.engine().unwrap().export_checkpoint().unwrap();fs::write(&args[2],&bytes).unwrap();
 println!("{{\"sequence\":{},\"engine_sha256\":\"{}\",\"engine_bytes\":{},\"old_receipt_present\":{}}}",store.committed_sequence(),mycity_core::sha256(&bytes),bytes.len(),store.receipt(1).unwrap().is_some());
}
