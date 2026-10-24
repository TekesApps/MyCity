//! Bounded pages-only policy for one already-owned exact air attempt.
//! No timers, grants, dispatch, retries, retirement, Store writes or authority facts.
use crate::air_page_metrics::{Nanoseconds, Summary};
use crate::air_turns::Quota;
use serde::{Deserialize, Serialize};

pub const POLICY_ID: &str = "air-exact-auto-pages-v1";
pub const VERSION: u16 = 1;
pub const MAX_TARGET_NS: u64 = 1_000_000_000;
const LEVELS: [u16; 5] = [1, 2, 4, 8, 16];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config { pub target_ns: u64 }

impl Config {
    pub fn validate(self) -> Result<Self, Error> {
        if (1..=MAX_TARGET_NS).contains(&self.target_ns) { Ok(self) }
        else { Err(Error::InvalidTarget) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error { InvalidTarget, InvalidQuota, TargetVersionOverflow }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase { Sources, Cells, Mixed, Integration }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyName { #[serde(rename = "air-exact-auto-pages-v1")] ExactAutoPagesV1 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    MissingSummary, InvalidSummary, EmptySummary, QuotaMismatch, ZeroWall,
    UnknownWall, UnclassifiedWork, PhaseChanged, Hold, LowStreak, HighStreak,
    StepUp, StepDown, AtMaximum, AtMinimum, FinalObservation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownSample {
    pub phase: Phase,
    pub pages: u64,
    /// Sum of actual page call wall intervals, never parent pause/request elapsed.
    pub wall_ns: u64,
    pub max_page_wall_ns: u64,
    pub integration_pages: u64,
    pub integration_max_page_wall_ns: u64,
    /// Observation only. Unknown CPU does not invalidate a known wall sample.
    pub cpu_total: Nanoseconds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub policy: PolicyName,
    pub policy_version: u16,
    pub target_version: u64,
    pub config: Config,
    pub current_quota: Quota,
    pub next_quota: Quota,
    pub reason: Reason,
    pub known_sample: Option<KnownSample>,
    pub turn_overrun: bool,
    pub integration_overrun: bool,
    /// At least one observed page itself exceeded target: pages-only adjustment
    /// cannot split that call. This is evidence about that call, not a future bound.
    pub irreducible_overrun: bool,
    pub still_unresolved: bool,
}

/// Private non-serialized attempt state. Root calls decide exactly once per verified
/// nonterminal boundary and retains its decision across uncertain send (no replay).
#[derive(Clone)]
pub struct Policy {
    quota: Quota,
    config: Config,
    target_version: u64,
    phase: Option<Phase>,
    low: u8,
    high: u8,
    held_turns: u8,
}

impl Policy {
    pub fn new(initial: Quota, config: Config) -> Result<Self, Error> {
        initial.validate().map_err(|_| Error::InvalidQuota)?;
        if !LEVELS.contains(&initial.pages) { return Err(Error::InvalidQuota); }
        Ok(Self { quota: initial, config: config.validate()?, target_version: 1,
            phase: None, low: 0, high: 0, held_turns: 0 })
    }
    pub fn current_quota(&self) -> Quota { self.quota }
    pub fn target_version(&self) -> u64 { self.target_version }
    /// Explicit target revision even if the numeric target is unchanged. No stale
    /// phase/streak crosses this boundary; fixed source/cell and current pages remain.
    pub fn retarget(&mut self, config: Config) -> Result<(), Error> {
        let config = config.validate()?;
        let version = self.target_version.checked_add(1).ok_or(Error::TargetVersionOverflow)?;
        self.config = config;
        self.target_version = version;
        self.reset_trend();
        Ok(())
    }
    fn reset_trend(&mut self) {
        self.phase = None; self.low = 0; self.high = 0; self.held_turns = 0;
    }
    pub fn decide(&mut self, summary: Option<&Summary>) -> Decision {
        let current = self.quota;
        let sample = sample(summary, current, false);
        let (reason, known_sample) = match sample {
            Err(reason) => {
                // Deliberate conservative exception to single-rung normal adjustment.
                // Necessary work remains unfinished; Unknown never becomes zero cost.
                self.quota.pages = 1;
                self.reset_trend();
                (reason, None)
            }
            Ok(value) => {
                let changed = self.phase != Some(value.phase);
                if changed { self.reset_trend(); self.phase = Some(value.phase); }
                self.held_turns = self.held_turns.saturating_add(1).min(2);
                // Use u128 for the exact half threshold; no u64 doubling overflow.
                let low = u128::from(value.wall_ns) * 2 < u128::from(self.config.target_ns);
                let high = value.wall_ns > self.config.target_ns;
                self.low = if low { self.low.saturating_add(1).min(2) } else { 0 };
                self.high = if high { self.high.saturating_add(1).min(2) } else { 0 };
                let reason = if self.held_turns >= 2 && self.low >= 2 {
                    let next = match current.pages { 1 => 2, 2 => 4, 4 => 8, 8 => 16, _ => 16 };
                    if next == current.pages { Reason::AtMaximum }
                    else { self.quota.pages = next; self.low = 0; self.high = 0;
                        self.held_turns = 0; Reason::StepUp }
                } else if self.held_turns >= 2 && self.high >= 2 {
                    let next = match current.pages { 16 => 8, 8 => 4, 4 => 2, 2 => 1, _ => 1 };
                    if next == current.pages { Reason::AtMinimum }
                    else { self.quota.pages = next; self.low = 0; self.high = 0;
                        self.held_turns = 0; Reason::StepDown }
                } else if changed { Reason::PhaseChanged }
                else if low { Reason::LowStreak }
                else if high { Reason::HighStreak }
                else { Reason::Hold };
                (reason, Some(value))
            }
        };
        Decision { policy: PolicyName::ExactAutoPagesV1,
            policy_version: VERSION, target_version: self.target_version,
            config: self.config, current_quota: current, next_quota: self.quota, reason,
            known_sample,
            turn_overrun: known_sample.is_some_and(|s| s.wall_ns > self.config.target_ns),
            integration_overrun: known_sample.is_some_and(|s|
                s.integration_pages > 0 && s.integration_max_page_wall_ns > self.config.target_ns),
            irreducible_overrun: known_sample.is_some_and(|s| s.max_page_wall_ns > self.config.target_ns),
            still_unresolved: true }
    }
}

impl Policy {
    /// Published final turn may be shorter than quota. Diagnostic only: no trend,
    /// target version or quota mutation, and no authorization to Continue.
    pub fn preview_final(&self, summary: Option<&Summary>) -> Decision {
        let (reason, known_sample) = match sample(summary, self.quota, true) {
            Ok(value) => (Reason::FinalObservation, Some(value)),
            Err(reason) => (reason, None),
        };
        Decision { policy: PolicyName::ExactAutoPagesV1, policy_version: VERSION,
            target_version: self.target_version, config: self.config,
            current_quota: self.quota, next_quota: self.quota, reason, known_sample,
            turn_overrun: known_sample.is_some_and(|s| s.wall_ns > self.config.target_ns),
            integration_overrun: known_sample.is_some_and(|s| s.integration_pages > 0
                && s.integration_max_page_wall_ns > self.config.target_ns),
            irreducible_overrun: known_sample.is_some_and(|s| s.max_page_wall_ns > self.config.target_ns),
            still_unresolved: true }
    }
}

fn sample(summary: Option<&Summary>, quota: Quota, final_turn: bool) -> Result<KnownSample, Reason> {
    let summary = summary.ok_or(Reason::MissingSummary)?;
    summary.validate().map_err(|_| Reason::InvalidSummary)?;
    let t = summary.turn.as_ref().ok_or(Reason::InvalidSummary)?;
    let w = t.work;
    if w.pages == 0 { return Err(Reason::EmptySummary); }
    // A nonterminal boundary exhausts the exact issued pages quota. A shorter final
    // turn must never be passed to this Continue policy as evidence for another turn.
    if w.pages > u64::from(quota.pages) || (!final_turn && w.pages != u64::from(quota.pages))
        || w.sources > u64::from(quota.source_limit) * w.pages
        || w.cells > u64::from(quota.cell_limit) * w.pages { return Err(Reason::QuotaMismatch); }
    let (Nanoseconds::Known { ns: wall_ns }, Nanoseconds::Known { ns: max_page_wall_ns },
        Nanoseconds::Known { ns: integration_max_page_wall_ns }) =
        (t.wall.total, t.wall.max_page, t.integration.wall.max_page)
        else { return Err(Reason::UnknownWall); };
    if wall_ns == 0 || max_page_wall_ns == 0 { return Err(Reason::ZeroWall); }
    let phase = if w.integrated_cells != 0 || w.segments != 0 { Phase::Integration }
        else if w.sources != 0 && w.cells != 0 { Phase::Mixed }
        else if w.sources != 0 { Phase::Sources }
        else if w.cells != 0 { Phase::Cells }
        else { return Err(Reason::UnclassifiedWork); };
    Ok(KnownSample { phase, pages: w.pages, wall_ns, max_page_wall_ns,
        integration_pages: t.integration.pages, integration_max_page_wall_ns, cpu_total: t.cpu.total })
}

#[cfg(test)]
#[path = "air_page_policy_tests.rs"]
mod tests;
