//! Independent CLI verifier. Only operate on a complete disposable copy of a world.
#![forbid(unsafe_code)]
use std::{env,fs,path::Path};
fn main() {
 let args:Vec<_>=env::args().collect();assert_eq!(args.len(),4);
 let expected=fs::read(&args[3]).expect("expected engine");
 let store=mycity_storage::Store::open(Path::new(&args[1]),&args[2]).expect("bound old world open");
 let actual=store.engine().expect("recovered engine").export_checkpoint().expect("full canonical bytes");
 assert_eq!(actual,expected,"old full Engine canonical bytes changed");
 println!("{{\"schema\":\"mycity-legacy-store-byte-compat-v1\",\"complete_engine_bytes_equal\":true,\"bytes\":{},\"sha256\":\"{}\",\"sequence\":{},\"runtime_budget_acceptance\":false}}",actual.len(),mycity_core::sha256(&actual),store.committed_sequence());
}
