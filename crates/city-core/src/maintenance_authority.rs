//! One explicit calendar-maintenance registration. Facts remain in the original writer.
use crate::{asset_maintenance as m, assembly::World, facility_work as w,
    transaction::TxError, units::{Carry,GameTime,Quantity}};
use serde::{Serialize,Deserialize};

pub const VERSION:u32=1;
pub const PURPOSE:u16=159;
pub const MAX_OWNED_BYTES:usize=8*1024;
// One fixed component, bounded integer fields and fixed strings; reserve the
// complete future snapshot before accepting registration.
pub const MAX_ENCODED_BYTES:usize=3*1024;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opening {pub version:u32,pub project:u64,pub component:u64,pub contract_id:u64,pub material_stock:u32}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Runtime {
    pub opening:Opening,
    pub snapshot:Option<m::Snapshot>,
    pub last_operation:u64,
    pub last_at:u64,
}
fn bad(_:impl std::fmt::Debug)->TxError{TxError::InvalidOperation}
fn q(n:i64)->Quantity{Quantity::new(n).expect("fixed maintenance quantity")}
pub(crate) fn profile(project:u64)->m::Profile {
    let source=|id|m::Source{id,revision:1};
    m::Profile {revision:m::MAINTENANCE_ONLY_REVISION,kind:m::Kind::OrdinaryEquipment,
        facility:source(u32::try_from(project).expect("checked project")),maintenance:source(159),
        clock:m::ClockKind::LocalDevelopment,inspection_period_seconds:30*86400,
        service_period_seconds:30*86400,inspection_worker_seconds:1440,
        service_worker_seconds:14400,material:source(36),service_material:q(25),
        service_improvement_bps:200,replacement_worker_seconds:0,replacement_material:Quantity::ZERO}
}
impl Opening {
    pub(crate) fn validate(self,world:&World,work:&w::Opening)->Result<(),TxError>{
        if self.version!=VERSION||self.project==0||self.project>u64::from(u32::MAX)||self.component==0||self.contract_id==0||self.material_stock==0{return Err(TxError::InvalidOperation);}
        let project=world.construction.as_ref().ok_or(TxError::InvalidOperation)?;
        let capital=world.asset_capital.as_deref().ok_or(TxError::InvalidOperation)?;
        if project.id!=self.project||capital.opening.project!=self.project||capital.opening.owner!=project.owner
            ||project.operating_base_capacity!=0||project.operating_resource==0||project.operating_units==0
            ||world.region.is_some()||world.continuous_supply.is_some(){return Err(TxError::InvalidOperation);}
        let mut contracts=work.contracts.iter().flatten();let c=contracts.next().ok_or(TxError::InvalidOperation)?;
        if contracts.next().is_some()||c.id!=self.contract_id||c.device_id!=self.project||c.role!=w::Role::ElectricalMaintenance
            ||c.version!=2||c.employer_account!=project.owner||c.members.end.checked_sub(c.members.start)!=Some(4){return Err(TxError::InvalidOperation);}
        let stock=world.stocks.get(world.stock_handle(self.material_stock).map_err(bad)?).map_err(bad)?;
        if stock.owner!=world.account_handle(project.owner).map_err(bad)?||stock.commodity!=crate::assembly::Commodity::Raw{return Err(TxError::Stock);}
        Ok(())
    }
}
impl Runtime {
    pub(crate) fn new(opening:Opening)->Self{Self{opening,snapshot:None,last_operation:0,last_at:0}}
    pub(crate) fn owned_bytes(&self)->usize{std::mem::size_of::<Self>()+self.snapshot.as_ref().map_or(0,|s|s.owned_capacity_bytes()-std::mem::size_of::<m::Snapshot>())}
    pub(crate) fn commissioned(&mut self,state:crate::asset_cost_authority::State,at:u64,operation:u64)->Result<(),TxError>{
        if state.commission_operation!=operation||state.commissioned_at!=at||self.snapshot.is_some(){return Err(TxError::InvalidOperation);}
        let time=GameTime::new(i64::try_from(at).map_err(|_|TxError::Overflow)?).map_err(bad)?;
        self.snapshot=Some(m::Snapshot::commissioned(self.opening.project,operation,time,profile(self.opening.project),
            &[(self.opening.component,Carry::new(10,86400).map_err(bad)?,Carry::new(0,1).map_err(bad)?)],Carry::new(0,1).map_err(bad)?).map_err(bad)?);
        self.last_operation=operation;self.last_at=at;Ok(())
    }
    pub(crate) fn validate(&self,world:&World,capital:Option<crate::asset_cost_authority::State>)->Result<(),TxError>{
        if self.owned_bytes()>MAX_OWNED_BYTES||serde_json::to_vec(self).map_err(bad)?.len()>MAX_ENCODED_BYTES{return Err(TxError::WorkingLimit);}
        let work=world.facility_work.as_deref().ok_or(TxError::InvalidOperation)?;
        self.opening.validate(world,work.state.opening())?;
        match (&self.snapshot,capital){
            (None,None) if self.last_operation==0&&self.last_at==0=>Ok(()),
            (Some(s),Some(capital))=>{
                s.validate().map_err(bad)?;
                if s.asset_id()!=self.opening.project||s.source_snapshot()!=capital.commission_operation
                    ||s.clock_origin().value()<0||s.clock_origin().value()as u64!=capital.commissioned_at
                    ||s.profile()!=&profile(self.opening.project)||!s.single_component(self.opening.component)
                    ||s.at().value()<0||s.at().value()as u64!=self.last_at||self.last_operation==0{return Err(TxError::InvalidOperation);}
                // At this integration step only commissioning is implemented.
                // Rebuild that exact origin from the original capital fact, never
                // infer elapsed operation, damage, or completed work from a save.
                let mut original=Self::new(self.opening);
                original.commissioned(capital,capital.commissioned_at,capital.commission_operation)?;
                if *self!=original{return Err(TxError::InvalidOperation);}
                Ok(())
            }
            _=>Err(TxError::InvalidOperation)
        }
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
pub struct View {pub project:u64,pub component:u64,pub commissioned_at:u64,pub commission_operation:u64,pub condition_bps:u16}
pub(crate) fn view(r:&Runtime)->Option<View>{r.snapshot.as_ref().map(|s|View{project:r.opening.project,component:r.opening.component,
    commissioned_at:s.clock_origin().value()as u64,commission_operation:s.source_snapshot(),condition_bps:s.condition_bps(r.opening.component).expect("validated component")})}
