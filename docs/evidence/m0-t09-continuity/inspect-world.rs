//! Read-only inspection of the completed run, linked to its frozen Release libraries.
use mycity_storage::{Query,Store};
use serde_json::{json,Value};
use std::{fs,path::Path};
fn main(){
 let a:Vec<_>=std::env::args().collect();assert_eq!(a.len(),4,"WORLD RULES OUTPUT_DIR");
 let mut s=Store::open(Path::new(&a[1]),&a[2]).unwrap();let before=s.state_hash().unwrap();let sequence=s.committed_sequence();let out=Path::new(&a[3]);fs::create_dir_all(out).unwrap();
 fs::write(out.join("world.json"),serde_json::to_vec_pretty(&s.engine().unwrap().world().view()).unwrap()).unwrap();
 let mut examples=vec![];
 for q in [Query::Contracts,Query::Fulfilment,Query::Arrears,Query::Capacity,Query::Blocked,Query::Recovery,Query::Receipts]{examples.push(json!({"query":q,"page":s.query(q,None,3).unwrap()}));}
 fs::write(out.join("query-examples.json"),serde_json::to_vec_pretty(&examples).unwrap()).unwrap();
 let mut capacity=vec![];let mut next=None;
 loop{let p=s.query(Query::Capacity,next.as_deref(),3).unwrap();capacity.extend(p.rows);next=p.next_cursor;if next.is_none(){break;}}
 fs::write(out.join("capacity-rows.json"),serde_json::to_vec_pretty(&capacity).unwrap()).unwrap();
 let mut cursor=None;let mut scanned=0usize;let mut selected:Vec<Value>=vec![];
 loop{
  let page=s.query(Query::Receipts,cursor.as_deref(),64).unwrap();
  for row in page.rows{
   scanned+=1;let record=&row["record"];
   let created=record["outcome"]["Ok"]["created_facts"].as_array().is_some_and(|v|v.iter().any(|n|n.as_u64().is_some()));
   let paid=record["outcome"]["Ok"]["settled"].as_array().is_some_and(|v|v.iter().any(|n|n.as_i64().is_some_and(|n|n>0)));
   let management=record["actions"].as_array().is_some_and(|v|v.iter().any(|a|["handoff","archive"].contains(&a["command"]["kind"].as_str().unwrap_or(""))));
   if created||paid||management{selected.push(row);}
  }
  cursor=page.next_cursor;if cursor.is_none(){break;}
 }
 assert_eq!(scanned as u64,sequence);assert_eq!(s.state_hash().unwrap(),before);assert_eq!(s.committed_sequence(),sequence);
 fs::write(out.join("trace-receipts.json"),serde_json::to_vec_pretty(&selected).unwrap()).unwrap();
 fs::write(out.join("inspection.json"),serde_json::to_vec_pretty(&json!({"store_state_sha256":before,"sequence":sequence,"receipts_scanned":scanned,"financial_or_management_receipts":selected.len(),"source_after_read_only_reopen":s.source_status().unwrap(),"unchanged_by_queries":true})).unwrap()).unwrap();
 println!("PASS: read-only world and seven query examples; scanned {scanned} committed receipts, selected {} financial/management traces; queries changed neither state hash nor sequence",selected.len());
}
