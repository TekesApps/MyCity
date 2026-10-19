//! Independent probe linked to the same frozen Release libraries as the sustained run.
use mycity_core::{inbox::{Command as I,ExternalFact,ExternalReason},transaction::{Action,Caller,Envelope,Resource,TxError},units::Money};
use mycity_storage::Store;
fn env(s:&Store,command:I)->Envelope{Envelope{world:1,branch:1,operation_id:s.committed_sequence()+1,expected_revision:s.engine().unwrap().world().view().revision,actions:vec![Action::Inbox{command}]}}
fn step(s:&mut Store,command:I){let e=env(s,command);let r=s.execute(Caller::System,e).unwrap();assert!(r.has_persistent_receipt);assert!(r.model.is_ok(),"{:?}",r.model);}
fn main(){
 let dir=std::env::temp_dir().join(format!("mycity-retained-capacity-{}",std::process::id()));
 let rules="m0-retained-capacity-independent-v1";
 let mut s=Store::create_spatial(&dir,include_bytes!("../../../fixtures/m0-exchange-opening.json"),[(101,1),(102,1),(103,1),(104,1),(201,2000),(202,2000)].into_iter().map(|(id,n)|Resource::new(id,n).unwrap()).collect(),rules,include_bytes!("../../../fixtures/m0-exchange-spatial.json"),include_bytes!("../../../fixtures/terrain/m0-flat16.png")).unwrap();
 step(&mut s,I::Enable);s.register_external_source().unwrap();
 for event in 1..=17 {
  let fact=ExternalFact{source:1,event,debtor:4,creditor:2,amount:Money::new(1).unwrap(),due:100,reason:ExternalReason::ContractAward};
  s.queue_external(fact).unwrap();let receipt=s.receive_external(fact).unwrap().model.unwrap();assert_eq!(receipt.created_facts[0],Some(u64::from(event)));
  if event<=16{step(&mut s,I::Handoff{sequence:u64::from(event),manager:1});}
 }
 let snapshot=s.engine().unwrap().world().view();let inbox=snapshot.exchange.unwrap().inbox;
 assert_eq!(inbox.retained.iter().flatten().count(),16);assert_eq!(inbox.facts.iter().flatten().count(),1);
 let failed=env(&s,I::Handoff{sequence:17,manager:1});let hash=s.state_hash().unwrap();assert_eq!(s.execute(Caller::System,failed.clone()).unwrap().model,Err(TxError::InboxFull));assert_eq!(s.state_hash().unwrap(),hash);
 let payment=env(&s,I::Settle{sequence:1});assert_eq!(s.execute(Caller::System,payment).unwrap().model.unwrap().settled[0].unwrap().value(),1);
 step(&mut s,I::Archive{sequence:1});step(&mut s,I::Handoff{sequence:17,manager:1});
 s.reconcile_external_source(18).unwrap();let hash=s.state_hash().unwrap();s.checkpoint().unwrap();drop(s);
 let mut s=Store::open(&dir,rules).unwrap();assert_eq!(s.state_hash().unwrap(),hash);s.reconcile_external_source(18).unwrap();
 assert_eq!(s.execute(Caller::System,failed).unwrap().model,Err(TxError::InboxFull));assert_eq!(s.state_hash().unwrap(),hash);
 let w=s.engine().unwrap().world().view();let inbox=w.exchange.unwrap().inbox;assert_eq!(inbox.retained.iter().flatten().count(),16);assert_eq!(inbox.all().map(|f|f.fee.value()-f.paid.value()).sum::<i64>(),16);assert_eq!(inbox.paid.value(),1);assert_eq!(inbox.accrued.value(),17);assert_eq!(w.accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3000000);
 println!("PASS: retained=16/16; seventeenth handoff rejected atomically; old claims preserved; real 1c payment and archive free one slot; new handoff succeeds; old rejected ID stays rejected across checkpoint/reopen; outstanding16c + paid1c = accrued17c; total cash3000000c");
 println!("store_state_sha256={hash}; owned_capacity_bytes={}",s.owned_capacity_bytes().unwrap());drop(s);std::fs::remove_dir_all(dir).unwrap();
}
