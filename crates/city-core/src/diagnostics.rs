//! Transient bounded observations. Never part of Engine, receipts or checkpoints.
//! Durations are elapsed compute-region time (including preemption), not thread CPU time.
use serde::Serialize;
use std::time::Instant;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Execution,
    Quote,
    Replay,
}
impl Role {
    pub fn index(self) -> usize {
        match self {
            Self::Execution => 0,
            Self::Quote => 1,
            Self::Replay => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[repr(usize)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Prepare,
    Apply,
    Encode,
    Claims,
    Payroll,
    Region,
    Capture,
    Admit,
    AirSources,
    AirCells,
    AirEmpty,
}
pub const CLASSES: [&str; 11] = [
    "prepare",
    "apply",
    "encode",
    "claims",
    "payroll",
    "region",
    "capture",
    "admit",
    "air_sources",
    "air_cells",
    "air_empty",
];
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Counter {
    pub nanos: u64,
    pub calls: u64,
    pub scanned: u64,
    pub work: u64,
    pub paid: u64,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub page_sink_nanos: [u64; 3],
    pub payroll_subtypes: [[u64; 2]; 3],
    pub region_subtypes: [[u64; 8]; 3],
    pub roles: [[Counter; 11]; 3],
    pub hook_succeeded: bool,
    pub applied: bool,
    pub replayed: bool,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub struct AirPage {
    pub role: Role,
    pub class: Class,
    pub nanos: u64,
    pub items: u32,
    pub source_limit: u32,
    pub cell_limit: u32,
    pub ok: bool,
}
pub struct Clock {
    at: Instant,
    excluded: u64,
}
pub struct Trace<'a> {
    enabled: bool,
    excluded_nanos: u64,
    pub role: Role,
    pub summary: Summary,
    sink: Option<&'a mut dyn FnMut(AirPage)>,
}
impl Default for Trace<'_> {
    fn default() -> Self {
        Self::disabled()
    }
}
impl<'a> Trace<'a> {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            excluded_nanos: 0,
            role: Role::Execution,
            summary: Summary::default(),
            sink: None,
        }
    }
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Self::disabled()
        }
    }
    pub fn with_pages(sink: &'a mut dyn FnMut(AirPage)) -> Self {
        Self {
            sink: Some(sink),
            ..Self::enabled()
        }
    }
    pub fn start(&self) -> Option<Clock> {
        self.enabled.then(|| Clock {
            at: Instant::now(),
            excluded: self.excluded_nanos,
        })
    }
    pub fn nanos(&self, start: Option<Clock>) -> u64 {
        start.map_or(0, |s| {
            (s.at.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64)
                .saturating_sub(self.excluded_nanos.saturating_sub(s.excluded))
        })
    }
    pub fn add(&mut self, class: Class, nanos: u64, scanned: u64, work: u64, paid: u64) {
        if !self.enabled {
            return;
        }
        let c = &mut self.summary.roles[self.role.index()][class as usize];
        c.nanos = c.nanos.saturating_add(nanos);
        c.calls = c.calls.saturating_add(1);
        c.scanned = c.scanned.saturating_add(scanned);
        c.work = c.work.saturating_add(work);
        c.paid = c.paid.saturating_add(paid);
    }
    pub fn end(&mut self, class: Class, start: Option<Clock>, scanned: u64, work: u64, paid: u64) {
        self.add(class, self.nanos(start), scanned, work, paid);
    }
    pub fn page(&mut self, page: AirPage) {
        self.add(
            page.class,
            page.nanos,
            u64::from(page.items),
            u64::from(page.items) * u64::from(page.ok),
            0,
        );
        if let Some(sink) = self.sink.as_mut() {
            let started = Instant::now();
            sink(page);
            let cost = started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
            self.excluded_nanos = self.excluded_nanos.saturating_add(cost);
            let slot = &mut self.summary.page_sink_nanos[page.role.index()];
            *slot = slot.saturating_add(cost);
        }
    }
}
