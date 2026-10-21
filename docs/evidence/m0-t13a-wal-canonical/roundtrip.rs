//! Full old world -> successful+failed new WAL -> actual WAL reopen -> save/reopen.
#![forbid(unsafe_code)]
use std::{env,fs,path::Path};
use mycity_storage::{Store,Query};
use mycity_core::{transaction::{Caller,Envelope,Action,TxError},units::Money};
use sha2::{Digest,Sha256};
fn receipts(s:&mut Store,skip:usize)->(usize,String){
 let mut cursor=None;let mut seen=0;let mut count=0;let mut hash=Sha256::new();
 loop {
  let page=s.query(Query::Receipts,cursor.as_deref(),64).expect("receipt page");
  for row in &page.rows {
   assert_eq!(row["committed_sequence"].as_u64().unwrap(),s.committed_sequence()-seen as u64);
   if seen>=skip {let bytes=serde_json::to_vec(row).unwrap();hash.update((bytes.len() as u64).to_le_bytes());hash.update(bytes);count+=1;}
   seen+=1;
  }
  cursor=page.next_cursor;if cursor.is_none(){break}
 }
 (count,format!("{:x}",hash.finalize()))
}
fn wire_oracle(path:&Path)->usize{
 #[derive(serde::Serialize)]
 struct Old {sequence:u64,previous:String,envelope:Envelope,record:serde_json::Value}
 let current:serde_json::Value=serde_json::from_slice(&fs::read(path.join("CURRENT.json")).unwrap()).unwrap();
 let root:serde_json::Value=serde_json::from_slice(&fs::read(path.join(format!("root-{}.json",current["slot"].as_u64().unwrap()))).unwrap()).unwrap();
 let segment=root["segment"].as_u64().unwrap();
 let file=if segment==0 {"wal.bin".to_owned()}else{format!("wal-{segment}.bin")};
 let bytes=fs::read(path.join(file)).unwrap();let mut at=0;let mut count=0;
 while at<root["wal_end"].as_u64().unwrap() as usize {
  let length=&bytes[at..at+4];let n=u32::from_le_bytes(length.try_into().unwrap()) as usize;
  let payload=&bytes[at+4..at+4+n];let value:serde_json::Value=serde_json::from_slice(payload).unwrap();
  let old=Old {sequence:value["sequence"].as_u64().unwrap(),previous:value["previous"].as_str().unwrap().into(),envelope:serde_json::from_value(value["envelope"].clone()).unwrap(),record:value["record"].clone()};
  assert_eq!(serde_json::to_vec(&old).unwrap(),payload,"actual WAL differs from old serializer");
  let mut hash=Sha256::new();hash.update(length);hash.update(payload);
  assert_eq!(&hash.finalize()[..],&bytes[at+4+n..at+4+n+32]);at+=4+n+32;count+=1;
 }
 assert_eq!(at,bytes.len());count
}
fn main(){
 let args:Vec<_>=env::args().collect();assert_eq!(args.len(),4);let path=Path::new(&args[1]);let old=fs::read(&args[3]).unwrap();
 let mut s=Store::open(path,&args[2]).expect("complete old fork");assert_eq!(s.committed_sequence(),3013);
 assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(),old);let original=receipts(&mut s,0);assert_eq!(original.0,3013);
 let good=Envelope {world:1,branch:1,operation_id:0x0bbb0001,expected_revision:s.engine().unwrap().world().view().revision,
  actions:vec![Action::Transfer {from:s.engine().unwrap().world().account_ref(15).unwrap(),to:s.engine().unwrap().world().account_ref(16).unwrap(),amount:Money::ZERO}]};
 let good_result=s.execute(Caller::System,good.clone()).unwrap();assert!(good_result.has_persistent_receipt);assert!(good_result.model.is_ok());
 let mut bad=good.clone();bad.operation_id+=1;bad.expected_revision=u64::MAX;
 let bad_result=s.execute(Caller::System,bad.clone()).unwrap();assert!(bad_result.has_persistent_receipt);assert_eq!(bad_result.model,Err(TxError::StaleRevision));
 assert_eq!(receipts(&mut s,2),original);let all=receipts(&mut s,0);assert_eq!(all.0,3015);
 let expected=s.engine().unwrap().export_checkpoint().unwrap();let frames=wire_oracle(path);assert!(frames>=2);drop(s);
 let mut s=Store::open(path,&args[2]).expect("actual new WAL replay");assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(),expected);
 assert_eq!(s.execute(Caller::System,good).unwrap().model,good_result.model);assert_eq!(s.execute(Caller::System,bad).unwrap().model,bad_result.model);assert_eq!(s.committed_sequence(),3015);
 assert_eq!(receipts(&mut s,0),all);s.checkpoint().unwrap();s.checkpoint().unwrap();drop(s);
 let mut s=Store::open(path,&args[2]).expect("actual saved new WAL outcomes");assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(),expected);assert_eq!(receipts(&mut s,0),all);assert_eq!(receipts(&mut s,2),original);
 println!("{{\"schema\":\"mycity-wal-canonical-full-roundtrip-v1\",\"original_sequence\":3013,\"final_sequence\":3015,\"original_engine_bytes\":{},\"original_engine_sha256\":\"{}\",\"final_engine_sha256\":\"{}\",\"new_successes\":1,\"new_failures\":1,\"actual_wal_frames_old_byte_oracle\":{},\"receipt_rows\":3015,\"old_receipts_unchanged\":true,\"new_wal_replayed\":true,\"saved_and_reopened\":true,\"performance_accepted\":false}}",old.len(),mycity_core::sha256(&old),mycity_core::sha256(&expected),frames);
}
