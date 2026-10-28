//! #159 opening-source gate only; not maintenance delivery or site acceptance.
use mycity_core::{assembly, member_qualification::{BaseQualification,MemberRange},
    qualification_authority::{JobRequirements,JobStatus},timeline,transaction::{Engine,Caller,Envelope,Action}};
#[path="../../../tests/fixtures/finite-city/maintenance_opening.rs"] mod fixture;
#[path="../../../tests/fixtures/finite-city/asset_opening.rs"] mod original;
#[test]
fn explicit_maintenance_sources_preserve_people_money_stocks_and_old_service_qualification() {
    let raw=fixture::base_input();let old=original::input();
    for key in ["accounts","sources","stocks","member_activity","housing"] {assert_eq!(raw[key],old[key],"no new endowment or people: {key}");}
    let mut engine=Engine::new(assembly::assemble(&serde_json::to_vec(&raw).unwrap()).unwrap(),fixture::resources()).unwrap();
    let (scope,revision)=engine.world().scope_revision();
    engine.execute(Caller::System,Envelope{world:scope.world(),branch:scope.branch(),operation_id:1,expected_revision:revision,
        actions:vec![Action::Timeline{command:timeline::Command::Enable}]}).unwrap();
    for (end,required) in [(4,BaseQualification::Electric.bit()),(2,BaseQualification::Driver.bit()|BaseQualification::Police.bit())] {
        let read=engine.check_job_qualification(JobRequirements{members:MemberRange{start:0,end},minimum_background:None,
            minimum_skill:None,required_base_mask:required,required_endorsement_mask:0}).unwrap();
        assert!(matches!(read.status,JobStatus::Eligible),"{read:?}");
    }
    assert_eq!(engine.world().view().accounts.iter().map(|a|a.cash.value()).sum::<i64>(),3_100_000);
    assert_eq!(fixture::resources().iter().find(|r|r.id==81).unwrap().capacity,0);
    assert!(raw.get("venue").is_none(),"no worksite is claimed by this initial source gate");
}
