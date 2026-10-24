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
