//! Literal projection/state counterexamples, not real clock/transport or Store evidence.
use super::*;
use crate::air_page_metrics::{Axis,Group,Turn,Nanoseconds as Ns,VERSION};
fn summary(n:u64)->Summary {
    let axis=Axis{total:Ns::Known{ns:n},max_page:Ns::Known{ns:n}};
    let zero=Axis{total:Ns::Known{ns:0},max_page:Ns::Known{ns:0}};
    let work=air_turns::WorkCounters{cells:1,pages:1,..Default::default()};
    Summary{version:VERSION,work,unavailable:None,turn:Some(Turn{work,
        ordinary:Group{pages:1,wall:axis,cpu:axis},integration:Group{pages:0,wall:zero,cpu:zero},wall:axis,cpu:axis})}
}
fn session(client:&mut Client)->AdaptiveSession<'_> {
    let quota=Quota{source_limit:1,cell_limit:1,pages:1};
    AdaptiveSession{client,policy:Policy::new(quota,Config{target_ns:100}).unwrap(),job:1,phase:Phase::Running,
        ticket:None,instance:None,expected_turn:1,quota,wire_boundary:None,final_sample:None,final_metrics_status:None,
        pending:None,last_attempted:None,query:None}
}
#[test]
fn complete_missing_or_changed_summary_is_unavailable_and_result_is_not_rejected() {
    for next in [None,Some(summary(2))] {
        let mut client=Client::new();let mut session=session(&mut client);
        let boundary=Boundary{plan_version:1,turn:1,process_instance:"1".into(),work:summary(1).work};
        session.final_sample=Some((boundary.clone(),Some(summary(1))));
        assert!(session.accept_final_metrics(&boundary,next).unwrap());
        assert_eq!(session.final_metrics_status(),Some("changed_or_missing_at_complete"));
        assert_eq!(session.phase(),Phase::Running);assert!(session.pending.is_none());
        let event=Event{protocol:"mycity-sim-v1".into(),sim_pid:123,id:1,clock:None,kind:"result".into(),payload:serde_json::json!({"Ok":{"literal":true}})};
        assert!(session.receive(&event).unwrap().is_none());assert_eq!(session.phase(),Phase::Finished);
    }
}
#[test]
fn changed_final_business_boundary_remains_a_hard_error() {
    let mut client=Client::new();let mut session=session(&mut client);
    let mut boundary=Boundary{plan_version:1,turn:1,process_instance:"1".into(),work:summary(1).work};
    session.final_sample=Some((boundary.clone(),Some(summary(1))));boundary.turn=2;
    assert!(session.accept_final_metrics(&boundary,None).is_err());
    assert_eq!(session.final_sample.as_ref().unwrap().0.turn,1);
}
