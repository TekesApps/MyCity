//! Original-ledger production prefix costs. This module is arithmetic, not a
//! provider, materialization grant, price estimate or additional cost ledger.
use crate::{continuous_genset::InputLot, transaction::TxError};

pub(crate) fn lot_prefix(lot:&InputLot, amount_num:u128, amount_den:u128)->Result<u64,TxError> {
    let qty=u128::try_from(lot.quantity.value()).map_err(|_|TxError::Stock)?;
    let cents=u128::try_from(lot.basis.value()).map_err(|_|TxError::Stock)?;
    if qty==0 || amount_den==0 { return Err(TxError::InvalidOperation); }
    let den=qty.checked_mul(amount_den).ok_or(TxError::Overflow)?;
    if amount_num>den { return Err(TxError::Stock); }
    let value=cents.checked_mul(amount_num).ok_or(TxError::Overflow)?/den;
    u64::try_from(value).map_err(|_|TxError::Overflow)
}

/// Explicit proposed V2 two-identical-pump policy. The root constructs this only
/// from an authenticated original allocation-cycle origin, never a Timeline/P5
/// cut, current cursor or model DTO. This origin and its start fee stay fixed
/// throughout the same physical/cost policy; only observed bounds grow. No V2
/// runtime invokes it yet. V1 retains its existing all-cost-to-process-water rule.
///
/// Pumps each produce10,000mL/s. Half of the genset's actual converted-input cost
/// funds process water; the other half (including odd-cent remainder) belongs to
/// actual treatment or explicitly retained WIP/loss, never to old process water.
#[derive(Clone, Debug)]
pub(crate) struct TwinPortAllocationCycle<'a> {
    pub fuel:&'a InputLot,
    pub cooling:&'a InputLot,
    pub raw:&'a InputLot,
    pub origin_active_seconds:u64,
    pub origin_raw_ml:u64,
    /// Actual observed production bound, not a denominator/future cost allocator.
    pub observed_each_ml:u64,
    /// Frozen water-side share of actual original startup payment. Root keeps
    /// the complementary share and zero-production WIP/expense independently.
    pub water_start_cents:u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Prefix {
    pub ports_ml:[u64;2],
    pub ports_cents:[u64;2],
}

/// V2 arithmetic only. Root must bind these terms/carry to an original contract
/// and the actual covered allocation-cycle prefix before using this price.
/// Paid cash is deliberately absent: paying an earned wage later cannot reprice
/// a previously produced or transferred water prefix. This is no work grant.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LaborPrefix {
    pub hourly_cents_per_member:u64,
    pub members:u64,
    pub carried_numerator:u64,
    pub actual_seconds:u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LaborShares {
    pub water:u64,
    pub other:u64,
}

impl LaborPrefix {
    /// Exact position inside already covered work; no future input or final
    /// output denominator. Original wage carry survives a real cycle boundary.
    pub(crate) fn earned_at(&self,num:u128,den:u128)->Result<u64,TxError> {
        if den==0||self.members==0||self.carried_numerator>=3600 {
            return Err(TxError::InvalidOperation);
        }
        let covered=u128::from(self.actual_seconds).checked_mul(den).ok_or(TxError::Overflow)?;
        if num>covered {return Err(TxError::InvalidOperation);}
        let rate=u128::from(self.hourly_cents_per_member).checked_mul(u128::from(self.members)).ok_or(TxError::Overflow)?;
        let carry=u128::from(self.carried_numerator).checked_mul(den).ok_or(TxError::Overflow)?;
        let numerator=rate.checked_mul(num).and_then(|n|n.checked_add(carry)).ok_or(TxError::Overflow)?;
        let denominator=den.checked_mul(3600).ok_or(TxError::Overflow)?;
        let earned=u64::try_from(numerator/denominator).map_err(|_|TxError::Overflow)?;
        if earned>i64::MAX as u64 {return Err(TxError::Overflow);}
        Ok(earned)
    }
    pub(crate) fn shares_at(&self,num:u128,den:u128)->Result<LaborShares,TxError> {
        let earned=self.earned_at(num,den)?;
        Ok(LaborShares{water:earned/2,other:earned-earned/2})
    }
    pub(crate) fn between(&self,from:u64,until:u64)->Result<LaborShares,TxError> {
        if until<from {return Err(TxError::InvalidOperation);}
        let a=self.shares_at(u128::from(from),1)?;
        let b=self.shares_at(u128::from(until),1)?;
        Ok(LaborShares{water:b.water-a.water,other:b.other-a.other})
    }
}

