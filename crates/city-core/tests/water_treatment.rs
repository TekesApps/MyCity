//! Source-only #92 tests. Execution is reserved for root's serial validation window.
#[path = "../src/water_treatment.rs"]
#[allow(dead_code)]
mod water_treatment;
use water_treatment::*;

fn e(n:u64)->Exact { Exact::integer(n).unwrap() }
fn f(n:u128,d:u128)->Exact { Exact::fraction(n,d).unwrap() }
fn m(n:u64)->Mass { Mass::Known(e(n)) }
fn config()->Config {
    let mut raw=Composition::known_zero(e(100_000_000));
    raw.harmful_mq=m(100); raw.suspended_mq=m(10);
    Config { version:VERSION,precision_version:PRECISION_VERSION,profile:Profile::RawPurificationV1,
        binding:Binding { world_ref:1,root_epoch:2,source_version:3,city_id:4,operation:5,origin_cursor:0 },
        raw:LotQuality { stock_id:1,lot_ref:11,composition:raw,
            references:[Some(SourceRef { source:7,version:1,coverage_from:0,coverage_until:3600,observed_at:0,purpose_ref:8 }),None,None,None] },
        raw_origin:InputLot { stock_id:1,lot_ref:11,quantity_milli:100_000_000,basis_cents:101 },
        reagent_origin:InputLot { stock_id:2,lot_ref:12,quantity_milli:100,basis_cents:37 },
        raw_basis:RawBasisMode::FrozenLot,output_stock:3,concentrate_stock:4,sludge_stock:5 }
}
/// Synthetic fixture authority only; never exported by the production module.
struct FixtureVerifier;
impl RootVerifier for FixtureVerifier {
    fn verifies(&self,_:&Config,_:&Checkpoint,_:Request,_:&RootFacts)->bool { true }
}
struct RejectVerifier;
impl RootVerifier for RejectVerifier {
    fn verifies(&self,_:&Config,_:&Checkpoint,_:Request,_:&RootFacts)->bool { false }
}
fn facts(c:&Config,from:u64,until:u64)->RootFacts {
    RootFacts { binding:c.binding,from,until,read_version:1,
        raw_stock_available_ml:100_000_000,reagent_stock_available_mq:100,electricity_available_mwh:e(20_000_000),raw_stream_basis_after:None,
        output_free_ml:e(100_000_000),concentrate_free_ml:e(2_000_000),sludge_free_mq:e(80),
        electricity_basis_cents:17,personnel_cents:11,residual_cents:3,cash_available_cents:31,
        cost_receipt_ref:9,supply_receipt_ref:10 }
}
fn advance(c:&Config,s:&State,until:u64,input_ml:Exact)->(State,Candidate) {
    let facts=facts(c,s.checkpoint().cursor,until);
    let candidate=c.prepare(&FixtureVerifier,s,Request { until,input_ml },&facts,1).unwrap();
    let next=c.revalidate(&FixtureVerifier,s,&candidate,&facts).unwrap(); (next,candidate)
}
#[test]
fn hundred_cubic_meter_seed_closes_water_solids_and_harmful_mass() {
    let c=config(); let (s,can)=advance(&c,&c.open().unwrap(),3600,e(100_000_000));
    let d=can.delta;
    assert_eq!(d.reagent_mq,e(100)); assert_eq!(d.electricity_mwh,e(20_000_000));
    assert_eq!(d.product.water_ml,e(98_000_000)); assert_eq!(d.concentrate.water_ml,e(2_000_000));
    assert_eq!(d.product.harmful_mq,m(1)); assert_eq!(d.concentrate.harmful_mq,m(99));
    assert_eq!(d.product.allowed_dissolved_mq,m(10)); assert_eq!(d.concentrate.allowed_dissolved_mq,m(20));
    assert_eq!(d.sludge.allowed_dissolved_mq,m(70)); assert_eq!(d.sludge.suspended_mq,m(10));
    let sum=d.product.mix(d.concentrate).unwrap().mix(d.sludge).unwrap();
    assert_eq!(sum.water_ml,c.raw.composition.water_ml);
    assert_eq!(sum.harmful_mq,c.raw.composition.harmful_mq);
    assert_eq!(sum.suspended_mq,c.raw.composition.suspended_mq);
    assert_eq!(sum.allowed_dissolved_mq,m(100));
    assert_eq!(c.meters(&s).unwrap().reagent_wip_mq,Exact::ZERO);
    assert_eq!(judge(d.product,Standard::OrdinaryGameWaterV1).unwrap(),QualityJudgment::MeetsGameChemicalStandard(Standard::OrdinaryGameWaterV1));
}
#[test]
fn twenty_liters_consumes_fractional_reagent_and_keeps_prefetch_with_cost() {
    let c=config(); let (s,can)=advance(&c,&c.open().unwrap(),1,e(20_000));
    assert_eq!(can.delta.reagent_mq,f(1,50)); assert_eq!(can.delta.reagent_take_mq,1);
    assert_eq!(can.delta.electricity_mwh,e(4000));
    let meters=c.meters(&s).unwrap();
    assert_eq!(meters.reagent_wip_mq,f(49,50));
    assert_eq!(meters.reagent_converted_basis,f(37,5000));
    assert_eq!(meters.reagent_wip_basis,f(1813,5000));
    assert_eq!(meters.reagent_held_basis_cents,0);
}
#[test]
fn fractions_split_batch_restore_and_last_outlet_are_identical() {
    let c=config(); let s=c.open().unwrap();
    let (whole,_)=advance(&c,&s,3,e(20_000));
    let (a,_)=advance(&c,&s,1,f(20_000,3));
    let bytes=serde_json::to_vec(&a.checkpoint()).unwrap();
    let saved:Checkpoint=serde_json::from_slice(&bytes).unwrap();
    let a=c.restore(saved).unwrap();
    let (b,_)=advance(&c,&a,2,f(20_000,3));
    let (split,_)=advance(&c,&b,3,f(20_000,3));
    assert_eq!(c.meters(&split),c.meters(&whole));
    let mut remain=c.meters(&whole).unwrap().product;
    let origin=remain;
    let (a,b)=remain.split(f(19_600,3)).unwrap(); remain=b;
    let (b,last)=remain.split(f(19_600,3)).unwrap();
    assert_eq!(a.mix(b).unwrap().mix(last).unwrap(),origin);
}
#[test]
fn actual_suspended_and_original_dissolved_mass_are_not_invented_or_removed() {
    let mut c=config(); c.raw.composition.suspended_mq=m(0); c.raw.composition.allowed_dissolved_mq=m(50);
    let (_,can)=advance(&c,&c.open().unwrap(),3600,e(100_000_000));
    assert_eq!(can.delta.sludge.suspended_mq,m(0));
    assert_eq!(can.delta.product.allowed_dissolved_mq,m(59));
    assert_eq!(can.delta.concentrate.allowed_dissolved_mq,m(21));
    assert_eq!(can.delta.sludge.allowed_dissolved_mq,m(70));
}
#[test]
fn unknown_mixing_evaporation_and_zero_water_cannot_keep_good_label() {
    let known=Composition::known_zero(e(50_000_000));
    let mut unknown=known; unknown.other_mq=Mass::Unknown;
    let mixed=known.mix(unknown).unwrap();
    assert_eq!(judge(mixed,Standard::OrdinaryGameWaterV1).unwrap(),QualityJudgment::CompositionUnknown);
    let (take,rest)=mixed.split(e(20_000)).unwrap();
    assert_eq!(take.other_mq,Mass::Unknown); assert_eq!(rest.other_mq,Mass::Unknown);
    let mut dirty=known; dirty.harmful_mq=m(1);
    assert!(matches!(judge(dirty,Standard::OrdinaryGameWaterV1).unwrap(),QualityJudgment::MeetsGameChemicalStandard(_)));
    let (dry,vapor)=dirty.evaporate(e(50_000_000)).unwrap();
    assert_eq!(dry.harmful_mq,m(1)); assert_eq!(vapor.harmful_mq,m(0));
    assert_eq!(judge(dry,Standard::OrdinaryGameWaterV1).unwrap(),QualityJudgment::KnownOutOfSpec);
    let (concentrated,_)=dirty.evaporate(e(1)).unwrap();
    assert_eq!(judge(concentrated,Standard::OrdinaryGameWaterV1).unwrap(),QualityJudgment::KnownOutOfSpec);
}
#[test]
fn out_of_spec_output_remains_out_of_spec_despite_registered_recipe() {
    let mut c=config(); c.raw.composition.harmful_mq=m(1000);
    let (_,can)=advance(&c,&c.open().unwrap(),3600,e(100_000_000));
    assert_eq!(judge(can.delta.product,Standard::OrdinaryGameWaterV1).unwrap(),QualityJudgment::KnownOutOfSpec);
    assert_eq!(judge(can.delta.product,Standard::ReclaimedIndustrialGreenV1).unwrap(),QualityJudgment::KnownOutOfSpec);
}
#[test]
fn missing_composition_unsupported_other_and_sewage_are_explicit() {
    let mut c=config(); c.raw.composition.other_mq=Mass::Unknown;
    assert_eq!(c.open(),Err(Error::UnknownComposition));
    c.raw.composition.other_mq=m(1); assert_eq!(c.open(),Err(Error::UnsupportedComponents));
    c.raw.composition.other_mq=m(0); c.profile=Profile::SewageUnsupportedV1;
    assert_eq!(c.open(),Err(Error::UnsupportedProfile));
}
#[test]
fn resource_energy_sink_cash_and_zero_work_fail_without_state_changes() {
    let c=config(); let s=c.open().unwrap(); let before=s.checkpoint();
    let req=Request { until:3600,input_ml:e(100_000_000) };
    let original=facts(&c,0,3600);
    let mut z=original.clone(); z.raw_stock_available_ml=99_999_999;
    assert_eq!(c.prepare(&FixtureVerifier,&s,req,&z,1),Err(Error::UnresolvedInput));
    z=original.clone(); z.reagent_stock_available_mq=99;
    assert_eq!(c.prepare(&FixtureVerifier,&s,req,&z,1),Err(Error::UnresolvedInput));
    z=original.clone(); z.electricity_available_mwh=e(19_999_999);
    assert_eq!(c.prepare(&FixtureVerifier,&s,req,&z,1),Err(Error::UnresolvedEnergy));
    for kind in 0..3 { z=original.clone(); match kind { 0=>z.output_free_ml=e(97_999_999),1=>z.concentrate_free_ml=e(1_999_999),_=>z.sludge_free_mq=e(79) }
        assert_eq!(c.prepare(&FixtureVerifier,&s,req,&z,1),Err(Error::UnresolvedSink)); }
    z=original.clone(); z.cash_available_cents=30;
    assert_eq!(c.prepare(&FixtureVerifier,&s,req,&z,1),Err(Error::UnresolvedCost));
    assert_eq!(c.prepare(&FixtureVerifier,&s,req,&original,0),Err(Error::WorkBudget));
    assert_eq!(s.checkpoint(),before);
}
#[test]
fn wip_cannot_be_counted_twice_and_fresh_stock_prefetch_is_required() {
    let c=config(); let (s,_)=advance(&c,&c.open().unwrap(),1,f(1,2));
    let mut z=facts(&c,1,2); z.raw_stock_available_ml=0; z.reagent_stock_available_mq=0;
    // The remaining half mL and almost-one mQ are held WIP; no new Stock required.
    let req=Request { until:2,input_ml:f(1,2) };
    let can=c.prepare(&FixtureVerifier,&s,req,&z,1).unwrap();
    assert_eq!(can.delta.raw_take_ml,0); assert_eq!(can.delta.reagent_take_mq,0);
    // One whole mL cannot be sourced from only the held half mL.
    assert_eq!(c.prepare(&FixtureVerifier,&s,Request { until:2,input_ml:e(1) },&z,1),Err(Error::UnresolvedInput));
}
#[test]
fn stale_candidates_tampering_and_unsigned_restore_are_rejected_by_root() {
    let c=config(); let s=c.open().unwrap(); let (next,can)=advance(&c,&s,1,e(20_000));
    assert_eq!(c.revalidate(&FixtureVerifier,&next,&can,&can.facts),Err(Error::Stale));
    let mut z=can.facts.clone(); z.binding.root_epoch+=1;
    assert_eq!(c.revalidate(&FixtureVerifier,&s,&can,&z),Err(Error::Stale));
    let mut altered=can.clone(); altered.delta.product.harmful_mq=m(0);
    assert_eq!(c.revalidate(&FixtureVerifier,&s,&altered,&can.facts),Err(Error::Stale));
    assert_eq!(c.revalidate(&RejectVerifier,&s,&can,&can.facts),Err(Error::Unauthorized));
    let restored=c.restore(next.checkpoint()).unwrap();
    let z=facts(&c,1,2);
    assert_eq!(c.prepare(&RejectVerifier,&restored,Request { until:2,input_ml:e(1) },&z,1),Err(Error::Unauthorized));
}
#[test]
fn canonical_precision_and_restore_bounds_reject_invalid_saved_state() {
    for invalid in [r#"{"whole":0,"rem":0,"den":0}"#,r#"{"whole":0,"rem":2,"den":2}"#,
        r#"{"whole":0,"rem":1,"den":1000000000000000001}"#,r#"{"whole":1,"rem":0,"den":2}"#,
        r#"{"whole":9223372036854775808,"rem":0,"den":1}"#] {
        assert!(serde_json::from_str::<Exact>(invalid).is_err());
    }
    assert_eq!(Exact::fraction(1,1_000_000_000_000_000_001),Err(Error::Precision));
    assert_eq!(e(MAX_WHOLE).add(e(1)),Err(Error::Overflow));
    let c=config(); let mut cp=c.open().unwrap().checkpoint(); cp.processed_ml=e(1);
    assert_eq!(c.restore(cp),Err(Error::Stale));
    let mut cp=c.open().unwrap().checkpoint(); cp.cursor=1; cp.sequence=1; cp.processed_ml=e(100_000_001);
    assert_eq!(c.restore(cp),Err(Error::UnresolvedInput));
}
#[test]
fn provenance_bound_fails_without_dropping_old_sources() {
    let c=config(); let mut a=c.raw.clone();
    for i in 0..MAX_REFERENCES { let mut r=a.references[0].unwrap(); r.source=100+i as u64; a.references[i]=Some(r); }
    let b=c.raw;
    assert_eq!(a.mixed(&b,8,8),Err(Error::ReferenceLimit));
    assert_eq!(a.references.iter().flatten().count(),4);
}
#[test]
fn envelope_planning_checks_are_only_local_layout_and_serialization_checks() {
    let c=config(); let (s,can)=advance(&c,&c.open().unwrap(),3600,e(100_000_000));
    let saved=serde_json::to_vec(&(c,s.checkpoint())).unwrap();
    assert!(saved.len()<=SAVED_ENVELOPE_ESTIMATE);
    assert!(SAVED_ENVELOPE_ESTIMATE<PERSISTENT_PLANNING_CAP);
    assert!(serde_json::to_vec(&can).unwrap().len()<=CANDIDATE_ENVELOPE_ESTIMATE);
    assert!(std::mem::size_of::<Config>()+std::mem::size_of::<State>()<PERSISTENT_PLANNING_CAP);
    // Includes old/new state, proposal, recomputation, serialization and restore overlap.
    assert!(2*CANDIDATE_ENVELOPE_ESTIMATE+2*SAVED_ENVELOPE_ESTIMATE<=WORKSPACE_PLANNING_CAP);
}
#[test]
fn small_seed_oracle_is_independent_of_source_constructor_values() {
    let oracle:serde_json::Value=serde_json::from_str(include_str!("../../../tests/fixtures/water-treatment/raw-v1.json")).unwrap();
    let c=config(); let (_,can)=advance(&c,&c.open().unwrap(),3600,e(100_000_000));
    for (name,actual) in [("product",can.delta.product),("concentrate",can.delta.concentrate),("sludge",can.delta.sludge)] {
        let expected=&oracle[name];
        assert_eq!(actual.water_ml,e(expected["water"].as_u64().unwrap()));
        assert_eq!(actual.harmful_mq,m(expected["harmful"].as_u64().unwrap()));
        assert_eq!(actual.suspended_mq,m(expected["suspended"].as_u64().unwrap()));
        assert_eq!(actual.allowed_dissolved_mq,m(expected["allowed_dissolved"].as_u64().unwrap()));
    }
}
#[test]
fn twelve_kw_budget_accepts_fractional_sixty_m3_per_hour_not_full_pump_output() {
    let c=config(); let s=c.open().unwrap(); let mut z=facts(&c,0,1);
    // Root fixture grants one second of only the 12kW left after two 6kW pumps.
    // This is an arithmetic budget counterexample, not evidence of an actual electric source.
    z.electricity_available_mwh=f(10_000,3);
    let req=Request { until:1,input_ml:f(50_000,3) };
    let can=c.prepare(&FixtureVerifier,&s,req,&z,1).unwrap();
    assert_eq!(can.delta.electricity_mwh,f(10_000,3));
    assert_eq!(can.delta.reagent_mq,f(1,60));
    assert_eq!(can.delta.product.water_ml,f(49_000,3));
    assert_eq!(c.prepare(&FixtureVerifier,&s,Request { until:1,input_ml:e(20_000) },&z,1),Err(Error::UnresolvedEnergy));
}
#[test]
fn original_process_ports_use_root_actual_basis_not_frozen_or_future_average() {
    let mut c=config(); c.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] };
    c.raw.stock_id=103; c.raw_origin.stock_id=103;
    c.raw_origin.basis_cents=MAX_WHOLE; // irrelevant to original-port production cost
    let s=c.open().unwrap(); let mut z=facts(&c,0,1);
    z.raw_stock_available_ml=20_000;
    z.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(7),wip:e(0) });
    let can=c.prepare(&FixtureVerifier,&s,Request { until:1,input_ml:e(20_000) },&z,1).unwrap();
    assert_eq!(can.delta.raw_take_ml,20_000);
    assert_eq!(can.delta.raw_take_basis_cents,7); assert_eq!(can.delta.raw_converted_basis,e(7));
    let next=c.revalidate(&FixtureVerifier,&s,&can,&z).unwrap();
    let restored=c.restore(next.checkpoint()).unwrap();
    assert_eq!(c.meters(&restored).unwrap().raw_converted_basis,e(7));
    z.raw_stream_basis_after=None;
    assert_eq!(c.prepare(&FixtureVerifier,&s,Request { until:1,input_ml:e(20_000) },&z,1),Err(Error::Invalid));
}
#[test]
fn stream_fractional_wip_basis_survives_restore_without_repricing_or_second_take() {
    let mut c=config(); c.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] };
    let s=c.open().unwrap(); let mut z=facts(&c,0,1);
    z.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(2),wip:e(5) });
    let first=c.prepare(&FixtureVerifier,&s,Request { until:1,input_ml:f(1,2) },&z,1).unwrap();
    assert_eq!(first.delta.raw_take_ml,1); assert_eq!(first.delta.raw_take_basis_cents,7);
    let s=c.revalidate(&FixtureVerifier,&s,&first,&z).unwrap();
    let saved:Checkpoint=serde_json::from_slice(&serde_json::to_vec(&s.checkpoint()).unwrap()).unwrap();
    let s=c.restore(saved).unwrap();
    z=facts(&c,1,2); z.raw_stock_available_ml=0; z.reagent_stock_available_mq=0;
    z.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(7),wip:e(0) });
    let next=c.prepare(&FixtureVerifier,&s,Request { until:2,input_ml:f(1,2) },&z,1).unwrap();
    assert_eq!(next.delta.raw_take_ml,0); assert_eq!(next.delta.raw_take_basis_cents,0);
    assert_eq!(next.delta.raw_converted_basis,e(5));
    let s=c.revalidate(&FixtureVerifier,&s,&next,&z).unwrap();
    assert_eq!(c.meters(&s).unwrap().raw_wip_basis,e(0));
    let mut bad=z.clone(); bad.from=2; bad.until=3;
    bad.raw_stream_basis_after=Some(StreamBasis { taken_cents:6,converted:e(6),wip:e(0) });
    assert!(c.prepare(&FixtureVerifier,&s,Request { until:3,input_ml:e(1) },&bad,1).is_err());
}
#[test]
fn stream_wrong_ports_or_nonconserving_root_basis_cannot_enter_checkpoint() {
    let mut c=config(); c.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,104] };
    assert_eq!(c.open(),Err(Error::Invalid));
    c.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] };
    let s=c.open().unwrap(); let mut z=facts(&c,0,1);
    z.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(7),wip:e(1) });
    assert_eq!(c.prepare(&FixtureVerifier,&s,Request { until:1,input_ml:e(1) },&z,1),Err(Error::Invalid));
    let mut cp=s.checkpoint(); cp.raw_stream_basis=None;
    assert_eq!(c.restore(cp),Err(Error::Stale));
}

