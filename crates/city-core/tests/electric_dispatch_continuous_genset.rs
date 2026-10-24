//! Explicit synthetic receipts/Stock lots. Test source only; this session runs nothing.
use mycity_core::{arena, units};
#[path = "../src/electric_dispatch/continuous_genset.rs"]
mod continuous_genset;
use continuous_genset::*;
use arena::Scope;
use units::{Money, Quantity};
fn q(n:i64)->Quantity { Quantity::new(n).unwrap() }
fn m(n:i64)->Money { Money::new(n).unwrap() }
fn config()->Config {
    Config {version:VERSION,profile:Profile::ContinuousGensetV1,scope:Scope::new(1,7).unwrap(),
        city_id:42,clock:ClockBinding::Development {city_id:42},genset_id:86,source_version:1,reception_denominator:9,
        // Synthetic prices only: Fuel 1cent/mQ; water 3cent/L. Not profile prices.
        fuel:InputLot {stock_id:101,lot_ref:1001,quantity:q(100_000),basis:m(100_000)},
        water:InputLot {stock_id:102,lot_ref:1002,quantity:q(6_000_000),basis:m(18_000)},
        heat_sink_id:103,wastewater_sink_id:104,heat_capacity_mwh:100_000_000,heat_capacity_remainder_numerator:0,
        wastewater_capacity_ml:6_000_000,heat_peak_w:60_000,wastewater_peak_ml_per_second:1000,
        consumer_ref:Some(90),consumer_version:1}
}
fn instant(c:&Config,second:u64)->Instant { Instant {scope:c.scope,clock:c.clock,second} }
fn opening(c:&Config)->State { ContinuousGensetV1::opening(c,instant(c,0)).unwrap() }
fn view(c:&Config,s:&State,at:u64)->AuthorityView {
    let meters=ContinuousGensetV1::meters(c,s).unwrap();
    let den=c.reception_denominator;
    let free_heat=(c.heat_capacity_mwh*den+c.heat_capacity_remainder_numerator)
        -(meters.heat_mwh*den+meters.heat_remainder_numerator);
    AuthorityView {at:instant(c,at),root:[8;32],read_version:1,source_version:c.source_version,
        observed_at:s.cursor,covers_from:s.cursor,covers_until:at,max_age:10_000,
        fuel_materialization:Materialization::ExistingStock,water_materialization:Materialization::ExistingStock,
        fuel_arrived_at:0,water_arrived_at:0,
        fuel_available_mq:q(c.fuel.quantity.value()-(s.fuel_taken_mq-s.fuel_returned_mq) as i64),
        water_available_ml:q(c.water.quantity.value()-(s.water_taken_ml-s.water_returned_ml) as i64),
        heat_free_mwh:free_heat/den,heat_free_remainder_numerator:free_heat%den,
        wastewater_free_ml:c.wastewater_capacity_ml-meters.wastewater_ml,payer_available:m(10_000)}
}
fn start(c:&Config,s:&State,at:u64,run_id:u64)->Candidate {
    ContinuousGensetV1::propose(c,s,&view(c,s,at),Command::Start {at:instant(c,at),run_id}).unwrap()
}
fn command(c:&Config,s:&State,at:u64,received_delta:u64)->Command {
    command_exact(c,s,at,received_delta,0)
}
fn command_exact(c:&Config,s:&State,at:u64,whole_delta:u64,remainder_delta:u64)->Command {
    let den=c.reception_denominator;
    let received=(s.received_mwh+whole_delta)*den+s.received_remainder_numerator+remainder_delta;
    Command::Advance {at:instant(c,at),reception:Reception {scope:c.scope,clock:c.clock,
        genset_id:c.genset_id,source_version:c.source_version,run_id:s.run_id,
        consumer_ref:c.consumer_ref,consumer_version:c.consumer_version,receipt_ref:900,
        root:[8;32],from:s.cursor,until:at,cumulative_received_mwh:received/den,received_remainder_numerator:received%den,
        actual_peak_w:if whole_delta==0 && remainder_delta==0 {0} else {24_000}}}
}
fn advance(c:&Config,s:&State,at:u64,received_delta:u64)->Candidate {
    ContinuousGensetV1::propose(c,s,&view(c,s,at),command(c,s,at,received_delta)).unwrap()
}
fn stop(c:&Config,s:&State)->Candidate {
    ContinuousGensetV1::propose(c,s,&view(c,s,s.cursor),Command::Stop {at:instant(c,s.cursor)}).unwrap()
}
fn no_load_run(c:&Config,step:u64,total:u64)->State {
    let mut s=start(c,&opening(c),0,1).next;
    while s.cursor<total {
        let target=(s.cursor+step).min(total);
        s=advance(c,&s,target,0).next;
    }
    s
}
#[test]
fn start_is_one_fee_and_material_wip_not_combustion() {
    let c=config();let b=opening(&c);let p=start(&c,&b,0,1);
    assert_eq!(b,opening(&c));assert_eq!(p.delta.start_fee,m(300));
    assert_eq!(p.delta.fuel_take_mq,17);assert_eq!(p.delta.water_take_ml,1000);
    assert_eq!(p.delta.fuel_take_basis,m(17));assert_eq!(p.delta.water_take_basis,m(3));
    assert_eq!(p.delta.fuel_burned_thirds_mq,0);assert_eq!(p.delta.electricity_generated_mwh,0);
    assert_eq!(p.delta.after.fuel_wip_thirds_mq,51);assert_eq!(p.delta.after.water_wip_ml,1000);
    assert_eq!(p.delta.after.fuel_wip_basis,m(17));assert_eq!(p.delta.after.water_wip_basis,m(3));
    assert_eq!(ContinuousGensetV1::confirm_actual_basis(&p,&ActualBasis {
        fuel_take:m(17),water_take:m(3),fuel_return:m(0),water_return:m(0),fee_paid:m(300)}),Ok(()));
    assert_eq!(ContinuousGensetV1::confirm_actual_basis(&p,&ActualBasis {
        fuel_take:m(300),water_take:m(0),fuel_return:m(0),water_return:m(0),fee_paid:m(300)}),Err(Error::Basis));
}
#[test]
fn literal_one_second_conservation_with_fractional_fuel_and_electric_wip() {
    let c=config();let started=start(&c,&opening(&c),0,1);let p=advance(&c,&started.next,1,0);
    let z=&p.delta.after;
    assert_eq!(p.delta.fuel_take_mq,0);assert_eq!(p.delta.water_take_ml,0);
    assert_eq!(z.fuel_burned_thirds_mq,50);assert_eq!(z.fuel_wip_thirds_mq,1);
    assert_eq!(z.water_consumed_ml,1000);assert_eq!(z.water_wip_ml,0);
    assert_eq!(z.electricity_generated_mwh,6666);assert_eq!(z.electricity_remainder_numerator,6);
    assert_eq!(z.intrinsic_heat_mwh,10_000);assert_eq!(z.heat_mwh,16_666);
    assert_eq!(z.wastewater_ml,1000);assert_eq!(z.fuel_converted_basis,m(16));
    assert_eq!(z.fuel_wip_basis,m(1));assert_eq!(z.water_converted_basis,m(3));
    // The generated fraction is already in Heat, not a second output WIP balance.
    assert_eq!(z.heat_remainder_numerator,6);
    assert_eq!((p.next.received_mwh+z.heat_mwh)*9+p.next.received_remainder_numerator+z.heat_remainder_numerator,150_000);
    assert_eq!(p.delta.start_fee,m(0));assert_eq!(z.start_fees,m(300));
}
#[test]
fn literal_sixty_seconds_actual_reception_no_electric_stock() {
    let c=config();let b=start(&c,&opening(&c),0,1).next;let p=advance(&c,&b,60,400_000);
    assert_eq!(p.next.active_seconds,60);assert_eq!(p.next.fuel_taken_mq,1000);
    assert_eq!(p.next.water_taken_ml,60_000);assert_eq!(p.next.received_mwh,400_000);
    assert_eq!(p.delta.after.fuel_burned_thirds_mq,3000);assert_eq!(p.delta.after.fuel_wip_thirds_mq,0);
    assert_eq!(p.delta.after.intrinsic_heat_mwh,600_000);assert_eq!(p.delta.after.heat_mwh,600_000);
    assert_eq!(p.delta.after.rejected_electricity_heat_mwh,0);assert_eq!(p.delta.after.wastewater_ml,60_000);
    assert_eq!(p.delta.after.fuel_converted_basis,m(1000));assert_eq!(p.delta.after.water_converted_basis,m(180));
    assert_eq!(p.delta.after.start_fees,m(300));
}
#[test]
fn one_sixty_and_hour_split_have_identical_physics_carry_cost_and_run() {
    let c=config();let seconds=no_load_run(&c,1,3600);let minutes=no_load_run(&c,60,3600);
    let hour=no_load_run(&c,3600,3600);
    assert_eq!(seconds,minutes);assert_eq!(minutes,hour);
    let z=ContinuousGensetV1::meters(&c,&hour).unwrap();
    assert_eq!(hour.fuel_taken_mq,60_000);assert_eq!(hour.water_taken_ml,3_600_000);
    assert_eq!(z.electricity_generated_mwh,24_000_000);assert_eq!(z.intrinsic_heat_mwh,36_000_000);
    assert_eq!(z.heat_mwh,60_000_000);assert_eq!(z.wastewater_ml,3_600_000);
    assert_eq!(z.start_fees,m(300));assert_eq!(hour.run_count,1);
}
#[test]
fn cold_json_restore_then_split_retains_all_meters_and_run() {
    let c=config();let s=no_load_run(&c,1,7);
    let bytes=serde_json::to_vec(&s).unwrap();let digest=ContinuousGensetV1::state_digest(&s).unwrap();
    let restored=ContinuousGensetV1::restore(&c,&bytes,digest).unwrap();assert_eq!(restored,s);
    let p=advance(&c,&restored,60,0);assert_eq!(p.next,no_load_run(&c,60,60));
    assert_eq!(ContinuousGensetV1::restore(&c,&bytes,[0;32]),Err(Error::Restore));
    let mut bad=s.clone();bad.received_mwh=999_999;
    assert_eq!(ContinuousGensetV1::restore(&c,&serde_json::to_vec(&bad).unwrap(),digest),Err(Error::Restore));
    assert_eq!(ContinuousGensetV1::restore(&c,&[b' ';MAX_RESTORE_BYTES+1],digest),Err(Error::Restore));
}
#[test]
fn no_source_no_future_water_no_funding_start_is_atomic() {
    let c=config();let b=opening(&c);let cmd=Command::Start {at:instant(&c,0),run_id:1};
    let mut v=view(&c,&b,0);v.fuel_available_mq=q(0);
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,cmd.clone()),Err(Error::Unresolved(Constraint::Input)));
    let mut v=view(&c,&b,0);v.water_available_ml=q(0);
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,cmd.clone()),Err(Error::Unresolved(Constraint::Input)));
    let mut v=view(&c,&b,0);v.water_materialization=Materialization::FutureProduction;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,cmd.clone()),Err(Error::Unknown));
    let mut v=view(&c,&b,0);v.water_arrived_at=1;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,cmd.clone()),Err(Error::Unknown));
    let mut v=view(&c,&b,0);v.payer_available=m(299);
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,cmd),Err(Error::Unresolved(Constraint::Funding)));
    assert_eq!(b,opening(&c));assert_eq!(ContinuousGensetV1::meters(&c,&b).unwrap().start_fees,m(0));
}
#[test]
fn tiny_fuel_cannot_be_a_whole_cycle_or_produce_before_take() {
    let mut c=config();c.fuel.quantity=q(10);c.fuel.basis=m(10);let b=opening(&c);
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&view(&c,&b,0),Command::Start {at:instant(&c,0),run_id:1}),
        Err(Error::Unresolved(Constraint::Input)));
    c.fuel.quantity=q(17);c.fuel.basis=m(17);let s=start(&c,&opening(&c),0,1).next;
    let p=advance(&c,&s,1,0);assert_eq!(p.delta.electricity_generated_mwh,6666);
    assert_eq!(ContinuousGensetV1::propose(&c,&s,&view(&c,&s,60),command(&c,&s,60,0)),
        Err(Error::Unresolved(Constraint::Input)));
    assert_eq!(s.active_seconds,0);assert_eq!(s.received_mwh,0);
}
#[test]
fn immediate_stop_or_fault_returns_unconverted_wip_and_its_actual_basis() {
    let c=config();let s=start(&c,&opening(&c),0,1).next;
    let p=stop(&c,&s);
    assert_eq!(p.delta.fuel_return_mq,17);assert_eq!(p.delta.water_return_ml,1000);
    assert_eq!(p.delta.fuel_return_basis,m(17));assert_eq!(p.delta.water_return_basis,m(3));
    assert_eq!(p.delta.after.fuel_wip_thirds_mq,0);assert_eq!(p.delta.after.water_wip_ml,0);
    assert_eq!(p.delta.after.start_fees,m(300));assert_eq!(p.delta.start_fee,m(0));
    let f=ContinuousGensetV1::propose(&c,&s,&view(&c,&s,0),Command::Fault {at:instant(&c,0),fault_ref:88}).unwrap();
    assert_eq!(f.delta,p.delta);assert_eq!(f.next.status,Status::Faulted {fault_ref:88});
}
#[test]
fn burned_fuel_and_fee_are_not_refunded_fraction_survives_restart() {
    let c=config();let s=no_load_run(&c,1,1);
    let f=ContinuousGensetV1::propose(&c,&s,&view(&c,&s,1),Command::Fault {at:instant(&c,1),fault_ref:88}).unwrap();
    assert_eq!(f.delta.fuel_return_mq,0);assert_eq!(f.delta.water_return_ml,0);
    assert_eq!(f.delta.after.fuel_burned_thirds_mq,50);assert_eq!(f.delta.after.fuel_wip_thirds_mq,1);
    let bytes=serde_json::to_vec(&f.next).unwrap();let hash=ContinuousGensetV1::state_digest(&f.next).unwrap();
    let old=ContinuousGensetV1::restore(&c,&bytes,hash).unwrap();
    let restarted=start(&c,&old,10,2);assert_eq!(restarted.delta.start_fee,m(300));
    assert_eq!(restarted.next.run_count,2);assert_eq!(restarted.delta.fuel_take_mq,17);
    assert_eq!(restarted.delta.after.fuel_wip_thirds_mq,52);
    let after=advance(&c,&restarted.next,12,0);
    assert_eq!(after.next.active_seconds,3);assert_eq!(after.delta.after.fuel_burned_thirds_mq,150);
    assert_eq!(after.delta.after.electricity_generated_mwh,20_000);
    assert_eq!(after.delta.after.fuel_wip_thirds_mq,0);assert_eq!(after.delta.after.start_fees,m(600));
    assert_eq!(after.next.cursor,12); // stopped interval generated nothing
}
#[test]
fn one_run_one_fee_no_query_retry_fee_new_run_new_fee() {
    let c=config();let b=opening(&c);let p=start(&c,&b,0,1);
    assert_eq!(p,start(&c,&b,0,1)); // same proposal only; original op dedup belongs to root
    assert_eq!(ContinuousGensetV1::propose(&c,&p.next,&view(&c,&p.next,0),p.command.clone()),Err(Error::Boundary));
    let s=no_load_run(&c,60,3600);assert_eq!(ContinuousGensetV1::meters(&c,&s).unwrap().start_fees,m(300));
    let stopped=stop(&c,&s).next;
    assert_eq!(ContinuousGensetV1::propose(&c,&stopped,&view(&c,&stopped,3600),Command::Start {at:instant(&c,3600),run_id:1}),Err(Error::Boundary));
    let restarted=start(&c,&stopped,3601,2);assert_eq!(restarted.delta.start_fee,m(300));
    assert_eq!(restarted.delta.after.start_fees,m(600));
}
#[test]
fn sink_sixty_kw_and_liter_per_second_never_averaged() {
    let mut c=config();c.heat_peak_w=59_999;let b=opening(&c);
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&view(&c,&b,0),Command::Start {at:instant(&c,0),run_id:1}),
        Err(Error::Unresolved(Constraint::SinkPeak)));
    c.heat_peak_w=60_000;c.wastewater_peak_ml_per_second=999;let b=opening(&c);
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&view(&c,&b,0),Command::Start {at:instant(&c,0),run_id:1}),
        Err(Error::Unresolved(Constraint::SinkPeak)));
    let mut c=config();c.heat_capacity_mwh=60_000_000;let b=start(&c,&opening(&c),0,1).next;
    let p=advance(&c,&b,3600,0);assert_eq!(p.delta.heat_mwh,60_000_000);
    let mut v=view(&c,&b,3600);v.heat_free_mwh=59_999_999;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,3600,0)),Err(Error::Unresolved(Constraint::SinkCapacity)));
    let mut v=view(&c,&b,3600);v.wastewater_free_ml=3_599_999;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,3600,0)),Err(Error::Unresolved(Constraint::SinkCapacity)));
    assert_eq!(b.active_seconds,0);assert_eq!(b.received_mwh,0);
}
#[test]
fn actual_reception_is_window_limited_and_must_have_a_legal_peak() {
    let c=config();let b=start(&c,&opening(&c),0,1).next;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&view(&c,&b,1),command(&c,&b,1,6667)),Err(Error::Invalid));
    let s=advance(&c,&b,1,0).next;
    // Prior rejected electric is Heat; cannot sell it in next window.
    assert_eq!(ContinuousGensetV1::propose(&c,&s,&view(&c,&s,2),command(&c,&s,2,10_000)),Err(Error::Invalid));
    let mut cmd=command(&c,&s,2,6667);
    if let Command::Advance {reception,..}=&mut cmd {reception.actual_peak_w=24_001;}
    assert_eq!(ContinuousGensetV1::propose(&c,&s,&view(&c,&s,2),cmd),Err(Error::Invalid));
    let mut cmd=command(&c,&s,2,6667);
    if let Command::Advance {reception,..}=&mut cmd {reception.actual_peak_w=20_000;}
    assert_eq!(ContinuousGensetV1::propose(&c,&s,&view(&c,&s,2),cmd),Err(Error::Invalid));
    let good=ContinuousGensetV1::propose(&c,&s,&view(&c,&s,2),command_exact(&c,&s,2,6666,6)).unwrap();
    assert_eq!(good.delta.heat_mwh,10_000);assert_eq!(good.delta.heat_remainder_numerator,0);
}
#[test]
fn source_versions_clock_scope_roots_and_coverage_reject_old_data() {
    let c=config();let b=start(&c,&opening(&c),0,1).next;
    let mut v=view(&c,&b,1);v.source_version+=1;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,1,0)),Err(Error::Stale));
    let mut v=view(&c,&b,1);v.at.clock=ClockBinding::Interaction;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,1,0)),Err(Error::Clock));
    let mut v=view(&c,&b,1);v.at.scope=Scope::new(1,8).unwrap();
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,1,0)),Err(Error::Clock));
    let mut v=view(&c,&b,1);v.covers_until=0;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,1,0)),Err(Error::Stale));
    let mut v=view(&c,&b,1);v.max_age=0;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,1,0)),Err(Error::Stale));
    let mut cmd=command(&c,&b,1,0);
    if let Command::Advance {reception,..}=&mut cmd {reception.root=[9;32];}
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&view(&c,&b,1),cmd),Err(Error::Stale));
    let mut wrong=c.clone();wrong.clock=ClockBinding::World;
    assert_eq!(ContinuousGensetV1::opening(&wrong,instant(&wrong,0)),Err(Error::Clock));
    let mut changed=c.clone();changed.source_version+=1;
    assert_eq!(ContinuousGensetV1::meters(&changed,&b),Err(Error::Version));
}
#[test]
fn candidate_forgery_stale_root_and_old_base_are_not_receipts() {
    let c=config();let b=start(&c,&opening(&c),0,1).next;let p=advance(&c,&b,60,400_000);
    assert_eq!(ContinuousGensetV1::revalidate(&c,&b,&p.view,&p),Ok(p.next.clone()));
    let mut bad=p.clone();bad.delta.heat_mwh-=1;
    assert_eq!(ContinuousGensetV1::revalidate(&c,&b,&p.view,&bad),Err(Error::Stale));
    let mut v=p.view.clone();v.read_version+=1;
    assert_eq!(ContinuousGensetV1::revalidate(&c,&b,&v,&p),Err(Error::Stale));
    assert_eq!(ContinuousGensetV1::revalidate(&c,&p.next,&p.view,&p),Err(Error::Stale));
}
#[test]
fn fractional_basis_from_same_origin_is_query_split_invariant() {
    let mut c=config();c.fuel.basis=m(13_333);c.water.basis=m(91);
    let one=no_load_run(&c,1,3599);let chunk=no_load_run(&c,60,3599);
    assert_eq!(one,chunk);
    let z=ContinuousGensetV1::meters(&c,&one).unwrap();
    assert_eq!(z.fuel_held_basis.checked_add(z.water_held_basis).unwrap(),
        z.fuel_converted_basis.checked_add(z.water_converted_basis).unwrap()
            .checked_add(z.fuel_wip_basis).unwrap().checked_add(z.water_wip_basis).unwrap());
}
#[test]
fn fault_and_stop_require_exact_processed_cursor_and_zero_new_output() {
    let c=config();let s=no_load_run(&c,1,7);
    assert_eq!(ContinuousGensetV1::propose(&c,&s,&view(&c,&s,8),Command::Fault {at:instant(&c,8),fault_ref:88}),Err(Error::Boundary));
    let p=stop(&c,&s);assert_eq!(p.delta.fuel_burned_thirds_mq,0);assert_eq!(p.delta.heat_mwh,0);
    assert_eq!(p.delta.electricity_generated_mwh,0);assert_eq!(p.next.active_seconds,7);
    assert_eq!(ContinuousGensetV1::propose(&c,&p.next,&view(&c,&p.next,8),command(&c,&p.next,8,0)),Err(Error::Boundary));
}
#[test]
fn output_fraction_is_counted_once_in_heat_and_cost_tail_stays_in_input_wip() {
    let c=config();let s=no_load_run(&c,1,1);let p=stop(&c,&s);let z=&p.delta.after;
    assert_eq!(z.fuel_basis_remainder,200_000);assert_eq!(z.fuel_basis_denominator,300_000);
    assert_eq!(z.water_basis_remainder,0);assert_eq!(z.water_basis_denominator,6_000_000);
    let fuel_net=p.next.fuel_taken_mq-p.next.fuel_returned_mq;
    assert_eq!(fuel_net*9000,z.fuel_wip_thirds_mq*3000+
        (z.heat_mwh+p.next.received_mwh)*9+z.heat_remainder_numerator+p.next.received_remainder_numerator);
    assert_eq!(z.fuel_held_basis,z.fuel_converted_basis.checked_add(z.fuel_wip_basis).unwrap());
    assert_eq!(z.fuel_wip_thirds_mq,1);assert_eq!(z.electricity_remainder_numerator,6);
    assert_eq!(z.heat_remainder_numerator,6);
}
#[test]
fn future_water_loop_duplicate_source_and_foreign_run_reception_are_refused() {
    let mut c=config();c.water.stock_id=c.wastewater_sink_id;
    assert_eq!(ContinuousGensetV1::opening(&c,instant(&c,0)),Err(Error::Invalid));
    let c=config();let b=start(&c,&opening(&c),0,1).next;
    let mut cmd=command(&c,&b,1,0);
    if let Command::Advance {reception,..}=&mut cmd {reception.run_id=2;}
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&view(&c,&b,1),cmd),Err(Error::Stale));
    let mut cmd=command(&c,&b,1,0);
    if let Command::Advance {reception,..}=&mut cmd {reception.genset_id=87;}
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&view(&c,&b,1),cmd),Err(Error::Stale));
}
#[test]
fn after_sink_fills_no_implicit_capacity_renewal_and_no_state_mutation() {
    let mut c=config();c.heat_capacity_mwh=16_666;c.heat_capacity_remainder_numerator=6;
    let started=start(&c,&opening(&c),0,1).next;
    let full=advance(&c,&started,1,0).next;
    let mut v=view(&c,&full,2);v.heat_free_mwh=100_000_000; // fake renewed snapshot cannot extend fixed allowance
    assert_eq!(ContinuousGensetV1::propose(&c,&full,&v,command(&c,&full,2,0)),Err(Error::Unresolved(Constraint::SinkCapacity)));
    assert_eq!(full.active_seconds,1);assert_eq!(full.fuel_taken_mq,17);
    assert_eq!(ContinuousGensetV1::meters(&c,&full).unwrap().heat_mwh,16_666);
}
fn steady_hundred_watt_run(c:&Config,step:u64,total:u64)->State {
    let mut s=start(c,&opening(c),0,1).next;
    while s.cursor<total {
        let target=(s.cursor+step).min(total);
        // Independent 100W oracle: 100Wh/h = 250/9 mWh per game second.
        let received9=target*250;
        let mut cmd=command_exact(c,&s,target,0,0);
        if let Command::Advance {reception,..}=&mut cmd {
            reception.cumulative_received_mwh=received9/9;
            reception.received_remainder_numerator=received9%9;
            reception.actual_peak_w=100;
        }
        s=ContinuousGensetV1::propose(c,&s,&view(c,&s,target),cmd).unwrap().next;
    }
    s
}
#[test]
fn hundred_watt_exact_reception_one_second_has_no_false_heat_or_peak_jump() {
    let c=config();let s=steady_hundred_watt_run(&c,1,1);let z=ContinuousGensetV1::meters(&c,&s).unwrap();
    assert_eq!(s.received_mwh,27);assert_eq!(s.received_remainder_numerator,7);
    assert_eq!(z.electricity_generated_mwh,6666);assert_eq!(z.electricity_remainder_numerator,6);
    assert_eq!(z.rejected_electricity_heat_mwh,6638);assert_eq!(z.rejected_electricity_heat_remainder_numerator,8);
    assert_eq!(z.heat_mwh,16_638);assert_eq!(z.heat_remainder_numerator,8);
    assert_eq!((z.heat_mwh+s.received_mwh)*9+z.heat_remainder_numerator+s.received_remainder_numerator,150_000);
}
#[test]
fn hundred_watt_hour_one_sixty_hour_queries_are_exactly_equivalent() {
    let c=config();let seconds=steady_hundred_watt_run(&c,1,3600);
    assert_eq!(seconds,steady_hundred_watt_run(&c,60,3600));
    assert_eq!(seconds,steady_hundred_watt_run(&c,3600,3600));
    assert_eq!(seconds.received_mwh,100_000);assert_eq!(seconds.received_remainder_numerator,0);
    let z=ContinuousGensetV1::meters(&c,&seconds).unwrap();
    assert_eq!(z.heat_mwh,59_900_000);assert_eq!(z.heat_remainder_numerator,0);
    assert_eq!(z.electricity_generated_mwh,24_000_000);
}
#[test]
fn physical_exact_delta_and_whole_projection_are_distinct_without_energy_loss() {
    let c=config();let s=start(&c,&opening(&c),0,1).next;let first=advance(&c,&s,1,0);
    let second=advance(&c,&first.next,2,0);
    assert_eq!(second.delta.heat_mwh,16_666);assert_eq!(second.delta.heat_remainder_numerator,6);
    assert_eq!(second.delta.whole_meters.heat_mwh,16_667);
    assert_eq!(second.delta.after.heat_mwh,33_333);assert_eq!(second.delta.after.heat_remainder_numerator,3);
    assert_eq!(first.delta.whole_meters.heat_mwh+second.delta.whole_meters.heat_mwh,33_333);
}
#[test]
fn fractional_heat_capacity_is_exact_not_rounded_down_or_up() {
    let c=config();let b=start(&c,&opening(&c),0,1).next;
    let mut v=view(&c,&b,1);v.heat_free_mwh=16_666;v.heat_free_remainder_numerator=5;
    assert_eq!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,1,0)),Err(Error::Unresolved(Constraint::SinkCapacity)));
    v.heat_free_remainder_numerator=6;
    assert!(ContinuousGensetV1::propose(&c,&b,&v,command(&c,&b,1,0)).is_ok());
}
#[test]
fn precision_is_rejected_before_start_and_stop_keeps_original_fractions() {
    let mut c=config();c.reception_denominator=MAX_RECEPTION_DENOMINATOR+3;
    assert_eq!(ContinuousGensetV1::opening(&c,instant(&c,0)),Err(Error::Precision));
    c.reception_denominator=8;assert_eq!(ContinuousGensetV1::opening(&c,instant(&c,0)),Err(Error::Precision));
    let c=config();let s=steady_hundred_watt_run(&c,1,1);let hash=ContinuousGensetV1::state_digest(&s).unwrap();
    let bytes=serde_json::to_vec(&s).unwrap();
    let restored=ContinuousGensetV1::restore(&c,&bytes,hash).unwrap();
    let mut v=view(&c,&restored,1);v.heat_free_remainder_numerator=u64::MAX;
    // Stop needs no new sink admission and does not renegotiate precision.
    let p=ContinuousGensetV1::propose(&c,&restored,&v,Command::Stop {at:instant(&c,1)}).unwrap();
    assert_eq!(p.next.received_remainder_numerator,7);assert_eq!(p.delta.after.heat_remainder_numerator,8);
    assert_eq!(p.delta.after.fuel_wip_thirds_mq,1);
    let mut other=c.clone();other.reception_denominator=3600;
    assert_eq!(ContinuousGensetV1::meters(&other,&p.next),Err(Error::Version));
}