impl TwinPortAllocationCycle<'_> {
    fn generator_at(&self, seconds_num:u128, seconds_den:u128)->Result<u64,TxError> {
        let fuel=lot_prefix(self.fuel,seconds_num.checked_mul(50).ok_or(TxError::Overflow)?,seconds_den.checked_mul(3).ok_or(TxError::Overflow)?)?;
        let cooling=lot_prefix(self.cooling,seconds_num.checked_mul(1000).ok_or(TxError::Overflow)?,seconds_den)?;
        fuel.checked_add(cooling).ok_or(TxError::Overflow)
    }
    /// C_port(q) is frozen by source curve and original production position. A
    /// later larger observed_each_ml only permits more queries; it cannot change
    /// an old prefix price. It never divides by current or final gross output.
    pub(crate) fn port(&self, port:usize, ml:u64)->Result<u64,TxError> {
        if port>1 || ml>self.observed_each_ml { return Err(TxError::Stock); }
        if ml==0 { return Ok(0); }
        let t=u128::from(self.origin_active_seconds).checked_mul(10_000).and_then(|x|x.checked_add(u128::from(ml))).ok_or(TxError::Overflow)?;
        let gen_delta=self.generator_at(t,10_000)?.checked_sub(self.generator_at(u128::from(self.origin_active_seconds),1)?).ok_or(TxError::Stock)?;
        let raw_end=u128::from(self.origin_raw_ml).checked_add(u128::from(ml)*2).ok_or(TxError::Overflow)?;
        let raw_delta=lot_prefix(self.raw,raw_end,1)?.checked_sub(lot_prefix(self.raw,u128::from(self.origin_raw_ml),1)?).ok_or(TxError::Stock)?;
        let group=raw_delta.checked_add(gen_delta/2).and_then(|x|x.checked_add(self.water_start_cents)).ok_or(TxError::Overflow)?;
        Ok(if port==0 {group/2} else {group-group/2})
    }
    /// One original total-mL production order: A then B in each pair. Selection
    /// does not depend on which port happens to have more inventory at query time.
    /// Fractional treatment uses root ceil-prefetched original mL plus exact WIP;
    /// this helper does not round treatment or credit fractional output.
    pub(crate) fn prefix(&self, total_ml:u64)->Result<Prefix,TxError> {
        let a=total_ml/2+total_ml%2;let b=total_ml/2;
        Ok(Prefix{ports_ml:[a,b],ports_cents:[self.port(0,a)?,self.port(1,b)?]})
    }
    pub(crate) fn between(&self, from:u64, until:u64)->Result<Prefix,TxError> {
        if until<from { return Err(TxError::InvalidOperation); }
        let a=self.prefix(from)?;let b=self.prefix(until)?;
        Ok(Prefix{ports_ml:[b.ports_ml[0]-a.ports_ml[0],b.ports_ml[1]-a.ports_ml[1]],
            ports_cents:[b.ports_cents[0].checked_sub(a.ports_cents[0]).ok_or(TxError::Stock)?,b.ports_cents[1].checked_sub(a.ports_cents[1]).ok_or(TxError::Stock)?]})
    }
    fn port_with_labor(&self,labor:&LaborPrefix,port:usize,ml:u64)->Result<u64,TxError> {
        if port>1 {return Err(TxError::InvalidOperation);}
        // Both physical ports at this production position define H(q), before
        // the cumulative A/B split; adding half a wage to each port would round
        // twice and lose the complementary cent.
        let physical=self.port(0,ml)?.checked_add(self.port(1,ml)?).ok_or(TxError::Overflow)?;
        let share=labor.shares_at(u128::from(ml),10_000)?;
        let total=physical.checked_add(share.water).ok_or(TxError::Overflow)?;
        Ok(if port==0 {total/2}else{total-total/2})
    }
    pub(crate) fn prefix_with_labor(&self,labor:&LaborPrefix,total_ml:u64)->Result<Prefix,TxError> {
        let a=total_ml/2+total_ml%2;let b=total_ml/2;
        Ok(Prefix{ports_ml:[a,b],ports_cents:[self.port_with_labor(labor,0,a)?,self.port_with_labor(labor,1,b)?]})
    }
    pub(crate) fn between_with_labor(&self,labor:&LaborPrefix,from:u64,until:u64)->Result<Prefix,TxError> {
        if until<from {return Err(TxError::InvalidOperation);}
        let a=self.prefix_with_labor(labor,from)?;let b=self.prefix_with_labor(labor,until)?;
        Ok(Prefix{ports_ml:[b.ports_ml[0]-a.ports_ml[0],b.ports_ml[1]-a.ports_ml[1]],
            ports_cents:[b.ports_cents[0].checked_sub(a.ports_cents[0]).ok_or(TxError::Stock)?,b.ports_cents[1].checked_sub(a.ports_cents[1]).ok_or(TxError::Stock)?]})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{Money,Quantity};
    fn lot(id:u32,q:i64,c:i64)->InputLot{InputLot{stock_id:id,lot_ref:u64::from(id),quantity:Quantity::new(q).unwrap(),basis:Money::new(c).unwrap()}}
    #[test]
    fn exact_lot_prefix_is_bounded_and_does_not_materialize_fractional_cents(){
        let l=lot(1,3,2);assert_eq!(lot_prefix(&l,1,1).unwrap(),0);assert_eq!(lot_prefix(&l,2,1).unwrap(),1);assert_eq!(lot_prefix(&l,3,1).unwrap(),2);
        assert_eq!(lot_prefix(&l,1,3).unwrap(),0);
        assert_eq!(lot_prefix(&l,4,1),Err(TxError::Stock));assert_eq!(lot_prefix(&l,0,0),Err(TxError::InvalidOperation));
        assert_eq!(lot_prefix(&l,1,u128::MAX),Err(TxError::Overflow));
    }
    #[test]
    fn original_pair_order_and_frozen_cost_do_not_depend_on_later_production(){
        let f=lot(101,100_000,100_000);let w=lot(102,6_000_000,18_000);let raw=lot(103,100_000_000,50_000);
        let mut c=TwinPortAllocationCycle{fuel:&f,cooling:&w,raw:&raw,origin_active_seconds:0,origin_raw_ml:0,observed_each_ml:10_000,water_start_cents:550};
        let early=c.prefix(15_000).unwrap();assert_eq!(early.ports_ml,[7500,7500]);
        let odd=c.prefix(15_001).unwrap();assert_eq!(odd.ports_ml,[7501,7500]);
        c.observed_each_ml=50_000_000;assert_eq!(early,c.prefix(15_000).unwrap());assert_eq!(odd,c.prefix(15_001).unwrap());
        let whole=c.prefix(100_000_000).unwrap();assert_eq!(whole.ports_cents,[49_858,49_858]);
        assert_eq!(c.prefix(0).unwrap().ports_cents,[0,0]);
        assert_eq!(c.prefix(100_000_001),Err(TxError::Stock));
        let mut amounts=[0;2];let mut cents=[0;2];let mut before=0;
        for end in [1,2,3,15_001,19_999,20_000,41_017,100_000_000] {
            let d=c.between(before,end).unwrap();for i in 0..2{amounts[i]+=d.ports_ml[i];cents[i]+=d.ports_cents[i];}before=end;
        }
        assert_eq!(amounts,whole.ports_ml);assert_eq!(cents,whole.ports_cents);
        assert_eq!(c.between(20,19),Err(TxError::InvalidOperation));
        assert_eq!(c.port(2,1),Err(TxError::Stock));
    }
    #[test]
    fn common_cycle_origin_carries_both_allocations_across_ordinary_time_cuts(){
        let f=lot(101,100_000,100_000);let w=lot(102,6_000_000,18_000);let raw=lot(103,100_000_000,50_000);
        let first=TwinPortAllocationCycle{fuel:&f,cooling:&w,raw:&raw,origin_active_seconds:0,origin_raw_ml:0,observed_each_ml:50_000_000,water_start_cents:550};
        assert_eq!(first.generator_at(5000,1).unwrap(),98_333);
        assert_eq!(first.generator_at(5500,1).unwrap()-first.generator_at(5000,1).unwrap(),9_833);
        let old=first.prefix(100_000_000).unwrap();assert_eq!(old.ports_cents.iter().sum::<u64>(),99_716);
        // Independent oracle: actual original source at4s is fuel66+cooling12=78c,
        // raw40c and actual water startup550c. One cycle water cost=629c.
        let whole=first.prefix(80_000).unwrap();
        assert_eq!(whole.ports_ml,[40_000,40_000]);
        assert_eq!(whole.ports_cents,[314,315]);
        let early=first.between(0,60_000).unwrap();
        let late=first.between(60_000,80_000).unwrap();
        assert_eq!(early.ports_cents,[304,305]);assert_eq!(late.ports_cents,[10,10]);
        for i in 0..2 {
            assert_eq!(early.ports_ml[i]+late.ports_ml[i],whole.ports_ml[i]);
            assert_eq!(early.ports_cents[i]+late.ports_cents[i],whole.ports_cents[i]);
        }
        // The second interval receives10c of gen cost, not floor(19/2)=9.
        // Complementary treatment gen shares are30 then9 (plus initial150c),
        // identical to cumulative39+150. No cent moves merely because of a cut.
        let first_water=early.ports_cents.iter().sum::<u64>();
        let second_water=late.ports_cents.iter().sum::<u64>();
        let first_gen_water=first_water-550-30;
        let second_gen_water=second_water-10;
        assert_eq!((first_gen_water,second_gen_water),(29,10));
        let first_other=59-first_gen_water+150;let second_other=19-second_gen_water;
        assert_eq!((first_other,second_other),(180,9));
        assert_eq!(first_other+second_other,78-39+150);
        assert_eq!(first_water+second_water+first_other+second_other,78+40+700);
        assert_eq!(first.prefix(100_000_000).unwrap(),old);
    }
    #[test]
    fn v2_earned_labor_keeps_cycle_carry_and_refuses_uncovered_positions(){
        let e=LaborPrefix{hourly_cents_per_member:3601,members:1,carried_numerator:0,actual_seconds:5500};
        assert_eq!(e.shares_at(3,1).unwrap(),LaborShares{water:1,other:2});
        assert_eq!(e.shares_at(4,1).unwrap(),LaborShares{water:2,other:2});
        assert_eq!(e.between(3,4).unwrap(),LaborShares{water:1,other:0});
        assert_eq!(e.earned_at(5500,1).unwrap(),5501);
        assert_eq!(e.earned_at(5501,1),Err(TxError::InvalidOperation));
        assert_eq!(e.earned_at(1,0),Err(TxError::InvalidOperation));
        assert_eq!(e.between(4,3),Err(TxError::InvalidOperation));
        let carry=LaborPrefix{hourly_cents_per_member:1,members:1,carried_numerator:3599,actual_seconds:1};
        assert_eq!(carry.earned_at(0,1).unwrap(),0);
        assert_eq!(carry.earned_at(1,1).unwrap(),1);
        let huge=LaborPrefix{hourly_cents_per_member:u64::MAX,members:u64::MAX,carried_numerator:0,actual_seconds:2};
        assert_eq!(huge.earned_at(2,1),Err(TxError::Overflow));
    }
    #[test]
    fn v2_same_original_labor_and_physical_prefix_have_page_independent_prices(){
        let f=lot(101,100_000,100_000);let w=lot(102,6_000_000,18_000);let raw=lot(103,100_000_000,50_000);
        let mut c=TwinPortAllocationCycle{fuel:&f,cooling:&w,raw:&raw,origin_active_seconds:0,origin_raw_ml:0,observed_each_ml:40_000,water_start_cents:550};
        let e=LaborPrefix{hourly_cents_per_member:3601,members:1,carried_numerator:0,actual_seconds:5000};
        // Independent original lot/wage oracle in the root's V2 fixture:
        // Hphysical(4)=629; floor(E(4)/2)=2, giving631, split315/316.
        assert_eq!(c.prefix(80_000).unwrap().ports_cents,[314,315]);
        let full=c.prefix_with_labor(&e,80_000).unwrap();assert_eq!(full.ports_cents,[315,316]);
        let first=c.between_with_labor(&e,0,60_000).unwrap();assert_eq!(first.ports_cents,[305,305]);
        let next=c.between_with_labor(&e,60_000,80_000).unwrap();assert_eq!(next.ports_cents,[10,11]);
        for i in 0..2 {assert_eq!(first.ports_cents[i]+next.ports_cents[i],full.ports_cents[i]);}
        // Later production expands only the observed bound, never the old price.
        c.observed_each_ml=50_000_000;assert_eq!(c.prefix_with_labor(&e,80_000).unwrap(),full);
        assert_eq!(c.prefix_with_labor(&e,100_000_000).unwrap().ports_cents.iter().sum::<u64>(),102216);
        let uncovered=LaborPrefix{actual_seconds:3,..e};
        assert_eq!(c.prefix_with_labor(&uncovered,80_000),Err(TxError::InvalidOperation));
        // Actual Engine late-payment/Stock/WAL tests are still required; this
        // arithmetic helper cannot authenticate a contract or commit a wage.
    }
}