/// Synthetic historical interval evidence, never a real Timeline/provider verifier.
struct IdleHistory { from:u64, until:u64, accepted_work:Option<(u64,u64)> }
impl RootVerifier for IdleHistory {
    fn verifies(&self,_:&Config,_:&Checkpoint,_:Request,_:&RootFacts)->bool { true }
    fn verifies_idle(&self,c:&Config,base:&Checkpoint,facts:&IdleFacts)->bool {
        facts.binding==c.binding && facts.from==base.cursor && facts.from>=self.from
            && facts.until<=self.until && facts.read_version==1 && facts.timeline_revision==77
            && facts.idle_receipt_ref==88 && self.accepted_work.map_or(true,|(from,until)| {
                facts.until<=from || facts.from>=until
            })
    }
}
fn idle_facts(c:&Config,from:u64,until:u64)->IdleFacts {
    IdleFacts { policy_version:IDLE_POLICY_VERSION,binding:c.binding,from,until,
        read_version:1,timeline_revision:77,idle_receipt_ref:88 }
}
fn idle(c:&Config,s:&State,until:u64)->(State,IdleCandidate) {
    let from=s.checkpoint().cursor;
    let history=IdleHistory { from,until,accepted_work:None };
    let facts=idle_facts(c,from,until);
    let can=c.cover_idle(&history,s,&facts,1).unwrap();
    let next=c.revalidate_idle(&history,s,&can,&facts).unwrap();
    (next,can)
}
fn assert_idle_zero(delta:&Delta) {
    assert_eq!(delta.input_ml,Exact::ZERO); assert_eq!(delta.reagent_mq,Exact::ZERO);
    assert_eq!(delta.electricity_mwh,Exact::ZERO);
    for out in [delta.product,delta.concentrate,delta.sludge] { assert_eq!(out,Composition::known_zero(Exact::ZERO)); }
    assert_eq!(delta.raw_take_ml,0); assert_eq!(delta.reagent_take_mq,0);
    assert_eq!(delta.raw_take_basis_cents,0); assert_eq!(delta.reagent_take_basis_cents,0);
    assert_eq!(delta.raw_converted_basis,Exact::ZERO); assert_eq!(delta.reagent_converted_basis,Exact::ZERO);
    assert_eq!(delta.root_interval_cost_cents,0);
}
#[test]
fn initial_idle_restores_zero_material_and_original_stream_basis_without_production() {
    for stream in [false,true] {
        let mut c=config(); if stream { c.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] }; }
        let s=c.open().unwrap(); let before=c.meters(&s).unwrap();
        let (after,can)=idle(&c,&s,1000);
        assert_idle_zero(&can.delta);
        assert_eq!(after.checkpoint().processed_ml,Exact::ZERO);
        assert_eq!(after.checkpoint().raw_stream_basis,s.checkpoint().raw_stream_basis);
        assert_eq!(after.checkpoint().cursor,1000); assert_eq!(after.checkpoint().sequence,1);
        let bytes=serde_json::to_vec(&after.checkpoint()).unwrap();
        let saved:Checkpoint=serde_json::from_slice(&bytes).unwrap();
        let restored=c.restore(saved).unwrap();
        assert_eq!(c.meters(&restored).unwrap(),before);
        // Structural restore did not grant a production-only verifier idle authority.
        assert_eq!(c.cover_idle(&FixtureVerifier,&restored,&idle_facts(&c,1000,1001),1),Err(Error::Unauthorized));
    }
}
#[test]
fn production_to_three_idle_to_thousand_then_only_real_next_interval_is_used() {
    let c=config(); let (at_three,_)=advance(&c,&c.open().unwrap(),3,e(20_000));
    let before=c.meters(&at_three).unwrap(); let (at_thousand,can)=idle(&c,&at_three,1000);
    assert_idle_zero(&can.delta); assert_eq!(c.meters(&at_thousand).unwrap(),before);
    let z=facts(&c,1000,1001);
    let req=Request { until:1001,input_ml:e(1) };
    let production=c.prepare(&FixtureVerifier,&at_thousand,req,&z,1).unwrap();
    assert_eq!(production.facts.from,1000); assert_eq!(production.facts.until,1001);
    let after=c.revalidate(&FixtureVerifier,&at_thousand,&production,&z).unwrap();
    assert_eq!(after.checkpoint().processed_ml,e(20_001));
    let mut false_interval=z; false_interval.from=3;
    assert_eq!(c.prepare(&FixtureVerifier,&at_thousand,req,&false_interval,1),Err(Error::Stale));
    // Production remains positive-only, including after legitimate idle coverage.
    assert_eq!(c.prepare(&FixtureVerifier,&at_thousand,Request { until:1001,input_ml:Exact::ZERO },&facts(&c,1000,1001),1),Err(Error::Invalid));
}
#[test]
fn split_and_merged_idle_preserve_fractional_material_wip_and_every_cost_meter() {
    let mut c=config(); c.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] };
    let start=c.open().unwrap(); let mut z=facts(&c,0,3);
    z.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(2),wip:e(5) });
    let production=c.prepare(&FixtureVerifier,&start,Request { until:3,input_ml:f(1,2) },&z,1).unwrap();
    let base=c.revalidate(&FixtureVerifier,&start,&production,&z).unwrap();
    let (merged,one)=idle(&c,&base,1000);
    let (part,two)=idle(&c,&base,4); let (split,three)=idle(&c,&part,1000);
    for can in [&one,&two,&three] { assert_idle_zero(&can.delta); }
    assert_eq!(c.meters(&merged),c.meters(&base)); assert_eq!(c.meters(&split),c.meters(&base));
    assert_eq!(merged.checkpoint().processed_ml,split.checkpoint().processed_ml);
    assert_eq!(merged.checkpoint().raw_stream_basis,split.checkpoint().raw_stream_basis);
    assert_eq!(merged.checkpoint().cursor,split.checkpoint().cursor);
    assert_eq!(merged.checkpoint().sequence+1,split.checkpoint().sequence);
    assert_eq!(c.meters(&merged).unwrap().raw_wip_basis,e(5));
    // Only idle deltas are zero: this does not assert arbitrary external production
    // interval costs (the separate 31-cent synthetic fixture) are partition invariant.
}
#[test]
fn idle_rejects_stale_base_freshness_old_receipts_nonzero_deltas_and_basis_tampering() {
    let mut c=config(); c.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] };
    let base=c.open().unwrap(); let (next,can)=idle(&c,&base,1000);
    let history=IdleHistory { from:0,until:2000,accepted_work:None };
    assert_eq!(c.revalidate_idle(&history,&next,&can,&can.facts),Err(Error::Stale));
    let mut fresh=can.facts.clone(); fresh.timeline_revision+=1;
    assert_eq!(c.revalidate_idle(&history,&base,&can,&fresh),Err(Error::Stale));
    fresh=can.facts.clone(); fresh.idle_receipt_ref=87;
    assert_eq!(c.cover_idle(&history,&base,&fresh,1),Err(Error::Unauthorized));
    let mut altered=can.clone(); altered.policy_version+=1;
    assert_eq!(c.revalidate_idle(&history,&base,&altered,&can.facts),Err(Error::Stale));
    altered=can.clone(); altered.delta.raw_take_ml=1;
    assert_eq!(c.revalidate_idle(&history,&base,&altered,&can.facts),Err(Error::Stale));
    altered=can.clone(); altered.delta.root_interval_cost_cents=1;
    assert_eq!(c.revalidate_idle(&history,&base,&altered,&can.facts),Err(Error::Stale));
    altered=can.clone(); altered.delta.product.harmful_mq=m(1);
    assert_eq!(c.revalidate_idle(&history,&base,&altered,&can.facts),Err(Error::Stale));
    altered=can.clone(); altered.next.raw_stream_basis=Some(StreamBasis { taken_cents:1,converted:e(1),wip:e(0) });
    assert_eq!(c.revalidate_idle(&history,&base,&altered,&can.facts),Err(Error::Stale));
    altered=can.clone(); altered.next.processed_ml=e(1);
    assert_eq!(c.revalidate_idle(&history,&base,&altered,&can.facts),Err(Error::Stale));
    altered=can.clone(); altered.next.sequence=u64::MAX;
    assert_eq!(c.revalidate_idle(&history,&base,&altered,&can.facts),Err(Error::Stale));
    let mut forged=next.checkpoint(); forged.raw_stream_basis=Some(StreamBasis { taken_cents:1,converted:e(1),wip:e(0) });
    assert_eq!(c.restore(forged),Err(Error::Stale));
}
#[test]
fn idle_needs_full_history_and_cannot_skip_accepted_work_or_use_default_authority() {
    let c=config(); let base=c.open().unwrap(); let z=idle_facts(&c,0,1000);
    assert_eq!(c.cover_idle(&FixtureVerifier,&base,&z,1),Err(Error::Unauthorized));
    assert_eq!(c.cover_idle(&RejectVerifier,&base,&z,1),Err(Error::Unauthorized));
    let partial=IdleHistory { from:500,until:1000,accepted_work:None };
    assert_eq!(c.cover_idle(&partial,&base,&z,1),Err(Error::Unauthorized));
    let accepted=IdleHistory { from:0,until:1000,accepted_work:Some((30,31)) };
    assert_eq!(c.cover_idle(&accepted,&base,&z,1),Err(Error::Unauthorized));
    let history=IdleHistory { from:0,until:1000,accepted_work:None };
    let can=c.cover_idle(&history,&base,&z,1).unwrap();
    assert_eq!(c.revalidate_idle(&accepted,&base,&can,&z),Err(Error::Unauthorized));
    assert_eq!(base.checkpoint(),c.open().unwrap().checkpoint());
}
#[test]
fn idle_policy_bounds_epoch_zero_work_and_terminal_cursor_do_not_wrap() {
    let c=config(); let base=c.open().unwrap();
    let history=IdleHistory { from:0,until:u64::MAX,accepted_work:None };
    let original=idle_facts(&c,0,1);
    assert_eq!(c.cover_idle(&history,&base,&original,0),Err(Error::WorkBudget));
    let mut z=original.clone(); z.policy_version+=1;
    assert_eq!(c.cover_idle(&history,&base,&z,1),Err(Error::Invalid));
    z=original.clone(); z.until=0;
    assert_eq!(c.cover_idle(&history,&base,&z,1),Err(Error::Invalid));
    z=original.clone(); z.binding.root_epoch+=1;
    assert_eq!(c.cover_idle(&history,&base,&z,1),Err(Error::Stale));
    z=original.clone(); z.read_version=0;
    assert_eq!(c.cover_idle(&history,&base,&z,1),Err(Error::Unauthorized));
    z=original.clone(); z.timeline_revision=0;
    assert_eq!(c.cover_idle(&history,&base,&z,1),Err(Error::Unauthorized));
    z=original.clone(); z.idle_receipt_ref=0;
    assert_eq!(c.cover_idle(&history,&base,&z,1),Err(Error::Unauthorized));
    z=idle_facts(&c,0,u64::MAX);
    let can=c.cover_idle(&history,&base,&z,1).unwrap();
    let terminal=c.revalidate_idle(&history,&base,&can,&z).unwrap();
    assert_eq!(terminal.checkpoint().cursor,u64::MAX); assert_eq!(terminal.checkpoint().sequence,1);
    assert_eq!(c.cover_idle(&history,&terminal,&idle_facts(&c,u64::MAX,u64::MAX),1),Err(Error::Invalid));
    let mut invalid=terminal.checkpoint(); invalid.cursor=0;
    assert_eq!(c.restore(invalid),Err(Error::Stale));
}

