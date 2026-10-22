//! Reclaimable immutable R07 pressure. Logical budget is never a historical credit.
//!
//! Storage is grouped into physical extents, while all accounting, generation and
//! persistence semantics remain expressed in 64 KiB logical quanta.
use super::{err, MIB};
use memmap2::MmapMut;
use serde::{Deserialize, Serialize};
use std::{
    hint::black_box,
    io::{Read, Write},
    sync::Arc,
};
pub const QUANTUM: usize = 64 * 1024;
pub const MAX_EXTENT_QUANTA: usize = 16;
pub const MAX_EXTENT_BYTES: usize = QUANTUM * MAX_EXTENT_QUANTA;
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

struct Extent {
    logical_start: usize,
    quanta: usize,
    generation: u64,
    map: MmapMut,
}
impl Extent {
    fn mapped_bytes(&self) -> usize {
        self.quanta * QUANTUM
    }
    fn bytes_for(&self, logical_index: usize) -> &[u8] {
        let offset = (logical_index - self.logical_start) * QUANTUM;
        &self.map[offset..offset + QUANTUM]
    }
}

fn new_extent(
    logical_start: usize,
    quanta: usize,
    generation: u64,
    copy_prefix: Option<&Extent>,
) -> Result<Extent, String> {
    let map_bytes = quanta
        .checked_mul(QUANTUM)
        .ok_or("r07_extent_size_overflow")?;
    if quanta == 0 || map_bytes > MAX_EXTENT_BYTES {
        return Err("r07_extent_size_limit".into());
    }
    let mut map = MmapMut::map_anon(map_bytes).map_err(err)?;
    let copied = copy_prefix.map_or(0, |prefix| prefix.quanta.min(quanta));
    if let Some(prefix) = copy_prefix {
        debug_assert_eq!(prefix.logical_start, logical_start);
        let copied_bytes = copied * QUANTUM;
        map[..copied_bytes].copy_from_slice(&prefix.map[..copied_bytes]);
    }
    for logical_index in logical_start + copied..logical_start + quanta {
        let offset = (logical_index - logical_start) * QUANTUM;
        let mut state = 0x5230_3700_0000_0000u64.wrapping_add(logical_index as u64);
        for chunk in map[offset..offset + QUANTUM].chunks_exact_mut(8) {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut x = state;
            x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
            chunk.copy_from_slice(&(x ^ (x >> 31)).to_le_bytes());
        }
    }
    Ok(Extent {
        logical_start,
        quanta,
        generation,
        map,
    })
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
pub struct Backing {
    logical: usize,
    extents: Vec<Arc<Extent>>,
    epoch: u64,
    sequence: u64,
    next_generation: u64,
    transition_peak_bytes: usize,
    vm_trace_enabled: bool,
    #[cfg(test)]
    test_allocation_fail_after: Option<usize>,
}
impl Backing {
    pub fn full(logical: usize) -> Result<Self, String> {
        if logical > 512 * MIB {
            return Err("r07_logical_limit".into());
        }
        let count = logical / QUANTUM;
        let mut backing = Self {
            logical,
            // Keep the conservative logical-quantum slot cap so pinned partial
            // extents can fragment the physical list without growing metadata.
            extents: Vec::with_capacity(count),
            epoch: 0,
            sequence: 0,
            next_generation: 1,
            transition_peak_bytes: 0,
            vm_trace_enabled: false,
            #[cfg(test)]
            test_allocation_fail_after: None,
        };
        let mut initializing = true;
        backing.extend_to(count, &mut initializing)?;
        Ok(backing)
    }

    fn new_extent(
        &mut self,
        start: usize,
        quanta: usize,
        prefix: Option<&Extent>,
    ) -> Result<Arc<Extent>, String> {
        let generation = self.next_generation;
        let next = generation
            .checked_add(1)
            .ok_or("r07_allocation_generation_overflow")?;
        let extent = new_extent(start, quanta, generation, prefix)?;
        self.next_generation = next;
        Ok(Arc::new(extent))
    }

    fn allocate_extent(
        &mut self,
        start: usize,
        quanta: usize,
        prefix: Option<&Extent>,
    ) -> Result<Arc<Extent>, String> {
        #[cfg(test)]
        if self.test_allocation_fail_after == Some(0) {
            self.test_allocation_fail_after = None;
            return Err("r07_injected_allocation_failure".into());
        }
        #[cfg(test)]
        if let Some(remaining) = &mut self.test_allocation_fail_after {
            *remaining -= 1;
        }
        self.new_extent(start, quanta, prefix)
    }

    fn next_epoch(&self, changed: bool) -> Result<Option<u64>, String> {
        if changed {
            return Ok(None);
        }
        self.epoch
            .checked_add(1)
            .map(Some)
            .ok_or_else(|| "r07_epoch_overflow".into())
    }

    fn record_mapping_change(&mut self, changed: &mut bool, next_epoch: Option<u64>) {
        if let Some(epoch) = next_epoch {
            self.epoch = epoch;
        }
        *changed = true;
    }

    fn mapped_quanta(&self) -> usize {
        self.extents.iter().map(|extent| extent.quanta).sum()
    }

    fn extend_to(&mut self, desired: usize, changed: &mut bool) -> Result<(), String> {
        while self.mapped_quanta() < desired {
            let mapped = self.mapped_quanta();
            let remaining = desired - mapped;
            if let Some(last) = self.extents.last() {
                if last.quanta < MAX_EXTENT_QUANTA && Arc::strong_count(last) == 1 {
                    let old = Arc::clone(last);
                    let add = remaining.min(MAX_EXTENT_QUANTA - old.quanta);
                    let new_quanta = old.quanta + add;
                    let next_epoch = self.next_epoch(*changed)?;
                    let new = self.allocate_extent(old.logical_start, new_quanta, Some(&old))?;
                    // The replacement map is fully initialized before swapping;
                    // it is at most 1 MiB, while the old map remains live transiently.
                    self.note_transition_peak(new_quanta * QUANTUM);
                    *self.extents.last_mut().expect("last extent exists") = new;
                    drop(old);
                    self.record_mapping_change(changed, next_epoch);
                    continue;
                }
            }
            let add = remaining.min(MAX_EXTENT_QUANTA);
            let next_epoch = self.next_epoch(*changed)?;
            let extent = self.allocate_extent(mapped, add, None)?;
            self.extents.push(extent);
            self.record_mapping_change(changed, next_epoch);
        }
        Ok(())
    }

    fn note_transition_peak(&mut self, bytes: usize) {
        self.transition_peak_bytes = self.transition_peak_bytes.max(bytes);
    }

    pub fn enable_vm_trace(&mut self) -> Result<(), String> {
        // Generation ids are assigned at map creation, including before tracing.
        // This method remains an explicit opt-in and rejects duplicate activation.
        if self.vm_trace_enabled {
            return Err("r07_trace_already_enabled".into());
        }
        self.vm_trace_enabled = true;
        Ok(())
    }

    pub fn vm_ranges(&self, out: &mut crate::vm_trace::Writer) -> Result<(), String> {
        if !self.vm_trace_enabled {
            return Err("r07_trace_not_enabled".into());
        }
        for (ordinal, extent) in self.extents.iter().enumerate() {
            out.range(crate::vm_trace::Range {
                kind: "r07_immutable_map",
                ordinal,
                allocation_generation: extent.generation,
                start: extent.map.as_ptr() as usize,
                len: extent.map.len(),
                capacity: extent.map.len(),
                logical_start: Some(extent.logical_start * QUANTUM),
                logical_len: Some(extent.mapped_bytes()),
                nested_in_arena: false,
                grant_bytes: None,
            })?;
        }
        Ok(())
    }

    pub fn vm_range_count(&self) -> usize {
        self.extents.len()
    }

    pub fn transition_peak_bytes(&self) -> usize {
        self.transition_peak_bytes
    }

    pub fn control_bytes(&self) -> usize {
        // Vec backing, Arc header+extent metadata per mapping, a maximum snapshot
        // reference table, and generation fields. OS VM metadata remains excluded.
        std::mem::size_of::<usize>() * 2
            + self.extents.capacity()
                * (2 * std::mem::size_of::<Arc<Extent>>()
                    + 2 * std::mem::size_of::<usize>()
                    + std::mem::size_of::<Extent>())
    }

    pub fn state(&self) -> State {
        let mapped = self.mapped_quanta() * QUANTUM;
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
    /// Repayment appends/rebuilds physical extents while preserving logical bytes.
    pub fn apply(
        &mut self,
        permitted_bytes: usize,
        reading_sequence: u64,
    ) -> Result<State, String> {
        let full = self.logical / QUANTUM;
        let desired = full - (permitted_bytes / QUANTUM).min(full);
        self.sequence = self.sequence.max(reading_sequence);
        let mut changed = false;
        while self.mapped_quanta() > desired {
            let last = self.extents.last().expect("mapped extent exists");
            if Arc::strong_count(last) != 1 {
                break;
            }
            let last_start = last.logical_start;
            if last_start >= desired {
                let next_epoch = self.next_epoch(changed)?;
                self.extents.pop();
                self.record_mapping_change(&mut changed, next_epoch);
                continue;
            }
            let keep = desired - last_start;
            if keep < last.quanta {
                let old = Arc::clone(last);
                let next_epoch = self.next_epoch(changed)?;
                let new = self.allocate_extent(last_start, keep, Some(&old))?;
                self.note_transition_peak(keep * QUANTUM);
                *self.extents.last_mut().expect("last extent exists") = new;
                drop(old);
                self.record_mapping_change(&mut changed, next_epoch);
            }
            break;
        }
        if self.mapped_quanta() < desired {
            self.extend_to(desired, &mut changed)?;
        }
        Ok(self.state())
    }

    pub fn touch(&self) -> (usize, u64) {
        let mut checksum = 0u64;
        for extent in &self.extents {
            for b in extent.map.chunks(4096) {
                checksum = checksum.wrapping_add(u64::from(black_box(b[0])));
            }
        }
        (self.state().mapped_bytes, checksum)
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: self.state(),
            extents: self.extents.clone(),
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
        for logical_index in 0..old.mapped_bytes / QUANTUM {
            reader.read_exact(&mut scratch[..QUANTUM]).map_err(err)?;
            let extent = self.extent_for(logical_index).ok_or("r07_prefix_missing")?;
            if scratch[..QUANTUM] != extent.bytes_for(logical_index)[..] {
                return Err("r07_prefix_pattern".into());
            }
        }
        Ok(())
    }

    fn extent_for(&self, logical_index: usize) -> Option<&Extent> {
        self.extents.iter().find_map(|extent| {
            (logical_index >= extent.logical_start
                && logical_index < extent.logical_start + extent.quanta)
                .then_some(extent.as_ref())
        })
    }

    pub fn pattern() -> &'static str {
        PATTERN
    }
}

pub struct Snapshot {
    pub state: State,
    extents: Vec<Arc<Extent>>,
}
impl Snapshot {
    pub fn write(&self, w: &mut impl Write) -> Result<(), String> {
        for extent in &self.extents {
            for logical_index in extent.logical_start..extent.logical_start + extent.quanta {
                w.write_all(extent.bytes_for(logical_index)).map_err(err)?;
            }
        }
        Ok(())
    }
    pub fn verify(&self, reader: &mut dyn Read, scratch: &mut [u8]) -> Result<(), String> {
        if scratch.len() < QUANTUM {
            return Err("r07_read_buffer".into());
        }
        for extent in &self.extents {
            for logical_index in extent.logical_start..extent.logical_start + extent.quanta {
                reader.read_exact(&mut scratch[..QUANTUM]).map_err(err)?;
                if scratch[..QUANTUM] != extent.bytes_for(logical_index)[..] {
                    return Err("r07_snapshot_pattern".into());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_page(index: usize) -> Vec<u8> {
        // Exact pre-extent generator: seed resets for every logical 64 KiB block.
        let mut state = 0x5230_3700_0000_0000u64.wrapping_add(index as u64);
        let mut page = vec![0; QUANTUM];
        for chunk in page.chunks_exact_mut(8) {
            state = state.wrapping_add(0x9e3779b97f4a7c15);
            let mut x = state;
            x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
            chunk.copy_from_slice(&(x ^ (x >> 31)).to_le_bytes());
        }
        page
    }

    fn logical_bytes(snapshot: &Snapshot) -> Vec<u8> {
        let mut bytes = Vec::new();
        snapshot.write(&mut bytes).unwrap();
        bytes
    }

    fn assert_logical_trace_intervals_are_contiguous(b: &Backing) {
        let mut next_start = 0;
        let mut total = 0;
        for extent in &b.extents {
            let start = extent.logical_start * QUANTUM;
            let len = extent.mapped_bytes();
            assert_eq!(start, next_start, "logical extent intervals overlap or gap");
            assert_eq!(extent.map.len(), len);
            assert!(len > 0 && len <= MAX_EXTENT_BYTES);
            next_start += len;
            total += len;
        }
        assert_eq!(total, b.state().mapped_bytes);
    }

    #[test]
    fn generator_matches_legacy_bytes_at_each_logical_quantum() {
        let b = Backing::full(35 * QUANTUM + 17).unwrap();
        let snapshot = b.snapshot();
        for index in 0..35 {
            assert_eq!(
                &logical_bytes(&snapshot)[index * QUANTUM..(index + 1) * QUANTUM],
                &legacy_page(index)
            );
        }
        assert_eq!(b.vm_range_count(), 3);
    }

    #[test]
    fn split_and_partial_refill_keep_stream_bytes_and_bound_transition_peak() {
        let mut b = Backing::full(35 * QUANTUM).unwrap();
        let before = logical_bytes(&b.snapshot());
        assert_eq!(b.apply(4 * QUANTUM, 1).unwrap().mapped_bytes, 31 * QUANTUM);
        assert_eq!(b.vm_range_count(), 2);
        assert_eq!(b.transition_peak_bytes(), 15 * QUANTUM);
        assert!(b.transition_peak_bytes() <= MAX_EXTENT_BYTES);
        assert_eq!(logical_bytes(&b.snapshot()), before[..31 * QUANTUM]);
        assert_eq!(b.apply(0, 2).unwrap().mapped_bytes, 35 * QUANTUM);
        assert_eq!(logical_bytes(&b.snapshot()), before);
        assert!(b.transition_peak_bytes() <= MAX_EXTENT_BYTES);
    }

    #[test]
    fn shrinking_inside_a_larger_final_extent_copies_only_the_retained_prefix() {
        for (initial_quanta, retained_quanta) in [(32, 31), (20, 17)] {
            let mut b = Backing::full(initial_quanta * QUANTUM).unwrap();
            let pinned_full = b.snapshot();
            let before = logical_bytes(&pinned_full);
            let removed = initial_quanta - retained_quanta;
            let pinned_state = b.apply(removed * QUANTUM, 1).unwrap();
            assert_eq!(pinned_state.mapped_bytes, initial_quanta * QUANTUM);
            assert_eq!(pinned_state.replaced_bytes, 0);
            drop(pinned_full);

            let state = b.apply(removed * QUANTUM, 2).unwrap();
            assert_eq!(state.mapped_bytes, retained_quanta * QUANTUM);
            assert_eq!(state.replaced_bytes, removed * QUANTUM);
            let after = logical_bytes(&b.snapshot());
            assert_eq!(after.len(), retained_quanta * QUANTUM);
            for index in 0..retained_quanta {
                let range = index * QUANTUM..(index + 1) * QUANTUM;
                assert_eq!(&after[range.clone()], &before[range]);
            }
            assert!(b.transition_peak_bytes() <= MAX_EXTENT_BYTES);

            let restored = Backing::full(initial_quanta * QUANTUM).unwrap();
            restored
                .read_snapshot_prefix(&mut &after[..], state, &mut vec![0; QUANTUM])
                .unwrap();
            assert_eq!(restored.state().mapped_bytes, initial_quanta * QUANTUM);

            // The old immutable snapshot remains readable after the active extent
            // is split. Refill then restores every original logical block byte.
            let old_snapshot = b.snapshot();
            assert_eq!(
                b.apply(0, 3).unwrap().mapped_bytes,
                initial_quanta * QUANTUM
            );
            assert_eq!(logical_bytes(&old_snapshot), after);
            assert_eq!(logical_bytes(&b.snapshot()), before);
        }
    }

    #[test]
    fn allocation_failure_after_shrink_drop_keeps_actual_state_and_propagates_error() {
        let mut b = Backing::full(35 * QUANTUM).unwrap();
        let prior_epoch = b.state().epoch;
        b.test_allocation_fail_after = Some(0);
        let error = b.apply(4 * QUANTUM, 41).unwrap_err();
        assert_eq!(error, "r07_injected_allocation_failure");

        // The trailing 3Q extent was actually unmapped before the interior split
        // allocation failed. State and epoch report only that real release.
        let partial = b.state();
        assert_eq!(partial.mapped_bytes, 32 * QUANTUM);
        assert_eq!(partial.replaced_bytes, 3 * QUANTUM);
        assert_eq!(partial.epoch, prior_epoch + 1);
        assert_eq!(partial.reading_sequence, 41);
        assert_logical_trace_intervals_are_contiguous(&b);
        assert_eq!(logical_bytes(&b.snapshot()).len(), partial.mapped_bytes);

        // The caller receives Err (so refresh_r07 cannot publish Applied/state),
        // while a later retry operates on the measured 32Q backing.
        let resumed = b.apply(4 * QUANTUM, 42).unwrap();
        assert_eq!(resumed.mapped_bytes, 31 * QUANTUM);
        assert_eq!(resumed.replaced_bytes, 4 * QUANTUM);
        assert_eq!(resumed.epoch, prior_epoch + 2);
    }

    #[test]
    fn allocation_failure_during_multiextent_refill_counts_every_committed_extent() {
        let mut b = Backing::full(35 * QUANTUM).unwrap();
        assert_eq!(b.apply(34 * QUANTUM, 1).unwrap().mapped_bytes, QUANTUM);
        let prior_epoch = b.state().epoch;
        b.test_allocation_fail_after = Some(1);
        let error = b.apply(0, 2).unwrap_err();
        assert_eq!(error, "r07_injected_allocation_failure");

        // One 16Q extent was fully allocated and published before the next map
        // allocation failed. It remains payload in the exact mapped budget.
        let partial = b.state();
        assert_eq!(partial.mapped_bytes, 16 * QUANTUM);
        assert_eq!(partial.replaced_bytes, 19 * QUANTUM);
        assert_eq!(partial.epoch, prior_epoch + 1);
        assert_eq!(partial.reading_sequence, 2);
        assert_logical_trace_intervals_are_contiguous(&b);
        assert_eq!(logical_bytes(&b.snapshot()).len(), partial.mapped_bytes);
        assert_eq!(b.transition_peak_bytes(), MAX_EXTENT_BYTES);

        let completed = b.apply(0, 3).unwrap();
        assert_eq!(completed.mapped_bytes, 35 * QUANTUM);
        assert_eq!(completed.replaced_bytes, 0);
        assert_eq!(completed.epoch, prior_epoch + 2);
    }

    #[test]
    fn pins_defer_release_without_duplicate_prefix_or_false_credit() {
        let mut b = Backing::full(20 * QUANTUM + 17).unwrap();
        let pinned = b.snapshot();
        let pinned_maps: Vec<_> = b
            .extents
            .iter()
            .map(|extent| (extent.map.as_ptr() as usize, extent.generation))
            .collect();
        let initial = logical_bytes(&pinned);
        let held = b.apply(3 * QUANTUM, 1).unwrap();
        assert_eq!(held.mapped_bytes, 20 * QUANTUM);
        assert_eq!(held.replaced_bytes, 0);
        let still_pinned_maps: Vec<_> = b
            .extents
            .iter()
            .map(|extent| (extent.map.as_ptr() as usize, extent.generation))
            .collect();
        assert_eq!(still_pinned_maps, pinned_maps);
        assert_logical_trace_intervals_are_contiguous(&b);
        assert_eq!(b.apply(0, 2).unwrap().mapped_bytes, 20 * QUANTUM);
        drop(pinned);
        let shrunk = b.apply(3 * QUANTUM, 3).unwrap();
        assert_eq!(shrunk.mapped_bytes, 17 * QUANTUM);
        assert_eq!(shrunk.replaced_bytes, 3 * QUANTUM);
        let cut = logical_bytes(&b.snapshot());
        assert_eq!(cut, initial[..17 * QUANTUM]);
        assert!(b.transition_peak_bytes() <= MAX_EXTENT_BYTES);
    }

    #[test]
    fn snapshot_v3_stream_and_cold_prefix_restore_keep_schema_one_accounting() {
        let mut b = Backing::full(19 * QUANTUM + 23).unwrap();
        let state = b.apply(3 * QUANTUM, 77).unwrap();
        assert_eq!(state.schema, 1);
        assert_eq!(state.quantum_bytes, 65_536);
        let snapshot = b.snapshot();
        let bytes = logical_bytes(&snapshot);
        snapshot
            .verify(&mut &bytes[..], &mut vec![0; QUANTUM])
            .unwrap();
        let restored = Backing::full(19 * QUANTUM + 23).unwrap();
        restored
            .read_snapshot_prefix(&mut &bytes[..], state, &mut vec![0; QUANTUM])
            .unwrap();
        assert_eq!(restored.state().mapped_bytes, 19 * QUANTUM);
        assert_eq!(restored.state().replaced_bytes, 0);
        assert!(serde_json::to_string(&state)
            .unwrap()
            .contains("\"schema\":1"));
    }

    #[test]
    fn trace_generation_and_payload_counts_are_physical_extents_and_logical_bytes() {
        let mut b = Backing::full(32 * QUANTUM).unwrap();
        b.enable_vm_trace().unwrap();
        let first: Vec<_> = b.extents.iter().map(|extent| extent.generation).collect();
        b.apply(QUANTUM, 1).unwrap();
        b.apply(0, 2).unwrap();
        assert_eq!(b.state().mapped_bytes, 32 * QUANTUM);
        assert_eq!(b.vm_range_count(), 2);
        assert_eq!(b.extents[0].generation, first[0]);
        assert!(b.extents[1].generation > first[1]);
        assert_eq!(b.touch().0, 32 * QUANTUM);
        assert!(!serde_json::to_string(&b.state())
            .unwrap()
            .contains("generation"));
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
