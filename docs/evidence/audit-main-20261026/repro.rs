use mycity_core::{time_capacity::{Command as C,Registration,Unit},timeline::Command as T,transaction::{Action,Caller,Envelope,Resource}};
use mycity_storage::{Store,CapacityPolicy};
use std::{path::PathBuf,fs};
use serde_json::json;
const RULES:&str="timeline-target-emergency149-v1";
fn env(s:&Store,op:&mut u64,a:Action)->Envelope{let req=Envelope{world:1,branch:1,operation_id:*op,expected_revision:s.engine().unwrap().world().view().revision,actions:vec![a]};*op+=1;req}
fn apply(s:&mut Store,op:&mut u64,caller:Caller,a:Action){let req=env(s,op,a);assert!(s.execute(caller,req).unwrap().model.is_ok());}
fn cap(c:C)->Action{Action::TimeCapacity{command:c}}
fn main(){let args:Vec<_>=std::env::args().collect();let p=PathBuf::from(&args[1]);fs::create_dir(&p).unwrap();let mut s=Store::create(&p.join("world"),include_bytes!("/Users/frankqin/dev/MyCity/fixtures/m0-e0-opening.json"),vec![Resource::new(701,2).unwrap()],RULES).unwrap();let mut op=1;
 apply(&mut s,&mut op,Caller::System,Action::Timeline{command:T::Enable});apply(&mut s,&mut op,Caller::System,cap(C::Register{registration:Registration{canonical:70001,resource:701,aliases:[Some(1001),Some(1002),None,None],unit:Unit::Count,owner:1,operator:3,nominal_capacity:2,effective_capacity:2,identity_version:1,policy_version:1,open:true}}));
 apply(&mut s,&mut op,Caller::Account(3),cap(C::Reserve{alias:1001,account:3,consumer:301,consumer_version:1,start:0,end:10000,units:2}));let token=s.engine().unwrap().leases().iter().flatten().next().unwrap().token;apply(&mut s,&mut op,Caller::System,cap(C::Enter{token,occurred_at:0}));apply(&mut s,&mut op,Caller::System,Action::Timeline{command:T::Advance{target:7200,limit:32}});
 let t=s.engine().unwrap().timeline().unwrap();assert_eq!(t.progress.last_complete,3600);assert!(t.progress.phase.is_none());assert_eq!(t.progress.target,Some(7200));
 let used=s.capacity_status().unwrap().used_bytes;s.reduce_capacity_policy(CapacityPolicy{total_bytes:used+2*1024*1024,emergency_bytes:2*1024*1024+used/2}).unwrap();let status=s.capacity_status().unwrap();assert!(status.normal_blocked);fs::write(p.join("before-checkpoint.json"),s.engine().unwrap().export_checkpoint().unwrap()).unwrap();
 let mut outcomes=Vec::new();for (name,a) in [("exit",cap(C::Exit{token,occurred_at:3600})),("cancel",Action::Timeline{command:T::CancelAdvance}),("continue",Action::Timeline{command:T::Advance{target:7200,limit:32}})] {let req=env(&s,&mut op,a);let emergency=s.engine().unwrap().can_advance_existing_responsibility(Caller::System,&req);let hash=s.state_hash().unwrap();let result=s.execute(Caller::System,req.clone());let ok=result.as_ref().is_ok_and(|r|r.model.is_ok());outcomes.push(json!({"name":name,"request":req,"emergency":emergency,"result":format!("{result:?}"),"success":ok,"state_unchanged":s.state_hash().unwrap()==hash}));}
 fs::write(p.join("observations.json"),serde_json::to_vec_pretty(&json!({"initial_quota":status,"outcomes":outcomes,"state":s.engine().unwrap().timeline().unwrap(),"world":p.join("world")})).unwrap()).unwrap();
 let cancel=outcomes.iter().find(|o|o["name"]=="cancel").unwrap();assert_eq!(cancel["success"],true,"ordinary cancel of existing target at completed boundary must remain possible with emergency headroom");
}
