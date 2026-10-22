//! Reclaimable immutable R07 pressure. Logical budget is never a historical credit.
use super::{err, MIB};
use memmap2::MmapMut;
use serde::{Deserialize, Serialize};
use std::{
    hint::black_box,
    io::{Read, Write},
    sync::Arc,
};
pub const QUANTUM: usize = 64 * 1024;
const PATTERN: &str = "r07-immutable-splitmix64-v1";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub schema: u32,
    pub logical_bytes: usize,
    pub arena_tail_bytes: usize,
    pub quantum_bytes: usize,
    pub mapped_bytes: usize,
    pub replaced_bytes: usize,
    pub epoch: u64,
    pub reading_sequence: u64,
}
impl State {
    pub fn validate(&self, logical: usize) -> Result<(), String> {
        let capacity = mapped_capacity(logical);
        if self.schema != 1
            || self.logical_bytes != logical
            || self.arena_tail_bytes != logical % QUANTUM
            || self.quantum_bytes != QUANTUM
            || self.mapped_bytes > capacity
            || self.mapped_bytes % QUANTUM != 0
            || self.replaced_bytes != capacity - self.mapped_bytes
        {
            return Err("r07_state_shape".into());
        }
        Ok(())
    }
}
pub fn mapped_capacity(logical: usize) -> usize {
    logical / QUANTUM * QUANTUM
}
fn map(index: usize) -> Result<Arc<MmapMut>, String> {
    let mut page = MmapMut::map_anon(QUANTUM).map_err(err)?;
    // Full writes, not lazy virtual capacity. Stable by logical segment index.
    let mut state = 0x5230_3700_0000_0000u64.wrapping_add(index as u64);
    for chunk in page.chunks_exact_mut(8) {
        state = state.wrapping_add(0x9e3779b97f4a7c15);
        let mut x = state;
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        chunk.copy_from_slice(&(x ^ (x >> 31)).to_le_bytes());
    }
    Ok(Arc::new(page))
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Applied {
    pub source: crate::control_data::Text<64>,
    pub owner: Option<crate::native_observer::Identity>,
    pub state: State,
    pub permitted_bytes: usize,
    pub reason: crate::control_data::Text<64>,
}
struct Generations {
    ids: Vec<u64>,
    next: u64,
}
pub struct Backing {
    logical: usize,
    pages: Vec<Arc<MmapMut>>,
    epoch: u64,
    sequence: u64,
    generations: Option<Generations>,
}
impl Backing {
    pub fn full(logical: usize) -> Result<Self, String> {
        if logical > 512 * MIB {
            return Err("r07_logical_limit".into());
        }
        let count = logical / QUANTUM;
        let mut pages = Vec::with_capacity(count);
        for index in 0..count {
            pages.push(map(index)?);
        }
        Ok(Self {
            logical,
            pages,
            epoch: 0,
            sequence: 0,
            generations: None,
        })
    }
    pub fn enable_vm_trace(&mut self) -> Result<(), String> {
        if self.generations.is_some() {
            return Err("r07_trace_already_enabled".into());
        }
        let mut ids = Vec::with_capacity(self.logical / QUANTUM);
        ids.extend(1..=self.pages.len() as u64);
        self.generations = Some(Generations {
            ids,
            next: self.pages.len() as u64 + 1,
        });
        Ok(())
    }
    pub fn vm_ranges(&self, out: &mut crate::vm_trace::Writer) -> Result<(), String> {
        let trace = self.generations.as_ref().ok_or("r07_trace_not_enabled")?;
        for (i, (page, generation)) in self.pages.iter().zip(&trace.ids).enumerate() {
            out.range(crate::vm_trace::Range {
                kind: "r07_immutable_map",
                ordinal: i,
                allocation_generation: *generation,
                start: page.as_ptr() as usize,
                len: page.len(),
                capacity: page.len(),
                nested_in_arena: false,
                grant_bytes: None,
            })?;
        }
        Ok(())
    }
    pub fn vm_range_count(&self) -> usize {
        self.pages.len()
    }
    pub fn control_bytes(&self) -> usize {
        // Vec backing, Arc header+MmapMut per segment, and a maximum snapshot
        // reference table. Not OS VM metadata: whole-group physical samples cover that.
        self.generations
            .as_ref()
            .map_or(0, |g| g.ids.capacity() * std::mem::size_of::<u64>())
            + std::mem::size_of::<Option<Generations>>()
            + self.pages.capacity()
                * (2 * std::mem::size_of::<Arc<MmapMut>>()
                    + 2 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<MmapMut>())
    }
    pub fn state(&self) -> State {
        let mapped = self.pages.len() * QUANTUM;
        State {
            schema: 1,
            logical_bytes: self.logical,
            arena_tail_bytes: self.logical % QUANTUM,
            quantum_bytes: QUANTUM,
            mapped_bytes: mapped,
            replaced_bytes: mapped_capacity(self.logical) - mapped,
            epoch: self.epoch,
            reading_sequence: self.sequence,
        }
    }
    /// Round DOWN permitted replacement. Pins defer removal, never grant fictional credit.
    /// Repayment appends new immutable segments; an older snapshot's prefix stays unchanged.
    pub fn apply(
        &mut self,
        permitted_bytes: usize,
        reading_sequence: u64,
    ) -> Result<State, String> {
        let full = self.logical / QUANTUM;
        let desired = full - (permitted_bytes / QUANTUM).min(full);
        let before = self.pages.len();
        while self.pages.len() > desired {
            if self.pages.last().is_some_and(|p| Arc::strong_count(p) != 1) {
                break;
            }
            self.pages.pop(); // Last Arc drops the anonymous map: actual munmap.
            if let Some(g) = &mut self.generations {
                g.ids.pop();
            }
        }
        while self.pages.len() < desired {
            let mapped = map(self.pages.len())?;
            if let Some(g) = &mut self.generations {
                let next = g
                    .next
                    .checked_add(1)
                    .ok_or("r07_allocation_generation_overflow")?;
                g.ids.push(g.next);
                g.next = next;
            }
            self.pages.push(mapped);
        }
        if before != self.pages.len() {
            self.epoch = self.epoch.checked_add(1).ok_or("r07_epoch_overflow")?;
        }
        self.sequence = self.sequence.max(reading_sequence);
        Ok(self.state())
    }
    pub fn touch(&self) -> (usize, u64) {
        let mut checksum = 0u64;
        for p in &self.pages {
            for b in p.chunks(4096) {
                checksum = checksum.wrapping_add(u64::from(black_box(b[0])));
            }
        }
        (self.pages.len() * QUANTUM, checksum)
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: self.state(),
            pages: self.pages.clone(),
        }
    }
    /// Validate stored prefix but reconstruct ALL missing immutable padding locally.
    /// There is no transfer of an old process's credit into this new owner.
    pub fn read_snapshot_prefix(
        &self,
        reader: &mut dyn Read,
        old: State,
        scratch: &mut [u8],
    ) -> Result<(), String> {
        old.validate(self.logical)?;
        if scratch.len() < QUANTUM {
            return Err("r07_read_buffer".into());
        }
        for p in self.pages.iter().take(old.mapped_bytes / QUANTUM) {
            reader.read_exact(&mut scratch[..QUANTUM]).map_err(err)?;
            if scratch[..QUANTUM] != p[..] {
                return Err("r07_prefix_pattern".into());
            }
        }
        Ok(())
    }
    pub fn pattern() -> &'static str {
        PATTERN
    }
}
pub struct Snapshot {
    pub state: State,
    pages: Vec<Arc<MmapMut>>,
}
impl Snapshot {
    pub fn write(&self, w: &mut impl Write) -> Result<(), String> {
        for p in &self.pages {
            w.write_all(p).map_err(err)?;
        }
        Ok(())
    }
    pub fn verify(&self, reader: &mut dyn Read, scratch: &mut [u8]) -> Result<(), String> {
        if scratch.len() < QUANTUM {
            return Err("r07_read_buffer".into());
        }
        for p in &self.pages {
            reader.read_exact(&mut scratch[..QUANTUM]).map_err(err)?;
            if scratch[..QUANTUM] != p[..] {
                return Err("r07_snapshot_pattern".into());
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pins_defer_actual_release_repayment_preserves_snapshot_and_cold_credit_is_zero() {
        let mut b = Backing::full(4 * QUANTUM + 17).unwrap();
        let all = b.snapshot();
        assert_eq!(b.apply(2 * QUANTUM, 1).unwrap().replaced_bytes, 0);
        drop(all);
        assert_eq!(
            b.apply(2 * QUANTUM + 1, 2).unwrap().replaced_bytes,
            2 * QUANTUM
        );
        let old = b.snapshot();
        let mut bytes = Vec::new();
        old.write(&mut bytes).unwrap();
        assert_eq!(b.apply(0, 3).unwrap().mapped_bytes, 4 * QUANTUM);
        old.verify(&mut &bytes[..], &mut vec![0; QUANTUM]).unwrap();
        let cold = Backing::full(4 * QUANTUM + 17).unwrap();
        cold.read_snapshot_prefix(&mut &bytes[..], old.state, &mut vec![0; QUANTUM])
            .unwrap();
        assert_eq!(cold.state().replaced_bytes, 0);
        assert_eq!(cold.state().arena_tail_bytes, 17);
        assert_eq!(cold.touch().0, 4 * QUANTUM);
        bytes[0] ^= 1;
        assert!(cold
            .read_snapshot_prefix(&mut &bytes[..], old.state, &mut vec![0; QUANTUM])
            .is_err());
    }
    #[test]
    fn trace_generation_changes_on_remap_without_entering_snapshot_state() {
        let mut b = Backing::full(4 * QUANTUM).unwrap();
        b.enable_vm_trace().unwrap();
        let ids = b.generations.as_ref().unwrap().ids.clone();
        b.apply(2 * QUANTUM, 1).unwrap();
        b.apply(0, 2).unwrap();
        let current = &b.generations.as_ref().unwrap().ids;
        assert_eq!(&current[..2], &ids[..2]);
        assert!(current[2] > ids[3] && current[3] > current[2]);
        assert!(!serde_json::to_string(&b.state())
            .unwrap()
            .contains("generation"));
        assert_eq!(b.state().mapped_bytes, 4 * QUANTUM);
    }
    #[test]
    fn forged_lengths_and_overflow_budget_reject() {
        let b = Backing::full(QUANTUM).unwrap();
        let mut state = b.state();
        state.mapped_bytes += 1;
        assert!(state.validate(QUANTUM).is_err());
        assert!(Backing::full(512 * MIB + 1).is_err());
    }
}