fn funding_config()->FundingConfigV2 {
    FundingConfigV2 { version:FUNDING_VERSION,profile:FundingProfileV2::ActualFundingV2,
        material:config(),accrual_policy:None }
}
fn funding_evidence(kind:CostKindV2,from:u64,until:u64)->FundingEvidenceV2 {
    let index=match kind { CostKindV2::RawInput=>0,CostKindV2::ReagentInput=>1,CostKindV2::ElectricityInput=>2,
        CostKindV2::Personnel=>3,CostKindV2::EquipmentOperation=>4,CostKindV2::MaintenanceUse=>5,
        CostKindV2::ResidualAcceptance=>6,CostKindV2::ResidualCustody=>7 };
    let basis=match index { 0=>e(101),1=>e(37),2=>e(17),3=>e(11),4=>e(5),5=>e(7),6=>e(3),_=>e(2) };
    FundingEvidenceV2 { source_ref:100+index,source_revision:3,from,until,purpose_ref:1000+index,
        allocation_ref:200+index,cost_from:e(0),cost_until:basis }
}
fn funding_facts(c:&FundingConfigV2,from:u64,until:u64,raw:Exact,reagent:Exact)->FundingFactsV2 {
    let old=facts(&c.material,from,until);
    let physical=MaterialFactsV2 { binding:old.binding,from,until,read_version:old.read_version,
        raw_stock_available_ml:old.raw_stock_available_ml,reagent_stock_available_mq:old.reagent_stock_available_mq,
        electricity_available_mwh:old.electricity_available_mwh,raw_stream_basis_after:old.raw_stream_basis_after,
        output_free_ml:old.output_free_ml,concentrate_free_ml:old.concentrate_free_ml,sludge_free_mq:old.sludge_free_mq,
        supply_receipt_ref:old.supply_receipt_ref };
    let kinds=[CostKindV2::RawInput,CostKindV2::ReagentInput,CostKindV2::ElectricityInput,CostKindV2::Personnel,
        CostKindV2::EquipmentOperation,CostKindV2::MaintenanceUse,CostKindV2::ResidualAcceptance,CostKindV2::ResidualCustody];
    let amounts=[raw,reagent,e(17),e(11),e(5),e(7),e(3),e(2)];
    let mut items=[None;MAX_FUNDING_ITEMS]; let mut n=0;
    for (i,(kind,basis)) in kinds.into_iter().zip(amounts).enumerate() {
        if basis.is_zero() { continue; }
        let mut evidence=funding_evidence(kind,from,until); evidence.cost_until=basis;
        let coverage=match i { 0..=2=>CoverageV2::OriginalStockConversion { evidence },
            3..=4=>CoverageV2::SamePreparedPaid { evidence },_=>CoverageV2::PriorReceiptPaid { evidence } };
        items[n]=Some(FundingItemV2 { kind,basis,coverage }); n+=1;
    }
    FundingFactsV2 { version:FUNDING_VERSION,physical,
        basis:IntervalBasisV2 { electricity_cents:17,personnel_cents:11,equipment_operation_cents:5,
            maintenance_use_cents:7,residual_acceptance_cents:3,residual_custody_cents:2 },
        items,cash_available_cents:0,funding_revision:1 }
}
/// Exact payload fixture for a single synthetic root view. It proves no real source,
/// payment, unique global allocation, original Envelope, wallet or Store custody.
struct FundingFixture { expected:FundingFactsV2, allow_accrual:bool }
impl RootVerifier for FundingFixture {
    fn verifies(&self,_:&Config,_:&Checkpoint,_:Request,_:&RootFacts)->bool { true }
    fn verifies_funding_v2(&self,c:&FundingConfigV2,base:&FundingCheckpointV2,r:Request,f:&FundingFactsV2)->bool {
        *f==self.expected && f.physical.binding==c.material.binding
            && f.physical.from==base.physical.cursor && f.physical.until==r.until
    }
    fn verifies_accrual_v2(&self,c:&FundingConfigV2,_:&FundingCheckpointV2,_:Request,f:&FundingFactsV2,item:usize)->bool {
        self.allow_accrual && f==&self.expected && matches!(f.items[item],Some(FundingItemV2 {
            coverage:CoverageV2::AccruedPayable { policy,.. },..
        }) if c.accrual_policy==Some(policy))
    }
}
fn funded(c:&FundingConfigV2,s:&FundingStateV2,r:Request,f:&FundingFactsV2)->(FundingStateV2,FundingCandidateV2) {
    let v=FundingFixture { expected:f.clone(),allow_accrual:false };
    let can=c.prepare(&v,s,r,f,1).unwrap();
    let next=c.revalidate(&v,s,&can,f).unwrap(); (next,can)
}
#[test]
fn funding_v2_preserves_legacy_v1_wire_and_cash_gate_including_old_verifiers() {
    let c=config(); let s=c.open().unwrap(); let mut old=facts(&c,0,3600);
    let r=Request { until:3600,input_ml:e(100_000_000) };
    let can=c.prepare(&FixtureVerifier,&s,r,&old,1).unwrap();
    assert_eq!(can.version,1); assert_eq!(can.delta.root_interval_cost_cents,31);
    let config_json=serde_json::to_value(&c).unwrap();
    assert_eq!(config_json.as_object().unwrap().len(),11);
    assert!(config_json.get("funding").is_none()); assert!(config_json.get("accrual_policy").is_none());
    assert_eq!(serde_json::to_value(&can.next).unwrap().as_object().unwrap().len(),6);
    old.cash_available_cents=0;
    assert_eq!(c.prepare(&FixtureVerifier,&s,r,&old,1),Err(Error::UnresolvedCost));
    let v2=funding_config(); let f=funding_facts(&v2,0,3600,e(101),e(37));
    assert_eq!(v2.prepare(&FixtureVerifier,&v2.open().unwrap(),r,&f,1),Err(Error::Unauthorized));
    // Existing idle and positive-input tests above are unchanged; no new verifier
    // adapter adds inventory/paid basis to cash or suppresses legacy electricity basis.
}
#[test]
fn funding_v2_zero_cash_keeps_real_stock_electricity_and_paid_wages_in_full_basis() {
    let c=funding_config(); let s=c.open().unwrap(); let f=funding_facts(&c,0,3600,e(101),e(37));
    let (next,can)=funded(&c,&s,Request { until:3600,input_ml:e(100_000_000) },&f);
    assert_eq!(f.cash_available_cents,0); assert_eq!(f.basis.electricity_cents,17);
    assert_eq!(can.delta.costs.total_basis,e(183)); assert_eq!(can.delta.costs.original_stock,e(155));
    assert_eq!(can.delta.costs.same_prepared_paid,e(16)); assert_eq!(can.delta.costs.prior_receipt_paid,e(12));
    assert_eq!(can.delta.costs.cash_due_now,e(0)); assert_eq!(can.delta.costs.accrued_payable,e(0));
    assert_eq!(can.delta.materials.raw_converted_basis,e(101));
    assert_eq!(can.delta.materials.reagent_converted_basis,e(37));
    let saved:FundingCheckpointV2=serde_json::from_slice(&serde_json::to_vec(&next.checkpoint()).unwrap()).unwrap();
    let restored=c.restore(saved).unwrap(); assert_eq!(c.meters(&restored),c.meters(&next));
    let oracle:serde_json::Value=serde_json::from_str(include_str!("../../../tests/fixtures/water-treatment/funding-v2.json")).unwrap();
    assert_eq!(can.delta.costs.total_basis,e(oracle["expected"]["total_basis_cents"].as_u64().unwrap()));
}
#[test]
fn funding_v2_real_cash_fee_is_not_covered_by_rich_inventory_or_already_paid_wage() {
    let c=funding_config(); let s=c.open().unwrap(); let mut f=funding_facts(&c,0,3600,e(101),e(37));
    let mut item=f.items[7].unwrap(); item.coverage=CoverageV2::CashDueNow { evidence:funding_evidence(item.kind,0,3600) };
    f.items[7]=Some(item); f.cash_available_cents=1;
    let v=FundingFixture { expected:f.clone(),allow_accrual:false };
    let r=Request { until:3600,input_ml:e(100_000_000) };
    assert_eq!(c.prepare(&v,&s,r,&f,1),Err(Error::UnresolvedCost));
    assert_eq!(s,c.open().unwrap());
    f.cash_available_cents=2;
    let (_,can)=funded(&c,&s,r,&f);
    assert_eq!(can.delta.costs.cash_due_now,e(2)); assert_eq!(can.delta.costs.total_basis,e(183));
}
#[test]
fn funding_v2_rejects_wrong_source_interval_amount_duplicate_and_missing_coverage() {
    let c=funding_config(); let s=c.open().unwrap(); let f=funding_facts(&c,0,3600,e(101),e(37));
    let r=Request { until:3600,input_ml:e(100_000_000) };
    let trusted=FundingFixture { expected:f.clone(),allow_accrual:false };
    let mut wrong=f.clone(); let mut item=wrong.items[2].unwrap();
    let mut evidence=funding_evidence(item.kind,0,3600); evidence.source_ref=999;
    item.coverage=CoverageV2::OriginalStockConversion { evidence }; wrong.items[2]=Some(item);
    assert_eq!(c.prepare(&trusted,&s,r,&wrong,1),Err(Error::Unauthorized));
    wrong=f.clone(); let mut item=wrong.items[2].unwrap(); item.basis=e(18); wrong.items[2]=Some(item);
    assert_eq!(c.prepare(&trusted,&s,r,&wrong,1),Err(Error::Unauthorized));
    // Even a fixture accepting the malformed payload cannot bypass local reconciliation.
    let bad=FundingFixture { expected:wrong.clone(),allow_accrual:false };
    assert_eq!(c.prepare(&bad,&s,r,&wrong,1),Err(Error::Invalid));
    wrong=f.clone(); let mut item=wrong.items[2].unwrap();
    item.coverage=CoverageV2::OriginalStockConversion { evidence:funding_evidence(item.kind,1,3600) }; wrong.items[2]=Some(item);
    let bad=FundingFixture { expected:wrong.clone(),allow_accrual:false };
    assert_eq!(c.prepare(&bad,&s,r,&wrong,1),Err(Error::Invalid));
    wrong=f.clone(); let mut item=wrong.items[2].unwrap();
    item.coverage=CoverageV2::OriginalStockConversion { evidence:funding_evidence(CostKindV2::RawInput,0,3600) }; wrong.items[2]=Some(item);
    let bad=FundingFixture { expected:wrong.clone(),allow_accrual:false };
    assert_eq!(c.prepare(&bad,&s,r,&wrong,1),Err(Error::Invalid));
    wrong=f.clone(); wrong.items[7]=None;
    let bad=FundingFixture { expected:wrong.clone(),allow_accrual:false };
    assert_eq!(c.prepare(&bad,&s,r,&wrong,1),Err(Error::Invalid));
    wrong=f.clone(); wrong.items[8]=wrong.items[7]; wrong.items[7]=None;
    let bad=FundingFixture { expected:wrong.clone(),allow_accrual:false };
    assert_eq!(c.prepare(&bad,&s,r,&wrong,1),Err(Error::Invalid));
}
#[test]
fn funding_v2_accrual_requires_original_explicit_policy_and_additional_default_deny_gate() {
    let mut c=funding_config(); let mut f=funding_facts(&c,0,3600,e(101),e(37));
    let policy=AccrualPolicyV2 { policy_ref:700,version:1 };
    let mut item=f.items[3].unwrap();
    item.coverage=CoverageV2::AccruedPayable { evidence:funding_evidence(item.kind,0,3600),policy }; f.items[3]=Some(item);
    let r=Request { until:3600,input_ml:e(100_000_000) };
    let v=FundingFixture { expected:f.clone(),allow_accrual:true };
    assert_eq!(c.prepare(&v,&c.open().unwrap(),r,&f,1),Err(Error::Unauthorized));
    c.accrual_policy=Some(policy); let s=c.open().unwrap();
    let no_accrual=FundingFixture { expected:f.clone(),allow_accrual:false };
    assert_eq!(c.prepare(&no_accrual,&s,r,&f,1),Err(Error::Unauthorized));
    let can=c.prepare(&v,&s,r,&f,1).unwrap();
    assert_eq!(can.delta.costs.accrued_payable,e(11)); assert_eq!(can.delta.costs.total_basis,e(183));
    let next=c.revalidate(&v,&s,&can,&f).unwrap();
    // No settlement API exists here. Later real payment changes only root's original
    // cash/paid/debt receipt; these historical production-cost totals stay unchanged.
    let before=next.checkpoint(); assert_eq!(c.restore(before.clone()).unwrap().checkpoint(),before);
}
#[test]
fn funding_v2_candidate_cas_tamper_versions_sequence_and_overflow_preserve_base() {
    let c=funding_config(); let base=c.open().unwrap(); let f=funding_facts(&c,0,3600,e(101),e(37));
    let r=Request { until:3600,input_ml:e(100_000_000) };
    let (next,can)=funded(&c,&base,r,&f); let v=FundingFixture { expected:f.clone(),allow_accrual:false };
    assert_eq!(c.revalidate(&v,&next,&can,&f),Err(Error::Stale));
    let mut altered=can.clone(); altered.delta.costs.total_basis=e(182);
    assert_eq!(c.revalidate(&v,&base,&altered,&f),Err(Error::Stale));
    altered=can.clone(); altered.next.physical.sequence+=1;
    assert_eq!(c.revalidate(&v,&base,&altered,&f),Err(Error::Stale));
    altered=can.clone(); altered.version=1;
    assert_eq!(c.revalidate(&v,&base,&altered,&f),Err(Error::Stale));
    let mut fresh=f.clone(); fresh.funding_revision+=1;
    assert_eq!(c.revalidate(&v,&base,&can,&fresh),Err(Error::Stale));
    fresh=f.clone(); fresh.physical.binding.root_epoch+=1;
    assert_eq!(c.prepare(&v,&base,r,&fresh,1),Err(Error::Stale));
    fresh=f.clone(); fresh.version=1;
    assert_eq!(c.prepare(&v,&base,r,&fresh,1),Err(Error::Invalid));
    assert_eq!(c.prepare(&v,&base,r,&f,0),Err(Error::WorkBudget));
    assert_eq!(c.prepare(&v,&base,Request { until:3600,input_ml:e(0) },&f,1),Err(Error::Invalid));
    let mut overflow=can.next.clone(); overflow.funding.original_stock=e(MAX_WHOLE);
    overflow.funding.same_prepared_paid=e(1); overflow.funding.total_basis=e(MAX_WHOLE);
    assert_eq!(c.restore(overflow),Err(Error::Overflow));
    let mut old=can.next.clone(); old.version=1; assert_eq!(c.restore(old),Err(Error::Stale));
    let v1_bytes=serde_json::to_vec(&config().open().unwrap().checkpoint()).unwrap();
    assert!(serde_json::from_slice::<FundingCheckpointV2>(&v1_bytes).is_err());
    assert_eq!(base,c.open().unwrap());
}
#[test]
fn funding_v2_stream_fractional_wip_cost_and_checkpoint_do_not_reset_or_reprice() {
    let mut c=funding_config(); c.material.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] };
    let base=c.open().unwrap(); let mut funding=funding_facts(&c,0,1,e(2),f(37,200_000_000));
    funding.physical.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(2),wip:e(5) });
    let (next,can)=funded(&c,&base,Request { until:1,input_ml:f(1,2) },&funding);
    assert_eq!(can.delta.materials.raw_take_basis_cents,7); assert_eq!(can.delta.materials.raw_converted_basis,e(2));
    let saved:FundingCheckpointV2=serde_json::from_slice(&serde_json::to_vec(&next.checkpoint()).unwrap()).unwrap();
    let restored=c.restore(saved).unwrap(); let m=c.meters(&restored).unwrap();
    assert_eq!(m.materials.raw_wip_basis,e(5)); assert_eq!(m.materials.raw_wip.water_ml,f(1,2));
    assert_eq!(m.materials.reagent_wip_mq,e(1).sub(f(1,2_000_000)).unwrap());
    let mut altered=can.clone(); altered.next.physical.raw_stream_basis=Some(StreamBasis { taken_cents:7,converted:e(3),wip:e(4) });
    let v=FundingFixture { expected:funding.clone(),allow_accrual:false };
    assert_eq!(c.revalidate(&v,&base,&altered,&funding),Err(Error::Stale));
}
#[test]
fn funding_v2_one_original_envelope_can_cover_many_distinct_purpose_allocations() {
    let c=funding_config(); let mut f=funding_facts(&c,0,3600,e(101),e(37));
    for item in f.items.iter_mut().flatten() {
        let mut evidence=item.coverage.evidence_for_fixture(); evidence.source_ref=900;
        item.coverage=match item.coverage {
            CoverageV2::OriginalStockConversion { .. }=>CoverageV2::OriginalStockConversion { evidence },
            CoverageV2::SamePreparedPaid { .. }=>CoverageV2::SamePreparedPaid { evidence },
            _=>CoverageV2::PriorReceiptPaid { evidence },
        };
    }
    let (_,can)=funded(&c,&c.open().unwrap(),Request { until:3600,input_ml:e(100_000_000) },&f);
    assert_eq!(can.delta.costs.total_basis,e(183));
}
// Test-side access, not an extra production authorization API.
trait FixtureEvidence { fn evidence_for_fixture(self)->FundingEvidenceV2; }
impl FixtureEvidence for CoverageV2 {
    fn evidence_for_fixture(self)->FundingEvidenceV2 { match self {
        Self::OriginalStockConversion { evidence }|Self::SamePreparedPaid { evidence }
        |Self::PriorReceiptPaid { evidence }|Self::CashDueNow { evidence }
        |Self::AccruedPayable { evidence,.. }=>evidence,
    } }
}
#[test]
fn funding_v2_disjoint_cost_spans_are_legal_but_overlap_or_unknown_remaining_is_rejected() {
    let c=funding_config(); let base=c.open().unwrap(); let mut f=funding_facts(&c,0,3600,e(101),e(37));
    let mut reagent=f.items[1].unwrap(); let mut evidence=funding_evidence(CostKindV2::ReagentInput,0,3600);
    evidence.source_ref=100; evidence.allocation_ref=200; evidence.cost_from=e(101); evidence.cost_until=e(138);
    reagent.coverage=CoverageV2::OriginalStockConversion { evidence }; f.items[1]=Some(reagent);
    let r=Request { until:3600,input_ml:e(100_000_000) };
    let (_,can)=funded(&c,&base,r,&f); assert_eq!(can.delta.costs.total_basis,e(183));
    let trusted=FundingFixture { expected:f.clone(),allow_accrual:false };
    let mut wrong=f.clone(); evidence.cost_from=e(100); evidence.cost_until=e(137);
    reagent.coverage=CoverageV2::OriginalStockConversion { evidence }; wrong.items[1]=Some(reagent);
    let malformed=FundingFixture { expected:wrong.clone(),allow_accrual:false };
    assert_eq!(c.prepare(&malformed,&base,r,&wrong,1),Err(Error::Invalid));
    // A disjoint mathematical span is not proof that the original source funded it.
    evidence.cost_from=e(999); evidence.cost_until=e(1036);
    reagent.coverage=CoverageV2::OriginalStockConversion { evidence }; wrong.items[1]=Some(reagent);
    assert_eq!(c.prepare(&trusted,&base,r,&wrong,1),Err(Error::Unauthorized));
}
#[test]
fn funding_v2_prior_prefetch_wip_can_convert_later_without_another_input_take() {
    let mut c=funding_config(); c.material.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] };
    let base=c.open().unwrap(); let mut first=funding_facts(&c,0,1,e(2),f(37,200_000_000));
    first.physical.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(2),wip:e(5) });
    let (after_first,_)=funded(&c,&base,Request { until:1,input_ml:f(1,2) },&first);
    let mut second=funding_facts(&c,1,2,e(5),f(37,200_000_000));
    second.physical.raw_stock_available_ml=0; second.physical.reagent_stock_available_mq=0;
    second.physical.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(7),wip:e(0) });
    for (i,item) in second.items.iter_mut().flatten().enumerate() {
        let mut evidence=item.coverage.evidence_for_fixture();
        evidence.cost_from=first.items[i].unwrap().basis;
        evidence.cost_until=evidence.cost_from.add(item.basis).unwrap();
        item.coverage=match item.coverage {
            CoverageV2::OriginalStockConversion { .. }=>CoverageV2::OriginalStockConversion { evidence },
            CoverageV2::SamePreparedPaid { .. }=>CoverageV2::SamePreparedPaid { evidence },
            _=>CoverageV2::PriorReceiptPaid { evidence },
        };
    }
    let (after_second,can)=funded(&c,&after_first,Request { until:2,input_ml:f(1,2) },&second);
    assert_eq!(can.delta.materials.raw_take_ml,0); assert_eq!(can.delta.materials.raw_take_basis_cents,0);
    assert_eq!(can.delta.materials.reagent_take_mq,0); assert_eq!(can.delta.materials.raw_converted_basis,e(5));
    assert_eq!(c.meters(&after_second).unwrap().materials.raw_wip_basis,e(0));
}
struct FundingIdleFixture { expected:FundingIdleFactsV2, accepted_work:Option<(u64,u64)> }
impl RootVerifier for FundingIdleFixture {
    fn verifies(&self,_:&Config,_:&Checkpoint,_:Request,_:&RootFacts)->bool { false }
    fn verifies_funding_idle_v2(&self,c:&FundingConfigV2,base:&FundingCheckpointV2,f:&FundingIdleFactsV2)->bool {
        f==&self.expected && f.material.binding==c.material.binding && f.material.from==base.physical.cursor
            && self.accepted_work.map_or(true,|(from,until)| f.material.until<=from || f.material.from>=until)
    }
}
fn funding_idle_facts(c:&FundingConfigV2,from:u64,until:u64)->FundingIdleFactsV2 {
    FundingIdleFactsV2 { version:FUNDING_VERSION,material:idle_facts(&c.material,from,until),funding_revision:1 }
}
#[test]
fn funding_v2_idle_has_own_default_deny_gate_preserves_all_funding_and_fractional_wip() {
    let mut c=funding_config(); c.material.raw_basis=RawBasisMode::RootConfirmedStream { ports:[104,105] };
    let initial=c.open().unwrap(); let z=funding_idle_facts(&c,0,1);
    let explicit=FundingIdleFixture { expected:z.clone(),accepted_work:None };
    let initial_idle=c.cover_idle(&explicit,&initial,&z,1).unwrap();
    let restored=c.restore(c.revalidate_idle(&explicit,&initial,&initial_idle,&z).unwrap().checkpoint()).unwrap();
    assert_eq!(c.meters(&initial),c.meters(&restored));
    let mut physical_facts=funding_facts(&c,0,3,e(2),f(37,200_000_000));
    physical_facts.physical.raw_stream_basis_after=Some(StreamBasis { taken_cents:7,converted:e(2),wip:e(5) });
    let (base,_)=funded(&c,&initial,Request { until:3,input_ml:f(1,2) },&physical_facts);
    let z=funding_idle_facts(&c,3,1000);
    let old_idle=IdleHistory { from:3,until:1000,accepted_work:None };
    assert_eq!(c.cover_idle(&old_idle,&base,&z,1),Err(Error::Unauthorized));
    let production_only=FundingFixture { expected:physical_facts,allow_accrual:false };
    assert_eq!(c.cover_idle(&production_only,&base,&z,1),Err(Error::Unauthorized));
    let explicit=FundingIdleFixture { expected:z.clone(),accepted_work:None };
    let can=c.cover_idle(&explicit,&base,&z,1).unwrap();
    assert_eq!(can.delta.costs,FundingTotalsV2::ZERO);
    assert_eq!(can.delta.materials,MaterialDeltaV2::from(Delta::zero_for_fixture()));
    let next=c.revalidate_idle(&explicit,&base,&can,&z).unwrap();
    assert_eq!(c.meters(&next),c.meters(&base)); assert_eq!(next.checkpoint().funding,base.checkpoint().funding);
    let restored=c.restore(next.checkpoint()).unwrap(); assert_eq!(c.meters(&restored),c.meters(&base));
}
trait FixtureZeroDelta { fn zero_for_fixture()->Delta; }
impl FixtureZeroDelta for Delta {
    fn zero_for_fixture()->Delta { Delta { input_ml:e(0),reagent_mq:e(0),electricity_mwh:e(0),
        product:Composition::known_zero(e(0)),concentrate:Composition::known_zero(e(0)),sludge:Composition::known_zero(e(0)),
        raw_take_ml:0,reagent_take_mq:0,raw_take_basis_cents:0,reagent_take_basis_cents:0,
        raw_converted_basis:e(0),reagent_converted_basis:e(0),root_interval_cost_cents:0 } }
}
#[test]
fn funding_v2_idle_rejects_accepted_work_stale_and_cost_coverage_tampering() {
    let c=funding_config(); let base=c.open().unwrap(); let z=funding_idle_facts(&c,0,1000);
    let explicit=FundingIdleFixture { expected:z.clone(),accepted_work:None };
    let can=c.cover_idle(&explicit,&base,&z,1).unwrap();
    let accepted=FundingIdleFixture { expected:z.clone(),accepted_work:Some((50,51)) };
    assert_eq!(c.revalidate_idle(&accepted,&base,&can,&z),Err(Error::Unauthorized));
    let next=c.revalidate_idle(&explicit,&base,&can,&z).unwrap();
    assert_eq!(c.revalidate_idle(&explicit,&next,&can,&z),Err(Error::Stale));
    let mut bad=can.clone(); bad.delta.costs.cash_due_now=e(1);
    assert_eq!(c.revalidate_idle(&explicit,&base,&bad,&z),Err(Error::Stale));
    bad=can.clone(); bad.next.funding.total_basis=e(1);
    assert_eq!(c.revalidate_idle(&explicit,&base,&bad,&z),Err(Error::Stale));
    bad=can.clone(); bad.delta.materials.reagent_take_mq=1;
    assert_eq!(c.revalidate_idle(&explicit,&base,&bad,&z),Err(Error::Stale));
    let mut fresh=z.clone(); fresh.funding_revision+=1;
    assert_eq!(c.revalidate_idle(&explicit,&base,&can,&fresh),Err(Error::Stale));
    fresh=z.clone(); fresh.version=1;
    assert_eq!(c.cover_idle(&explicit,&base,&fresh,1),Err(Error::Invalid));
    assert_eq!(c.cover_idle(&explicit,&base,&z,0),Err(Error::WorkBudget));
}
#[test]
fn funding_v2_fixed_envelope_checks_are_not_real_root_resource_acceptance() {
    let c=funding_config(); let (s,can)=funded(&c,&c.open().unwrap(),Request { until:3600,input_ml:e(100_000_000) },
        &funding_facts(&c,0,3600,e(101),e(37)));
    assert!(std::mem::size_of::<FundingFactsV2>()+std::mem::size_of::<FundingCandidateV2>()<WORKSPACE_PLANNING_CAP);
    assert!(std::mem::size_of::<FundingConfigV2>()+std::mem::size_of::<FundingStateV2>()<PERSISTENT_PLANNING_CAP);
    assert!(serde_json::to_vec(&(c,s.checkpoint())).unwrap().len()<=SAVED_ENVELOPE_ESTIMATE);
    assert!(serde_json::to_vec(&can).unwrap().len()<=CANDIDATE_ENVELOPE_ESTIMATE);
    // Original shared root, allocator, stack, revalidation and cold-restore overlap
    // still need root's actual admission/measurement; these fixed DTO checks do not prove it.
}
