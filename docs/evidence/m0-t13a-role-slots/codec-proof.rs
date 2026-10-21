#![forbid(unsafe_code)]
use std::{env,fs,path::Path,time::Instant};
use mycity_core::{canonical::Codec,sha256};
use serde_json::{json,Value};
fn main(){
 let args:Vec<_>=env::args().collect();assert_eq!(args.len(),4);
 let store=mycity_storage::Store::open(Path::new(&args[1]),&args[2]).unwrap();
 let e=store.engine().unwrap();let full=e.export_checkpoint().unwrap();assert_eq!(full,fs::read(&args[3]).unwrap());
 let v:Value=serde_json::from_slice(&full).unwrap();let mut world=v["world"].clone();world["revision"]=json!(0);
 let old=serde_json::to_vec(&json!({"schema":"mycity-business-comparison-v1","world":world,"resources":v["resources"],"leases":v["leases"],"next_token":v["next_token"],"timeline":v["timeline"]})).unwrap();
 let oldstate=serde_json::to_vec(&json!({"world":e.world().view(),"resources":v["resources"],"leases":v["leases"],"next_token":v["next_token"],"timeline":v["timeline"],"background":v["background"]})).unwrap();
 let mut codec=Codec::new().unwrap();let mut bytes=Vec::new();e.write_business_comparison(&mut codec,&mut bytes).unwrap();assert_eq!(bytes,old);
 let mut state=Vec::new();e.write_state_comparison(&mut codec,&mut state).unwrap();assert_eq!(state,oldstate);
 assert_eq!(e.business_digest_with(&mut codec).unwrap(),sha256(&old));assert_eq!(e.state_hash_with(&mut codec).unwrap(),sha256(&oldstate));
 let joint=serde_json::to_vec(&json!({"schema":"mycity-m0-store-state-v1","engine":sha256(&oldstate),"spatial":store.spatial_reference().unwrap()})).unwrap();assert_eq!(store.state_hash().unwrap(),sha256(&joint));
 let start=Instant::now();for _ in 0..10 {assert_eq!(e.business_digest_with(&mut codec).unwrap(),sha256(&old));}let ten=start.elapsed().as_secs_f64();
 println!("{}",json!({"schema":"mycity-model-comparison-byte-oracle-v1","sequence":store.committed_sequence(),"full_engine_bytes":full.len(),"full_engine_sha256":sha256(&full),"business_bytes":bytes.len(),"business_sha256":sha256(&bytes),"state_bytes":state.len(),"state_sha256":sha256(&state),"store_sha256":sha256(&joint),"old_business_and_state_entire_bytes_equal":true,"store_old_outer_hash_equal":true,"codec_owned_capacity_bytes":codec.owned_capacity_bytes(),"store_owned_capacity_bytes":store.owned_capacity_bytes().unwrap(),"ten_reused_hash_checks_seconds_diagnostic_only":ten,"runtime_pool_accepted":false,"performance_accepted":false}));
}
