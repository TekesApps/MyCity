//! Rebuilt directory of original owners. No queue, clock, ACK or commit authority.
use crate::required_dispatch::{self as due, DueError, Selection, ServiceSelection, SupplySelection};
use mycity_core::{arena::Scope, task_registry::{self as registry, ClockInstant, ClockKind,
    ScopeIdentity, TaskKey, WorkWindow}, timeline::Progress};
use mycity_storage::{Continuation, Store, CONTINUATION_MAX_PAGES, CONTINUATION_PAGE_BYTES};

pub const PROVIDERS: usize = 4;
/// Reverse-design plan for NEW directory metadata/reporting, not an allocator grant
/// or a proof of the whole original-provider/IPC/native working peak.
pub const METADATA_PLAN_BYTES: usize = 32 * 1024;
pub const REPORT_CAP_BYTES: usize = 4096;
pub const ADMISSION_PAGE_LIMIT: u16 = 32;
pub const POLICY_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider { Air, Service, Supply, Timeline }
impl Provider {
    pub fn name(self) -> &'static str { match self { Self::Air=>"air",Self::Service=>"service_needs",Self::Supply=>"continuous_supply",Self::Timeline=>"timeline" } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage { Unknown, Point(u64), Through(u64) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State { NoWork, Ready, Blocked }
/// Original domain state identity; schema versions are not state revisions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceBinding {
    Air(crate::task_registry_adapter::ActualJobBinding),
    Service(mycity_core::service_needs_authority::DueView),
    Supply(mycity_core::continuous_supply::DueView),
    Timeline(Progress),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub provider: Provider,
    pub state: State,
    pub key: Option<TaskKey>,
    /// Scalar required by the original pure registry. For immutable service and
    /// supply profiles this is a schema; SourceBinding carries actual changes.
    pub registry_version: Option<u64>,
    pub required_registry_version: Option<u64>,
    pub binding: Option<SourceBinding>,
    pub clock: Option<ClockKind>,
    pub coverage: Coverage,
    /// Lag behind this original writer's accepted processing boundary. This is
    /// not a sampling timestamp or permission to reuse a model result.
    pub coverage_lag_game_seconds: Option<u64>,
    pub cache_observation_age_seconds: Option<u64>,
    /// Derived only from an original pending run or uncovered accepted interval.
    pub dirty: Option<bool>,
    pub registry_wake: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedTimeline {
    scope: Scope,
    sequence: u64,
    progress: Progress,
    continuation: Option<Continuation>,
}
impl SelectedTimeline {
    pub fn target(&self)->u64 { self.progress.target.expect("selected accepted target") }
    pub fn page_limit(&self)->u16 { self.continuation.as_ref().map_or(ADMISSION_PAGE_LIMIT,|c|c.page_limit) }
    pub fn is_admission(&self)->bool { self.continuation.is_none() }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Air(due::SelectedRequired), Service(due::SelectedServiceContinuation),
    Supply(due::SelectedSupplyContinuation), Timeline(SelectedTimeline),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route { NoWork, Busy, Blocked(due::RequiredBlock), Selected(Action) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directory {
    scope: Scope,
    sequence: u64,
    progress: Option<Progress>,
    entries: [Entry;PROVIDERS],
    route: Route,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { Owner(DueError), InvalidTimeline, Workspace, Stale }
impl std::fmt::Display for Error {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result { write!(f,"task_directory_{self:?}") }
}
impl From<DueError> for Error { fn from(e:DueError)->Self { Self::Owner(e) } }

fn timeline_snapshot(store:&Store)->Result<Option<SelectedTimeline>,Error> {
    let e=store.engine().map_err(|_|Error::Owner(DueError::StoreUnavailable))?;
    let c=store.continuation_status().map_err(|_|Error::InvalidTimeline)?;
    let Some(t)=e.timeline() else { return if c.is_none(){Ok(None)}else{Err(Error::InvalidTimeline)}; };
    let p=t.progress;
    let Some(target)=p.target else { return if p.phase.is_none()&&c.is_none(){Ok(None)}else{Err(Error::InvalidTimeline)}; };
    if p.at<p.last_complete||target<p.at {return Err(Error::InvalidTimeline);}
    match (&c,p.phase) {
        (Some(c),Some(_)) if c.start==p.last_complete&&c.at==p.at&&c.target==target
            &&c.next_serial==p.serial&&c.page_limit>0&&c.page_limit<=mycity_core::timeline::PAGE_LIMIT
            &&c.remaining_pages>0&&c.remaining_pages<=CONTINUATION_MAX_PAGES
            &&c.remaining_bytes==u64::from(c.remaining_pages)*CONTINUATION_PAGE_BYTES=>{},
        (None,None) if target>p.at=>{},
        _=>return Err(Error::InvalidTimeline),
    }
    Ok(Some(SelectedTimeline{scope:e.world().scope_revision().0,sequence:store.committed_sequence(),progress:p,continuation:c}))
}

/// Evaluate one provider/scope/clock partition. Its cache can never satisfy another
/// provider even when both original source IDs happen to have the same number.
fn evaluate_entry(entry:&mut Entry,now:ClockInstant)->Result<(),Error> {
    let (Some(key),Some(version))=(entry.key,entry.registry_version) else {return Ok(());};
    if !key.window.is_valid() {return Err(Error::InvalidTimeline);}
    let source=registry::Source{scope:key.scope,id:key.source_id,version,clock:key.clock,
        due:Some(key.window.end),dirty:entry.dirty.unwrap_or(false),necessary:true,window:key.window,cursor:key.cursor};
    let demand=registry::ConsumerDemand{scope:key.scope,consumer_id:key.source_id,source_id:key.source_id,
        clock:key.clock,required_version:entry.required_registry_version.ok_or(Error::InvalidTimeline)?,window:key.window,max_age_seconds:0,necessary:true};
    // Current owners provide processing coverage, not reusable-cache sampling
    // evidence. Never manufacture observed_at from a cursor. The pure demand's
    // max_age=0 grants no credit because this partition has no cache provider.
    let caches=&[];
    let snapshot=registry::Snapshot{scope:key.scope,token:1,sources:std::slice::from_ref(&source),
        demands:std::slice::from_ref(&demand),caches,outstanding:&[]};
    // Token 1 is local to this immutable evaluation; it is never a durable or
    // Store token. Directory validation compares the full original owner data.
    let page=registry::evaluate(snapshot,now,registry::RequestCursor::begin(key.scope,1,now),1).map_err(|_|Error::Workspace)?;
    entry.registry_wake=page.waiting.is_empty()&&page.items.iter().any(|x|x.necessary&&x.key.source_id==key.source_id
        &&x.key.source_version==version&&x.key.cursor==key.cursor&&x.key.window==key.window);
    Ok(())
}
fn entry(provider:Provider)->Entry { Entry{provider,state:State::NoWork,key:None,registry_version:None,required_registry_version:None,binding:None,clock:None,
    coverage:Coverage::Unknown,coverage_lag_game_seconds:None,cache_observation_age_seconds:None,dirty:None,registry_wake:false} }

/// Collect actual current owners. Registry hints never erase mandatory work;
/// NoWork requires every relevant owner to be empty, including Timeline itself.
pub fn collect(store:&Store,active_worker:bool)->Result<Directory,Error> {
    let e=store.engine().map_err(|_|Error::Owner(DueError::StoreUnavailable))?;
    let scope=e.world().scope_revision().0;
    let progress=e.timeline().map(|t|t.progress);
    let mut d=Directory{scope,sequence:store.committed_sequence(),progress,
        entries:[entry(Provider::Air),entry(Provider::Service),entry(Provider::Supply),entry(Provider::Timeline)],route:Route::NoWork};
    if active_worker {d.route=Route::Busy;return Ok(d);}
    let air=due::select_required(store,false)?;
    let service=due::select_service_continuation(store,false)?;
    let supply=due::select_supply_continuation(store,false)?;
    let timeline=timeline_snapshot(store)?;
    if let Some(b)=e.background() {
        d.entries[0].clock=Some(ClockKind::World);
        d.entries[0].registry_version=b.version().checked_add(1);
        d.entries[0].required_registry_version=b.pending().and_then(|j|j.field_version.checked_add(1));
        d.entries[0].coverage=b.field().map_or(Coverage::Unknown,|_|Coverage::Point(u64::from(b.time())));
        if b.pending().is_some() {
            let p=progress.ok_or(Error::InvalidTimeline)?;
            let observation=crate::task_registry_adapter::observe_required(b,crate::task_registry_adapter::OwnerView{
                scope,city_id:None,world_time_seconds:p.at,snapshot_token:store.committed_sequence().checked_add(1),
            },&[]);
            d.entries[0].key=observation.business_key;
            d.entries[0].binding=observation.attempt.map(SourceBinding::Air);
            d.entries[0].dirty=Some(true);
        }
    }
    if let Some(v)=e.service_needs_due_state().map_err(|_|Error::Owner(DueError::InvalidDueView))? {
        d.entries[1].registry_version=Some(u64::from(v.version));d.entries[1].coverage=Coverage::Through(v.covered_through);
        d.entries[1].required_registry_version=Some(u64::from(mycity_core::service_needs::VERSION));
        d.entries[1].binding=Some(SourceBinding::Service(v));
        d.entries[1].clock=Some(ClockKind::Development{city_id:v.city_id});
    }
    if let Some(v)=e.continuous_supply_due_state().map_err(|_|Error::Owner(DueError::InvalidSupplyDueView))? {
        d.entries[2].registry_version=Some(u64::from(v.version));d.entries[2].coverage=Coverage::Through(v.cursor);
        d.entries[2].required_registry_version=Some(u64::from(v.version)); // original selector validates supported profile version
        d.entries[2].binding=Some(SourceBinding::Supply(v));
        d.entries[2].clock=Some(ClockKind::Development{city_id:v.city_id});
    }
    if let Some(p)=progress {d.entries[3].registry_version=Some(1);d.entries[3].required_registry_version=Some(1);d.entries[3].binding=Some(SourceBinding::Timeline(p));d.entries[3].clock=Some(ClockKind::World);d.entries[3].coverage=Coverage::Through(p.last_complete);}
    match air {
        Selection::Selected(s)=>{d.entries[0].state=State::Ready;d.entries[0].key=Some(s.directory_key());d.entries[0].binding=Some(SourceBinding::Air(s.directory_binding().clone()));d.entries[0].dirty=Some(true);d.route=Route::Selected(Action::Air(s));},
        Selection::Backpressure(b)=>{d.entries[0].state=State::Blocked;d.route=Route::Blocked(b);},
        Selection::Busy=>d.route=Route::Busy,
        Selection::NoWork=>{},
    }
    if let ServiceSelection::Selected(s)=service {
        d.entries[1].state=State::Ready;d.entries[1].key=Some(*s.key());d.entries[1].dirty=Some(true);
        if d.route==Route::NoWork {d.route=Route::Selected(Action::Service(s));}
    }
    if let SupplySelection::Selected(s)=supply {
        d.entries[2].state=State::Ready;d.entries[2].key=s.key().copied();d.entries[2].dirty=Some(true);
        if d.route==Route::NoWork {d.route=Route::Selected(Action::Supply(s));}
    }
    if let Some(s)=timeline {
        let p=s.progress;d.entries[3].state=State::Ready;d.entries[3].dirty=Some(true);
        if p.last_complete<p.at {d.entries[3].key=Some(TaskKey{scope:ScopeIdentity{world_id:scope.world(),branch_id:scope.branch(),city_id:None},clock:ClockKind::World,source_id:1,source_version:1,window:WorkWindow{start:p.last_complete,end:p.at},cursor:p.serial});}
        if d.route==Route::NoWork {d.route=Route::Selected(Action::Timeline(s));}
    }
    for v in &mut d.entries {
        if let Some(key)=v.key {
            // This mapping is supplied by each original local provider/writer,
            // not a generic conversion between World/Development/Interaction.
            let now=ClockInstant{kind:key.clock,seconds:progress.ok_or(Error::InvalidTimeline)?.at};
            if v.clock!=Some(key.clock) {return Err(Error::InvalidTimeline);}
            if let Coverage::Through(end)=v.coverage {v.coverage_lag_game_seconds=now.age_since(ClockInstant{kind:key.clock,seconds:end});}
            evaluate_entry(v,now)?;
        }
    }
    match &mut d.route {
        Route::Selected(Action::Air(s))=>s.directory_hint(d.entries[0].registry_wake),
        Route::Selected(Action::Service(s))=>s.directory_hint(d.entries[1].registry_wake),
        Route::Selected(Action::Supply(s))=>s.directory_hint(d.entries[2].registry_wake),
        _=>{},
    }
    if 3*std::mem::size_of::<Directory>()+4096+4*REPORT_CAP_BYTES>METADATA_PLAN_BYTES {return Err(Error::Workspace);}
    Ok(d)
}
impl Directory {
    pub fn entries(&self)->&[Entry;PROVIDERS] {&self.entries}
    pub fn route(&self)->&Route {&self.route}
    pub fn validate(&self,store:&Store,active_worker:bool)->Result<(),Error> {
        if active_worker {return if self.route==Route::Busy {Ok(())}else{Err(Error::Stale)};}
        if collect(store,false)?!=*self {return Err(Error::Stale);}Ok(())
    }
    pub fn report(&self)->Result<serde_json::Value,Error> {
        use serde_json::json;
        let rows=self.entries.iter().map(|v|json!({"provider":v.provider.name(),"state":format!("{:?}",v.state),
            "registry_version":v.registry_version,"required_registry_version":v.required_registry_version,
            "source_binding":match &v.binding {
                None=>serde_json::Value::Null,
                Some(SourceBinding::Air(b))=>json!({"kind":"air","field_version":b.field_version,"input_identity":b.input_identity,"checkpoint_sha256_hex":digest_hex(&b.checkpoint_sha256),"incarnation":b.incarnation,"generation":b.generation,"attempt":b.attempt,"running":b.running,"failure":format!("{:?}",b.failure)}),
                Some(SourceBinding::Service(b))=>json!({"kind":"service_needs","schema_version":b.version,"city_id":b.city_id,"fingerprint_hex":digest_hex(&b.fingerprint),"registration_hex":digest_hex(&b.registration),"covered_through":b.covered_through,"pending":b.pending,"next_due":b.next_due}),
                Some(SourceBinding::Supply(b))=>json!({"kind":"continuous_supply","schema_version":b.version,"scope":b.scope,"city_id":b.city_id,"source_id":b.source_id,"fingerprint_hex":digest_hex(&b.fingerprint),"registration_hex":digest_hex(&b.registration),"cursor":b.cursor,"status":b.status}),
                Some(SourceBinding::Timeline(b))=>json!({"kind":"timeline","state":b}),
            },
            "clock":v.clock.map(|c|format!("{c:?}")),
            "coverage":match v.coverage{Coverage::Unknown=>json!({"kind":"unknown"}),Coverage::Point(t)=>json!({"kind":"point","at":t}),Coverage::Through(t)=>json!({"kind":"through","at":t})},
            "coverage_lag_game_seconds":v.coverage_lag_game_seconds,"cache_observation_age_seconds":v.cache_observation_age_seconds,
            "dirty":v.dirty,"dirty_basis":if v.dirty==Some(true){Some("original_pending_or_uncovered_accepted_interval")}else{None},"registry_wake":v.registry_wake,
            "task":v.key.map(|k|json!({"source_id":k.source_id,"schema_version":k.source_version,"cursor":k.cursor,"start":k.window.start,"end":k.window.end,"city_id":k.scope.city_id,"clock":format!("{:?}",k.clock)}))})).collect::<Vec<_>>();
        let report=json!({"version":POLICY_VERSION,"sequence":self.sequence,"scope":{"world":self.scope.world(),"branch":self.scope.branch()},"providers":rows,"cache_policy":"unknown_reusable_cache_requires_original_work","directory_owned_bytes":std::mem::size_of::<Self>(),"metadata_plan_bytes":METADATA_PLAN_BYTES,"metadata_plan_is_peak_proof":false,"owner_read_scratch_separate":true,"completion_authority":"original_owners_only"});
        if serde_json::to_vec(&report).map_err(|_|Error::Workspace)?.len()>REPORT_CAP_BYTES{return Err(Error::Workspace);}Ok(report)
    }
}

pub fn validate_timeline(store:&Store,selected:&SelectedTimeline,active_worker:bool)->Result<(),Error> {
    if active_worker||timeline_snapshot(store)?.as_ref()!=Some(selected){return Err(Error::Stale);}Ok(())
}

fn digest_hex(bytes:&[u8;32])->String {
    use std::fmt::Write;
    let mut out=String::with_capacity(64);for b in bytes {write!(&mut out,"{b:02x}").expect("bounded digest formatting");}out
}
