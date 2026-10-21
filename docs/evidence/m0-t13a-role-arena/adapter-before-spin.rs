//! Audited GlobalAlloc adapter. No allocation, formatting or unwinding under lock.
use linked_list_allocator::Heap;
use serde::{Deserialize, Serialize};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    mem::size_of,
    ptr::{self, NonNull},
    sync::{Mutex, MutexGuard},
};
const STARTUP_LIMIT: usize = 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Activation {
    pub pid: u32,
    pub grant_bytes: usize,
    pub arena_start: usize,
    pub arena_bytes: usize,
    pub bootstrap_charged_bytes: usize,
    pub adapter_header_bytes: usize,
    pub initialized_bytes: usize,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub activation: Option<Activation>,
    pub current_requested_bytes: usize,
    pub lifetime_peak_requested_bytes: usize,
    pub bootstrap_live_bytes: usize,
    pub bootstrap_peak_bytes: usize,
    pub arena_used_bytes: usize,
    pub arena_free_bytes: usize,
    pub allocation_failures: u64,
}
struct State {
    heap: Heap,
    activation: Option<Activation>,
    current: usize,
    peak: usize,
    system_live: usize,
    system_peak: usize,
    failures: u64,
}
pub struct Allocator {
    state: Mutex<State>,
}
impl Default for Allocator {
    fn default() -> Self {
        Self::new()
    }
}
impl Allocator {
    pub const fn new() -> Self {
        Self { state: Mutex::new(State {
            heap: Heap::empty(), activation: None, current: 0, peak: 0,
            system_live: 0, system_peak: 0, failures: 0,
        }) }
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        // Allocator cannot unwind through GlobalAlloc. A poisoned state is fatal,
        // never a reason to route an arena pointer to System or fabricate credit.
        match self.state.lock() { Ok(s) => s, Err(_) => std::process::abort() }
    }
    /// Atomically switch all subsequent Rust requests to one fixed arena.
    /// Existing System allocations stay charged for the entire role lifetime.
    /// The backing is deliberately never freed or reactivated before process exit.
    pub fn activate(&self, grant_bytes: usize) -> Result<Activation, &'static str> {
        let mut s = self.lock();
        if s.activation.is_some() { return Err("role_allocator_already_active"); }
        let header = size_of::<Self>();
        if s.system_peak.checked_add(header).is_none_or(|n| n > STARTUP_LIMIT) {
            return Err("role_allocator_startup_limit");
        }
        let arena_bytes = grant_bytes.checked_sub(s.system_live)
            .and_then(|n| n.checked_sub(header)).filter(|n| *n >= 4096)
            .ok_or("role_allocator_grant_too_small")?;
        let layout = Layout::from_size_align(arena_bytes, 4096)
            .map_err(|_| "role_allocator_layout")?;
        // SAFETY: a valid nonzero Layout, allocated directly from System to avoid
        // recursion. The allocation is unique and leaked for the process lifetime.
        let p = unsafe { System.alloc(layout) };
        if p.is_null() { return Err("role_allocator_backing_allocation"); }
        if (p as usize).checked_add(arena_bytes).is_none() { std::process::abort(); }
        // SAFETY: the entire newly allocated region is writable and nonaliasing.
        // Initialize every byte, not merely a logical byte counter or sparse page.
        unsafe { ptr::write_bytes(p, 0xa5, arena_bytes); }
        // Read one byte on every page and the end to observe initialization; the
        // allocator thereafter uses this SAME region for all role allocations.
        let mut touch = 0u8;
        for i in (0..arena_bytes).step_by(4096) {
            // SAFETY: i is within this live, initialized allocation.
            touch ^= unsafe { p.add(i).read_volatile() };
        }
        // SAFETY: arena_bytes >=4096, so last byte is in bounds and initialized.
        touch ^= unsafe { p.add(arena_bytes-1).read_volatile() };
        std::hint::black_box(touch);
        // SAFETY: heap is empty, this exclusive region is valid forever and is
        // sufficiently large/aligned for the pinned library's hole metadata.
        unsafe { s.heap.init(p, arena_bytes); }
        let a = Activation { pid:std::process::id(),grant_bytes,arena_start:p as usize,
            arena_bytes,bootstrap_charged_bytes:s.system_live,adapter_header_bytes:header,
            initialized_bytes:arena_bytes };
        s.activation=Some(a);
        Ok(a)
    }
    pub fn snapshot(&self) -> Snapshot {
        let s=self.lock();
        Snapshot { activation:s.activation,current_requested_bytes:s.current,
            lifetime_peak_requested_bytes:s.peak,bootstrap_live_bytes:s.system_live,
            bootstrap_peak_bytes:s.system_peak,arena_used_bytes:s.heap.used(),
            arena_free_bytes:s.heap.free(),allocation_failures:s.failures }
    }
    pub fn current_usage(&self) -> usize { self.snapshot().current_requested_bytes }
    pub fn peak_usage(&self) -> usize { self.snapshot().lifetime_peak_requested_bytes }
}
impl State {
    fn arena_pointer(&self,p:*mut u8)->bool {
        self.activation.is_some_and(|a| (p as usize)>=a.arena_start
            && (p as usize)-a.arena_start<a.arena_bytes)
    }
    fn allocate(&mut self,l:Layout,zero:bool)->*mut u8 {
        // GlobalAlloc callers use nonzero layouts; also give a harmless aligned
        // dangling value for explicit boundary tests of zero-size delegation.
        if l.size()==0 { return l.align() as *mut u8; }
        let Some(next)=self.current.checked_add(l.size()) else { return ptr::null_mut(); };
        let p=if self.activation.is_some() {
            match self.heap.allocate_first_fit(l) { Ok(p)=>p.as_ptr(),Err(_)=>ptr::null_mut() }
        } else {
            // SAFETY: caller supplies a valid Layout; direct System avoids recursion.
            unsafe { System.alloc(l) }
        };
        if p.is_null() { self.failures=self.failures.saturating_add(1);return p; }
        // SAFETY: freshly allocated l.size() bytes, disjoint from live allocations.
        // Startup allocations are fully initialized too, before arena activation.
        unsafe { ptr::write_bytes(p,if zero {0} else {0xa5},l.size()); }
        self.current=next;self.peak=self.peak.max(next);
        if self.activation.is_none() {
            self.system_live=match self.system_live.checked_add(l.size()) {Some(n)=>n,None=>std::process::abort()};
            self.system_peak=self.system_peak.max(self.system_live);
        }
        p
    }
    unsafe fn release(&mut self,p:*mut u8,l:Layout) {
        if l.size()==0 {return;}
        if self.arena_pointer(p) {
            // SAFETY: GlobalAlloc caller guarantees same live pointer and Layout;
            // range dispatch selects the backend which originally allocated it.
            unsafe { self.heap.deallocate(NonNull::new_unchecked(p),l); }
        } else {
            // SAFETY: this pointer predates activation or allocator is inactive;
            // caller guarantees a live System allocation with the original Layout.
            unsafe { System.dealloc(p,l); }
            self.system_live=match self.system_live.checked_sub(l.size()) {Some(n)=>n,None=>std::process::abort()};
        }
        self.current=match self.current.checked_sub(l.size()) {Some(n)=>n,None=>std::process::abort()};
    }
}
// SAFETY: one mutex serializes allocation, free, realloc and one-time activation.
// Each nonzero allocation is uniquely owned, properly aligned by System/Heap;
// no fallback after activation and no arena teardown while pointers may survive.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self,l:Layout)->*mut u8 { self.lock().allocate(l,false) }
    unsafe fn alloc_zeroed(&self,l:Layout)->*mut u8 { self.lock().allocate(l,true) }
    unsafe fn dealloc(&self,p:*mut u8,l:Layout) {
        // SAFETY: GlobalAlloc caller's pointer/Layout contract passed unchanged.
        unsafe { self.lock().release(p,l); }
    }
    unsafe fn realloc(&self,p:*mut u8,l:Layout,new_size:usize)->*mut u8 {
        let Ok(next)=Layout::from_size_align(new_size,l.align()) else{return ptr::null_mut();};
        let mut s=self.lock();
        if new_size==0 {
            // SAFETY: preserves deallocation contract; special result never dereferenced.
            unsafe { s.release(p,l); }
            return l.align() as *mut u8;
        }
        let out=s.allocate(next,false);
        if !out.is_null() {
            // SAFETY: the successful new allocation cannot overlap the still-live
            // old one; both hold at least min(old,new) bytes. Failure keeps old valid.
            unsafe { ptr::copy_nonoverlapping(p,out,l.size().min(new_size));s.release(p,l); }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const MIB:usize=1024*1024;
    #[test]
    fn activation_is_once_and_charges_real_prefix_header_and_full_region() {
        let a=Allocator::new();let l=Layout::from_size_align(127,64).unwrap();
        // SAFETY: exact allocation/layout pair, used only while live.
        let p=unsafe{a.alloc(l)};assert!(!p.is_null());
        let on=a.activate(2*MIB).unwrap();
        assert_eq!(on.arena_bytes+on.bootstrap_charged_bytes+on.adapter_header_bytes,2*MIB);
        assert_eq!(on.bootstrap_charged_bytes,127);
        assert_eq!(on.initialized_bytes,on.arena_bytes);
        assert!(a.activate(2*MIB).is_err());
        // SAFETY: retain live bytes across switch, release using original layout.
        unsafe {assert_eq!(p.read(),0xa5);a.dealloc(p,l);}
        assert_eq!(a.snapshot().bootstrap_live_bytes,0);
        assert_eq!(a.snapshot().activation,Some(on));
    }
    #[test]
    fn alignments_zeroing_failed_realloc_and_cross_activation_move_keep_bytes() {
        let a=Allocator::new();let l=Layout::from_size_align(257,128).unwrap();
        // SAFETY: all requests use valid Layouts and matching frees; reads within size.
        unsafe {
            let p=a.alloc(l);ptr::write_bytes(p,7,257);a.activate(2*MIB).unwrap();
            let moved=a.realloc(p,l,4097);assert!(!moved.is_null());
            assert!(std::slice::from_raw_parts(moved,257).iter().all(|b|*b==7));
            let m=Layout::from_size_align(4097,128).unwrap();
            assert!(a.realloc(moved,m,4*MIB).is_null());
            assert_eq!(moved.read(),7);a.dealloc(moved,m);
            for align in [1,2,8,64,4096,65536] {
                let l=Layout::from_size_align(333,align).unwrap();let p=a.alloc_zeroed(l);
                assert!(!p.is_null());assert_eq!(p as usize%align,0);
                assert!(std::slice::from_raw_parts(p,333).iter().all(|b|*b==0));a.dealloc(p,l);
            }
            let z=Layout::from_size_align(0,4096).unwrap();let p=a.alloc(z);assert_eq!(p as usize%4096,0);a.dealloc(p,z);
        }
        assert_eq!(a.current_usage(),0);assert_eq!(a.snapshot().arena_used_bytes,0);
        assert_eq!(a.snapshot().allocation_failures,1);
    }
    #[test]
    fn repeated_fragmentation_coalesces_without_system_fallback() {
        let a=Allocator::new();a.activate(4*MIB).unwrap();
        for _ in 0..64 {
            let mut live=Vec::new();
            for i in 0..512 {
                let l=Layout::from_size_align(19+i*7,1usize<<(i%9)).unwrap();
                // SAFETY: Layout is valid; address/layout retained until matching free.
                let p=unsafe{a.alloc(l)};assert!(!p.is_null());live.push((p,l));
            }
            for offset in [0,1] {for (p,l) in live.iter().skip(offset).step_by(2) {
                // SAFETY: each allocation freed once in disjoint parity groups.
                unsafe{a.dealloc(*p,*l);}
            }}
            assert_eq!(a.snapshot().arena_used_bytes,0);
        }
        assert_eq!(a.snapshot().bootstrap_live_bytes,0);assert_eq!(a.snapshot().allocation_failures,0);
        let l=Layout::from_size_align(3*MIB,4096).unwrap();
        // SAFETY: successful large allocation released with its original Layout.
        unsafe{let p=a.alloc(l);assert!(!p.is_null());a.dealloc(p,l);}
    }
    #[test]
    fn startup_limit_rejects_before_switch_and_never_hides_previous_peak() {
        let a=Allocator::new();let l=Layout::from_size_align(STARTUP_LIMIT+1,8).unwrap();
        // SAFETY: valid allocation and original matching free.
        unsafe{let p=a.alloc(l);assert!(!p.is_null());a.dealloc(p,l);}
        assert!(a.activate(4*MIB).is_err());assert!(a.snapshot().activation.is_none());
        assert_eq!(a.current_usage(),0);assert!(a.peak_usage()>STARTUP_LIMIT);
    }
}
