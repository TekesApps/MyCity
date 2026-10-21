//! Full original business world -> current streaming checkpoint -> actual reopen.
#![forbid(unsafe_code)]
use std::{env,fs,path::Path};
use mycity_storage::{Store,Query};
fn receipts(s: &mut Store) -> (usize,String) {
 let mut cursor=None;let mut count=0;let mut hashes=String::new();
 loop {
  let page=s.query(Query::Receipts,cursor.as_deref(),64).expect("receipt page");
  for row in &page.rows {
   assert_eq!(row["committed_sequence"].as_u64().unwrap(),s.committed_sequence()-count as u64);
   count+=1;
  }
  hashes.push_str(&mycity_core::sha256(&serde_json::to_vec(&page.rows).unwrap()));
  cursor=page.next_cursor;if cursor.is_none(){break}
 }
 (count,mycity_core::sha256(hashes.as_bytes()))
}
fn main() {
 let args:Vec<_>=env::args().collect();assert_eq!(args.len(),4);
 let path=Path::new(&args[1]);let expected=fs::read(&args[3]).unwrap();
 let mut s=Store::open(path,&args[2]).expect("old complete fork");
 let before=receipts(&mut s);assert_eq!(before.0,3013);
 assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(),expected);
 let start=std::time::Instant::now();
 s.checkpoint().expect("new streaming world/body output");
 s.checkpoint().expect("empty save retains immutable receipts");
 let save_seconds=start.elapsed().as_secs_f64();drop(s);
 let start=std::time::Instant::now();
 let mut s=Store::open(path,&args[2]).expect("actual streamed checkpoint restore");
 let restore_seconds=start.elapsed().as_secs_f64();
 assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(),expected);
 assert_eq!(s.committed_sequence(),3013);assert_eq!(receipts(&mut s),before);
 println!("{{\"schema\":\"mycity-streamed-checkpoint-full-roundtrip-v1\",\"sequence\":3013,\"engine_bytes\":{},\"engine_sha256\":\"{}\",\"receipt_rows\":{},\"receipt_page_hash\":\"{}\",\"saves\":2,\"save_seconds_diagnostic_only\":{},\"restore_seconds_diagnostic_only\":{},\"performance_accepted\":false}}",expected.len(),mycity_core::sha256(&expected),before.0,before.1,save_seconds,restore_seconds);
}
