//! Unique initialized pages. Drop unmaps before releasing the reservation.
use super::{err, FILL, MIB};
use memmap2::MmapMut;
use serde::Serialize;
use std::{
    ops::Deref,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
};
pub(super) struct Budget {
    limit: usize,
    reserved: AtomicUsize,
    live: AtomicUsize,
    peak: AtomicUsize,
    next_generation: AtomicU64,
    epoch: AtomicU64,
}
#[derive(Serialize)]
pub(super) struct Report {
    pub limit_bytes: usize,
    pub reserved_bytes: usize,
    pub initialized_mapped_bytes: usize,
    pub peak_initialized_mapped_bytes: usize,
    pub epoch: u64,
}
impl Budget {
    pub fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit,
            reserved: AtomicUsize::new(0),
            live: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            next_generation: AtomicU64::new(1),
            epoch: AtomicU64::new(0),
        })
    }
    pub fn report(&self) -> Report {
        Report {
            limit_bytes: self.limit,
            reserved_bytes: self.reserved.load(Ordering::SeqCst),
            initialized_mapped_bytes: self.live.load(Ordering::SeqCst),
            peak_initialized_mapped_bytes: self.peak.load(Ordering::SeqCst),
            epoch: self.epoch.load(Ordering::SeqCst),
        }
    }
}
pub(super) struct Page {
    map: Option<MmapMut>,
    budget: Arc<Budget>,
    pub generation: u64,
}
impl Page {
    pub fn new(budget: &Arc<Budget>) -> Result<Self, String> {
        budget
            .reserved
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                n.checked_add(MIB).filter(|n| *n <= budget.limit)
            })
            .map_err(|_| "normal_pool_mapped_payload_cap")?;
        let generation =
            match budget
                .next_generation
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_add(1))
            {
                Ok(id) => id,
                Err(_) => {
                    budget.reserved.fetch_sub(MIB, Ordering::SeqCst);
                    return Err("normal_pool_map_generation_overflow".into());
                }
            };
        let mut map = match MmapMut::map_anon(MIB) {
            Ok(m) => m,
            Err(e) => {
                budget.reserved.fetch_sub(MIB, Ordering::SeqCst);
                return Err(err(e));
            }
        };
        map.fill(FILL);
        let live = budget.live.fetch_add(MIB, Ordering::SeqCst) + MIB;
        budget.peak.fetch_max(live, Ordering::SeqCst);
        budget.epoch.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            map: Some(map),
            budget: Arc::clone(budget),
            generation,
        })
    }
    pub fn capacity(&self) -> usize {
        self.len()
    }
}
impl Deref for Page {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.map.as_ref().expect("live mapped page")
    }
}
impl Drop for Page {
    fn drop(&mut self) {
        drop(self.map.take()); // Unique owner: unmap completes before credit/bookkeeping.
        self.budget.live.fetch_sub(MIB, Ordering::SeqCst);
        self.budget.reserved.fetch_sub(MIB, Ordering::SeqCst);
        self.budget.epoch.fetch_add(1, Ordering::SeqCst);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_payload_cap_and_replacement_identity_are_independent_of_rust_heap() {
        let b = Budget::new(2 * MIB);
        let a = Page::new(&b).unwrap();
        let c = Page::new(&b).unwrap();
        assert!(a.iter().all(|v| *v == FILL));
        assert!(Page::new(&b).is_err());
        assert_eq!(b.report().initialized_mapped_bytes, 2 * MIB);
        let old = a.generation;
        drop(a);
        assert_eq!(b.report().initialized_mapped_bytes, MIB);
        let d = Page::new(&b).unwrap();
        assert!(d.generation > old && d.generation > c.generation);
        assert_eq!(d.capacity(), MIB);
        drop(c);
        drop(d);
        assert_eq!(b.report().reserved_bytes, 0);
        assert_eq!(b.report().initialized_mapped_bytes, 0);
        assert_eq!(b.report().peak_initialized_mapped_bytes, 2 * MIB);
    }
}
